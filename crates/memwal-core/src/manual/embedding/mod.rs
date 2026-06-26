use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::MemWalError;

pub mod ollama;
pub mod openai;

pub use self::ollama::OllamaEmbeddingProvider;
pub use self::openai::OpenAiEmbeddingProvider;

pub trait EmbeddingProvider: Send + Sync {
    fn embed<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<f32>, MemWalError>> + Send + 'a>>;
}

pub(crate) type SharedEmbeddingProvider = Arc<dyn EmbeddingProvider>;
