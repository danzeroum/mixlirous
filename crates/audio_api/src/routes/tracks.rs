use crate::middleware::{AuthContext, TenantScope, TraceParent};
use crate::state::AppState;
use audio_core::ports::repo_trait::{TrackRecord, TrackStatus};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateTrackRequest {
    pub object_key: String,
    pub display_name: String,
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct TrackResponse {
    pub track_id: Uuid,
    pub status: String,
    pub stream_url: String,
    pub display_name: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct TrackPeaksResponse {
    pub resolution: u32,
    pub peaks: Vec<[f32; 2]>,
}

impl From<&TrackRecord> for TrackResponse {
    fn from(t: &TrackRecord) -> Self {
        Self {
            track_id: t.id,
            status: format!("{:?}", t.status),
            stream_url: format!("/api/v1/tracks/{}/events", t.id),
            display_name: t.display_name.clone(),
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

/// POST /api/v1/tracks
/// Registers a track in the database.
pub async fn create_track(
    State(state): State<AppState>,
    AuthContext(_claims): AuthContext,
    TenantScope(tenant_id): TenantScope,
    _trace: TraceParent,
    Json(payload): Json<CreateTrackRequest>,
) -> Result<(StatusCode, Json<TrackResponse>), (StatusCode, String)> {
    let track_id = Uuid::new_v4();
    let now = Utc::now();

    let track = TrackRecord {
        id: track_id,
        tenant_id,
        project_id: payload.project_id,
        object_key: payload.object_key,
        display_name: payload.display_name,
        status: TrackStatus::Uploaded,
        duration_sec: None,
        sample_rate: None,
        channels: None,
        sha256: None,
        analysis: None,
        created_at: now,
        updated_at: now,
    };

    state.repo.save_track(&track).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save_track: {e}"),
        )
    })?;

    tracing::info!(%track_id, "track registered");

    let resp = TrackResponse::from(&track);
    Ok((StatusCode::CREATED, Json(resp)))
}

/// GET /api/v1/tracks/{track_id}
pub async fn get_track(
    State(state): State<AppState>,
    _auth: AuthContext,
    TenantScope(tenant_id): TenantScope,
    Path(track_id): Path<Uuid>,
) -> Result<(StatusCode, Json<TrackResponse>), (StatusCode, String)> {
    let track = state
        .repo
        .get_track(track_id, tenant_id)
        .await
        .map_err(|e| match e {
            audio_core::ports::repo_trait::RepoError::NotFound(_) => (
                StatusCode::NOT_FOUND,
                format!("track not found: {track_id}"),
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("get_track: {e}")),
        })?;

    Ok((StatusCode::OK, Json(TrackResponse::from(&track))))
}

/// GET /api/v1/tracks
pub async fn list_tracks(
    State(state): State<AppState>,
    _auth: AuthContext,
    TenantScope(tenant_id): TenantScope,
) -> Result<Json<Vec<TrackResponse>>, (StatusCode, String)> {
    let tracks = state.repo.list_tracks(tenant_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("list_tracks: {e}"),
        )
    })?;

    Ok(Json(tracks.iter().map(TrackResponse::from).collect()))
}

/// GET /api/v1/tracks/{track_id}/peaks?resolution=1024
/// Returns waveform peaks for visualization.
pub async fn get_track_peaks(
    State(state): State<AppState>,
    _auth: AuthContext,
    TenantScope(tenant_id): TenantScope,
    Path(track_id): Path<Uuid>,
) -> Result<(StatusCode, Json<TrackPeaksResponse>), (StatusCode, String)> {
    let track = state
        .repo
        .get_track(track_id, tenant_id)
        .await
        .map_err(|e| match e {
            audio_core::ports::repo_trait::RepoError::NotFound(_) => (
                StatusCode::NOT_FOUND,
                format!("track not found: {track_id}"),
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("get_track: {e}")),
        })?;

    // TODO: compute peaks from audio file via storage.get + decode_to_pcm
    // in spawn_blocking. For now return empty.
    let _ = track.object_key;
    let peaks = Vec::new();

    Ok((
        StatusCode::OK,
        Json(TrackPeaksResponse {
            resolution: 1024,
            peaks,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_track_request_deserializes() {
        let json =
            concat!(r#"{ "object_key": "tenant-123/raw/test.wav", "display_name": "Test Track" }"#);
        let req: CreateTrackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.object_key, "tenant-123/raw/test.wav");
        assert_eq!(req.display_name, "Test Track");
        assert!(req.project_id.is_none());
    }

    #[test]
    fn test_create_track_request_with_project() {
        let json = concat!(
            r#"{ "object_key": "tenant-123/raw/test.wav", "#,
            r#""display_name": "Test Track", "#,
            r#""project_id": "550e8400-e29b-41d4-a716-446655440000" }"#
        );
        let req: CreateTrackRequest = serde_json::from_str(json).unwrap();
        assert!(req.project_id.is_some());
    }

    #[test]
    fn test_track_response_serializes() {
        let resp = TrackResponse {
            track_id: Uuid::new_v4(),
            status: "pending".to_string(),
            stream_url: "/api/v1/tracks/123/events".to_string(),
            display_name: "Test".to_string(),
            created_at: "2026-08-10T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("pending"));
        assert!(json.contains("Test"));
    }

    #[test]
    fn test_track_peaks_response_serializes() {
        let resp = TrackPeaksResponse {
            resolution: 1024,
            peaks: vec![[-0.5, 0.5], [-0.3, 0.3]],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("1024"));
        assert!(json.contains("-0.5"));
    }
}
