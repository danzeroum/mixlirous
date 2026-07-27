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

    // Rota de diagnóstico da fatia vertical: fail-closed, atrás de uma
    // variável própria.
    //
    // `CONFIG_ENV` não serve como trava aqui. O compose de VPS roda
    // `CONFIG_ENV=default` (ver `docker-compose.yml`), então `== "local"`
    // desligaria a rota justamente no servidor onde ela precisa rodar; e
    // `!= "production"` é a formulação que este repositório já removeu uma
    // vez como falha de segurança (ver o comentário em `middleware/auth.rs`
    // — `default` é modo VPS real, não o laptop do desenvolvedor).
    // `#[cfg(debug_assertions)]` também não serve: a imagem Docker é
    // `--release`.
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
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Remix AI API listening on 0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}
