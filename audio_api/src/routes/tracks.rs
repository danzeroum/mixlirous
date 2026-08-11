use crate::middleware::{AuthContext, TenantScope, TraceParent};
use crate::state::AppState;
use axum::{extract::Path, extract::State, http::StatusCode, Json};
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

/// POST /api/v1/tracks
/// Registers a track and enqueues analysis.
pub async fn create_track(
    State(state): State<AppState>,
    AuthContext(claims): AuthContext,
    _trace: TraceParent,
    Json(payload): Json<CreateTrackRequest>,
) -> Result<(StatusCode, Json<TrackResponse>), (StatusCode, String)> {
    let track_id = Uuid::new_v4();

    tracing::info!(
        %track_id,
        object_key = %payload.object_key,
        display_name = %payload.display_name,
        "track registrada"
    );

    // TODO: enqueue analysis job (Sprint 2)
    // For now, just return the track with "pending" status

    Ok((
        StatusCode::ACCEPTED,
        Json(TrackResponse {
            track_id,
            status: "pending".to_string(),
            stream_url: format!("/api/v1/tracks/{track_id}/events"),
            display_name: payload.display_name,
            created_at: chrono::Utc::now().to_rfc3339(),
        }),
    ))
}

/// GET /api/v1/tracks/{track_id}
/// Returns track data including analysis status.
pub async fn get_track(
    State(_state): State<AppState>,
    TenantScope(_tenant_id): TenantScope,
    Path(track_id): Path<Uuid>,
) -> Result<(StatusCode, Json<TrackResponse>), (StatusCode, String)> {
    // TODO: fetch track from database (Sprint 2)
    // For now, return a placeholder response

    Ok((
        StatusCode::OK,
        Json(TrackResponse {
            track_id,
            status: "pending".to_string(),
            stream_url: format!("/api/v1/tracks/{track_id}/events"),
            display_name: "Unknown".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }),
    ))
}

/// GET /api/v1/tracks/{track_id}/peaks?resolution=1024
/// Returns waveform peaks for visualization.
pub async fn get_track_peaks(
    State(_state): State<AppState>,
    TenantScope(_tenant_id): TenantScope,
    Path(track_id): Path<Uuid>,
) -> Result<(StatusCode, Json<TrackPeaksResponse>), (StatusCode, String)> {
    // TODO: compute peaks from audio file (Sprint 2)
    // For now, return empty peaks

    tracing::info!(%track_id, "peaks requested");

    Ok((
        StatusCode::OK,
        Json(TrackPeaksResponse {
            resolution: 1024,
            peaks: Vec::new(),
        }),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_track_request_deserializes() {
        let json = r#"{
            "object_key": "tenant-123/raw/test.wav",
            "display_name": "Test Track"
        }"#;
        let req: CreateTrackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.object_key, "tenant-123/raw/test.wav");
        assert_eq!(req.display_name, "Test Track");
        assert!(req.project_id.is_none());
    }

    #[test]
    fn test_create_track_request_with_project() {
        let json = r#"{
            "object_key": "tenant-123/raw/test.wav",
            "display_name": "Test Track",
            "project_id": "550e8400-e29b-41d4-a716-446655440000"
        }"#;
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