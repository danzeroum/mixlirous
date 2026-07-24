use crate::domain::{AudioFingerprint, BeatBlock, PipelineConfig};
use uuid::Uuid;

#[async_trait::async_trait]
pub trait AudioRepo: Send + Sync {
    async fn save_job(
        &self,
        job_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        config: &PipelineConfig,
        blocks: &[BeatBlock],
    ) -> Result<(), RepoError>;
    /// Escopado por `tenant_id`: se o job existe mas pertence a outro
    /// tenant, o resultado é o mesmo `RepoError::NotFound` de um job que não
    /// existe — nunca dá para diferenciar os dois casos a partir da
    /// resposta. Um 403 (job existe, mas não é seu) vaza que o ID é válido;
    /// `docs/08-SEGURANCA-MULTITENANCY.md` §3 exige 404 nos dois casos.
    async fn get_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError>;
    /// Escopado por `tenant_id`, nunca por `user_id` — um tenant pode ter mais
    /// de um usuário, e `docs/08-SEGURANCA-MULTITENANCY.md` §1 é explícito que
    /// o isolamento é por tenant. `JobRecord` carrega os dois campos
    /// separados de propósito para que essa distinção não dependa de quem
    /// chama `save_job` passar o valor certo no campo certo.
    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError>;
    async fn save_fingerprint(
        &self,
        job_id: Uuid,
        fingerprint: &AudioFingerprint,
    ) -> Result<(), RepoError>;

    /// Muda o status do job e grava o `audit_event` correspondente como uma
    /// única operação atômica — nunca um sem o outro. `docs/06` §2 lista as
    /// transições (`JOB_STARTED`, `JOB_COMPLETED`, `JOB_FAILED`, ...) que
    /// **sempre** geram auditoria, e a ADR-0005 justifica fila-no-banco
    /// exatamente por mudar o estado do job e o próximo passo acontecerem na
    /// mesma transação. Um adapter SQL implementa isto com `BEGIN`/`COMMIT`
    /// em volta de dois statements; nenhuma implementação pode expor status
    /// e auditoria como duas chamadas separadas — é assim que a inconsistência
    /// que a ADR quer eliminar volta a existir.
    async fn transition_job(
        &self,
        job_id: Uuid,
        new_status: JobStatus,
        audit_action: &str,
    ) -> Result<(), RepoError>;

    /// Histórico de auditoria de um job, na ordem em que ocorreram. Existe
    /// hoje para que `transition_job` seja testável (a atomicidade só é uma
    /// garantia real se der para observar as duas escritas juntas); consumo
    /// pelo endpoint de auditoria fica para quando `docs/06` §2 for
    /// implementado de fato.
    async fn list_audit_records(&self, job_id: Uuid) -> Result<Vec<AuditRecord>, RepoError>;
}

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub job_id: Uuid,
    pub action: String,
    pub new_status: JobStatus,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub config: serde_json::Value,
    pub blocks: serde_json::Value,
    pub status: JobStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    RolledBack,
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("job not found: {0}")]
    NotFound(Uuid),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("storage backend error: {0}")]
    Backend(String),
}
