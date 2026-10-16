//! Durable replacement: a failed write never truncates the destination.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn unique_id() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // Follow an existing symlink instead of replacing the link itself.
    let destination = if path.exists() || path.is_symlink() {
        fs::canonicalize(path)?
    } else {
        path.to_path_buf()
    };
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let metadata = fs::metadata(&destination).ok();
    if metadata
        .as_ref()
        .is_some_and(|m| m.permissions().readonly())
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "The file is read-only",
        ));
    }
    let temporary = parent.join(format!(".zero-{}.tmp", unique_id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        if let Some(metadata) = metadata {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &destination)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_replacement_preserves_original_and_removes_temporary() {
        let dir = std::env::temp_dir().join(unique_id());
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("file");
        atomic_write(&target, b"original").unwrap();
        let mut perms = fs::metadata(&target).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&target, perms).unwrap();
        assert!(atomic_write(&target, b"replacement").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn replacement_follows_symlink_and_preserves_mode() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = std::env::temp_dir().join(unique_id());
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("script");
        fs::write(&target, b"old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
        let link = dir.join("link");
        symlink(&target, &link).unwrap();
        atomic_write(&link, b"new").unwrap();
        assert!(link.is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
