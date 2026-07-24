use crate::config::AppConfig;
use audio_agent::ReActOrchestrator;
use audio_core::ports::repo_trait::AudioRepo;
use std::sync::Arc;

/// Estado compartilhado da aplicação, injetado em todos os handlers via
/// `axum::extract::State`. `orchestrator` e `config` ainda não são lidos por
/// nenhum handler — entram em uso quando o loop ReAct for ligado de fato
/// (Sprint 2), mas já ficam montados aqui para não reabrir a assinatura do
/// estado depois.
#[allow(dead_code)]
#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn AudioRepo>,
    pub orchestrator: Arc<ReActOrchestrator>,
    pub config: Arc<AppConfig>,
}
