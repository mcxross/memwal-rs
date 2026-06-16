use serde::Deserialize;
use std::str::FromStr;

use crate::error::MemWalError;

#[derive(Clone, Debug, Deserialize)]
pub struct MinSupportedSdk {
    pub typescript: String,
    pub python: String,
    pub mcp: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayerDeprecationNotice {
    pub surface: String,
    pub deprecated_since: Option<String>,
    pub removal_api_version: Option<String>,
    pub guidance: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RelayerBuildMetadata {
    pub commit: Option<String>,
    pub build_timestamp: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayerVersionMetadata {
    #[serde(rename = "relayerVersion")]
    pub relayer_version: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(rename = "minSupportedSdk")]
    pub min_supported_sdk: MinSupportedSdk,
    #[serde(rename = "featureFlags")]
    pub feature_flags: std::collections::BTreeMap<String, bool>,
    pub deprecations: Vec<RelayerDeprecationNotice>,
    pub build: RelayerBuildMetadata,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HealthResult {
    pub status: String,
    pub version: String,
    #[serde(rename = "relayerVersion")]
    pub relayer_version: Option<String>,
    #[serde(rename = "apiVersion")]
    pub api_version: Option<String>,
    #[serde(rename = "minSupportedSdk")]
    pub min_supported_sdk: Option<MinSupportedSdk>,
    #[serde(rename = "featureFlags")]
    pub feature_flags: Option<std::collections::BTreeMap<String, bool>>,
    pub deprecations: Option<Vec<RelayerDeprecationNotice>>,
    pub build: Option<RelayerBuildMetadata>,
    pub mode: Option<String>,
    pub prompt_versions: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayerConfig {
    #[serde(rename = "packageId")]
    package_id: String,
    #[serde(default)]
    network: Option<String>,
    #[serde(rename = "suiRpcUrl")]
    sui_rpc_url: String,
    #[serde(rename = "registryId")]
    registry_id: Option<String>,
    #[serde(skip)]
    server_url: Option<String>,
}

impl RelayerConfig {
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    pub fn network(&self) -> Option<&str> {
        self.network.as_deref()
    }

    pub fn sui_rpc_url(&self) -> &str {
        &self.sui_rpc_url
    }

    pub fn registry_id(&self) -> Option<&str> {
        self.registry_id.as_deref()
    }

    pub fn server_url(&self) -> Option<&str> {
        self.server_url.as_deref()
    }

    pub(crate) fn with_server_url(mut self, server_url: String) -> Self {
        self.server_url = Some(server_url);
        self
    }

    pub(crate) fn package_address(&self) -> Result<sui_sdk_types::Address, MemWalError> {
        sui_sdk_types::Address::from_str(&self.package_id).map_err(MemWalError::object_id_parse)
    }
}
