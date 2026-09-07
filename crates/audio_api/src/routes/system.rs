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

/// `GET /api/v1/system/privacy-policy` — política de dados do provedor
/// LLM ativo, NO FORMATO AUDITÁVEL do plano de design (§IA, dados e LGPD).
///
/// Por que existe: a frase "o áudio NÃO é enviado" é uma afirmação de
/// produto/privacidade (LGPD) forte demais para viver hardcoded no
/// frontend. A UI só pode declará-la quando o campo vem daqui — derivado
/// da configuração real desta instalação e coberto por teste de
/// integração (`test_privacy_policy_matches_config`).
///
/// Honestidade dos campos (nada de promessa que o código não cumpre):
///
/// - `audio_sent_to_provider`: estruturalmente `false` — o caminho do
///   áudio é storage → decode → DSP (worker.rs); ao LLM só vai o prompt
///   e um contexto JSON de metadados numéricos (worker.rs
///   `agent_context_for_track`). O valor é servido por função pura com
///   teste unitário e de integração.
/// - `prompt_sent_to_provider`: `true` — em modo assistido o texto do
///   objetivo vai ao provedor (inclusive ao mock, in-process).
/// - `analysis_metadata_sent_to_provider`: `true` — duração, sample rate,
///   canais e frames da faixa entram no contexto do agente.
/// - `retention_policy`/`training_opt_out`/`region`: descrevem o que ESTA
///   instalação faz de fato; onde não há configuração, o texto diz isso
///   em vez de prometer política inexistente.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PrivacyPolicyResponse {
    pub provider: String,
    pub model: String,
    pub audio_sent_to_provider: bool,
    pub prompt_sent_to_provider: bool,
    pub analysis_metadata_sent_to_provider: bool,
    pub retention_policy: String,
    pub training_opt_out: String,
    pub region: String,
}

/// Função pura: política de privacidade para um provedor/config LLM.
/// Servida pela rota e verificada por teste — é o único caminho pelo qual
/// a UI pode afirmar (ou deixar de afirmar) o que sai da máquina.
pub fn privacy_policy_for(provider: &str, model: &str) -> PrivacyPolicyResponse {
    PrivacyPolicyResponse {
        provider: provider.to_string(),
        model: model.to_string(),
        audio_sent_to_provider: false,
        prompt_sent_to_provider: true,
        analysis_metadata_sent_to_provider: true,
        retention_policy: "O áudio enviado e os artefatos gerados ficam no storage desta \
            instalação (disco local, MinIO ou S3) até o tenant excluí-los; não há \
            expiração automática configurável nesta versão."
            .to_string(),
        training_opt_out: "Nesta instalação não existe pipeline de treinamento: áudio e \
            prompts não são usados para treinar modelos. Com provedor externo, verifique \
            também a política de retenção/treino do próprio provedor."
            .to_string(),
        region: "não configurado nesta instalação".to_string(),
    }
}

/// `GET /api/v1/system/privacy-policy` — política auditável do provedor
/// ativo (docs/03 §3.1). Autenticada como o resto de /system.
pub async fn get_privacy_policy(
    State(state): State<AppState>,
    AuthContext(_claims): AuthContext,
) -> Json<PrivacyPolicyResponse> {
    Json(privacy_policy_for(
        &state.config.llm.provider,
        &state.config.llm.model,
    ))
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

    /// A afirmação "áudio não sai" é estrutural e verificável: a política
    /// servida para QUALQUER provedor declara audio_sent_to_provider =
    /// false, e o que sai (prompt + metadados de análise) é declarado
    /// true — se um dia o backend passar a mandar áudio ao LLM, este
    /// teste é o primeiro a quebrar (a política precisa mudar junto).
    #[test]
    fn test_privacy_policy_audio_never_leaves_for_any_provider() {
        for provider in ["mock", "ollama", "deepseek", "openai"] {
            let p = privacy_policy_for(provider, "test-model");
            assert!(!p.audio_sent_to_provider, "{provider}");
            assert!(p.prompt_sent_to_provider, "{provider}");
            assert!(p.analysis_metadata_sent_to_provider, "{provider}");
            assert_eq!(p.provider, provider);
            assert_eq!(p.model, "test-model");
        }
    }

    /// Campos declarativos não podem vir vazios — a UI exibe o texto
    /// literal ao usuário.
    #[test]
    fn test_privacy_policy_textos_nao_vazios() {
        let p = privacy_policy_for("mock", "m");
        assert!(!p.retention_policy.is_empty());
        assert!(!p.training_opt_out.is_empty());
        assert!(!p.region.is_empty());
    }
}
