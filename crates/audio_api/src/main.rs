use axum::Router;
use std::sync::Arc;
use tokio::net::TcpListener;

// Items do `audio_api` (lib) — ver `src/lib.rs`. Antes eram `mod` inline
// no binário; movidos para a lib para integration tests conseguirem
// importar (`use audio_api::worker::Worker;` em tests/e2e.rs).
use audio_agent::{llm::mock::MockLlm, validator::ValidationLayer, ReActOrchestrator};
use audio_api::{
    adapters::{InMemoryRepo, SqliteRepo},
    config::AppConfig,
    middleware::{self, rate_limit::RateLimiter},
    recovery, routes, sse,
    state::AppState,
    storage::LocalFsStorage,
    worker,
};
use audio_core::ports::{AudioRepo, Storage};

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

    // --- Repo: SQLite default, InMemory for tests ---
    let repo: Arc<dyn AudioRepo> = match app_config.database.type_db.as_str() {
        "sqlite" => {
            // Ensure data/ directory exists
            if let Some(db_dir) = app_config.database.url.strip_prefix("sqlite:") {
                let db_path = std::path::Path::new(db_dir);
                if let Some(parent) = db_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let sqlite = SqliteRepo::new(&app_config.database.url).await?;
            Arc::new(sqlite)
        },
        _ => {
            tracing::warn!(db_type = %app_config.database.type_db, "unknown db type, using InMemory");
            InMemoryRepo::new() as Arc<dyn AudioRepo>
        },
    };

    // --- Storage: local_fs ---
    let storage_base = std::path::PathBuf::from("data/storage");
    let storage: Arc<dyn Storage> = Arc::new(LocalFsStorage::new(storage_base)?);

    // --- Orchestrator (MockLlm for now; Ollama adapter exists for future) ---
    let validator = Arc::new(ValidationLayer::new());
    let mock = Arc::new(MockLlm::new());
    let orchestrator = Arc::new(ReActOrchestrator::<MockLlm>::new(
        validator,
        mock,
        app_config.llm.max_tools,
    ));
    let hub = Arc::new(sse::EventHub::new());

    let state = AppState::new(
        repo.clone(),
        orchestrator,
        Arc::new(app_config.clone()),
        hub.clone(),
        storage,
    );

    let dev_slice = std::env::var("MIXLIROUS_DEV_SLICE").as_deref() == Ok("1");
    let mut api = routes::api_router();
    if dev_slice && config_env != "production" {
        tracing::warn!(
            "MIXLIROUS_DEV_SLICE=1 -- rota de diagnostico ATIVA em /api/v1/dev/slice. \
             Ela aceita upload sem autenticacao de aplicacao; nao exponha esta porta \
             sem auth_basic a frente (docs/18-DEPLOY-PUBLICO-NGINX.md)."
        );
        api = api.merge(routes::dev_router());
    } else if dev_slice {
        tracing::error!(
            "MIXLIROUS_DEV_SLICE=1 ignorado sob CONFIG_ENV=production -- \
             rota de diagnostico nao registrada"
        );
    }

    // Build the app: health routes (no state) + API routes (with state)
    let health = routes::health_router();
    let mut app = Router::new()
        .merge(health)
        .nest("/api/v1", api)
        .with_state(state.clone());

    // Rate limiter middleware (optional via config)
    if app_config.features.rate_limit {
        let limiter = Arc::new(RateLimiter::new(60));
        let mw = middleware::rate_limit::rate_limit_middleware(limiter);
        app = app.layer(axum::middleware::from_fn(mw));
    }

    // Boot recovery
    recovery::run_recovery(&state).await.unwrap_or_default();

    // Spawn worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::start_worker(worker_state).await;
    });

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Remix AI API listening on 0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}
