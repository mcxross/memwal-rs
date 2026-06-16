use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::MemWalError;
use crate::sui::MemWalSigner;
use crate::sui::add_delegate_key_builder;
use crate::sui::create_account_builder;
use crate::sui::created_account_id;
use crate::sui::execute_account_transaction;
use crate::sui::remove_delegate_key_builder;
use crate::sui::transaction_digest;
use crate::types::AddDelegateKeyResult;
use crate::types::CreateAccountResult;
use sui_rpc::proto::sui::rpc::v2::GetObjectRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvisionAccountMode {
    ReuseExisting,
    ReuseOrCreate,
}

pub struct AccountClient {
    rpc_client: Mutex<sui_rpc::Client>,
    signer: Arc<dyn MemWalSigner>,
    package_id: sui_sdk_types::Address,
}

impl AccountClient {
    pub fn new(
        rpc_client: sui_rpc::Client,
        signer: Arc<dyn MemWalSigner>,
        package_id: sui_sdk_types::Address,
    ) -> Self {
        Self {
            rpc_client: Mutex::new(rpc_client),
            signer,
            package_id,
        }
    }

    pub async fn create_account(
        &self,
        registry_id: sui_sdk_types::Address,
    ) -> Result<CreateAccountResult, MemWalError> {
        let sender = self.signer.address()?;
        let builder = create_account_builder(self.package_id, registry_id, sender)?;
        let response =
            execute_account_transaction(&self.rpc_client, self.signer.as_ref(), builder).await?;
        Ok(CreateAccountResult {
            account_id: created_account_id(&response)?,
            owner: sender,
            digest: transaction_digest(&response)?,
        })
    }

    pub async fn add_delegate_key(
        &self,
        account_id: sui_sdk_types::Address,
        public_key: [u8; 32],
        label: &str,
    ) -> Result<AddDelegateKeyResult, MemWalError> {
        if label.len() > 64 {
            return Err(MemWalError::config(
                "delegate label must be 64 bytes or fewer",
            ));
        }

        let sender = self.signer.address()?;
        let delegate_address = sui_sdk_types::Ed25519PublicKey::new(public_key).derive_address();
        let builder = add_delegate_key_builder(
            self.package_id,
            account_id,
            sender,
            &public_key,
            delegate_address,
            label,
        )?;
        let response =
            execute_account_transaction(&self.rpc_client, self.signer.as_ref(), builder).await?;
        Ok(AddDelegateKeyResult {
            digest: transaction_digest(&response)?,
            public_key_hex: hex::encode(public_key),
            sui_address: delegate_address,
        })
    }

    pub async fn remove_delegate_key(
        &self,
        account_id: sui_sdk_types::Address,
        public_key: [u8; 32],
    ) -> Result<sui_sdk_types::Digest, MemWalError> {
        let sender = self.signer.address()?;
        let builder =
            remove_delegate_key_builder(self.package_id, account_id, sender, &public_key)?;
        let response =
            execute_account_transaction(&self.rpc_client, self.signer.as_ref(), builder).await?;
        transaction_digest(&response)
    }

    pub async fn ensure_delegate_key(
        &self,
        account_id: sui_sdk_types::Address,
        public_key: [u8; 32],
        label: &str,
    ) -> Result<(), MemWalError> {
        match self.add_delegate_key(account_id, public_key, label).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if is_duplicate_delegate_key_abort(&e) {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    pub async fn get_account_from_registry(
        &self,
        registry_id: sui_sdk_types::Address,
        wallet_address: sui_sdk_types::Address,
    ) -> Result<Option<sui_sdk_types::Address>, MemWalError> {
        let mut client = self.rpc_client.lock().await.clone();

        let mut req = GetObjectRequest::default();
        req.object_id = Some(registry_id.to_string());
        req.read_mask = Some(prost_types::FieldMask {
            paths: vec!["contents".to_string()],
        });

        let resp = client
            .ledger_client()
            .get_object(req)
            .await
            .map_err(|e| MemWalError::config(format!("failed to fetch registry object: {e}")))?
            .into_inner();

        let obj = resp
            .object
            .ok_or_else(|| MemWalError::config("registry object not found"))?;
        let bcs_data = obj
            .contents
            .ok_or_else(|| MemWalError::config("no contents data for registry"))?;

        let bcs_bytes = bcs_data
            .value
            .ok_or_else(|| MemWalError::config("registry contents value is empty"))?;

        let registry = crate::types::registry::AccountRegistryMove::from_bcs_prefix(&bcs_bytes)
            .map_err(|e| MemWalError::config(format!("failed to deserialize registry: {e}")))?;

        let key_bytes = bcs::to_bytes(&wallet_address)
            .map_err(|_| MemWalError::config("failed to serialize wallet address"))?;
        let child_id = registry
            .accounts
            .id
            .derive_dynamic_child_id(&sui_sdk_types::TypeTag::Address, &key_bytes);

        let mut child_req = GetObjectRequest::default();
        child_req.object_id = Some(child_id.to_string());
        child_req.read_mask = Some(prost_types::FieldMask {
            paths: vec!["contents".to_string()],
        });

        let child_resp = match client.ledger_client().get_object(child_req).await {
            Ok(r) => r.into_inner(),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("NotFound") || msg.contains("not found") {
                    return Ok(None);
                }
                return Err(MemWalError::config(format!(
                    "GetObjectRequest failed for registry child {child_id}: {e}"
                )));
            }
        };

        let child_obj = match child_resp.object {
            Some(o) => o,
            None => return Ok(None),
        };
        let child_bcs = child_obj
            .contents
            .ok_or_else(|| MemWalError::config("no contents data for registry dynamic field"))?;
        let child_bcs_bytes = child_bcs
            .value
            .ok_or_else(|| MemWalError::config("registry dynamic field contents value is empty"))?;
        let account_id =
            crate::types::registry::account_id_from_registry_field_bcs(child_bcs_bytes.as_ref())
                .map_err(|e| {
                    MemWalError::config(format!("failed to deserialize registry field: {e}"))
                })?;

        Ok(Some(account_id))
    }

    pub async fn provision_account(
        &self,
        registry_id_str: &str,
        mode: ProvisionAccountMode,
        delegate_public_key: [u8; 32],
        label: &str,
    ) -> Result<sui_sdk_types::Address, MemWalError> {
        let sender = self.signer.address()?;

        let registry_id = sui_sdk_types::Address::from_str(registry_id_str)
            .map_err(MemWalError::object_id_parse)?;

        let account_id = match self.get_account_from_registry(registry_id, sender).await? {
            Some(id) => id,
            None => {
                if mode == ProvisionAccountMode::ReuseExisting {
                    return Err(MemWalError::config("account not found in registry"));
                }

                let create_result = self.create_account(registry_id).await?;
                create_result.account_id
            }
        };

        self.ensure_delegate_key(account_id, delegate_public_key, label)
            .await?;

        Ok(account_id)
    }
}

fn is_duplicate_delegate_key_abort(error: &MemWalError) -> bool {
    let message = error.to_string();
    (message.contains("MOVE_ABORT") || message.contains("MoveAbort"))
        && (message.contains("function_name: Some(\"add_delegate_key\")")
            || message.contains("::account::add_delegate_key"))
        && (message.contains("}, 0")
            || message.contains("}, 0)")
            || message.contains("abort_code: 0")
            || message.contains("code: 0"))
}
