use axum::Router;
use std::sync::Arc;
use tokio::net::TcpListener;

mod adapters;
mod config;
mod middleware;
mod routes;
mod state;

use adapters::InMemoryRepo;
use audio_agent::{validator::ValidationLayer, ReActOrchestrator};
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
    let orchestrator = Arc::new(ReActOrchestrator::new(validator, app_config.llm.max_tools));

    let state = AppState {
        repo,
        orchestrator,
        config: Arc::new(app_config),
    };

    let app = Router::new()
        .merge(routes::health_router())
        .nest("/api/v1", routes::api_router())
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Remix AI API listening on 0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}
