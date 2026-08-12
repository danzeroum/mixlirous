use audio_core::ports::storage_trait::{validate_object_key, StorageError};
use audio_core::ports::Storage;
use bytes::Bytes;
use std::path::PathBuf;

/// Storage local em disco com escrita atomica (`.tmp` -> `fsync` -> `rename`).
pub struct LocalFsStorage {
    base_path: PathBuf,
}

impl LocalFsStorage {
    pub fn new(base_path: PathBuf) -> Result<Self, StorageError> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }

    fn resolve(&self, object_key: &str) -> PathBuf {
        self.base_path.join(object_key)
    }
}

#[async_trait::async_trait]
impl Storage for LocalFsStorage {
    async fn put(&self, object_key: &str, data: Bytes) -> Result<(), StorageError> {
        validate_object_key(object_key)?;
        let path = self.resolve(object_key);

        let path_clone = path.clone();
        let data_vec = data.to_vec();
        let write_result: Result<Result<(), std::io::Error>, _> =
            tokio::task::spawn_blocking(move || {
                crate::atomic::atomic_write(&path_clone, &data_vec)
            })
            .await;

        let inner = write_result.map_err(|e| StorageError::Backend(format!("join error: {e}")))?;
        inner.map_err(StorageError::Io)?;
        Ok(())
    }

    async fn get(&self, object_key: &str) -> Result<Bytes, StorageError> {
        validate_object_key(object_key)?;
        let path = self.resolve(object_key);
        let key_owned = object_key.to_string();

        let data = tokio::task::spawn_blocking(move || {
            std::fs::read(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::NotFound(key_owned)
                } else {
                    StorageError::Io(e)
                }
            })
        })
        .await
        .map_err(|e| StorageError::Backend(format!("join error: {e}")))??;

        Ok(Bytes::from(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalFsStorage::new(dir.path().to_path_buf()).unwrap();
        let data = Bytes::from_static(b"hello world");

        storage
            .put("tenant-1/raw/test.wav", data.clone())
            .await
            .unwrap();
        let fetched = storage.get("tenant-1/raw/test.wav").await.unwrap();
        assert_eq!(fetched, data);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalFsStorage::new(dir.path().to_path_buf()).unwrap();
        let err = storage.get("no/such/file.wav").await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_put_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalFsStorage::new(dir.path().to_path_buf()).unwrap();
        let err = storage
            .put("../etc/passwd", Bytes::new())
            .await
            .unwrap_err();
        assert!(matches!(err, StorageError::InvalidKey(_)));
    }
}
