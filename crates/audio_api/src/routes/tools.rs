use crate::middleware::AuthContext;
use audio_agent::limits::tool_registry;
use axum::Json;
use serde_json::{json, Value};

/// `GET /api/v1/tools` ÔÇö registry com os limites (docs/03-CONTRATOS-API.md ┬º3.7).
/// A UI l├¬ os limites daqui em vez de hardcodar `max: 3000`.
pub async fn list_tools(AuthContext(_claims): AuthContext) -> Json<Value> {
    Json(json!({ "tools": tool_registry() }))
}
