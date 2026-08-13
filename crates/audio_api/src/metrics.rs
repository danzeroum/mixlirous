//! Sprint 4 — Observability metrics (docs/07-OBSERVABILIDADE.md §4).
//!
//! Prometheus-compatible metrics exposed via `GET /metrics`.
//! All metric names follow the `mixlirous_*` convention from the spec.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Instant;

// ── Business metrics (docs/07 §4) ─────────────────────────────────

pub static JOBS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static JOB_DURATION_SECONDS_BUCKETS: parking_lot::RwLock<Vec<(String, f64)>> =
    parking_lot::RwLock::new(Vec::new());
pub static QUEUE_DEPTH: AtomicI64 = AtomicI64::new(0);
pub static WORKERS_ACTIVE: AtomicI64 = AtomicI64::new(0);
pub static PROPOSALS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static PARAM_OVERRIDES_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── LLM metrics ──────────────────────────────────────────────────

pub static LLM_CALLS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static LLM_DURATION_SECONDS_BUCKETS: parking_lot::RwLock<Vec<(String, f64)>> =
    parking_lot::RwLock::new(Vec::new());
pub static LLM_TOKENS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static LLM_VALIDATION_FAILURES: AtomicU64 = AtomicU64::new(0);

// ── DSP metrics ──────────────────────────────────────────────────

pub static DSP_STAGE_DURATION_BUCKETS: parking_lot::RwLock<Vec<(String, f64)>> =
    parking_lot::RwLock::new(Vec::new());
pub static DSP_AUDIO_SECONDS_PROCESSED: AtomicU64 = AtomicU64::new(0);
pub static DSP_WARNINGS_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── Infra metrics ────────────────────────────────────────────────

pub static DB_QUERY_DURATION_BUCKETS: parking_lot::RwLock<Vec<(String, f64)>> =
    parking_lot::RwLock::new(Vec::new());
pub static STORAGE_OP_DURATION_BUCKETS: parking_lot::RwLock<Vec<(String, f64)>> =
    parking_lot::RwLock::new(Vec::new());
pub static SSE_CONNECTIONS_ACTIVE: AtomicI64 = AtomicI64::new(0);
pub static RECOVERY_JOBS_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── Helper: record a duration into a bucket vector ───────────────

#[allow(dead_code)]
pub fn record_duration(
    buckets: &parking_lot::RwLock<Vec<(String, f64)>>,
    label: &str,
    duration_sec: f64,
) {
    let mut guard = buckets.write();
    guard.push((label.to_string(), duration_sec));
    // Keep last 10 000 observations to bound memory.
    if guard.len() > 10_000 {
        let new_len = guard.len() - 10_000;
        guard.drain(..new_len);
    }
}

// ── Helper: percentile from sorted samples ───────────────────────

