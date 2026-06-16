use base64::Engine;
use sha2::Digest;
use sha2::Sha256;
use url::Url;

use crate::error::MemWalError;
use crate::types::RelayerConfig;

pub const DEFAULT_RELAYER_URL: &str = "https://relayer.memory.walrus.xyz";

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn normalize_server_url(url: &str) -> Result<Url, url::ParseError> {
    let mut parsed = Url::parse(url)?;
    if parsed.path().ends_with('/') && parsed.path() != "/" {
        let trimmed = parsed.path().trim_end_matches('/').to_owned();
        parsed.set_path(&trimmed);
    }
    Ok(parsed)
}

pub fn sanitize_server_error(status: u16, raw_body: &str) -> (String, Option<String>) {
    if status == 401 {
        return (
            "401 from relayer: wrong delegate key, key not registered on this account, account mismatch, or network mismatch".to_owned(),
            Some("AUTH_REJECTED".to_owned()),
        );
    }

    let mut server_code = None;
    let mut text = raw_body.to_owned();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_body) {
        if let Some(code) = value.get("code").and_then(serde_json::Value::as_str) {
            server_code = Some(code.to_owned());
        } else if let Some(code) = value.get("error").and_then(serde_json::Value::as_str) {
            server_code = Some(code.to_owned());
        }

        if let Some(message) = value.get("message").and_then(serde_json::Value::as_str) {
            text = message.to_owned();
        }
    }

    let sanitized = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .trim()
        .chars()
        .take(200)
        .collect::<String>();

    let message = if sanitized.is_empty() {
        format!("MemWal server error ({status})")
    } else {
        format!("MemWal server error ({status}): {sanitized}")
    };
    (message, server_code)
}

pub fn encode_base64_json<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let json = serde_json::to_vec(value)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(json))
}

pub async fn get_relayer_config(
    relayer_url: Option<&str>,
    relayer_config_url: Option<&str>,
) -> Result<RelayerConfig, MemWalError> {
    let server_url = normalize_server_url(relayer_url.unwrap_or(DEFAULT_RELAYER_URL))?;
    let server_url = server_url.as_str().trim_end_matches('/').to_owned();
    let config_url = relayer_config_url
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{server_url}/config"));

    let http = reqwest::Client::new();
    let relayer_config = http
        .get(&config_url)
        .send()
        .await?
        .json::<RelayerConfig>()
        .await?;

    Ok(relayer_config.with_server_url(server_url))
}
