use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

mod dev_slice;
mod health;
mod jobs;
mod prompts;
mod sse;
mod system;
mod tenants;
mod tools;

pub fn health_router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/jobs", post(jobs::create_job).get(jobs::list_jobs))
        .route("/jobs/{job_id}", get(jobs::get_job))
        .route("/jobs/{job_id}/cancel", post(jobs::cancel_job))
        .route("/jobs/{job_id}/events", get(sse::job_stream))
        .route("/prompts", get(prompts::list_prompts))
        .route("/prompts/{prompt_id}", get(prompts::get_prompt))
        .route("/tools", get(tools::list_tools))
        .route("/tenants/me/quota", get(tenants::get_quota))
        .route(
            "/tenants/me/consent",
            get(tenants::get_consent).post(tenants::post_consent),
        )
        .route("/system/info", get(system::get_system_info))
}

/// Rotas de diagnóstico. **Só entram no router se `MIXLIROUS_DEV_SLICE=1`**
/// (ver `main.rs`) — não existem por padrão.
///
/// Ficam sem `AuthContext` de propósito: quem protege é o `auth_basic` do
/// nginx à frente (`docs/18-DEPLOY-PUBLICO-NGINX.md`). Não exponha o vhost
/// sem ele.
pub fn dev_router() -> Router<AppState> {
    Router::new()
        .route(
            "/dev/slice",
            get(dev_slice::pagina).post(dev_slice::processar),
        )
        .route("/dev/slice/{id}", get(dev_slice::audio))
        // O default do axum é 2 MB — uma faixa real em WAV passa de 50 MB.
        .layer(DefaultBodyLimit::max(dev_slice::LIMITE_UPLOAD_BYTES))
}
