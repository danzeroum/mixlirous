//! Adapter OpenAI-compatible para `LlmProvider` (task 3.1 do
//! `docs/13-ROADMAP-SPRINTS.md`).
//!
//! Usa `reqwest` + `serde` diretos (sem SDK oficial) para maximizar controle
//! e permitir trocar provedor mudando só `base_url` e `model`. Funciona com
//! OpenAI, DeepSeek, Groq e qualquer provedor compatível com
//! `/chat/completions`.

use super::{LlmError, LlmProvider, LlmRequest, LlmResponse};
use async_trait::async_trait;
use futures::TryStreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: String,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    delta: Option<StreamDeltaContent>,
}

#[derive(Debug, Deserialize)]
struct StreamDeltaContent {
    content: Option<String>,
}

/// Provedor compatível com a API OpenAI de `/chat/completions`.
///
/// `base_url` é a raiz do provedor (ex.: `https://api.openai.com/v1` ou
/// `http://localhost:11434/v1` para o servidor OpenAI-compatível do Ollama).
pub struct OpenAiProvider {
    base_url: String,
    model: String,
    api_key: String,
    timeout: Duration,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            timeout: Duration::from_secs(30),
            client: Client::new(),
        }
    }

    pub fn local(model: &str) -> Self {
        Self::new("http://localhost:11434/v1", model, "")
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn request(&self, req: &LlmRequest, stream: bool) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: req.system_prompt.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: req.user_prompt.clone(),
                },
            ],
            temperature: req.temperature,
            stream,
        }
    }

    fn http_error(&self, e: reqwest::Error) -> LlmError {
        if e.is_timeout() {
            LlmError::Timeout
        } else {
            LlmError::Http(e.to_string())
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .timeout(self.timeout)
            .json(&self.request(&req, false));

        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = builder.send().await.map_err(|e| self.http_error(e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("OpenAI API {status}: {body}")));
        }

        let chat_resp: ChatCompletionResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let content = chat_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let tool_call = Self::parse_tool_call(&content);

        Ok(LlmResponse {
            thought: content.clone(),
            tool_call,
            raw_json: content,
        })
    }

    async fn stream(
        &self,
        req: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Send + Unpin>, LlmError>
    {
        let url = format!("{}/chat/completions", self.base_url);
        let mut builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .timeout(self.timeout)
            .json(&self.request(&req, true));

        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = builder.send().await.map_err(|e| self.http_error(e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("OpenAI API {status}: {body}")));
        }

        let byte_stream = resp.bytes_stream();
        let filtered = byte_stream
            .map_err(|e| LlmError::Http(e.to_string()))
            .map_ok(|chunk| {
                let text = String::from_utf8_lossy(&chunk).to_string();
                let mut deltas = Vec::new();
                for line in text.split('\n') {
                    let line = line.trim();
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }
                        if let Ok(delta) = serde_json::from_str::<StreamDelta>(data) {
                            if let Some(content) = delta.delta.and_then(|d| d.content) {
                                deltas.push(Ok::<_, LlmError>(content));
                            }
                        }
                    }
                }
                futures::stream::iter(deltas)
            })
            .try_flatten();

        Ok(Box::new(filtered))
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

impl OpenAiProvider {
    /// Extrai o JSON da tool call do texto da resposta (o modelo costuma
    /// embrulhar o JSON em prosa).
    fn parse_tool_call(response: &str) -> Option<serde_json::Value> {
        let start = response.find('{')?;
        let end = response.rfind('}')?;
        let json_str = &response[start..=end];
        let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
        if value.get("tool").is_some() || value.get("name").is_some() {
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_call_from_json() {
        let response = r#"I'll use compression. {"tool": "compression", "params": {"ratio": 4.0}}"#;
        let tc = OpenAiProvider::parse_tool_call(response);
        assert!(tc.is_some());
        assert_eq!(tc.unwrap()["tool"], "compression");
    }

    #[test]
    fn test_parse_tool_call_none() {
        let response = "I don't think any tool is needed here.";
        assert!(OpenAiProvider::parse_tool_call(response).is_none());
    }

    #[test]
    fn test_construction() {
        let p = OpenAiProvider::new("https://api.openai.com/v1", "gpt-4o", "sk-test");
        assert_eq!(p.model_id(), "gpt-4o");
        assert!(p.supports_tools());
        assert_eq!(p.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_with_timeout() {
        let p = OpenAiProvider::new("http://localhost", "test", "test")
            .with_timeout(Duration::from_secs(60));
        assert_eq!(p.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_trim_trailing_slash() {
        let p = OpenAiProvider::new("https://api.openai.com/v1/", "gpt-4o", "x");
        assert_eq!(p.base_url, "https://api.openai.com/v1");
    }
}
