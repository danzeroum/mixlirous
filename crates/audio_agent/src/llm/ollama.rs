use super::{LlmError, LlmProvider, LlmRequest, LlmResponse};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    #[serde(default)]
    response: String,
}

pub struct OllamaProvider {
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    /// Default Ollama running locally
    pub fn local(model: &str) -> Self {
        Self::new("http://localhost:11434", model)
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let ollama_req = OllamaRequest {
            model: self.model.clone(),
            prompt: req.user_prompt.clone(),
            system: req.system_prompt.clone(),
            stream: false,
            temperature: req.temperature,
        };

        let client = reqwest::Client::new();
        let url = format!("{}/api/generate", self.base_url);

        let resp = client
            .post(&url)
            .json(&ollama_req)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        let ollama_resp: OllamaResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        // Parse tool call from response if present
        let tool_call = Self::parse_tool_call(&ollama_resp.response);

        Ok(LlmResponse {
            thought: ollama_resp.response.clone(),
            tool_call,
            raw_json: ollama_resp.response,
        })
    }

    async fn stream(
        &self,
        _req: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Send + Unpin>, LlmError>
    {
        Err(LlmError::Provider(
            "Streaming not yet implemented for Ollama".to_string(),
        ))
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        true // Ollama supports function calling
    }
}

impl OllamaProvider {
    fn parse_tool_call(response: &str) -> Option<serde_json::Value> {
        // Try to extract JSON tool call from response
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                let json_str = &response[start..=end];
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if value.get("tool").is_some() || value.get("name").is_some() {
                        return Some(value);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_call_from_json() {
        let response = r#"I'll use compression. {"tool": "compression", "params": {"ratio": 4.0}}"#;
        let tc = OllamaProvider::parse_tool_call(response);
        assert!(tc.is_some());
        assert_eq!(tc.unwrap()["tool"], "compression");
    }

    #[test]
    fn test_parse_tool_call_none() {
        let response = "I don't think any tool is needed here.";
        let tc = OllamaProvider::parse_tool_call(response);
        assert!(tc.is_none());
    }
}
