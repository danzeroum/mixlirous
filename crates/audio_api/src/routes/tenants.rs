use crate::middleware::{AuthContext, TenantScope};
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct TenantQuota {
    pub jobs: JobsQuota,
    pub storage: StorageQuota,
}

#[derive(Debug, Serialize)]
pub struct JobsQuota {
    pub used: u32,
    pub limit: u32,
    pub period: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StorageQuota {
    pub used_gb: f32,
    pub limit_gb: f32,
}

pub async fn get_quota(TenantScope(_tenant_id): TenantScope) -> axum::Json<TenantQuota> {
    axum::Json(TenantQuota {
        jobs: JobsQuota {
            used: 0,
            limit: 1000,
            period: "month",
        },
        storage: StorageQuota {
            used_gb: 0.0,
            limit_gb: 10.0,
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct ConsentRequest {
    pub accepted: bool,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct ConsentResponse {
    pub assisted_mode_accepted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub provider_at_accept: Option<String>,
}

impl From<audio_core::ports::repo_trait::ConsentRecord> for ConsentResponse {
    fn from(record: audio_core::ports::repo_trait::ConsentRecord) -> Self {
        Self {
            assisted_mode_accepted_at: Some(record.assisted_mode_accepted_at),
            provider_at_accept: Some(record.provider_at_accept),
        }
    }
}

pub async fn get_consent(
    State(state): State<AppState>,
    TenantScope(tenant_id): TenantScope,
) -> Result<Json<ConsentResponse>, (StatusCode, String)> {
    let consent = state
        .repo
        .get_consent(tenant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(consent.map(ConsentResponse::from).unwrap_or(
        ConsentResponse {
            assisted_mode_accepted_at: None,
            provider_at_accept: None,
        },
    )))
}

pub async fn post_consent(
    State(state): State<AppState>,
    TenantScope(tenant_id): TenantScope,
    Json(payload): Json<ConsentRequest>,
) -> Result<Json<ConsentResponse>, (StatusCode, String)> {
    if !payload.accepted {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "consent_not_accepted".to_string(),
        ));
    }

    let current_provider = &state.config.llm.provider;
    if &payload.provider != current_provider {
        return Err((StatusCode::CONFLICT, "provider_mismatch".to_string()));
    }

    let record = state
        .repo
        .save_consent(tenant_id, current_provider.clone())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ConsentResponse::from(record)))
}

/// `DELETE /api/v1/tenants/me/consent` — revogação REAL do consentimento
/// do modo assistido (plano de design §IA/dados/LGPD).
///
/// Trocar para modo manual na UI não revoga nada persistido — isto aqui
/// remove o registro ativo do tenant (`AudioRepo::revoke_consent`).
///
/// Semântica honesta, comunicada na UI:
/// - a revogação vale para o USO FUTURO do modo assistido (o wizard volta
///   a exigir consentimento);
/// - NÃO apaga jobs, artefatos nem registros de auditoria já existentes;
/// - registro da revogação: data/hora, tenant e ator (sub do JWT) em log
///   estruturado, e o `provider` vigente quando aplicável (quando havia
///   consentimento ativo).
///
/// Idempotente: revogar sem consentimento ativo responde 200 com o estado
/// atualizado (nulls) — DELETE não é 404 quando nada havia (mesma regra
/// de não-vazamento de docs/08 §3).
pub async fn delete_consent(
    State(state): State<AppState>,
    AuthContext(claims): AuthContext,
    TenantScope(tenant_id): TenantScope,
) -> Result<Json<ConsentResponse>, (StatusCode, String)> {
    // Provider vigente quando aplicável — entra no registro da revogação.
    let provider_anterior = state
        .repo
        .get_consent(tenant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(|c| c.provider_at_accept);

    state
        .repo
        .revoke_consent(tenant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        %tenant_id,
        actor = %claims.sub,
        provider_anterior = provider_anterior.as_deref(),
        "consentimento de modo assistido REVOGADO — registro com data/ator/tenant/provider"
    );

    Ok(Json(ConsentResponse {
        assisted_mode_accepted_at: None,
        provider_at_accept: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemoryRepo;
    use crate::config::{
        AppConfig, AudioConfig, DatabaseConfig, LlmConfig, ObservabilityConfig, StorageConfig,
    };
    use audio_agent::llm::mock::MockLlm;
    use audio_agent::validator::ValidationLayer;
    use audio_agent::ReActOrchestrator;
    use std::sync::Arc;
    use uuid::Uuid;

    fn state_with_provider(provider: &str) -> AppState {
        let config = AppConfig {
            database: DatabaseConfig {
                type_db: "sqlite".to_string(),
                url: ":memory:".to_string(),
                max_connections: 1,
            },
            storage: StorageConfig {
                type_storage: "local".to_string(),
                endpoint: None,
                bucket: "test".to_string(),
                access_key: None,
                secret_key: None,
                region: None,
            },
            audio: AudioConfig {
                sample_rate: 44100,
                channels: 2,
                frame_size: 2048,
                hop_size: 512,
                crossfade_max_ms: 3000,
                rms_window_ms: 50,
            },
            llm: LlmConfig {
                provider: provider.to_string(),
                model: "test-model".to_string(),
                base_url: String::new(),
                temperature: 0.7,
                max_tools: 5,
                timeout_sec: 30,
            },
            observability: ObservabilityConfig {
                otel_collector_endpoint: String::new(),
                prometheus_port: 9090,
                grafana_url: String::new(),
            },
            features: Default::default(),
            config_env: "local".to_string(),
        };
        let validator = Arc::new(ValidationLayer::new());
        let mock = Arc::new(MockLlm::new());
        let orchestrator = Arc::new(ReActOrchestrator::<MockLlm>::new(
            validator,
            mock,
            config.llm.max_tools,
        ));
        let hub = Arc::new(crate::sse::hub::EventHub::new());
        let storage: Arc<dyn audio_core::ports::Storage> = Arc::new(
            crate::storage::LocalFsStorage::new(tempfile::tempdir().unwrap().keep()).unwrap(),
        );
        AppState::new(
            InMemoryRepo::new(),
            orchestrator,
            Arc::new(config),
            hub,
            storage,
        )
    }

    #[tokio::test]
    async fn test_get_consent_before_acceptance_returns_nulls() {
        let state = state_with_provider("deepseek");
        let Json(body) = get_consent(State(state), TenantScope(Uuid::new_v4()))
            .await
            .unwrap();
        assert!(body.assisted_mode_accepted_at.is_none());
        assert!(body.provider_at_accept.is_none());
    }

    #[tokio::test]
    async fn test_post_consent_with_matching_provider_records_it() {
        let state = state_with_provider("deepseek");
        let tenant_id = Uuid::new_v4();
        let Json(body) = post_consent(
            State(state.clone()),
            TenantScope(tenant_id),
            Json(ConsentRequest {
                accepted: true,
                provider: "deepseek".to_string(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(body.provider_at_accept.as_deref(), Some("deepseek"));
        assert!(body.assisted_mode_accepted_at.is_some());
        let Json(read_back) = get_consent(State(state), TenantScope(tenant_id))
            .await
            .unwrap();
        assert_eq!(read_back.provider_at_accept.as_deref(), Some("deepseek"));
    }

    #[tokio::test]
    async fn test_post_consent_rejects_provider_mismatch() {
        let state = state_with_provider("ollama");
        let err = post_consent(
            State(state),
            TenantScope(Uuid::new_v4()),
            Json(ConsentRequest {
                accepted: true,
                provider: "deepseek".to_string(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::CONFLICT);
        assert_eq!(err.1, "provider_mismatch");
    }

    #[tokio::test]
    async fn test_post_consent_rejects_accepted_false() {
        let state = state_with_provider("deepseek");
        let err = post_consent(
            State(state),
            TenantScope(Uuid::new_v4()),
            Json(ConsentRequest {
                accepted: false,
                provider: "deepseek".to_string(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.1, "consent_not_accepted");
    }

    #[tokio::test]
    async fn test_consent_is_scoped_by_tenant() {
        let state = state_with_provider("deepseek");
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let Json(_) = post_consent(
            State(state.clone()),
            TenantScope(tenant_a),
            Json(ConsentRequest {
                accepted: true,
                provider: "deepseek".to_string(),
            }),
        )
        .await
        .unwrap();
        let Json(body_b) = get_consent(State(state), TenantScope(tenant_b))
            .await
            .unwrap();
        assert!(body_b.provider_at_accept.is_none());
    }

    // ─── Revogação real (plano de design §LGPD) ────────────────────────

    #[tokio::test]
    async fn test_revoke_consent_limpa_e_get_volta_nulo() {
        let state = state_with_provider("deepseek");
        let tenant_id = Uuid::new_v4();
        let Json(_) = post_consent(
            State(state.clone()),
            TenantScope(tenant_id),
            Json(ConsentRequest {
                accepted: true,
                provider: "deepseek".to_string(),
            }),
        )
        .await
        .unwrap();

        let Json(revogado) = delete_consent(
            State(state.clone()),
            AuthContext(crate::middleware::auth::TenantClaims {
                sub: Uuid::new_v4(),
                tenant_id,
                roles: vec!["owner".to_string()],
                plan: "free".to_string(),
                iat: 0,
                exp: i64::MAX as usize,
            }),
            TenantScope(tenant_id),
        )
        .await
        .unwrap();
        assert!(revogado.assisted_mode_accepted_at.is_none());
        assert!(revogado.provider_at_accept.is_none());

        // GET volta nulls — o estado persistido foi removido.
        let Json(read_back) = get_consent(State(state), TenantScope(tenant_id))
            .await
            .unwrap();
        assert!(read_back.assisted_mode_accepted_at.is_none());
        assert!(read_back.provider_at_accept.is_none());
    }

    #[tokio::test]
    async fn test_revoke_consent_e_idempotente() {
        let state = state_with_provider("deepseek");
        let tenant_id = Uuid::new_v4();
        let claims = AuthContext(crate::middleware::auth::TenantClaims {
            sub: Uuid::new_v4(),
            tenant_id,
            roles: vec!["owner".to_string()],
            plan: "free".to_string(),
            iat: 0,
            exp: i64::MAX as usize,
        });
        // Revogar sem nunca ter aceito é 200 (não 404 — DELETE não vaza
        // existência de registro, docs/08 §3).
        let Json(_) = delete_consent(State(state.clone()), claims.clone(), TenantScope(tenant_id))
            .await
            .unwrap();
        // E revogar duas vezes também.
        let Json(_) = delete_consent(State(state), claims, TenantScope(tenant_id))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_revoke_consent_scoped_por_tenant() {
        let state = state_with_provider("deepseek");
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let Json(_) = post_consent(
            State(state.clone()),
            TenantScope(tenant_a),
            Json(ConsentRequest {
                accepted: true,
                provider: "deepseek".to_string(),
            }),
        )
        .await
        .unwrap();

        let claims_b = AuthContext(crate::middleware::auth::TenantClaims {
            sub: Uuid::new_v4(),
            tenant_id: tenant_b,
            roles: vec!["owner".to_string()],
            plan: "free".to_string(),
            iat: 0,
            exp: i64::MAX as usize,
        });
        let Json(_) = delete_consent(State(state.clone()), claims_b, TenantScope(tenant_b))
            .await
            .unwrap();

        // Revogação do tenant B não afeta o consentimento do tenant A.
        let Json(a) = get_consent(State(state), TenantScope(tenant_a))
            .await
            .unwrap();
        assert_eq!(a.provider_at_accept.as_deref(), Some("deepseek"));
    }
}
