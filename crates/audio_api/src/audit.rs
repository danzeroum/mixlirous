//! Sprint 4 — Audit logging (docs/06-PERSISTENCIA-RESILIENCIA.md §2).
//!
//! Records immutable audit events for all sensitive actions.
//! Actions that MUST generate an audit_event (from the spec):
//!   PROMPT_SUBMITTED, TOOL_CALL_ATTEMPT, TOOL_CALL_DENIED, PARAM_OVERRIDE,
//!   PROPOSAL_CREATED, PROPOSAL_DECIDED, JOB_STARTED, JOB_COMPLETED,
//!   JOB_FAILED, WORKER_SCALE_ACTION, RECOVERY_ACTION,
//!   VERSION_FREEZE_CHANGED, MALICIOUS_PROMPT_BLOCKED

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Actor that triggered the audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ActorType {
    User,
    Llm,
    System,
}

impl std::fmt::Display for ActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorType::User => write!(f, "USER"),
            ActorType::Llm => write!(f, "LLM"),
            ActorType::System => write!(f, "SYSTEM"),
        }
    }
}

/// All auditable actions in the system (from docs/06 §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditAction {
    PromptSubmitted,
    ToolCallAttempt,
    ToolCallDenied,
    ParamOverride,
    ProposalCreated,
    ProposalDecided,
    JobStarted,
    JobCompleted,
    JobFailed,
    WorkerScaleAction,
    RecoveryAction,
    VersionFreezeChanged,
    MaliciousPromptBlocked,
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::PromptSubmitted => write!(f, "PROMPT_SUBMITTED"),
            AuditAction::ToolCallAttempt => write!(f, "TOOL_CALL_ATTEMPT"),
            AuditAction::ToolCallDenied => write!(f, "TOOL_CALL_DENIED"),
            AuditAction::ParamOverride => write!(f, "PARAM_OVERRIDE"),
            AuditAction::ProposalCreated => write!(f, "PROPOSAL_CREATED"),
            AuditAction::ProposalDecided => write!(f, "PROPOSAL_DECIDED"),
            AuditAction::JobStarted => write!(f, "JOB_STARTED"),
            AuditAction::JobCompleted => write!(f, "JOB_COMPLETED"),
            AuditAction::JobFailed => write!(f, "JOB_FAILED"),
            AuditAction::WorkerScaleAction => write!(f, "WORKER_SCALE_ACTION"),
            AuditAction::RecoveryAction => write!(f, "RECOVERY_ACTION"),
            AuditAction::VersionFreezeChanged => write!(f, "VERSION_FREEZE_CHANGED"),
            AuditAction::MaliciousPromptBlocked => write!(f, "MALICIOUS_PROMPT_BLOCKED"),
        }
    }
}

/// A complete audit event record.
/// Corresponds to the `audit_events` table schema (docs/06 §2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub actor_type: ActorType,
    pub actor_detail: Option<serde_json::Value>,
    pub action: AuditAction,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub occurred_at: chrono::DateTime<Utc>,
}

#[allow(dead_code)]
impl AuditEvent {
    /// Create a new audit event with the current timestamp.
    pub fn new(tenant_id: Uuid, actor_type: ActorType, action: AuditAction) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            user_id: None,
            actor_type,
            actor_detail: None,
            action,
            resource_type: None,
            resource_id: None,
            before: None,
            after: None,
            metadata: None,
            occurred_at: Utc::now(),
        }
    }

    /// Builder-pattern setter for user_id.
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Builder-pattern setter for actor_detail (e.g., model name, worker id).
    pub fn with_actor_detail(mut self, detail: serde_json::Value) -> Self {
        self.actor_detail = Some(detail);
        self
    }

    /// Builder-pattern setter for resource.
    pub fn with_resource(mut self, resource_type: &str, resource_id: &str) -> Self {
        self.resource_type = Some(resource_type.to_string());
        self.resource_id = Some(resource_id.to_string());
        self
    }

    /// Builder-pattern setter for before/after snapshots.
    pub fn with_change(
        mut self,
        before: Option<serde_json::Value>,
        after: Option<serde_json::Value>,
    ) -> Self {
        self.before = before;
        self.after = after;
        self
    }

    /// Builder-pattern setter for metadata (trace_id, ip, etc.).
    pub fn with_metadata(mut self, meta: serde_json::Value) -> Self {
        self.metadata = Some(meta);
        self
    }

    /// Log the audit event as structured tracing output.
    /// In production, this would also persist to the `audit_events` table.
    pub fn log(&self) {
        tracing::info!(
            audit_id = %self.id,
            audit_action = %self.action,
            tenant_id = %self.tenant_id,
            actor_type = %self.actor_type,
            resource_type = ?self.resource_type,
            resource_id = ?self.resource_id,
            timestamp = %self.occurred_at.to_rfc3339(),
            "audit_event"
        );
    }
}

