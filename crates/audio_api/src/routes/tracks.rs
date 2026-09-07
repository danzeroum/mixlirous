use crate::middleware::{AuthContext, TenantScope, TraceParent};
use crate::state::AppState;
use audio_core::ports::repo_trait::{TrackRecord, TrackStatus};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
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

/// Parâmetros de query de `GET /tracks/{id}/peaks` (contrato docs/03 §3.2:
/// `?resolution=1024`).
#[derive(Debug, Deserialize)]
pub struct PeaksQuery {
    resolution: Option<u32>,
}

/// Reduz o PCM intercalado para `resolution` buckets [min, max].
///
/// C9 (CHANGELOG): a rota retornava sempre `[]` — a waveform (feature de
/// UX mais visível do plano, adendo Pareto §3.5) nunca existiu de fato.
/// O min/max por bucket é sobre TODAS as amostras do bucket, de todos os
/// canais (intercalado) — para desenhar a envoltória isso é o mesmo
/// resultado que por canal, com metade do trabalho.
pub fn compute_peaks(interleaved: &[f32], resolution: u32) -> Vec<[f32; 2]> {
    let resolution = resolution.clamp(1, 8192) as usize;
    if interleaved.is_empty() {
        return Vec::new();
    }
    let bucket_len = interleaved.len().div_ceil(resolution);
    let mut peaks = Vec::with_capacity(resolution);
    for bucket in interleaved.chunks(bucket_len) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &s in bucket {
            if !s.is_finite() {
                continue; // I15: amostra NaN/Inf não vira pico fantasma
            }
            if s < min {
                min = s;
            }
            if s > max {
                max = s;
            }
        }
        if min == f32::MAX || max == f32::MIN {
            // bucket inteiro não-finito — plano zero, sem inventar pico
            peaks.push([0.0, 0.0]);
        } else {
            peaks.push([min, max]);
        }
    }
    peaks
}

/// GET /api/v1/tracks/{track_id}/peaks?resolution=1024
///
/// Implementação real (C9): lê o objeto do storage, decodifica para PCM
/// (`spawn_blocking` — decode é CPU-bound, docs/02 §4) e reduz para
/// `resolution` buckets [min, max]. Sem pico de memória além do próprio
/// arquivo: o decode acontece uma vez, sobre os bytes já em memória.
pub async fn get_track_peaks(
    State(state): State<AppState>,
    _auth: AuthContext,
    TenantScope(tenant_id): TenantScope,
    Path(track_id): Path<Uuid>,
    Query(params): Query<PeaksQuery>,
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

    let resolution = params.resolution.unwrap_or(1024).clamp(1, 8192);

    let bytes = state
        .storage
        .get(&track.object_key)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("audio_unavailable: {e}")))?;

    let decoded = tokio::task::spawn_blocking(move || audio_core::decode_to_pcm(&bytes))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("decode join: {e}")))?
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("decode: {e}")))?;

    let peaks = tokio::task::spawn_blocking(move || compute_peaks(&decoded.interleaved, resolution))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("peaks join: {e}")))?;

    Ok((
        StatusCode::OK,
        Json(TrackPeaksResponse {
            resolution,
            peaks,
        }),
    ))
}

/// GET /api/v1/tracks/{track_id}/raw — stream do arquivo original.
///
/// Complemento necessário do Lote 2 (item 2, adendo Pareto §3.6): o player
/// A/B da UI precisa do áudio ORIGINAL por `track_id` — hoje ele depende de
/// upload manual no próprio player ("a costura mais estranha do fluxo").
/// Não há endpoint documentado para isso; este segue as mesmas regras de
/// escopo do `download_artifact` (tenant-scoped, 404 para outro tenant).
pub async fn get_track_raw(
    State(state): State<AppState>,
    _auth: AuthContext,
    TenantScope(tenant_id): TenantScope,
    Path(track_id): Path<Uuid>,
) -> Result<Response, (StatusCode, String)> {
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

    let bytes = state
        .storage
        .get(&track.object_key)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("audio_unavailable: {e}")))?;

    // `inline` — o player A/B toca direto; é o navegador que decide o
    // download via atributo `download` do <a>, não aqui.
    let body = Body::from(bytes);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/wav")
        .header(header::CONTENT_DISPOSITION, "inline")
        .header(header::CACHE_CONTROL, "private, max-age=0, must-revalidate")
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("body: {e}")))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_track_request_deserializes() {
        let json = r#"{ "object_key": "tenant-123/raw/test.wav", "display_name": "Test Track" }"#;
        let req: CreateTrackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.object_key, "tenant-123/raw/test.wav");
        assert_eq!(req.display_name, "Test Track");
        assert!(req.project_id.is_none());
    }

    #[test]
    fn test_create_track_request_with_project() {
        let json = r#"{ "object_key": "tenant-123/raw/test.wav", "display_name": "Test Track", "project_id": "550e8400-e29b-41d4-a716-446655440000" }"#;
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

    /// C9 — comportamento do reducer de picos: 8 amostras em 4 buckets.
    #[test]
    fn compute_peaks_min_max_por_bucket() {
        let pcm = vec![0.0f32, 0.5, -0.5, 0.25, -0.25, 1.0, -1.0, 0.1];
        let peaks = compute_peaks(&pcm, 4);
        assert_eq!(peaks.len(), 4);
        assert_eq!(peaks[0], [0.0, 0.5]);
        assert_eq!(peaks[1], [-0.5, 0.25]);
        assert_eq!(peaks[2], [-0.25, 1.0]);
        assert_eq!(peaks[3], [-1.0, 0.1]);
    }

    /// Bucket não divide igual: 5 amostras em 2 buckets (3+2).
    #[test]
    fn compute_peaks_bucket_desigual() {
        let pcm = vec![1.0f32, -1.0, 0.5, 0.25, -0.75];
        let peaks = compute_peaks(&pcm, 2);
        assert_eq!(peaks.len(), 2);
        assert_eq!(peaks[0], [-1.0, 1.0]);
        assert_eq!(peaks[1], [-0.75, 0.25]);
    }

    /// Resolution maior que o número de amostras: buckets vazios não
    /// aparecem (último bucket parcial só).
    #[test]
    fn compute_peaks_resolution_maior_que_amostras() {
        let pcm = vec![0.3f32, -0.3];
        let peaks = compute_peaks(&pcm, 1024);
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0], [-0.3, 0.3]);
    }

    /// Amostra NaN não vira pico fantasma (I15 na costura do peaks).
    #[test]
    fn compute_peaks_ignora_amostra_nao_finita() {
        let pcm = vec![0.5f32, f32::NAN, -0.5, 0.2];
        let peaks = compute_peaks(&pcm, 1);
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0], [-0.5, 0.5]);
    }

    /// Entrada vazia → resposta vazia (nunca pânico de div/zero).
    #[test]
    fn compute_peaks_entrada_vazia() {
        assert!(compute_peaks(&[], 1024).is_empty());
    }

    /// Query param: default 1024 (clamping no handler), clamp no reducer.
    #[test]
    fn peaks_query_default_e_clamp() {
        let q: PeaksQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.resolution, None);
        assert_eq!(compute_peaks(&[0.1f32], u32::MAX).len(), 1);
    }
}
