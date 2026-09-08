use crate::middleware::AuthContext;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::fs;

const CATALOG_PATH: &str = "prompts/catalog.json";
const PROMPTS_DIR: &str = "prompts";

#[derive(Debug, Deserialize)]
struct CatalogFile {
    prompts: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CatalogEntry {
    id: String,
    name: String,
    description: String,
    version: String,
    file: String,
    #[serde(default)]
    tags: Vec<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
pub struct ListPromptsQuery {
    pub tags: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PromptListItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub tags: Vec<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct PromptSpecResponse {
    pub id: String,
    pub name: String,
    pub version: String,
    pub status: String,
    pub description: String,
    pub tags: Vec<String>,
    pub parameters: serde_json::Value,
    pub tool_sequence: serde_json::Value,
    pub constraints: serde_json::Value,
}

fn load_catalog() -> Result<Vec<CatalogEntry>, (StatusCode, String)> {
    load_catalog_from(CATALOG_PATH)
}

fn load_catalog_from(path: &str) -> Result<Vec<CatalogEntry>, (StatusCode, String)> {
    let content = fs::read_to_string(path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("catalog unavailable: {e}"),
        )
    })?;
    let catalog: CatalogFile = serde_json::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(catalog.prompts)
}

pub async fn list_prompts(
    AuthContext(_claims): AuthContext,
    Query(query): Query<ListPromptsQuery>,
) -> Result<Json<Vec<PromptListItem>>, (StatusCode, String)> {
    let prompts = load_catalog()?;

    let filtered = prompts
        .into_iter()
        .filter(|p| match &query.tags {
            Some(tag) => p.tags.iter().any(|t| t == tag),
            None => true,
        })
        .map(|p| PromptListItem {
            id: p.id,
            name: p.name,
            version: p.version,
            tags: p.tags,
            status: p.status,
        })
        .collect();

    Ok(Json(filtered))
}

pub async fn get_prompt(
    AuthContext(_claims): AuthContext,
    Path(prompt_id): Path<String>,
) -> Result<Json<PromptSpecResponse>, (StatusCode, String)> {
    let prompts = load_catalog()?;
    let entry = prompts
        .into_iter()
        .find(|p| p.id == prompt_id)
        .ok_or((StatusCode::NOT_FOUND, "prompt_not_found".to_string()))?;

    let path = format!("{PROMPTS_DIR}/{}", entry.file);
    let spec = audio_agent::prompt_loader::load_prompt_file(&path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let empty = serde_yaml::Value::Null;
    let parameters = yaml_to_json(spec.raw.get("parameters").unwrap_or(&empty));
    let tool_sequence = yaml_to_json(spec.raw.get("tool_sequence").unwrap_or(&empty));
    let constraints = yaml_to_json(spec.raw.get("constraints").unwrap_or(&empty));

    Ok(Json(PromptSpecResponse {
        id: entry.id,
        name: entry.name,
        version: entry.version,
        status: entry.status,
        description: entry.description,
        tags: entry.tags,
        parameters,
        tool_sequence,
        constraints,
    }))
}

fn yaml_to_json(value: &serde_yaml::Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_path(relative: &str) -> String {
        // `cargo test` roda com CWD = diretório do pacote, não a raiz do
        // workspace; CARGO_MANIFEST_DIR aponta para crates/audio_api.
        format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn test_catalog_parses() {
        let prompts = load_catalog_from(&workspace_path("prompts/catalog.json"))
            .expect("catalog.json deve existir e ser válido");
        assert!(prompts.iter().any(|p| p.id == "tiktok_aggressive_v2"));
    }
}
