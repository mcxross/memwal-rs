use std::env;
use std::time::Duration;

use memwal_core::MemWal;
use memwal_core::MemWalProvisionConfig;
use memwal_core::RecallParams;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provisioned = match MemWal::provision(provision_config_from_env()?).await {
        Ok(provisioned) => provisioned,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    println!("  Delegate SUI address: {}", provisioned.delegate_address());
    println!("  Using MemWal account: {}", provisioned.account_id());

    let memwal = provisioned.memwal();

    println!("  Remembering text...");
    let result = memwal
        .remember(
            "User prefers dark mode and uses Rust.",
            Duration::from_millis(1500),
            Duration::from_secs(60),
        )
        .await?;
    println!("  Stored Memory ID: {}", result.id);

    println!("  Recalling memory...");
    let recall_res = memwal
        .recall(RecallParams {
            query: "What does the user prefer?".to_owned(),
            limit: Some(5),
            namespace: None,
            top_k: None,
            max_distance: None,
        })
        .await?;

    for memory in recall_res.results {
        println!("  - (distance: {:.3}) {}", memory.distance, memory.text);
    }

    println!("  Revoking delegate key...");
    let digest = provisioned.revoke_delegate_key().await?;
    println!("  Delegate key revoked! Tx digest: {digest}");

    Ok(())
}

fn provision_config_from_env() -> Result<MemWalProvisionConfig, Box<dyn std::error::Error>> {
    let delegate_suiprivkey = env::var("MEMWAL_PRIVATE_KEY")
        .map_err(|_| "MEMWAL_PRIVATE_KEY is not set. Please set it to your bech32 suiprivkey.")?;
    let mut config = MemWalProvisionConfig::new(delegate_suiprivkey).delegate_label("memwal-demo");

    if let Ok(wallet_suiprivkey) = env::var("MEMWAL_WALLET_KEY") {
        config = config.wallet_suiprivkey(wallet_suiprivkey);
    }
    if let Ok(account_id) = env::var("MEMWAL_ACCOUNT_ID") {
        config = config.account_id(account_id);
    }
    if let Ok(registry_id) = env::var("MEMWAL_REGISTRY_ID") {
        config = config.registry_id(registry_id);
    }
    if let Ok(server_url) =
        env::var("MEMWAL_SERVER_URL").or_else(|_| env::var("MEMWAL_RELAYER_URL"))
    {
        config = config.server_url(server_url);
    }
    if let Ok(relayer_config_url) =
        env::var("MEMWAL_RELAYER_CONFIG_URL").or_else(|_| env::var("MEMWAL_RELAYER_CONFIG_PATH"))
    {
        config = config.relayer_config_url(relayer_config_url);
    }
    if let Ok(namespace) = env::var("MEMWAL_NAMESPACE") {
        config = config.namespace(namespace);
    }

    Ok(config)
}
