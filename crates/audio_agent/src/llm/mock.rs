use super::{LlmError, LlmProvider, LlmRequest, LlmResponse};
use async_trait::async_trait;
use std::collections::HashMap;

/// Mock LLM provider for testing ReAct scenarios A1-A10
pub struct MockLlm {
    responses: HashMap<String, LlmResponse>,
    model: String,
}

impl MockLlm {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            model: "mock".to_string(),
        }
    }

    pub fn with_response(mut self, key: &str, response: LlmResponse) -> Self {
        self.responses.insert(key.to_string(), response);
        self
    }
}

impl Default for MockLlm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for MockLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        // Return response based on request content
        for (key, response) in &self.responses {
            if req.user_prompt.contains(key) {
                return Ok(response.clone());
            }
        }

        // Default: return empty response (no tool call)
        Ok(LlmResponse {
            thought: "Mock LLM response".to_string(),
            tool_call: None,
            raw_json: "{}".to_string(),
        })
    }

    async fn stream(
        &self,
        _req: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Send + Unpin>, LlmError>
    {
        Err(LlmError::Provider(
            "Mock does not support streaming".to_string(),
        ))
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_returns_default() {
        let mock = MockLlm::new();
        let req = LlmRequest {
            system_prompt: "test".to_string(),
            user_prompt: "test".to_string(),
            tools_schema: vec![],
            temperature: 0.3,
        };
        let resp = mock.complete(req).await.unwrap();
        assert_eq!(resp.thought, "Mock LLM response");
        assert!(resp.tool_call.is_none());
    }

    #[tokio::test]
    async fn test_mock_returns_specific_response() {
        let mock = MockLlm::new().with_response(
            "compression",
            LlmResponse {
                thought: "Applying compression".to_string(),
                tool_call: Some(
                    serde_json::json!({"name": "compression", "params": {"ratio": 4.0}}),
                ),
                raw_json: "{}".to_string(),
            },
        );

        let req = LlmRequest {
            system_prompt: "test".to_string(),
            user_prompt: "use compression".to_string(),
            tools_schema: vec![],
            temperature: 0.3,
        };
        let resp = mock.complete(req).await.unwrap();
        assert!(resp.tool_call.is_some());
    }
}
