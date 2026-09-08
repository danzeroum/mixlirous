use crate::state::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};

/// `GET /healthz` — liveness. Sem auth (docs/03-CONTRATOS-API.md §3.1).
pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// `GET /readyz` — readiness: repo acessível. Sem auth.
pub async fn readyz(State(state): State<AppState>) -> Json<Value> {
    let repo_ok = state.repo.list_jobs(uuid::Uuid::nil()).await.is_ok();
    Json(json!({ "status": if repo_ok { "ok" } else { "degraded" } }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_healthz_returns_ok() {
        let Json(body) = healthz().await;
        assert_eq!(body["status"], "ok");
    }
}
