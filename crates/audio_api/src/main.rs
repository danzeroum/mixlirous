use axum::Router;
use std::sync::Arc;
use tokio::net::TcpListener;

mod adapters;
mod atomic;
mod recovery;
mod instrument;
mod config;
mod middleware;
mod routes;
mod sse;
mod state;
mod worker;

use adapters::InMemoryRepo;
use audio_agent::{validator::ValidationLayer, llm::mock::MockLlm, ReActOrchestrator};
use config::AppConfig;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app_config = AppConfig::load()?;

    let config_env = std::env::var("CONFIG_ENV").unwrap_or_else(|_| "local".to_string());
    middleware::auth::assert_secret_configured_for_production(
        &config_env,
        std::env::var("JWT_SECRET").is_ok(),
    );

    let repo = InMemoryRepo::new();
    let validator = Arc::new(ValidationLayer::new());
    let mock = Arc::new(MockLlm::new());
    let orchestrator = Arc::new(ReActOrchestrator::<MockLlm>::new(
        validator,
        mock,
        app_config.llm.max_tools,
    ));
    let hub = Arc::new(sse::EventHub::new());

    let state = AppState::new(repo, orchestrator, Arc::new(app_config), hub.clone());

    let dev_slice = std::env::var("MIXLIROUS_DEV_SLICE").as_deref() == Ok("1");
    let mut api = routes::api_router();
    if dev_slice && config_env != "production" {
        tracing::warn!(
            "MIXLIROUS_DEV_SLICE=1 — rota de diagnóstico ATIVA em /api/v1/dev/slice. \
             Ela aceita upload sem autenticação de aplicação; não exponha esta porta \
             sem auth_basic à frente (docs/18-DEPLOY-PUBLICO-NGINX.md)."
        );
        api = api.merge(routes::dev_router());
    } else if dev_slice {
        tracing::error!(
            "MIXLIROUS_DEV_SLICE=1 ignorado sob CONFIG_ENV=production — \
             rota de diagnóstico não registrada"
        );
    }

    let app = Router::new()
        .merge(routes::health_router())
        .nest("/api/v1", api)
        .with_state(state.clone());

    recovery::run_recovery(&state).await.unwrap_or_default();

    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::start_worker(worker_state).await;
    });

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Remix AI API listening on 0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}