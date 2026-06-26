use std::future::Future;
use std::pin::Pin;

use crate::error::MemWalError;
use super::EmbeddingProvider;

#[derive(Clone)]
pub struct OllamaEmbeddingProvider {
    client: reqwest::Client,
    api_url: String,
    model: String,
}

impl OllamaEmbeddingProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: "http://127.0.0.1:11434/api/embed".to_owned(),
            model: model.into(),
        }
    }

    pub fn with_api_url(mut self, api_url: impl Into<String>) -> Self {
        self.api_url = api_url.into();
        self
    }
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn embed<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<f32>, MemWalError>> + Send + 'a>> {
        Box::pin(async move {
            #[derive(serde::Serialize)]
            struct EmbedRequest<'a> {
                model: &'a str,
                input: &'a str,
            }

            #[derive(serde::Deserialize)]
            struct EmbedResponse {
                embeddings: Vec<Vec<f32>>,
            }

            let response = self
                .client
                .post(&self.api_url)
                .json(&EmbedRequest {
                    model: &self.model,
                    input: text,
                })
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(MemWalError::embedding(
                    response.text().await.unwrap_or_default(),
                ));
            }

            let payload = response.json::<EmbedResponse>().await?;
            let mut vector = payload
                .embeddings
                .into_iter()
                .next()
                .ok_or_else(|| MemWalError::embedding("embedding API returned no embedding"))?;

            // Hack: The public MemWal relayer expects exactly 1536 dimensions.
            // If the local model (like nomic) outputs fewer dimensions (e.g. 768), 
            // we pad the remainder with zeroes so the relayer database accepts the insertion.
            if vector.len() < 1536 {
                vector.resize(1536, 0.0);
            }

            Ok(vector)
        })
    }
}
