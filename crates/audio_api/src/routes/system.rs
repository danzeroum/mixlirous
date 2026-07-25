use crate::middleware::AuthContext;
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Serialize;

/// Provedores que rodam na própria máquina — nenhum dado sai dela. Todo o
/// resto (`deepseek`, `openai`, `anthropic`, ...) é serviço externo: o prompt
/// e os metadados da faixa saem, o áudio nunca (`docs/08-SEGURANCA-MULTITENANCY.md`
/// §8). Lista de um elemento porque hoje só o Ollama é suportado local
/// (ADR-0009) — cresce se um segundo provedor local entrar.
const LOCAL_PROVIDERS: &[&str] = &["ollama"];

pub fn data_egress_for(provider: &str) -> bool {
    !LOCAL_PROVIDERS.contains(&provider)
}

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub version: &'static str,
    pub database_backend: String,
    pub llm_provider: String,
    pub llm_model: String,
    /// `true` quando o provedor ativo é externo — prompt e metadados saem da
    /// máquina. Nome e valor existem para o aviso de consentimento
    /// (`docs/03-ADENDO-R2-CONTRATOS.md` §7) nomear o provedor e dizer se há
    /// saída de dados, não para decidir isso silenciosamente.
    pub data_egress: bool,
    pub cpu_cores: usize,
}

/// `GET /api/v1/system/info` (`docs/03-CONTRATOS-API.md` §3.1) — versão,
/// backend de banco, provedor LLM e núcleos. É a fonte que a tela de
/// consentimento lê para nomear o provedor ativo antes da primeira execução
/// em modo assistido.
pub async fn get_system_info(
    State(state): State<AppState>,
    AuthContext(_claims): AuthContext,
) -> Json<SystemInfo> {
    let provider = state.config.llm.provider.clone();
    Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION"),
        database_backend: state.config.database.type_db.clone(),
        data_egress: data_egress_for(&provider),
        llm_provider: provider,
        llm_model: state.config.llm.model.clone(),
        cpu_cores: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_is_local_no_egress() {
        assert!(!data_egress_for("ollama"));
    }

    #[test]
    fn test_external_providers_have_egress() {
        for provider in ["deepseek", "openai", "anthropic"] {
            assert!(data_egress_for(provider), "{provider} deveria ter egress");
        }
    }
}
