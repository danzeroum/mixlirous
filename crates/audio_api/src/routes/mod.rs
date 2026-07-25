use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

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
