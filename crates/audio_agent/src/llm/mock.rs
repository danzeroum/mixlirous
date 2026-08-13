use super::{LlmError, LlmProvider, LlmRequest, LlmResponse};
use async_trait::async_trait;
use futures::stream;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Mock LLM provider para testes dos cenários A1–A10
/// (`docs/10-TESTES-QUALIDADE.md` §4, task 3.11).
///
/// Seleciona resposta por substring no `user_prompt`: a resposta registrada
/// para a chave que aparecer no prompt é devolvida. Sem match, devolve uma
/// resposta neutra (sem tool call). Também suporta injeção de erro, streaming
/// por chunks e sequências de resposta (para simular replan: a primeira
/// resposta errada, a segunda certa).
pub struct MockLlm {
    responses: HashMap<String, LlmResponse>,
    streaming_responses: HashMap<String, Vec<String>>,
    errors: HashMap<String, LlmError>,
    next_responses: Mutex<VecDeque<LlmResponse>>,
    model: String,
}

impl MockLlm {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            streaming_responses: HashMap::new(),
            errors: HashMap::new(),
            next_responses: Mutex::new(VecDeque::new()),
            model: "mock".to_string(),
        }
    }

    pub fn with_response(mut self, key: &str, response: LlmResponse) -> Self {
        self.responses.insert(key.to_string(), response);
        self
    }

    pub fn with_streaming_response(mut self, key: &str, chunks: Vec<String>) -> Self {
        self.streaming_responses.insert(key.to_string(), chunks);
        self
    }

    pub fn with_error(mut self, key: &str, error: LlmError) -> Self {
        self.errors.insert(key.to_string(), error);
        self
    }

    /// Respostas consumidas em ordem (primeiro a primeira). Depois de esgotar,
    /// o mock cai no comportamento normal (match por substring).
    pub fn with_sequence(mut self, responses: Vec<LlmResponse>) -> Self {
        let mut q = VecDeque::new();
        q.extend(responses);
        self.next_responses = Mutex::new(q);
        self
    }

    fn pop_next(&self) -> Option<LlmResponse> {
        self.next_responses.lock().ok()?.pop_front()
    }
}

impl Default for MockLlm {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLlm {
    fn find_error(&self, req: &LlmRequest) -> Option<LlmError> {
        self.errors
            .iter()
            .find(|(k, _)| req.user_prompt.contains(*k))
            .map(|(_, e)| e.clone())
    }

    fn find_response(&self, req: &LlmRequest) -> Option<LlmResponse> {
        self.responses
            .iter()
            .find(|(k, _)| req.user_prompt.contains(*k))
            .map(|(_, r)| r.clone())
    }

    fn find_streaming(&self, req: &LlmRequest) -> Option<Vec<String>> {
        self.streaming_responses
            .iter()
            .find(|(k, _)| req.user_prompt.contains(*k))
            .map(|(_, c)| c.clone())
    }
}

#[async_trait]
impl LlmProvider for MockLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        if let Some(e) = self.find_error(&req) {
            return Err(e);
        }
        if let Some(r) = self.pop_next() {
            return Ok(r);
        }
        if let Some(r) = self.find_response(&req) {
            return Ok(r);
        }

        // Padrão: sem tool call.
        Ok(LlmResponse {
            thought: "Mock LLM response".to_string(),
            tool_call: None,
            raw_json: "{}".to_string(),
        })
    }

    async fn stream(
        &self,
        req: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Send + Unpin>, LlmError>
    {
        if let Some(e) = self.find_error(&req) {
            return Err(e);
        }
        if let Some(chunks) = self.find_streaming(&req) {
            let stream = stream::iter(chunks.into_iter().map(Ok::<_, LlmError>));
            return Ok(Box::new(stream));
        }
        // Fallback: transforma a resposta completa em chunks por palavra.
        if let Some(r) = self.find_response(&req) {
            let words: Vec<String> = r
                .thought
                .split_whitespace()
                .map(|w| format!("{} ", w))
                .collect();
            let stream = stream::iter(words.into_iter().map(Ok::<_, LlmError>));
            return Ok(Box::new(stream));
        }

        let stream = stream::empty();
        Ok(Box::new(stream))
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
    use futures::StreamExt;

    fn req(prompt: &str) -> LlmRequest {
        LlmRequest {
            system_prompt: "test".to_string(),
            user_prompt: prompt.to_string(),
            tools_schema: vec![],
            temperature: 0.3,
        }
    }

    #[tokio::test]
    async fn test_mock_returns_default() {
        let mock = MockLlm::new();
        let resp = mock.complete(req("test")).await.unwrap();
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

        let resp = mock.complete(req("use compression")).await.unwrap();
        assert!(resp.tool_call.is_some());
    }

    #[tokio::test]
    async fn test_mock_returns_error() {
        let mock = MockLlm::new().with_error("timeout", LlmError::Timeout);
        let err = mock.complete(req("cause timeout")).await.unwrap_err();
        assert!(matches!(err, LlmError::Timeout));
    }

    #[tokio::test]
    async fn test_mock_streaming() {
        let mock = MockLlm::new().with_streaming_response(
            "stream_me",
            vec!["chunk1 ".to_string(), "chunk2".to_string()],
        );
        let mut stream = mock.stream(req("stream_me")).await.unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), "chunk1 ");
        assert_eq!(stream.next().await.unwrap().unwrap(), "chunk2");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_mock_streaming_from_response() {
        let mock = MockLlm::new().with_response(
            "compression",
            LlmResponse {
                thought: "a b c".to_string(),
                tool_call: None,
                raw_json: "{}".to_string(),
            },
        );
        let mut stream = mock.stream(req("use compression")).await.unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), "a ");
        assert_eq!(stream.next().await.unwrap().unwrap(), "b ");
        assert_eq!(stream.next().await.unwrap().unwrap(), "c ");
    }
}
