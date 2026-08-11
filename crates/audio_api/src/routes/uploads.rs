use crate::middleware::{AuthContext, TenantScope, TraceParent};
use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PresignRequest {
    pub filename: String,
    #[allow(dead_code)]
    pub size_bytes: u64,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct PresignResponse {
    pub object_key: String,
    pub upload_url: String,
    pub method: String,
    pub headers: std::collections::HashMap<String, String>,
    pub expires_at: String,
}

/// POST /api/v1/uploads/presign
/// Generates a presigned URL for direct upload. In local mode, returns
/// a URL pointing to PUT /api/v1/uploads/{object_key} on this server.
pub async fn presign_upload(
    State(_state): State<AppState>,
    AuthContext(_claims): AuthContext,
    TenantScope(tenant_id): TenantScope,
    _trace: TraceParent,
    Json(payload): Json<PresignRequest>,
) -> Result<(StatusCode, Json<PresignResponse>), (StatusCode, String)> {
    // Sanitize filename: remove path separators, keep only the base name
    let safe_name = payload
        .filename
        .rsplit('/')
        .next()
        .or_else(|| payload.filename.rsplit('\\').next())
        .unwrap_or("unknown");
    // Strip any remaining path components from the safe name
    let safe_name = safe_name.rsplit(['/', '\\']).next().unwrap_or("unknown");

    let object_key = format!("tenant-{}/raw/{}", tenant_id, safe_name);
    let upload_url = format!("/api/v1/uploads/{object_key}");

    let mut headers = std::collections::HashMap::new();
    headers.insert("Content-Type".to_string(), payload.content_type);

    let expires_at = (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339();

    Ok((
        StatusCode::OK,
        Json(PresignResponse {
            object_key,
            upload_url,
            method: "PUT".to_string(),
            headers,
            expires_at,
        }),
    ))
}

/// PUT /api/v1/uploads/{object_key}
/// Receives raw bytes and stores via Storage.
pub async fn upload_put(
    State(state): State<AppState>,
    AuthContext(_claims): AuthContext,
    TenantScope(tenant_id): TenantScope,
    Path(object_key): Path<String>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    // Verify the object_key belongs to this tenant
    let expected_prefix = format!("tenant-{tenant_id}/");
    if !object_key.starts_with(&expected_prefix) {
        return Err((
            StatusCode::FORBIDDEN,
            "object_key does not belong to your tenant".to_string(),
        ));
    }

    state.storage.put(&object_key, body).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("storage put: {e}"),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presign_request_deserializes() {
        let json = r#"{
            "filename": "test.wav",
            "size_bytes": 1048576,
            "content_type": "audio/wav"
        }"#;
        let req: PresignRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.filename, "test.wav");
        assert_eq!(req.size_bytes, 1048576);
        assert_eq!(req.content_type, "audio/wav");
    }

    #[test]
    fn test_presign_response_serializes() {
        let resp = PresignResponse {
            object_key: "tenant-123/raw/test.wav".to_string(),
            upload_url: "/api/v1/uploads/tenant-123/raw/test.wav".to_string(),
            method: "PUT".to_string(),
            headers: std::collections::HashMap::new(),
            expires_at: "2026-08-10T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("PUT"));
        assert!(json.contains("tenant-123"));
    }
}
