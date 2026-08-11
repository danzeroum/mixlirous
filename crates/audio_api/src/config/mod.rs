use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub audio: AudioConfig,
    pub llm: LlmConfig,
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub features: FeaturesConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub type_db: String,
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(rename = "type")]
    pub type_storage: String,
    pub endpoint: Option<String>,
    pub bucket: String,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_size: usize,
    pub hop_size: usize,
    #[serde(default)]
    pub crossfade_max_ms: u32,
    #[serde(default)]
    pub rms_window_ms: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: String,
    pub temperature: f32,
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,
    #[serde(default)]
    pub timeout_sec: u32,
}

fn default_max_tools() -> usize {
    5
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObservabilityConfig {
    pub otel_collector_endpoint: String,
    pub prometheus_port: u16,
    #[serde(default)]
    pub grafana_url: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FeaturesConfig {
    #[serde(default)]
    pub version_freeze: bool,
    #[serde(default)]
    pub canary_rollout_pct: u8,
    #[serde(default)]
    pub prompt_lint_enabled: bool,
    #[serde(default)]
    pub golden_master_enabled: bool,
    #[serde(default = "default_true")]
    pub rate_limit: bool,
}

fn default_true() -> bool {
    true
}

impl AppConfig {
    /// Carrega `config/default.yaml` e sobrep├Áe com `config/{CONFIG_ENV}.yaml`
    /// (se existir) e vari├íveis de ambiente com prefixo `REMIX__` (ex.:
    /// `REMIX__DATABASE__URL`). `CONFIG_ENV` default ├® `local`.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let env = std::env::var("CONFIG_ENV").unwrap_or_else(|_| "local".to_string());

        let cfg = Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(File::with_name(&format!("config/{env}")).required(false))
            .add_source(Environment::with_prefix("REMIX").separator("__"))
            .build()?;

        Ok(cfg.try_deserialize()?)
    }
}
