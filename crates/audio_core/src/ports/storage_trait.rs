/// Abstração de storage para objetos binários (faixas de áudio, artefatos).
///
/// Implementações: [`LocalFsStorage`](crate::storage::local_fs::LocalFsStorage)
/// (no crate `audio_api`).
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Grava `data` em `object_key`. Deve ser atômica (write-then-rename).
    async fn put(&self, object_key: &str, data: bytes::Bytes) -> Result<(), StorageError>;

    /// Lê os bytes de `object_key`. Retorna erro se não existir.
    async fn get(&self, object_key: &str) -> Result<bytes::Bytes, StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("object not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage backend error: {0}")]
    Backend(String),
    #[error("invalid object key: {0}")]
    InvalidKey(String),
}

/// Valida `object_key` contra path traversal.
///
/// Rejeita chaves que contenham `..` ou começam com `/`.
pub fn validate_object_key(key: &str) -> Result<(), StorageError> {
    if key.is_empty() {
        return Err(StorageError::InvalidKey("empty key".to_string()));
    }
    if key.contains("..") {
        return Err(StorageError::InvalidKey(
            "path traversal detected (..)".to_string(),
        ));
    }
    if key.starts_with('/') {
        return Err(StorageError::InvalidKey(
            "absolute path not allowed".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_key() {
        assert!(validate_object_key("").is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_object_key("tenant-1/raw/../../etc/passwd").is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        assert!(validate_object_key("/etc/passwd").is_err());
    }

    #[test]
    fn accepts_valid_key() {
        assert!(validate_object_key("tenant-abc/raw/song.wav").is_ok());
    }
}
