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
    /// Modo de operação capturado de `CONFIG_ENV` no boot (`load()`).
    ///
    /// fix CI (PR #59): rotas fail-closed como `GET /auth/local-session` LEM
    /// `config_env` do estado da aplicação em vez de `std::env::var` em
    /// tempo de request — ler a env global por request tornava o
    /// comportamento da rota mutável por qualquer thread do processo e
    /// tornava testes de integração que rodam em paralelo não-determinísticos
    /// (dois testes no mesmo binário setando valores diferentes).
    #[serde(default = "default_config_env")]
    pub config_env: String,
}

fn default_config_env() -> String {
    "local".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: Default::default(),
            storage: Default::default(),
            audio: Default::default(),
            llm: Default::default(),
            observability: Default::default(),
            features: Default::default(),
            config_env: default_config_env(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub type_db: String,
    pub url: String,
    #[serde(default)]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(rename = "type")]
    pub type_storage: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    pub bucket: String,
    #[serde(default)]
    pub access_key: Option<String>,
    #[serde(default)]
    pub secret_key: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub otel_collector_endpoint: String,
    #[serde(default)]
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
    /// Orçamento do rate limiter (req/min por chave, default 60).
    /// O browser excede 60 req/min numa única sessão (bootstrap + SSE +
    /// polling de Atividade/Biblioteca), então o E2E usa o perfil local
    /// com um valor adequado a single-user — o limiter continua ATIVO.
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,
}

fn default_rate_limit_per_minute() -> u32 {
    60
}

fn default_true() -> bool {
    true
}

impl AppConfig {
    /// Carrega `config/default.yaml` e sobrepõe com `config/{CONFIG_ENV}.yaml`
    /// (se existir) e variáveis de ambiente com prefixo `REMIX__` (ex.:
    /// `REMIX__DATABASE__URL`). `CONFIG_ENV` default é `local`.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let env = std::env::var("CONFIG_ENV").unwrap_or_else(|_| "local".to_string());

        let mut cfg: Self = Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(File::with_name(&format!("config/{env}")).required(false))
            .add_source(Environment::with_prefix("REMIX").separator("__"))
            .build()?
            .try_deserialize()?;

        // Fonte única: handlers fail-closed (ex.: GET /auth/local-session)
        // leem `state.config.config_env` — nunca a env global por request.
        cfg.config_env = env;
        Ok(cfg)
    }
}
