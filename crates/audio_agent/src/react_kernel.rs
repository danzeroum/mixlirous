use crate::{tools::AudioToolDef, validator::ValidationLayer};
use serde_json::Value;
use std::sync::Arc;

/// Orquestrador cognitivo baseado no padrão ReAct
/// Executa um loop de raciocínio + ação com budget finito de ferramentas
pub struct ReActOrchestrator {
    validation_layer: Arc<ValidationLayer>,
    max_tools: usize,
}

impl ReActOrchestrator {
    pub fn new(validation_layer: Arc<ValidationLayer>, max_tools: usize) -> Self {
        Self {
            validation_layer,
            max_tools,
        }
    }

    /// Executa o loop ReAct completo
    pub async fn run(&self, user_prompt: &str, context: &Value) -> Result<ReActOutput, ReActError> {
        let mut thoughts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut current_context = context.clone();

        for step in 0..self.max_tools {
            // 1. Gerar raciocínio + tool call
            let llm_request = self.build_llm_request(user_prompt, &current_context, step);
            let llm_response = self.call_llm(&llm_request).await?;

            thoughts.push(llm_response.thought.clone());

            // 2. Validar e tipar a tool call
            if let Some(tool_call) = &llm_response.tool_call {
                let validated = self
                    .validation_layer
                    .validate_tool_call(tool_call, &current_context)
                    .map_err(|e| ReActError::Validation(format!("Step {step}: {e}")))?;

                // 3. Executar ferramenta e atualizar contexto
                let tool_output = self.execute_tool(&validated).await?;
                current_context = self.update_context(&current_context, &validated, &tool_output);
                tool_calls.push(validated);
            } else {
                // Sem tool call — finalizou
                break;
            }
        }

        Ok(ReActOutput {
            thoughts,
            tool_calls,
            final_context: current_context,
        })
    }

    /// Placeholder — na prática, chama o provedor de LLM (ver `LlmProvider` em
    /// `docs/05-AGENTE-IA-HITL.md` §6) com streaming SSE do raciocínio.
    async fn call_llm(&self, _request: &LLMRequest) -> Result<LLMResponse, ReActError> {
        unimplemented!("integração com LlmProvider fica para a Sprint 2")
    }

    /// Placeholder — monta o prompt final a partir do template + contexto da faixa.
    fn build_llm_request(&self, _prompt: &str, _context: &Value, _step: usize) -> LLMRequest {
        unimplemented!("montagem do prompt fica para a Sprint 2")
    }

    /// Placeholder — despacha a tool call validada para o motor DSP ou storage.
    async fn execute_tool(&self, _tool: &AudioToolDef) -> Result<Value, ReActError> {
        unimplemented!("execução de ferramentas fica para a Sprint 2")
    }

    fn update_context(&self, prev: &Value, _tool: &AudioToolDef, _output: &Value) -> Value {
        prev.clone()
    }
}

pub struct ReActOutput {
    pub thoughts: Vec<String>,
    pub tool_calls: Vec<AudioToolDef>,
    pub final_context: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum ReActError {
    #[error("LLM API error: {0}")]
    LLM(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Tool execution failed: {0}")]
    ToolExecution(String),
    #[error("Timeout")]
    Timeout,
}

pub struct LLMRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub tools_schema: Vec<Value>,
    pub temperature: f32,
}

pub struct LLMResponse {
    pub thought: String,
    pub tool_call: Option<AudioToolDef>,
    pub raw_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_construction() {
        let validator = Arc::new(ValidationLayer::new());
        let orchestrator = ReActOrchestrator::new(validator, 5);
        assert_eq!(orchestrator.max_tools, 5);
    }
}
