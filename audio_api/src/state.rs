use crate::config::AppConfig;
use crate::sse::hub::EventHub;
use audio_agent::ReActOrchestrator;
use audio_core::ports::repo_trait::AudioRepo;
use std::sync::Arc;

#[allow(dead_code)]
#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn AudioRepo>,
    pub orchestrator: Arc<ReActOrchestrator>,
    pub config: Arc<AppConfig>,
    pub hub: Arc<EventHub>,
}

impl AppState {
    pub fn new(
        repo: Arc<dyn AudioRepo>,
        orchestrator: Arc<ReActOrchestrator>,
        config: Arc<AppConfig>,
        hub: Arc<EventHub>,
    ) -> Self {
        Self {
            repo,
            orchestrator,
            config,
            hub,
        }
    }
}