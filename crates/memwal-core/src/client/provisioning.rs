use std::str::FromStr;
use std::sync::Arc;
use zeroize::Zeroizing;

use crate::account::AccountClient;
use crate::account::ProvisionAccountMode;
use crate::auth::DelegateKey;
use crate::client::MemWal;
use crate::client::MemWalConfig;
use crate::error::MemWalError;
use crate::sui::Ed25519Signer;
use crate::sui::MemWalSigner;
use crate::types::RelayerConfig;
use crate::utils::DEFAULT_RELAYER_URL;
use crate::utils::get_relayer_config;

const DEFAULT_DELEGATE_LABEL: &str = "memwal-rust";

#[derive(Clone, Debug)]
pub struct MemWalProvisionConfig {
    delegate_suiprivkey: Zeroizing<String>,
    wallet_suiprivkey: Option<Zeroizing<String>>,
    account_id: Option<String>,
    registry_id: Option<String>,
    mode: ProvisionAccountMode,
    server_url: Option<String>,
    relayer_config_url: Option<String>,
    namespace: Option<String>,
    delegate_label: String,
}

impl MemWalProvisionConfig {
    pub fn new(delegate_suiprivkey: impl Into<String>) -> Self {
        Self {
            delegate_suiprivkey: Zeroizing::new(delegate_suiprivkey.into()),
            wallet_suiprivkey: None,
            account_id: None,
            registry_id: None,
            mode: ProvisionAccountMode::ReuseOrCreate,
            server_url: None,
            relayer_config_url: None,
            namespace: None,
            delegate_label: DEFAULT_DELEGATE_LABEL.to_owned(),
        }
    }

    pub fn wallet_suiprivkey(mut self, wallet_suiprivkey: impl Into<String>) -> Self {
        self.wallet_suiprivkey = Some(Zeroizing::new(wallet_suiprivkey.into()));
        self
    }

    pub fn account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    pub fn registry_id(mut self, registry_id: impl Into<String>) -> Self {
        self.registry_id = Some(registry_id.into());
        self
    }

    pub fn mode(mut self, mode: ProvisionAccountMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn server_url(mut self, server_url: impl Into<String>) -> Self {
        self.server_url = Some(server_url.into());
        self
    }

    pub fn relayer_config_url(mut self, relayer_config_url: impl Into<String>) -> Self {
        self.relayer_config_url = Some(relayer_config_url.into());
        self
    }

    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn delegate_label(mut self, delegate_label: impl Into<String>) -> Self {
        self.delegate_label = delegate_label.into();
        self
    }
}

pub struct ProvisionedMemWal {
    memwal: MemWal,
    account_client: AccountClient,
    account_id: sui_sdk_types::Address,
    delegate_public_key: [u8; 32],
    delegate_address: sui_sdk_types::Address,
    relayer_config: RelayerConfig,
}

impl ProvisionedMemWal {
    pub fn memwal(&self) -> &MemWal {
        &self.memwal
    }

    pub fn account_id(&self) -> String {
        self.account_id.to_string()
    }

    pub fn delegate_address(&self) -> String {
        self.delegate_address.to_string()
    }

    pub fn relayer_config(&self) -> &RelayerConfig {
        &self.relayer_config
    }

    pub async fn revoke_delegate_key(&self) -> Result<String, MemWalError> {
        self.account_client
            .remove_delegate_key(self.account_id, self.delegate_public_key)
            .await
            .map(|digest| digest.to_string())
    }
}

impl MemWal {
    pub async fn provision(
        config: MemWalProvisionConfig,
    ) -> Result<ProvisionedMemWal, MemWalError> {
        let delegate_key = DelegateKey::from_suiprivkey(&config.delegate_suiprivkey)?;
        let delegate_public_key = delegate_key.public_key().into_inner();
        let delegate_address = delegate_key.sui_address();
        let wallet_suiprivkey = config
            .wallet_suiprivkey
            .as_ref()
            .map(|k| k.as_str())
            .unwrap_or(config.delegate_suiprivkey.as_str());
        let wallet_signer = Ed25519Signer::from_suiprivkey(wallet_suiprivkey)?;
        let signer: Arc<dyn MemWalSigner> = Arc::new(wallet_signer);

        let relayer_config = get_relayer_config(
            config.server_url.as_deref(),
            config.relayer_config_url.as_deref(),
        )
        .await?;
        let rpc_client = sui_rpc::Client::new(relayer_config.sui_rpc_url())?;
        let account_client =
            AccountClient::new(rpc_client, signer, relayer_config.package_address()?);

        let account_id = if let Some(account_id) = config.account_id.as_deref() {
            let account_id = sui_sdk_types::Address::from_str(account_id)
                .map_err(MemWalError::object_id_parse)?;
            account_client
                .ensure_delegate_key(account_id, delegate_public_key, &config.delegate_label)
                .await?;
            account_id
        } else {
            let registry_id = config
                .registry_id
                .as_deref()
                .or_else(|| relayer_config.registry_id())
                .ok_or_else(|| {
                    MemWalError::config(
                        "registry_id is required when account_id is not set and relayer config \
                         does not include registryId",
                    )
                })?;
            account_client
                .provision_account(
                    registry_id,
                    config.mode,
                    delegate_public_key,
                    &config.delegate_label,
                )
                .await?
        };

        let server_url = relayer_config
            .server_url()
            .unwrap_or(DEFAULT_RELAYER_URL)
            .to_owned();
        let memwal_config =
            MemWalConfig::new(delegate_key, account_id, Some(server_url), config.namespace);
        let memwal = MemWal::new(memwal_config).await?;

        Ok(ProvisionedMemWal {
            memwal,
            account_client,
            account_id,
            delegate_public_key,
            delegate_address,
            relayer_config,
        })
    }
}
