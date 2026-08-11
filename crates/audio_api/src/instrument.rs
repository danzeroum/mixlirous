use std::time::Instant;

/// Registra métricas de execução de uma etapa do pipeline
pub struct PipelineMetrics {
    pub stage: String,
    pub start: Instant,
}

impl PipelineMetrics {
    pub fn start(stage: &str) -> Self {
        Self {
            stage: stage.to_string(),
            start: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn log(&self) {
        tracing::info!(
            stage = %self.stage,
            duration_ms = %self.elapsed_ms(),
            "pipeline stage completed"
        );
    }
}

/// Gera um trace_id quando não propagado do request
pub fn generate_trace_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Formata um audit_event para logging estruturado
pub fn audit_event(action: &str, job_id: &uuid::Uuid, detail: &str) {
    tracing::info!(
        audit_event = action,
        job_id = %job_id,
        detail = %detail,
        timestamp = %chrono::Utc::now().to_rfc3339(),
    );
}
