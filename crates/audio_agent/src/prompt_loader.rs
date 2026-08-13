//! Loader de `.prompt` com templating `minijinja` (task 3.2 do
//! `docs/13-ROADMAP-SPRINTS.md`).

use minijinja::Environment;
use serde_yaml::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Spec de um prompt carregado de arquivo `.prompt`.
#[derive(Debug, Clone)]
pub struct PromptSpec {
    pub id: String,
    pub version: String,
    pub raw: Value,
}

/// Prompt renderizado, pronto para montar um `LlmRequest`.
#[derive(Debug, Clone)]
pub struct RenderedPrompt {
    pub id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub constraints: Vec<String>,
    pub tool_sequence: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Invalid prompt format: {0}")]
    InvalidFormat(String),
    #[error("Template error: {0}")]
    Template(String),
}

/// Carrega um `.prompt` e valida o schema mínimo (`id` obrigatório).
pub fn load_prompt_file(path: impl AsRef<Path>) -> Result<PromptSpec, PromptError> {
    let content = fs::read_to_string(path)?;
    let yaml: Value = serde_yaml::from_str(&content)?;

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

/// Renderiza um `PromptSpec` com `minijinja`, usando `variables` como
/// contexto. Também extrai `constraints` e `tool_sequence` do YAML.
pub fn render_prompt(
    spec: &PromptSpec,
    variables: &HashMap<String, String>,
) -> Result<RenderedPrompt, PromptError> {
    let system_template = spec
        .raw
        .get("system")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let user_template = spec
        .raw
        .get("user_template")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut env = Environment::new();
    env.add_template("system", system_template)
        .map_err(|e| PromptError::Template(e.to_string()))?;
    env.add_template("user", user_template)
        .map_err(|e| PromptError::Template(e.to_string()))?;

    let tmpl = env
        .get_template("system")
        .map_err(|e| PromptError::Template(e.to_string()))?;
    let system_prompt = tmpl
        .render(variables)
        .map_err(|e| PromptError::Template(e.to_string()))?;

    let tmpl = env
        .get_template("user")
        .map_err(|e| PromptError::Template(e.to_string()))?;
    let user_prompt = tmpl
        .render(variables)
        .map_err(|e| PromptError::Template(e.to_string()))?;

    let constraints = spec
        .raw
        .get("constraints")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let tool_sequence = spec
        .raw
        .get("tool_sequence")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Ok(RenderedPrompt {
        id: spec.id.clone(),
        system_prompt,
        user_prompt,
        constraints,
        tool_sequence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_prompt_yaml() -> String {
        r#"
id: test_prompt_v1
version: "1.0"
system: |
  Você é um engenheiro de áudio mestre.
  BPM da faixa: {{bpm}}
user_template: |
  Quero uma versão {{tone}} para {{platform}}.
  Duração: {{duration_sec}}s
constraints:
  - compression.ratio <= 6.0
tool_sequence:
  - compression
  - crossfade
"#
        .to_string()
    }

    fn write_temp(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.prompt");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn test_load_and_render_prompt() {
        let (_dir, path) = write_temp(&sample_prompt_yaml());
        let spec = load_prompt_file(&path).unwrap();
        assert_eq!(spec.id, "test_prompt_v1");

        let mut vars = HashMap::new();
        vars.insert("bpm".to_string(), "128".to_string());
        vars.insert("tone".to_string(), "agressiva".to_string());
        vars.insert("platform".to_string(), "TikTok".to_string());
        vars.insert("duration_sec".to_string(), "30".to_string());

        let rendered = render_prompt(&spec, &vars).unwrap();
        assert!(rendered.system_prompt.contains("128"));
        assert!(rendered.user_prompt.contains("agressiva"));
        assert!(rendered.user_prompt.contains("TikTok"));
        assert_eq!(rendered.constraints, vec!["compression.ratio <= 6.0"]);
        assert_eq!(rendered.tool_sequence, vec!["compression", "crossfade"]);
    }

    #[test]
    fn test_render_with_missing_variable_keeps_placeholder() {
        let (_dir, path) = write_temp(&sample_prompt_yaml());
        let spec = load_prompt_file(&path).unwrap();
        let rendered = render_prompt(&spec, &HashMap::new()).unwrap();
        assert!(rendered.system_prompt.contains("BPM da faixa:"));
    }

    #[test]
    fn test_load_missing_id_errors() {
        let (_dir, path) = write_temp("version: '1.0'\nsystem: 'hello'\n");
        let err = load_prompt_file(&path).unwrap_err();
        assert!(matches!(err, PromptError::InvalidFormat(_)));
    }
}
