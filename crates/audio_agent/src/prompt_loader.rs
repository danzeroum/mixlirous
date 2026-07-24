use serde_yaml::Value;
use std::fs;
use std::path::Path;

/// Carrega um .prompt file e valida seu schema
pub fn load_prompt_file(path: impl AsRef<Path>) -> Result<PromptSpec, PromptError> {
    let content = fs::read_to_string(path)?;
    let yaml: Value = serde_yaml::from_str(&content)?;

    // Valida schema mínimo
    let id = yaml["id"]
        .as_str()
        .ok_or(PromptError::InvalidFormat("Missing 'id'".to_string()))?
        .to_string();
    let version = yaml["version"].as_str().unwrap_or("1.0").to_string();

    Ok(PromptSpec {
        id,
        version,
        raw: yaml,
    })
}

pub struct PromptSpec {
    pub id: String,
    pub version: String,
    pub raw: serde_yaml::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Invalid prompt format: {0}")]
    InvalidFormat(String),
}
