use crate::config::AppConfig;
use crate::routes::proposals::ProposalStore;
use crate::sse::hub::EventHub;
use audio_agent::llm::mock::MockLlm;
use audio_agent::ReActOrchestrator;
use audio_core::ports::repo_trait::AudioRepo;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type Orchestrator = ReActOrchestrator<MockLlm>;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn AudioRepo>,
    #[allow(dead_code)]
    pub orchestrator: Arc<Orchestrator>,
    pub config: Arc<AppConfig>,
    pub hub: Arc<EventHub>,
    pub proposal_store: Arc<RwLock<ProposalStore>>,
}

impl AppState {
    pub fn new(
        repo: Arc<dyn AudioRepo>,
        orchestrator: Arc<Orchestrator>,
        config: Arc<AppConfig>,
        hub: Arc<EventHub>,
    ) -> Self {
        Self {
            repo,
            orchestrator,
            config,
            hub,
            proposal_store: ProposalStore::new(),
        }
    }
}
