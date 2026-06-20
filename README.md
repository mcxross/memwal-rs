# memwal-rs

Rust SDK for MemWal.

This crate provides:

- `MemWal` for relayer-managed remember / recall / analyze / restore flows.
- `MemWal::provision` for account reuse/creation plus delegate-key registration.
- `MemWalManual` for client-side embedding, Seal encryption, Walrus retrieval, and local decrypt.
- `AccountClient` for on-chain MemWal account and delegate-key management.

## Quickstart

Use `MemWal::provision` when you want the SDK to resolve relayer config, reuse or create the
MemWal account, register the delegate key, and return a ready client.

```rust
use std::time::Duration;

use memwal_core::MemWal;
use memwal_core::MemWalProvisionConfig;
use memwal_core::RecallParams;

#[tokio::main]
async fn main() -> Result<(), memwal_core::MemWalError> {
    let provisioned = MemWal::provision(
        MemWalProvisionConfig::new("suiprivkey...")
            .registry_id("0x...")
            .delegate_label("my-app"),
    )
    .await?;

    let memwal = provisioned.memwal();

    let remembered = memwal
        .remember(
            "User prefers dark mode and uses Rust.",
            Duration::from_millis(1500),
            Duration::from_secs(60),
        )
        .await?;

    let recalled = memwal
        .recall(RecallParams {
            query: "What does the user prefer?".to_owned(),
            limit: Some(5),
            namespace: None,
            top_k: None,
            max_distance: None,
        })
        .await?;

    println!("stored memory: {}", remembered.id);
    println!("matches: {}", recalled.results.len());

    let revoke_digest = provisioned.revoke_delegate_key().await?;
    println!("revoked delegate key: {revoke_digest}");

    Ok(())
}
```

`MemWalProvisionConfig` accepts plain strings at the public API boundary:

- `new(delegate_suiprivkey)` sets the delegate key used for relayer requests.
- `wallet_suiprivkey(...)` optionally sets a separate Sui wallet key for on-chain transactions.
- `account_id(...)` reuses an explicit MemWal account and registers the delegate key.
- `registry_id(...)` enables registry lookup and account creation/reuse when no account ID is set.
- `server_url(...)` and `relayer_config_url(...)` override relayer discovery.
- `namespace(...)` sets the default memory namespace.
- `delegate_label(...)` sets the on-chain delegate-key label.

The basic SDK flow does not require callers to depend on `sui-sdk-types` or construct Sui clients.

## Run the Demo

The demo app maps environment variables into `MemWalProvisionConfig`, then runs the full lifecycle:
provision or reuse an account, register the delegate key, store memory, recall memory, and revoke
the delegate key.

Required:

```sh
export MEMWAL_PRIVATE_KEY=suiprivkey...
```

Then choose one account path:

```sh
# Reuse or create through the registry.
export MEMWAL_REGISTRY_ID=0x...

# Or use an explicit existing account.
export MEMWAL_ACCOUNT_ID=0x...
```

Run:

```sh
cargo run -p memwal-demo --bin memwal-demo
```

Optional demo variables:

- `MEMWAL_WALLET_KEY`: separate wallet key for on-chain provisioning.
- `MEMWAL_SERVER_URL` or `MEMWAL_RELAYER_URL`: relayer base URL.
- `MEMWAL_RELAYER_CONFIG_URL` or `MEMWAL_RELAYER_CONFIG_PATH`: explicit relayer config URL.
- `MEMWAL_NAMESPACE`: default namespace for memory operations.

## Lower-Level APIs

### MemWal Client

If you already have a delegate key and account ID, construct `MemWal` directly with `MemWalConfig`.
This bypasses provisioning but keeps the relayer API ergonomic.

```rust
use std::time::Duration;

use memwal_core::DelegateKey;
use memwal_core::MemWal;
use memwal_core::MemWalConfig;
use memwal_core::RecallParams;

# async fn example(account_id: sui_sdk_types::Address) -> Result<(), memwal_core::MemWalError> {
let delegate_key = DelegateKey::from_suiprivkey("suiprivkey...")?;
let memwal = MemWal::new(MemWalConfig::new(
    delegate_key,
    account_id,
    Some("https://relayer.memory.walrus.xyz".to_owned()),
    Some("default".to_owned()),
))
.await?;

let accepted = memwal.remember("User prefers dark mode.").await?;
let remembered = memwal
    .wait_for_remember_job(
        &accepted.job_id,
        Duration::from_millis(1500),
        Duration::from_secs(60),
    )
    .await?;

let recalled = memwal
    .recall(RecallParams {
        query: "What does the user prefer?".to_owned(),
        limit: Some(5),
        namespace: None,
        top_k: None,
        max_distance: None,
    })
    .await?;

println!("stored memory: {}", remembered.id);
println!("matches: {}", recalled.results.len());
# Ok(())
# }
```

Common `MemWal` methods:

- `remember_async(text)` submits a memory write and returns a job ID.
- `remember(text, poll_interval, timeout)` submits and waits for completion.
- `remember_bulk(items)` and `remember_bulk_and_wait(items, poll_interval, timeout)` handle batch writes.
- `recall(params)` searches memory.
- `analyze(options)` and `analyze_and_wait(options, poll_interval, timeout)` run relayer analysis jobs.
- `restore(blob_id)` restores a memory blob.
- `embed(text)` returns relayer embeddings.
- `remember_manual(...)` and `recall_manual(...)` expose manual encryption/retrieval flows.
- `health()` and `compatibility()` inspect relayer status.
- `delegate_public_key_hex()` returns the delegate public key used for request signing.

### Account Client

Use `AccountClient` directly when you need fine-grained transaction control. `provision_account`
accepts a registry ID string and a `ProvisionAccountMode`:

```rust
use memwal_core::AccountClient;
use memwal_core::ProvisionAccountMode;

# async fn example(account_client: AccountClient, delegate_public_key: [u8; 32]) -> Result<(), memwal_core::MemWalError> {
let account_id = account_client
    .provision_account(
        "0x...",
        ProvisionAccountMode::ReuseOrCreate,
        delegate_public_key,
        "my-app",
    )
    .await?;
# Ok(())
# }
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
