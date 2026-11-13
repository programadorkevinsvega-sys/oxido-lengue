use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ScratchError {
    #[error("scratch root could not be prepared")]
    Root(#[source] io::Error),
    #[error("scratch job identifier is invalid")]
    InvalidJobId,
    #[error("scratch retention configuration is invalid")]
    InvalidRetention,
    #[error("scratch directory operation failed")]
    Filesystem(#[source] io::Error),
}

#[derive(Clone)]
pub struct ScratchSpace {
    root: PathBuf,
    retention: Duration,
}

impl ScratchSpace {
    pub fn new(root: &Path, retention: Duration) -> Result<Self, ScratchError> {
        if retention.is_zero() {
            return Err(ScratchError::InvalidRetention);
        }
        fs::create_dir_all(root).map_err(ScratchError::Root)?;
        let metadata = fs::symlink_metadata(root).map_err(ScratchError::Root)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ScratchError::Root(io::Error::new(
                io::ErrorKind::InvalidInput,
                "scratch root must be a real directory",
            )));
        }
        let root = fs::canonicalize(root).map_err(ScratchError::Root)?;
        Ok(Self { root, retention })
    }

    pub fn create_job_directory(&self, job_id: Uuid) -> Result<PathBuf, ScratchError> {
        if job_id.is_nil() {
            return Err(ScratchError::InvalidJobId);
        }
        let path = self.root.join(job_id.to_string());
        match fs::create_dir(&path) {
            Ok(()) => Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&path).map_err(ScratchError::Filesystem)?;
                if metadata.is_dir() && !metadata.file_type().is_symlink() {
                    Ok(path)
                } else {
                    Err(ScratchError::Filesystem(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "job scratch entry must be a real directory",
                    )))
                }
            }
            Err(error) => Err(ScratchError::Filesystem(error)),
        }
    }

    pub fn remove_job_directory(&self, job_id: Uuid) -> Result<(), ScratchError> {
        if job_id.is_nil() {
            return Err(ScratchError::InvalidJobId);
        }
        let path = self.root.join(job_id.to_string());
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                fs::remove_dir_all(path).map_err(ScratchError::Filesystem)
            }
            Ok(_) => Err(ScratchError::Filesystem(io::Error::new(
                io::ErrorKind::InvalidInput,
                "job scratch entry must be a real directory",
            ))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ScratchError::Filesystem(error)),
        }
    }

    pub fn clean_expired(&self, now: SystemTime) -> Result<usize, ScratchError> {
        let entries = fs::read_dir(&self.root).map_err(ScratchError::Filesystem)?;
        let mut removed = 0;
        for entry in entries {
            let entry = entry.map_err(ScratchError::Filesystem)?;
            if Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err() {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(ScratchError::Filesystem)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                continue;
            }
            let modified_at = metadata.modified().map_err(ScratchError::Filesystem)?;
            if now
                .duration_since(modified_at)
                .is_ok_and(|age| age >= self.retention)
            {
                fs::remove_dir_all(entry.path()).map_err(ScratchError::Filesystem)?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs, time::Duration};

    use uuid::Uuid;

    use super::ScratchSpace;

    #[test]
    fn job_directories_are_confined_and_removed_after_expiry() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("xovyra-scratch-{}", Uuid::new_v4()));
        let scratch = ScratchSpace::new(&root, Duration::from_secs(1))?;
        let job_id = Uuid::new_v4();
        let job_path = scratch.create_job_directory(job_id)?;
        let canonical_root = scratch_root(&root)?;
        assert_eq!(job_path.parent(), Some(canonical_root.as_path()));
        assert!(job_path.is_dir());
        fs::write(job_path.join("work.bin"), b"actual scratch data")?;
        assert_eq!(scratch.create_job_directory(job_id)?, job_path);
        assert_eq!(
            scratch.clean_expired(std::time::SystemTime::now() + Duration::from_secs(2))?,
            1
        );
        assert!(!job_path.exists());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn scratch_root_rejects_symbolic_links() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join(format!("xovyra-scratch-root-{}", Uuid::new_v4()));
        let real_root = base.join("real");
        let linked_root = base.join("linked");
        fs::create_dir_all(&real_root)?;
        symlink(&real_root, &linked_root)?;
        let result = ScratchSpace::new(&linked_root, Duration::from_secs(1));
        fs::remove_dir_all(&base)?;
        assert!(matches!(result, Err(super::ScratchError::Root(_))));
        Ok(())
    }

    fn scratch_root(root: &std::path::Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
        Ok(fs::canonicalize(root)?)
    }
}
