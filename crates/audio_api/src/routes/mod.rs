use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post, put},
    Router,
};

mod auth;
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
        .route("/jobs/{job_id}/artifact", get(jobs::download_artifact))
        .route("/jobs/{job_id}/cancel", post(jobs::cancel_job))
        // Lote 2 (C12): requeue simples de job failed — docs/03 §3.3.
        .route("/jobs/{job_id}/retry", post(jobs::retry_job))
        .route("/jobs/{job_id}/events", get(sse::job_stream))
        .route("/prompts", get(prompts::list_prompts))
        .route("/prompts/{prompt_id}", get(prompts::get_prompt))
        .route("/tools", get(tools::list_tools))
        .route("/tenants/me/quota", get(tenants::get_quota))
        .route(
            "/tenants/me/consent",
            get(tenants::get_consent)
                .post(tenants::post_consent)
                // Revogação REAL do consentimento (plano de design §LGPD):
                // remove o registro ativo; uso futuro volta a exigir aceite.
                .delete(tenants::delete_consent),
        )
        .route("/system/info", get(system::get_system_info))
        // Política de privacidade auditável — a UI só declara "o áudio não
        // é enviado" quando este endpoint (testado) diz audio_sent=false.
        .route("/system/privacy-policy", get(system::get_privacy_policy))
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
        // Lote 2 (issue #33): sessão local + cookie de SSE same-origin.
        .route("/auth/local-session", get(auth::get_local_session))
        .route("/auth/sse-session", post(auth::post_sse_session))
        // Upload + Tracks
        .route("/uploads/presign", post(uploads::presign_upload))
        // QA-0001: o default do axum é 2 MB — o upload REAL de WAV (uma
        // faixa passa de 50 MB; ver uploads.rs) tomava 413 aqui, embora o
        // nginx de produção já liberasse 100 MB (client_max_body_size em
        // docs/18). Mesmo teto da rota de diagnóstico, fonte única:
        // uploads::LIMITE_UPLOAD_BYTES.
        .route(
            "/uploads/{*object_key}",
            put(uploads::upload_put).layer(DefaultBodyLimit::max(
                uploads::LIMITE_UPLOAD_BYTES,
            )),
        )
        .route(
            "/tracks",
            post(tracks::create_track).get(tracks::list_tracks),
        )
        .route("/tracks/{track_id}", get(tracks::get_track))
        .route("/tracks/{track_id}/peaks", get(tracks::get_track_peaks))
        // Lote 2 (item 2): áudio ORIGINAL por track_id — alimenta o lado
        // "original" do player A/B sem upload manual.
        .route("/tracks/{track_id}/raw", get(tracks::get_track_raw))
}

/// Rotas de diagnostico. **So entram no router se `MIXLIROUS_DEV_SLICE=1`**
/// (ver `main.rs`) -- nao existem por padrao.
///
/// Ficam sem `AuthContext` de proposito: quem protege e o `auth_basic` do
/// nginx a frente (`docs/18-DEPLOY-PUBLICO-NGINX.md`). Nao exponha o vhost
/// sem ele.
pub fn dev_router() -> Router<AppState> {
    Router::new()
        .route(
            "/dev/slice",
            get(dev_slice::pagina).post(dev_slice::processar),
        )
        .route("/dev/slice/{id}", get(dev_slice::audio))
        // O default do axum e 2 MB -- uma faixa real em WAV passa de 50 MB.
        // Mesma constante da rota real de upload (QA-0001).
        .layer(DefaultBodyLimit::max(uploads::LIMITE_UPLOAD_BYTES))
}
