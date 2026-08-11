use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_path = parent.to_path_buf();
    tmp_path.push(format!(".tmp_{}", uuid::Uuid::new_v4().simple()));

    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(data)?;
        f.flush()?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    let _ = fs::File::open(parent).and_then(|d| d.sync_all());
    Ok(())
}

pub fn artifact_exists(path: &Path) -> bool {
    path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_atomic_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/output.wav");
        let data = b"test data";
        atomic_write(&path, data).unwrap();
        assert!(artifact_exists(&path));
        let mut buf = Vec::new();
        fs::File::open(&path).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, data);
    }

    #[test]
    fn test_artifact_exists_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        fs::File::create(&path).unwrap();
        assert!(!artifact_exists(&path));
    }

    #[test]
    fn test_artifact_exists_missing() {
        assert!(!artifact_exists(Path::new("/nonexistent/file.wav")));
    }
}