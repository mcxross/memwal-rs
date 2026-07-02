use std::time::Duration;

use sui_rpc::field::FieldMask;
use sui_rpc::field::FieldMaskUtil;
use sui_rpc::proto::sui::rpc::v2::ExecuteTransactionRequest;
use sui_sdk_types::Address;
use sui_sdk_types::Digest;
use sui_sdk_types::Identifier;
use sui_transaction_builder::Function;
use sui_transaction_builder::ObjectInput;
use sui_transaction_builder::TransactionBuilder;

use crate::error::MemWalError;
use crate::sui::MemWalSigner;

pub(crate) const CLOCK_ID: Address =
    Address::from_static("0x0000000000000000000000000000000000000000000000000000000000000006");

pub(crate) async fn execute_account_transaction(
    client: &sui_rpc::Client,
    signer: &dyn MemWalSigner,
    builder: TransactionBuilder,
) -> Result<sui_rpc::proto::sui::rpc::v2::ExecuteTransactionResponse, MemWalError> {
    let mut cloned_client = client.clone();
    let transaction = builder
        .build(&mut cloned_client)
        .await
        .map_err(|error| MemWalError::config(error.to_string()))?;
    let signature = signer.sign_transaction(&transaction)?;
    let request = ExecuteTransactionRequest::new(transaction.into())
        .with_signatures(vec![signature.into()])
        .with_read_mask(FieldMask::from_str("*"));

    let response = client
        .clone()
        .execute_transaction_and_wait_for_checkpoint(request, Duration::from_secs(30))
        .await
        .map_err(|error| MemWalError::sui_rpc(tonic::Status::internal(error.to_string())))?
        .into_inner();

    let status = response.transaction().effects().status();
    if !status.success() {
        return Err(MemWalError::sui_rpc(tonic::Status::failed_precondition(
            status.error().to_string(),
        )));
    }

    Ok(response)
}

pub(crate) fn create_account_builder(
    package_id: Address,
    registry_id: Address,
    sender: Address,
) -> Result<TransactionBuilder, MemWalError> {
    let mut builder = TransactionBuilder::new();
    builder.set_sender(sender);

    let registry = builder.object(ObjectInput::new(registry_id).as_shared().with_mutable(true));
    let clock = builder.object(ObjectInput::new(CLOCK_ID).as_shared().with_mutable(false));
    builder.move_call(
        Function::new(
            package_id,
            Identifier::from_static("account"),
            Identifier::from_static("create_account"),
        ),
        vec![registry, clock],
    );

    Ok(builder)
}

pub(crate) fn add_delegate_key_builder(
    package_id: Address,
    account_id: Address,
    sender: Address,
    public_key: &[u8; 32],
    delegate_address: Address,
    label: &str,
) -> Result<TransactionBuilder, MemWalError> {
    let mut builder = TransactionBuilder::new();
    builder.set_sender(sender);

    let account = builder.object(ObjectInput::new(account_id).as_shared().with_mutable(true));
    let public_key = builder.pure(&public_key.to_vec());
    let delegate_address = builder.pure(&delegate_address);
    let label = builder.pure(&label.to_owned());
    let clock = builder.object(ObjectInput::new(CLOCK_ID).as_shared().with_mutable(false));

    builder.move_call(
        Function::new(
            package_id,
            Identifier::from_static("account"),
            Identifier::from_static("add_delegate_key"),
        ),
        vec![account, public_key, delegate_address, label, clock],
    );

    Ok(builder)
}

pub(crate) fn remove_delegate_key_builder(
    package_id: Address,
    account_id: Address,
    sender: Address,
    public_key: &[u8; 32],
) -> Result<TransactionBuilder, MemWalError> {
    let mut builder = TransactionBuilder::new();
    builder.set_sender(sender);

    let account = builder.object(ObjectInput::new(account_id).as_shared().with_mutable(true));
    let public_key = builder.pure(&public_key.to_vec());

    builder.move_call(
        Function::new(
            package_id,
            Identifier::from_static("account"),
            Identifier::from_static("remove_delegate_key"),
        ),
        vec![account, public_key],
    );

    Ok(builder)
}

pub(crate) fn created_account_id(
    response: &sui_rpc::proto::sui::rpc::v2::ExecuteTransactionResponse,
) -> Result<Address, MemWalError> {
    let result = response
        .transaction()
        .effects()
        .changed_objects()
        .iter()
        .find(|object| {
            // Check if the object type is MemWalAccount
            object.object_type().ends_with("::account::MemWalAccount")
                || object.object_type().ends_with("::account::Account")
        })
        .and_then(|object| object.object_id().parse::<Address>().ok());

    if let Some(id) = result {
        Ok(id)
    } else {
        let digest = response.transaction().digest();
        println!("ERROR: Account object not found in transaction {}", digest);
        println!("Changed objects:");
        for obj in response.transaction().effects().changed_objects() {
            println!(
                "  - ID: {}, Type: {}, State: {:?}",
                obj.object_id(),
                obj.object_type(),
                obj.output_state()
            );
        }
        Err(MemWalError::config(format!(
            "created MemWalAccount object not found in effects of tx {}",
            digest
        )))
    }
}

pub(crate) fn transaction_digest(
    response: &sui_rpc::proto::sui::rpc::v2::ExecuteTransactionResponse,
) -> Result<Digest, MemWalError> {
    response
        .transaction()
        .digest()
        .parse()
        .map_err(MemWalError::object_id_parse)
}
