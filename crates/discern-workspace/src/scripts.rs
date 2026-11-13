use crate::error::{IoContext, Result, WorkspaceError};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Path relative to the workspace root, `/`-separated.
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

/// Resolves a workspace-relative path, rejecting `..`, absolute paths and
/// anything else that could escape the root.
pub(crate) fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(WorkspaceError::InvalidPath(rel.to_owned()));
    }
    Ok(root.join(p))
}

/// Lists one directory (not recursive): directories first, then files, each
/// sorted case-insensitively. Hidden entries are skipped.
pub(crate) fn list_dir(root: &Path, rel: &str) -> Result<Vec<Entry>> {
    let dir = if rel.is_empty() { root.to_owned() } else { resolve(root, rel)? };
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(WorkspaceError::Io { path: dir, source }),
    };
    let mut out = Vec::new();
    for e in rd {
        let e = e.at(&dir)?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = e.file_type().at(&dir)?.is_dir();
        if !is_dir && !name.to_lowercase().ends_with(".sql") {
            continue;
        }
        let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        out.push(Entry { path, name, is_dir });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}
