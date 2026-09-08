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
    /// Tamanho declarado do arquivo. Usado para recusar cedo (QA-0001):
    /// falhar no presign com 413 é melhor do que falhar no meio do PUT.
    pub size_bytes: u64,
    pub content_type: String,
}

/// Teto de corpo do upload — a rota REAL (`PUT /uploads/{*object_key}`) e a
/// rota de diagnóstico (`dev_router`) usam este MESMO número, e ele casa com
/// o `client_max_body_size 100M` do nginx de produção
/// (`docs/18-DEPLOY-PUBLICO-NGINX.md`). Descasar qualquer dos três faz o
/// upload morrer com 413 numa camada sem que a outra explique o motivo.
///
/// Por que 100 MB: uma faixa real em WAV passa de 50 MB (o default do axum
/// é 2 MB — o bug QA-0001), e o teto de duração (`LIMITE_DURACAO_SEG` do
/// dev_slice) rejeita na entrada o que passar do que a VPS aguenta.
pub const LIMITE_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

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

    // QA-0001: recusa cedo quando o tamanho declarado já excede o teto —
    // o PUT falharia com 413 depois de o cliente subir metade do arquivo.
    if payload.size_bytes > LIMITE_UPLOAD_BYTES as u64 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "arquivo de {} bytes excede o teto de upload de {} MB",
                payload.size_bytes,
                LIMITE_UPLOAD_BYTES / 1024 / 1024
            ),
        ));
    }
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