/// Convenience function: record a system audit event and log it.
pub fn record_audit(
    tenant_id: Uuid,
    actor_type: ActorType,
    action: AuditAction,
    resource_type: &str,
    resource_id: &str,
) {
    let event =
        AuditEvent::new(tenant_id, actor_type, action).with_resource(resource_type, resource_id);
    event.log();
}

/// Convenience function: record a system audit event with detail and log it.
#[allow(dead_code)]
pub fn record_audit_with_detail(
    tenant_id: Uuid,
    actor_type: ActorType,
    action: AuditAction,
    resource_type: &str,
    resource_id: &str,
    detail: serde_json::Value,
) {
    let event = AuditEvent::new(tenant_id, actor_type, action)
        .with_resource(resource_type, resource_id)
        .with_actor_detail(detail);
    event.log();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_new() {
        let tenant_id = Uuid::new_v4();
        let event = AuditEvent::new(tenant_id, ActorType::System, AuditAction::JobStarted);
        assert_eq!(event.tenant_id, tenant_id);
        assert_eq!(event.actor_type, ActorType::System);
        assert_eq!(event.action, AuditAction::JobStarted);
        assert!(event.user_id.is_none());
        assert!(event.before.is_none());
        assert!(event.after.is_none());
        assert!(event.metadata.is_none());
    }

    #[test]
    fn test_audit_event_builder_pattern() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();

        let event = AuditEvent::new(tenant_id, ActorType::User, AuditAction::ProposalDecided)
            .with_user_id(user_id)
            .with_resource("proposal", &job_id.to_string())
            .with_change(
                Some(serde_json::json!({"status": "pending"})),
                Some(serde_json::json!({"status": "approved"})),
            )
            .with_metadata(serde_json::json!({"trace_id": "abc123"}));

        assert_eq!(event.user_id, Some(user_id));
        assert_eq!(event.resource_type.as_deref(), Some("proposal"));
        assert_eq!(
            event.resource_id.as_deref(),
            Some(job_id.to_string().as_str())
        );
        assert!(event.before.is_some());
        assert!(event.after.is_some());
        assert!(event.metadata.is_some());
    }

    #[test]
    fn test_audit_action_display() {
        assert_eq!(AuditAction::JobCompleted.to_string(), "JOB_COMPLETED");
        assert_eq!(AuditAction::RecoveryAction.to_string(), "RECOVERY_ACTION");
        assert_eq!(
            AuditAction::MaliciousPromptBlocked.to_string(),
            "MALICIOUS_PROMPT_BLOCKED"
        );
    }

    #[test]
    fn test_actor_type_display() {
        assert_eq!(ActorType::User.to_string(), "USER");
        assert_eq!(ActorType::Llm.to_string(), "LLM");
        assert_eq!(ActorType::System.to_string(), "SYSTEM");
    }

    #[test]
    fn test_audit_event_serialization() {
        let tenant_id = Uuid::new_v4();
        let event = AuditEvent::new(tenant_id, ActorType::System, AuditAction::RecoveryAction);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("RECOVERY_ACTION"));
        assert!(json.contains("SYSTEM"));
    }

    #[test]
    fn test_record_audit_convenience() {
        let tenant_id = Uuid::new_v4();
        // Should not panic
        record_audit(
            tenant_id,
            ActorType::System,
            AuditAction::JobCompleted,
            "job",
            &Uuid::new_v4().to_string(),
        );
    }

    #[test]
    fn test_record_audit_with_detail_convenience() {
        let tenant_id = Uuid::new_v4();
        record_audit_with_detail(
            tenant_id,
            ActorType::Llm,
            AuditAction::ToolCallAttempt,
            "tool",
            "crossfade",
            serde_json::json!({"model": "gpt-4o"}),
        );
    }

    #[test]
    fn test_all_audit_actions_have_display() {
        // Ensure every action can be displayed (no panic)
        let actions = [
            AuditAction::PromptSubmitted,
            AuditAction::ToolCallAttempt,
            AuditAction::ToolCallDenied,
            AuditAction::ParamOverride,
            AuditAction::ProposalCreated,
            AuditAction::ProposalDecided,
            AuditAction::JobStarted,
            AuditAction::JobCompleted,
            AuditAction::JobFailed,
            AuditAction::WorkerScaleAction,
            AuditAction::RecoveryAction,
            AuditAction::VersionFreezeChanged,
            AuditAction::MaliciousPromptBlocked,
        ];
        for a in &actions {
            let _ = a.to_string();
        }
        assert_eq!(actions.len(), 13);
    }
}
