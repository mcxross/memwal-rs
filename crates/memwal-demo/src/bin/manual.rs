use std::env;
use std::str::FromStr;
use std::sync::Arc;

use memwal_core::auth::DelegateKey;
use memwal_core::manual::{
    MemWalManual, MemWalManualConfig, OllamaEmbeddingProvider, SuiNetwork, WalrusHttpStore,
};
use memwal_core::sui::Ed25519Signer;
use memwal_core::types::ManualRecallOptions;
use memwal_core::{MemWal, MemWalProvisionConfig, MemWalSigner};

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

    let delegate_suiprivkey = env::var("MEMWAL_PRIVATE_KEY")
        .expect("MEMWAL_PRIVATE_KEY is not set. Please set it to your bech32 suiprivkey.");
    let delegate_key = DelegateKey::from_suiprivkey(&delegate_suiprivkey)?;

    let wallet_suiprivkey = env::var("MEMWAL_WALLET_KEY").unwrap_or(delegate_suiprivkey);
    let wallet_signer = Ed25519Signer::from_suiprivkey(&wallet_suiprivkey)?;
    let signer: Arc<dyn MemWalSigner> = Arc::new(wallet_signer);

    let relayer_config = provisioned.relayer_config();
    let package_id = relayer_config.package_address()?;

    let account_id = sui_sdk_types::Address::from_str(&provisioned.account_id())?;

    let network = match relayer_config.sui_rpc_url() {
        url if url.contains("mainnet") => SuiNetwork::Mainnet,
        _ => SuiNetwork::Testnet,
    };

    let embedder = Arc::new(OllamaEmbeddingProvider::new("nomic-embed-text-v2-moe"));

    let walrus_publisher_url = env::var("WALRUS_PUBLISHER_URL")
        .unwrap_or_else(|_| "https://publisher.walrus-testnet.walrus.space".to_owned());
    let walrus_aggregator_url = env::var("WALRUS_AGGREGATOR_URL")
        .unwrap_or_else(|_| "https://aggregator.walrus-testnet.walrus.space".to_owned());
    let walrus_store = Arc::new(WalrusHttpStore::testnet().with_urls(
        walrus_publisher_url,
        walrus_aggregator_url,
    ));

    let config = MemWalManualConfig::new(
        delegate_key,
        signer,
        package_id,
        account_id,
        network,
        embedder,
        walrus_store,
    );

    let memwal = MemWalManual::new(config)?;

    println!("  Remembering text manually...");
    let result = memwal
        .remember("User prefers manual memory management.", None)
        .await?;
    println!("  Stored Memory ID (blob): {}", result.blob_id);

    println!("  Recalling memory manually...");
    let recall_res = memwal
        .recall(
            "What does the user prefer?",
            ManualRecallOptions {
                limit: Some(5),
                namespace: None,
                scoring_weights: None,
            },
        )
        .await?;

    for memory in recall_res.results {
        println!("  - (distance: {:.3}) {}", memory.distance, memory.text);
    }

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
