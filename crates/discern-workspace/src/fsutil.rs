//! Crash-safe file helpers. A workspace must never be left half-written:
//! every write goes to a sibling temp file, is fsynced, then renamed over
//! the target (atomic on POSIX and on NTFS).

use crate::error::{IoContext, Result, WorkspaceError};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, io::Write, path::Path};

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).at(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    {
        let mut f = fs::File::create(&tmp).at(&tmp)?;
        f.write_all(bytes).at(&tmp)?;
        f.sync_all().at(&tmp)?;
    }
    fs::rename(&tmp, path).at(path)
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|source| WorkspaceError::Json { path: path.to_owned(), source })?;
    write_atomic(path, &bytes)
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).at(path)?;
    serde_json::from_slice(&bytes).map_err(|source| WorkspaceError::Json { path: path.to_owned(), source })
}

/// Reads JSON, returning `None` if the file does not exist.
pub(crate) fn read_json_opt<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|source| WorkspaceError::Json { path: path.to_owned(), source }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(WorkspaceError::Io { path: path.to_owned(), source }),
    }
}

pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Process-unique, time-ordered id (no RNG dependency needed).
pub(crate) fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:04x}{:04x}", nanos, std::process::id() & 0xffff, COUNTER.fetch_add(1, Ordering::Relaxed) & 0xffff)
}
