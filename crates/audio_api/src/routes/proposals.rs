#![allow(dead_code)]
use crate::middleware::AuthContext;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub proposal_id: Uuid,
    pub job_id: Uuid,
    pub tool: String,
    pub tool_label_ptbr: String,
    pub reason: String,
    pub confidence: f32,
    pub parameters_suggestion: serde_json::Value,
    pub status: ProposalStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Replanned,
    Expired,
}

#[derive(Default)]
pub struct ProposalStore {
    proposals: HashMap<Uuid, Proposal>,
}

impl ProposalStore {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self::default()))
    }

    pub async fn insert(&mut self, proposal: Proposal) {
        self.proposals.insert(proposal.proposal_id, proposal);
    }

    pub async fn get(&self, id: Uuid) -> Option<Proposal> {
        self.proposals.get(&id).cloned()
    }

    pub async fn list_for_job(&self, job_id: Uuid) -> Vec<Proposal> {
        self.proposals
            .values()
            .filter(|p| p.job_id == job_id)
            .cloned()
            .collect()
    }

    pub async fn update_status(&mut self, id: Uuid, status: ProposalStatus) -> Option<Proposal> {
        if let Some(p) = self.proposals.get_mut(&id) {
            p.status = status.clone();
            Some(p.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProposalResponse {
    pub proposal_id: Uuid,
    pub status: ProposalStatus,
    pub job_id: Uuid,
    pub tool: String,
    pub tool_label_ptbr: String,
    pub reason: String,
    pub confidence: f32,
    pub expires_at: String,
}

impl From<Proposal> for ProposalResponse {
    fn from(p: Proposal) -> Self {
        Self {
            proposal_id: p.proposal_id,
            status: p.status,
            job_id: p.job_id,
            tool: p.tool,
            tool_label_ptbr: p.tool_label_ptbr,
            reason: p.reason,
            confidence: p.confidence,
            expires_at: p.expires_at.to_rfc3339(),
        }
    }
}

pub struct ProposalHandlers {
    pub store: Arc<RwLock<ProposalStore>>,
}

impl ProposalHandlers {
    pub fn new() -> Self {
        Self {
            store: ProposalStore::new(),
        }
    }

    /// GET /api/v1/jobs/{job_id}/proposals
    pub async fn list_proposals(
        _auth: AuthContext,
        State(state): State<AppState>,
        Path(job_id): Path<Uuid>,
    ) -> Result<Json<Vec<ProposalResponse>>, (StatusCode, String)> {
        let store = state.proposal_store.write().await;
        let proposals = store.list_for_job(job_id).await;
        Ok(Json(
            proposals.into_iter().map(ProposalResponse::from).collect(),
        ))
    }

    /// POST /api/v1/jobs/{job_id}/proposals/{proposal_id}/approve
    pub async fn approve_proposal(
        _auth: AuthContext,
        State(state): State<AppState>,
        Path((job_id, proposal_id)): Path<(Uuid, Uuid)>,
        Json(_payload): Json<ApproveRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        let mut store = state.proposal_store.write().await;

        let proposal = store
            .get(proposal_id)
            .await
            .ok_or((StatusCode::NOT_FOUND, "proposal_not_found".to_string()))?;

        if proposal.job_id != job_id {
            return Err((StatusCode::NOT_FOUND, "proposal_not_found".to_string()));
        }

        if proposal.status != ProposalStatus::Pending {
            return Err((StatusCode::CONFLICT, "proposal_already_decided".to_string()));
        }

        // Check if expired
        if chrono::Utc::now() > proposal.expires_at {
            store
                .update_status(proposal_id, ProposalStatus::Expired)
                .await;
            return Err((StatusCode::CONFLICT, "proposal_expired".to_string()));
        }

        store
            .update_status(proposal_id, ProposalStatus::Approved)
            .await;

        // Publish SSE event
        state
            .hub
            .publish(
                job_id,
                "proposal.decided",
                serde_json::json!({
                    "job_id": job_id,
                    "proposal_id": proposal_id.to_string(),
                    "decision": "approved",
                    "node_id": Uuid::new_v4().to_string(),
                }),
            )
            .await;

        Ok(Json(serde_json::json!({
            "proposal_id": proposal_id.to_string(),
            "status": "approved",
            "created_node": {
                "id": Uuid::new_v4().to_string(),
                "type": "processor",
                "tool": proposal.tool,
                "status": "queued",
            },
        })))
    }

    /// POST /api/v1/jobs/{job_id}/proposals/{proposal_id}/reject
    pub async fn reject_proposal(
        _auth: AuthContext,
        State(state): State<AppState>,
        Path((job_id, proposal_id)): Path<(Uuid, Uuid)>,
        Json(_payload): Json<RejectRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        let mut store = state.proposal_store.write().await;

        let proposal = store
            .get(proposal_id)
            .await
            .ok_or((StatusCode::NOT_FOUND, "proposal_not_found".to_string()))?;

        if proposal.job_id != job_id {
            return Err((StatusCode::NOT_FOUND, "proposal_not_found".to_string()));
        }

        if proposal.status != ProposalStatus::Pending {
            return Err((StatusCode::CONFLICT, "proposal_already_decided".to_string()));
        }

        store
            .update_status(proposal_id, ProposalStatus::Rejected)
            .await;

        state
            .hub
            .publish(
                job_id,
                "proposal.decided",
                serde_json::json!({
                    "job_id": job_id,
                    "proposal_id": proposal_id.to_string(),
                    "decision": "rejected",
                    "agent_will_replan": true,
                }),
            )
            .await;

        Ok(Json(serde_json::json!({
            "proposal_id": proposal_id.to_string(),
            "status": "rejected",
            "agent_will_replan": true,
        })))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_store_insert_and_get() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut store = ProposalStore::default();
            let id = Uuid::new_v4();
            let proposal = Proposal {
                proposal_id: id,
                job_id: Uuid::new_v4(),
                tool: "compression".to_string(),
                tool_label_ptbr: "Compressão".to_string(),
                reason: "Melhorar dinâmica".to_string(),
                confidence: 0.92,
                parameters_suggestion: serde_json::json!({"ratio": 4.0}),
                status: ProposalStatus::Pending,
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::seconds(120),
            };
            store.insert(proposal).await;
            let found = store.get(id).await;
            assert!(found.is_some());
            assert_eq!(found.unwrap().tool, "compression");
        });
    }

    #[test]
    fn test_proposal_store_update_status() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut store = ProposalStore::default();
            let id = Uuid::new_v4();
            let proposal = Proposal {
                proposal_id: id,
                job_id: Uuid::new_v4(),
                tool: "crossfade".to_string(),
                tool_label_ptbr: "Transição".to_string(),
                reason: "Suavizar transições".to_string(),
                confidence: 0.85,
                parameters_suggestion: serde_json::json!({}),
                status: ProposalStatus::Pending,
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::seconds(120),
            };
            store.insert(proposal).await;
            store.update_status(id, ProposalStatus::Approved).await;
            let found = store.get(id).await.unwrap();
            assert_eq!(found.status, ProposalStatus::Approved);
        });
    }

    #[test]
    fn test_proposal_store_expired_not_found_after_status_change() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut store = ProposalStore::default();
            let id = Uuid::new_v4();
            let proposal = Proposal {
                proposal_id: id,
                job_id: Uuid::new_v4(),
                tool: "fade_out".to_string(),
                tool_label_ptbr: "Fade out".to_string(),
                reason: "Final suave".to_string(),
                confidence: 0.75,
                parameters_suggestion: serde_json::json!({}),
                status: ProposalStatus::Pending,
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::seconds(120),
            };
            store.insert(proposal).await;
            store.update_status(id, ProposalStatus::Rejected).await;
            let found = store.get(id).await.unwrap();
            assert_eq!(found.status, ProposalStatus::Rejected);
        });
    }
}
