//! Testes de integração HTTP das rotas do Lote 2 (plano Pareto):
//! - `GET /tracks/{id}/peaks` real (C9)
//! - `GET /tracks/{id}/raw` (player A/B por track_id)
//! - `POST /jobs/{id}/cancel` real (C6)
//! - `POST /jobs/{id}/retry` (C12)
//! - `GET /auth/local-session` + `POST /auth/sse-session` (issue #33)
//!
//! Usa `tower::ServiceExt::oneshot` sobre o `api_router()` real com
//! `InMemoryRepo` + `LocalFsStorage` em tempdir — sem rede.

use audio_agent::llm::mock::MockLlm;
use audio_agent::validator::ValidationLayer;
use audio_agent::ReActOrchestrator;
use audio_api::adapters::InMemoryRepo;
use audio_api::config::AppConfig;
use audio_api::middleware::auth::{encode_claims, TenantClaims};
use audio_api::routes::api_router;
use audio_api::state::AppState;
use audio_api::storage::LocalFsStorage;
use audio_core::domain::AudioCodec;
use audio_core::dsp::DefaultMixer;
use audio_core::ndarray::Array1;
use audio_core::ports::repo_trait::{AudioRepo, JobMeta, JobStatus, TrackRecord, TrackStatus};
use audio_core::ports::Storage;
use audio_core::{AudioFormat, PipelineConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::Utc;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_SECRET: &str = "test-secret-routes-lote2";

fn state() -> (AppState, tempfile::TempDir) {
    std::env::set_var("JWT_SECRET", TEST_SECRET);
    let repo: Arc<dyn AudioRepo> = InMemoryRepo::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage: Arc<dyn Storage> =
        Arc::new(LocalFsStorage::new(tmp.path().to_path_buf()).expect("local fs"));
    let validator = Arc::new(ValidationLayer::new());
    let mock = Arc::new(MockLlm::new());
    let orchestrator = Arc::new(ReActOrchestrator::<MockLlm>::new(validator, mock, 5));
    let hub = Arc::new(audio_api::sse::EventHub::new());
    let app = AppState::new(
        repo,
        orchestrator,
        Arc::new(AppConfig::default()),
        hub,
        storage,
    );
    (app, tmp)
}

fn router(state: AppState) -> Router {
    api_router().with_state(state)
}

fn claims(tenant_id: Uuid) -> TenantClaims {
    let now = Utc::now().timestamp() as usize;
    TenantClaims {
        sub: Uuid::new_v4(),
        tenant_id,
        roles: vec!["owner".to_string()],
        plan: "free".to_string(),
        iat: now,
        exp: now + 3600,
    }
}

fn bearer(tenant_id: Uuid) -> String {
    format!(
        "Bearer {}",
        encode_claims(&claims(tenant_id), TEST_SECRET).unwrap()
    )
}

fn get(path: &str, auth: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("Authorization", auth)
        .body(Body::empty())
        .unwrap()
}

fn post(path: &str, auth: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("Authorization", auth)
        .body(Body::empty())
        .unwrap()
}

/// WAV mono válido de 0.1s com amplitude definida (para peaks e raw).
fn wav_bytes(sample_rate: u32) -> Vec<u8> {
    let n = (sample_rate as f32 * 0.1) as usize;
    let pcm: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let config = PipelineConfig {
        format: AudioFormat {
            sample_rate,
            channels: 1,
            bit_depth: 32,
            codec: AudioCodec::WAV,
        },
        ..PipelineConfig::default()
    };
    DefaultMixer
        .encode_wav_to_vec(&Array1::from_vec(pcm), &config)
        .expect("encode wav")
}

/// Track com WAV real no storage; retorna (track_id, tenant, bytes).
async fn track_com_audio(app: &AppState) -> (Uuid, Uuid, Vec<u8>) {
    let tenant_id = Uuid::new_v4();
    let track_id = Uuid::new_v4();
    let bytes = wav_bytes(44100);
    let object_key = format!("tenant-{}/raw/{track_id}.wav", tenant_id.simple());
    app.storage
        .put(&object_key, bytes::Bytes::from(bytes.clone()))
        .await
        .expect("put wav");

    app.repo
        .save_track(&TrackRecord {
            id: track_id,
            tenant_id,
            project_id: None,
            object_key: object_key.clone(),
            display_name: "Fixture A/B".to_string(),
            status: TrackStatus::Uploaded,
            duration_sec: Some(0.1),
            sample_rate: Some(44100),
            channels: Some(1),
            sha256: None,
            analysis: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .await
        .expect("save track");

    (track_id, tenant_id, bytes)
}

#[tokio::test]
async fn peaks_da_faixa_voltam_reais_nao_array_vazio() {
    let (app, _tmp) = state();
    let (track_id, tenant_id, _bytes) = track_com_audio(&app).await;

    let resp = router(app)
        .oneshot(get(
            &format!("/api/v1/tracks/{track_id}/peaks?resolution=16"),
            &bearer(tenant_id),
        ))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["resolution"], 16);
    let peaks = json["peaks"].as_array().expect("peaks array");
    assert!(
        !peaks.is_empty(),
        "C9: peaks não pode mais ser array vazio para faixa com áudio real"
    );
    for p in peaks {
        let (min, max) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
        assert!(min <= max, "min {min} > max {max}");
    }
    // Sinal senoidal com pico 0.5 — nenhum bucket pode passar disso.
    let global_max = peaks
        .iter()
        .map(|p| p[1].as_f64().unwrap())
        .fold(f64::MIN, f64::max);
    assert!(global_max <= 0.51, "pico além do sinal: {global_max}");
}

#[tokio::test]
async fn peaks_sem_audio_no_storage_da_404() {
    let (app, _tmp) = state();
    let tenant_id = Uuid::new_v4();
    let track_id = Uuid::new_v4();
    app.repo
        .save_track(&TrackRecord {
            id: track_id,
            tenant_id,
            project_id: None,
            object_key: "tenant-x/raw/sumiu.wav".to_string(),
            display_name: "Sem áudio".to_string(),
            status: TrackStatus::Uploaded,
            duration_sec: None,
            sample_rate: None,
            channels: None,
            sha256: None,
            analysis: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .await
        .unwrap();

    let resp = router(app)
        .oneshot(get(
            &format!("/api/v1/tracks/{track_id}/peaks"),
            &bearer(tenant_id),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn raw_entrega_o_wav_original_escopado_por_tenant() {
    let (app, _tmp) = state();
    let (track_id, tenant_id, bytes) = track_com_audio(&app).await;

    let resp = router(app.clone())
        .oneshot(get(
            &format!("/api/v1/tracks/{track_id}/raw"),
            &bearer(tenant_id),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), bytes.as_slice(), "raw ≠ bytes enviados");

    // Outro tenant: 404 (mesma resposta de inexistente — docs/08 §3).
    let resp = router(app)
        .oneshot(get(
            &format!("/api/v1/tracks/{track_id}/raw"),
            &bearer(Uuid::new_v4()),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// C6 — cancel real: transição + auditoria + 409 no segundo cancel.
#[tokio::test]
async fn cancel_transiciona_para_cancelled_e_recusa_segunda_vez() {
    let (app, _tmp) = state();
    let tenant_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let track_id = Uuid::new_v4();
    app.repo
        .save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
            &JobMeta {
                mode: Some("manual".to_string()),
                user_prompt: None,
                track_id: Some(track_id),
            },
        )
        .await
        .unwrap();

    let resp = router(app.clone())
        .oneshot(post(
            &format!("/api/v1/jobs/{job_id}/cancel"),
            &bearer(tenant_id),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 16)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "cancelled");

    let job = app.repo.get_job(job_id, tenant_id).await.unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);
    let audit = app.repo.list_audit_records(job_id).await.unwrap();
    assert_eq!(audit.last().unwrap().action, "JOB_CANCELLED");

    // Segundo cancel: job já terminal → 409 job_not_editable.
    let resp = router(app)
        .oneshot(post(
            &format!("/api/v1/jobs/{job_id}/cancel"),
            &bearer(tenant_id),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

/// C12 — retry: só em failed, cria NOVO job reusando receita e track_id.
#[tokio::test]
async fn retry_de_job_failed_cria_novo_job_com_a_mesma_receita() {
    let (app, _tmp) = state();
    let tenant_id = Uuid::new_v4();
    let old_job = Uuid::new_v4();
    let track_id = Uuid::new_v4();
    let mut config = PipelineConfig::default();
    config.crossfade.max_duration_ms = audio_core::CrossfadeMs::try_from(1200).unwrap();
    app.repo
        .save_job(
            old_job,
            tenant_id,
            Uuid::new_v4(),
            &config,
            &[],
            &JobMeta {
                mode: Some("assisted".to_string()),
                user_prompt: Some("versão de 30s pra Reels".to_string()),
                track_id: Some(track_id),
            },
        )
        .await
        .unwrap();
    app.repo
        .transition_job(old_job, JobStatus::Failed, "JOB_FAILED")
        .await
        .unwrap();

    let resp = router(app.clone())
        .oneshot(post(
            &format!("/api/v1/jobs/{old_job}/retry"),
            &bearer(tenant_id),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 16)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let new_id: Uuid = json["job_id"].as_str().unwrap().parse().unwrap();
    assert_ne!(new_id, old_job, "contrato: retry cria NOVO job_id");

    let new_job = app.repo.get_job(new_id, tenant_id).await.unwrap();
    assert_eq!(new_job.status, JobStatus::Queued);
    assert_eq!(new_job.track_id, Some(track_id), "retry reusa o track_id");
    assert_eq!(new_job.mode.as_deref(), Some("assisted"));
    assert_eq!(
        new_job.user_prompt.as_deref(),
        Some("versão de 30s pra Reels")
    );
    let restored: PipelineConfig = serde_json::from_value(new_job.config.clone()).unwrap();
    assert_eq!(
        restored.crossfade.max_duration_ms.get(),
        1200,
        "retry reusa a receita"
    );

    // Original permanece failed como histórico.
    assert_eq!(
        app.repo.get_job(old_job, tenant_id).await.unwrap().status,
        JobStatus::Failed
    );
}

#[tokio::test]
async fn retry_de_job_nao_failed_da_409() {
    let (app, _tmp) = state();
    let tenant_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    app.repo
        .save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
            &JobMeta::default(),
        )
        .await
        .unwrap();

    let resp = router(app)
        .oneshot(post(
            &format!("/api/v1/jobs/{job_id}/retry"),
            &bearer(tenant_id),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

/// Issue #33 — o handshake SSE valida o cookie de sessão same-origin:
/// sem header Authorization, um token em `mixlirous_session` autentica.
#[tokio::test]
async fn handshake_sse_aceita_cookie_de_sessao() {
    let (app, _tmp) = state();
    let tenant_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    app.repo
        .save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
            &JobMeta::default(),
        )
        .await
        .unwrap();

    let token = encode_claims(&claims(tenant_id), TEST_SECRET).unwrap();

    // Sem auth nenhuma → 401 (não vaza eventos).
    let resp = router(app.clone())
        .oneshot(get(&format!("/api/v1/jobs/{job_id}/events"), "Bearer x"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Com cookie de sessão → 200 e o stream abre com `stream.ready`.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/jobs/{job_id}/events"))
        .header("Cookie", format!("mixlirous_session={token}"))
        .body(Body::empty())
        .unwrap();
    let resp = router(app).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.starts_with("text/event-stream"),
        "SSE deveria abrir com text/event-stream, veio {content_type}"
    );
}

/// Issue #33 — local-session no modo local: 200 com token + Set-Cookie.
#[tokio::test]
async fn local_session_emite_token_e_cookie_no_modo_local() {
    let (app, _tmp) = state();
    std::env::set_var("CONFIG_ENV", "local");

    let resp = router(app)
        .oneshot(get("/api/v1/auth/local-session", "Bearer irrelevante"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cookie = resp
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .expect("Set-Cookie")
        .to_string();
    assert!(cookie.starts_with("mixlirous_session="), "cookie: {cookie}");
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));

    let body = axum::body::to_bytes(resp.into_body(), 1 << 16)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["token"].as_str().is_some());
}

/// Issue #33 — local-session FAIL-CLOSED fora do modo local.
#[tokio::test]
async fn local_session_fora_do_modo_local_e_404() {
    let (app, _tmp) = state();
    std::env::set_var("CONFIG_ENV", "production");

    let resp = router(app)
        .oneshot(get("/api/v1/auth/local-session", "Bearer x"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
