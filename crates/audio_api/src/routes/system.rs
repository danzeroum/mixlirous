use crate::middleware::AuthContext;
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Serialize;

/// Provedores que rodam na pr├│pria m├íquina ÔÇö nenhum dado sai dela. Todo o
/// resto (`deepseek`, `openai`, `anthropic`, ...) ├® servi├ºo externo: o prompt
/// e os metadados da faixa saem, o ├íudio nunca (`docs/08-SEGURANCA-MULTITENANCY.md`
/// ┬º8). Lista de um elemento porque hoje s├│ o Ollama ├® suportado local
/// (ADR-0009) ÔÇö cresce se um segundo provedor local entrar.
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
    /// `true` quando o provedor ativo ├® externo ÔÇö prompt e metadados saem da
    /// m├íquina. Nome e valor existem para o aviso de consentimento
    /// (`docs/03-ADENDO-R2-CONTRATOS.md` ┬º7) nomear o provedor e dizer se h├í
    /// sa├¡da de dados, n├úo para decidir isso silenciosamente.
    pub data_egress: bool,
    pub cpu_cores: usize,
}

/// `GET /api/v1/system/info` (`docs/03-CONTRATOS-API.md` ┬º3.1) ÔÇö vers├úo,
/// backend de banco, provedor LLM e n├║cleos. ├ë a fonte que a tela de
/// consentimento l├¬ para nomear o provedor ativo antes da primeira execu├º├úo
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
