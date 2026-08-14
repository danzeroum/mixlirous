//! Primeiro boot: setup automático do ambiente de execução.
//!
//! Responsabilidades:
//! - Criar `~/.mixlirous/` com config, dados e prompts padrão
//! - Garantir que o diretório de dados do SQLite existe
//! - Detectar se Ollama está disponível (LLM local)
//! - Detectar se estamos dentro de um container Docker
//! - Exibir banner de boas-vindas no terminal

use crate::config::AppConfig;
use std::path::PathBuf;

/// Diretório padrão do usuário: `~/.mixlirous/`
fn mixlirous_home() -> PathBuf {
    dirs_home().join(".mixlirous")
}

/// Retorna o diretório home do usuário atual.
fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Verifica se estamos rodando dentro de um container Docker.
///
/// Heurísticas:
/// - `/proc/1/cgroup` contém "docker" ou "kubepods"
/// - `/.dockerenv` existe
pub fn is_running_in_docker() -> bool {
    if std::path::Path::new("/.dockerenv").exists() {
        return true;
    }
    if let Ok(content) = std::fs::read_to_string("/proc/1/cgroup") {
        if content.contains("docker") || content.contains("kubepods") {
            return true;
        }
    }
    false
}

/// Detecta se Ollama está rodando localmente.
fn detect_ollama() -> Option<String> {
    let url = "http://localhost:11434/api/tags";
    match ureq::get(url).call() {
        Ok(mut resp) => {
            tracing::info!("Ollama detectado em localhost:11434");
            let body_str = resp.body_mut().read_to_string().ok()?;
            let body: serde_json::Value = serde_json::from_str(&body_str).ok()?;
            let models = body.get("models")?.as_array()?;
            let names: Vec<String> = models
                .iter()
                .filter_map(|m| m.get("name")?.as_str().map(String::from))
                .collect();
            if names.is_empty() {
                Some("Ollama disponível (nenhum modelo instalado)".to_string())
            } else {
                Some(format!(
                    "Ollama disponível (modelos: {})",
                    names.join(", ")
                ))
            }
        }
        Err(_) => None,
    }
}

/// Setup executado em cada boot — não é apenas "primeiro".
///
/// Garante que a estrutura de diretórios existe e exibe informações
/// úteis ao usuário sobre o ambiente detectado.
pub async fn ensure_first_boot_setup(
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let home = mixlirous_home();

    // Criar diretório home do Mixlirous
    if !home.exists() {
        std::fs::create_dir_all(&home)?;
        tracing::info!(path = %home.display(), "Diretório ~/.mixlirous/ criado");
    }

    // Garantir subdiretórios
    for sub in ["data", "storage", "exports"] {
        let dir = home.join(sub);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
    }

    // Garantir que o diretório do SQLite existe (a partir da config)
    if let Some(db_dir) = config.database.url.strip_prefix("sqlite:") {
        let db_path = std::path::Path::new(db_dir);
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    // Verificar se é o primeiro boot (arquivo de marca)
    let first_boot_flag = home.join(".first_boot_done");
    let is_first = !first_boot_flag.exists();

    if is_first {
        print_banner(&home);
    }

    // Detectar Ollama
    if let Some(info) = detect_ollama() {
        tracing::info!(ollama = %info, "LLM local disponível");
    } else if is_first {
        tracing::info!(
            "Ollama não detectado. Para usar o agente de IA local, instale: https://ollama.com"
        );
    }

    if is_first {
        // Marcar primeiro boot como concluído
        std::fs::write(&first_boot_flag, chrono::Utc::now().to_rfc3339())?;
        tracing::info!("Primeiro boot concluído. Configure seu LLM em config/default.yaml.");
    }

    Ok(())
}

fn print_banner(home: &std::path::Path) {
    let version = env!("CARGO_PKG_VERSION");
    let banner = format!(
        r#"

   __  __            _  ____                  _
  |  \/  | __ _ _ __ | |/ ___|_ __ _ __ ___  __| |
  | |\/| |/ _` | '_ \| | |   | '__| '_ ` _ \/ _` |
  | |  | | (_| | | | | | |___| |  | | | | | (_| |
  |_|  |_|\__,_|_| |_|_\____|_|  |_| |_| |_|\__,_|

   Mixlirous v{version}
   ---------------------------

   Dados:   {home}
   API:     http://localhost:8080
   Docs:    docs/README.md

"#,
        version = version,
        home = home.display()
    );
    eprintln!("{banner}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixlirous_home_returns_path() {
        let home = mixlirous_home();
        assert!(home.to_string_lossy().contains(".mixlirous"));
    }

    #[test]
    fn test_is_running_in_docker_returns_bool() {
        let _ = is_running_in_docker();
    }

    #[test]
    fn test_dirs_home_fallback() {
        let _ = dirs_home();
    }
}