fn percentile(sorted: &mut [f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ── Render Prometheus text format ─────────────────────────────────

/// Render all metrics in Prometheus exposition format.
/// This is called by the `GET /metrics` handler.
pub fn render_prometheus() -> String {
    let mut out = String::with_capacity(4096);

    // Helper macros
    macro_rules! counter_line {
        ($name:expr, $val:expr) => {
            out.push_str(&format!("# TYPE {} counter\n{} {}\n\n", $name, $name, $val));
        };
    }
    macro_rules! gauge_line {
        ($name:expr, $val:expr) => {
            out.push_str(&format!("# TYPE {} gauge\n{} {}\n\n", $name, $name, $val));
        };
    }
    macro_rules! histogram_from_buckets {
        ($name:expr, $buckets_lock:expr) => {
            let guard = $buckets_lock.read();
            if !guard.is_empty() {
                let mut values: Vec<f64> = guard.iter().map(|(_, v)| *v).collect();
                let count = values.len() as f64;
                let sum: f64 = values.iter().sum();
                out.push_str(&format!("# TYPE {} histogram\n", $name));
                for &b in &[
                    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
                ] {
                    let le_count = values.iter().filter(|&&v| v <= b).count() as f64;
                    out.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", $name, b, le_count));
                }
                out.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", $name, count));
                out.push_str(&format!("{}_count {}\n", $name, count));
                out.push_str(&format!("{}_sum {}\n\n", $name, sum));

                // Also expose summary-style p50/p95/p99 as gauges for dashboards
                let p50 = percentile(&mut values, 50.0);
                let p95 = percentile(&mut values, 95.0);
                let p99 = percentile(&mut values, 99.0);
                out.push_str(&format!(
                    "# TYPE {}_p50 gauge\n{}_p50 {}\n\n",
                    $name, $name, p50
                ));
                out.push_str(&format!(
                    "# TYPE {}_p95 gauge\n{}_p95 {}\n\n",
                    $name, $name, p95
                ));
                out.push_str(&format!(
                    "# TYPE {}_p99 gauge\n{}_p99 {}\n\n",
                    $name, $name, p99
                ));
            }
        };
    }

    // Business
    counter_line!("mixlirous_jobs_total", JOBS_TOTAL.load(Ordering::Relaxed));
    gauge_line!("mixlirous_queue_depth", QUEUE_DEPTH.load(Ordering::Relaxed));
    gauge_line!(
        "mixlirous_workers_active",
        WORKERS_ACTIVE.load(Ordering::Relaxed)
    );
    counter_line!(
        "mixlirous_proposals_total",
        PROPOSALS_TOTAL.load(Ordering::Relaxed)
    );
    counter_line!(
        "mixlirous_param_overrides_total",
        PARAM_OVERRIDES_TOTAL.load(Ordering::Relaxed)
    );
    histogram_from_buckets!(
        "mixlirous_job_duration_seconds",
        &JOB_DURATION_SECONDS_BUCKETS
    );

    // LLM
    counter_line!(
        "mixlirous_llm_calls_total",
        LLM_CALLS_TOTAL.load(Ordering::Relaxed)
    );
    counter_line!(
        "mixlirous_llm_tokens_total",
        LLM_TOKENS_TOTAL.load(Ordering::Relaxed)
    );
    counter_line!(
        "mixlirous_llm_validation_failures_total",
        LLM_VALIDATION_FAILURES.load(Ordering::Relaxed)
    );
    histogram_from_buckets!(
        "mixlirous_llm_duration_seconds",
        &LLM_DURATION_SECONDS_BUCKETS
    );

    // DSP
    counter_line!(
        "mixlirous_dsp_audio_seconds_processed_total",
        DSP_AUDIO_SECONDS_PROCESSED.load(Ordering::Relaxed)
    );
    counter_line!(
        "mixlirous_dsp_warnings_total",
        DSP_WARNINGS_TOTAL.load(Ordering::Relaxed)
    );
    histogram_from_buckets!(
        "mixlirous_dsp_stage_duration_seconds",
        &DSP_STAGE_DURATION_BUCKETS
    );

    // Infra
    gauge_line!(
        "mixlirous_sse_connections_active",
        SSE_CONNECTIONS_ACTIVE.load(Ordering::Relaxed)
    );
    counter_line!(
        "mixlirous_recovery_jobs_total",
        RECOVERY_JOBS_TOTAL.load(Ordering::Relaxed)
    );
    histogram_from_buckets!(
        "mixlirous_db_query_duration_seconds",
        &DB_QUERY_DURATION_BUCKETS
    );
    histogram_from_buckets!(
        "mixlirous_storage_operation_duration_seconds",
        &STORAGE_OP_DURATION_BUCKETS
    );

    // Build info
    out.push_str(
        "# TYPE mixlirous_build_info gauge\nmixlirous_build_info{version=\"0.1.0-sprint4\"} 1\n",
    );

    out
}

// ── Convenience timer ─────────────────────────────────────────────

/// A guard that records the elapsed time on drop into the given bucket.
#[allow(dead_code)]
pub struct DurationTimer {
    label: String,
    buckets: &'static parking_lot::RwLock<Vec<(String, f64)>>,
    start: Instant,
}

impl DurationTimer {
    /// Create a new duration timer for a named DSP/API stage.
    #[allow(dead_code)]
    pub fn new(label: &str, buckets: &'static parking_lot::RwLock<Vec<(String, f64)>>) -> Self {
        Self {
            label: label.to_string(),
            buckets,
            start: Instant::now(),
        }
    }
}

impl Drop for DurationTimer {
    fn drop(&mut self) {
        record_duration(
            self.buckets,
            &self.label,
            self.start.elapsed().as_secs_f64(),
        );
    }
}

/// Increment a counter by 1.
#[allow(dead_code)]
pub fn inc_counter(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// Set a gauge value.
#[allow(dead_code)]
pub fn set_gauge(gauge: &AtomicI64, val: i64) {
    gauge.store(val, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_prometheus_contains_metric_names() {
        JOBS_TOTAL.store(42, Ordering::Relaxed);
        QUEUE_DEPTH.store(5, Ordering::Relaxed);
        let output = render_prometheus();
        assert!(
            output.contains("mixlirous_jobs_total"),
            "missing jobs_total"
        );
        assert!(
            output.contains("mixlirous_queue_depth"),
            "missing queue_depth"
        );
        assert!(output.contains("42"), "missing value");
        assert!(
            output.contains("mixlirous_build_info"),
            "missing build_info"
        );
    }

    #[test]
    fn test_render_prometheus_llm_metrics() {
        LLM_CALLS_TOTAL.store(10, Ordering::Relaxed);
        LLM_TOKENS_TOTAL.store(5000, Ordering::Relaxed);
        let output = render_prometheus();
        assert!(output.contains("mixlirous_llm_calls_total"));
        assert!(output.contains("mixlirous_llm_tokens_total"));
    }

    #[test]
    fn test_render_prometheus_dsp_metrics() {
        DSP_AUDIO_SECONDS_PROCESSED.store(120, Ordering::Relaxed);
        DSP_WARNINGS_TOTAL.store(3, Ordering::Relaxed);
        let output = render_prometheus();
        assert!(output.contains("mixlirous_dsp_audio_seconds_processed_total"));
        assert!(output.contains("mixlirous_dsp_warnings_total"));
    }

    #[test]
    fn test_render_prometheus_infra_metrics() {
        SSE_CONNECTIONS_ACTIVE.store(7, Ordering::Relaxed);
        RECOVERY_JOBS_TOTAL.store(2, Ordering::Relaxed);
        let output = render_prometheus();
        assert!(output.contains("mixlirous_sse_connections_active"));
        assert!(output.contains("mixlirous_recovery_jobs_total"));
    }

    #[test]
    fn test_duration_timer_records() {
        // Clear any previous data
        *DSP_STAGE_DURATION_BUCKETS.write() = Vec::new();
        {
            let _timer = DurationTimer::new("test_stage", &DSP_STAGE_DURATION_BUCKETS);
            // Drop happens at end of block
        }
        let guard = DSP_STAGE_DURATION_BUCKETS.read();
        assert!(!guard.is_empty());
        assert!(guard.iter().any(|(l, _)| l == "test_stage"));
    }

    #[test]
    fn test_record_duration_and_percentile() {
        // Reset
        *JOB_DURATION_SECONDS_BUCKETS.write() = Vec::new();

        record_duration(&JOB_DURATION_SECONDS_BUCKETS, "manual", 1.0);
        record_duration(&JOB_DURATION_SECONDS_BUCKETS, "manual", 2.0);
        record_duration(&JOB_DURATION_SECONDS_BUCKETS, "manual", 3.0);

        let output = render_prometheus();
        assert!(output.contains("mixlirous_job_duration_seconds_count 3"));
        assert!(output.contains("mixlirous_job_duration_seconds_sum 6"));
    }

    #[test]
    fn test_buckets_memory_bound() {
        *DSP_STAGE_DURATION_BUCKETS.write() = Vec::new();
        for i in 0..15_000 {
            record_duration(&DSP_STAGE_DURATION_BUCKETS, "stage_x", i as f64);
        }
        let guard = DSP_STAGE_DURATION_BUCKETS.read();
        assert!(
            guard.len() <= 10_000,
            "buckets should be bounded to 10k, got {}",
            guard.len()
        );
    }

    #[test]
    fn test_render_prometheus_histogram_has_percentiles() {
        *LLM_DURATION_SECONDS_BUCKETS.write() = Vec::new();
        record_duration(&LLM_DURATION_SECONDS_BUCKETS, "openai", 0.5);
        record_duration(&LLM_DURATION_SECONDS_BUCKETS, "openai", 1.0);
        record_duration(&LLM_DURATION_SECONDS_BUCKETS, "openai", 2.0);
        record_duration(&LLM_DURATION_SECONDS_BUCKETS, "openai", 5.0);
        record_duration(&LLM_DURATION_SECONDS_BUCKETS, "openai", 10.0);

        let output = render_prometheus();
        assert!(output.contains("mixlirous_llm_duration_seconds_p50"));
        assert!(output.contains("mixlirous_llm_duration_seconds_p95"));
        assert!(output.contains("mixlirous_llm_duration_seconds_p99"));
    }
}
