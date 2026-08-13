use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub tools_schema: Vec<serde_json::Value>,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub thought: String,
    pub tool_call: Option<serde_json::Value>,
    pub raw_json: String,
}

/// Um chunk de streaming — o delta de texto emitido pelo provedor.
#[derive(Debug, Clone)]
pub struct LlmChunk {
    pub delta: String,
    pub done: bool,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum LlmError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Timeout")]
    Timeout,
    #[error("Provider error: {0}")]
    Provider(String),
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;
    async fn stream(
        &self,
        req: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Send + Unpin>, LlmError>;
    fn model_id(&self) -> &str;
    fn supports_tools(&self) -> bool;
}

pub mod mock;
mod ollama;
pub mod openai;

pub use mock::MockLlm;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
