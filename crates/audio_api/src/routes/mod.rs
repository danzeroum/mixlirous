use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post, put},
    Router,
};

mod dev_slice;
mod health;
mod jobs;
mod metrics_endpoint;
mod prompts;
pub mod proposals;
mod sse;
mod system;
mod tenants;
mod tools;
pub mod tracks;
pub mod uploads;

pub fn health_router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/metrics", get(metrics_endpoint::prometheus_metrics))
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
        .route(
            "/jobs/{job_id}/proposals",
            get(proposals::ProposalHandlers::list_proposals),
        )
        .route(
            "/jobs/{job_id}/proposals/{proposal_id}/approve",
            post(proposals::ProposalHandlers::approve_proposal),
        )
        .route(
            "/jobs/{job_id}/proposals/{proposal_id}/reject",
            post(proposals::ProposalHandlers::reject_proposal),
        )
        .route(
            "/jobs/{job_id}/proposals/{proposal_id}/replan",
            post(proposals::ProposalHandlers::replan_proposal),
        )
        .route("/uploads/presign", post(uploads::presign_upload))
        .route("/uploads/{object_key}", put(uploads::upload_put))
        .route(
            "/tracks",
            post(tracks::create_track).get(tracks::list_tracks),
        )
        .route("/tracks/{track_id}", get(tracks::get_track))
        .route("/tracks/{track_id}/peaks", get(tracks::get_track_peaks))
}

/// Rotas de diagnostico. **So entram no router se `MIXLIROUS_DEV_SLICE=1`**
pub fn dev_router() -> Router<AppState> {
    Router::new()
        .route(
            "/dev/slice",
            get(dev_slice::pagina).post(dev_slice::processar),
        )
        .route("/dev/slice/{id}", get(dev_slice::audio))
        .layer(DefaultBodyLimit::max(dev_slice::LIMITE_UPLOAD_BYTES))
}
