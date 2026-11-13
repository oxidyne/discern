use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("i/o error on {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("invalid data in {path}: {source}")]
    Json { path: PathBuf, #[source] source: serde_json::Error },
    #[error("not a workspace: {0}")]
    NotAWorkspace(PathBuf),
    #[error("a workspace already exists at {0}")]
    AlreadyExists(PathBuf),
    #[error("workspace {path} was written by a newer version (format {found}, supported {supported})")]
    TooNew { path: PathBuf, found: u32, supported: u32 },
    #[error("path escapes the workspace: {0}")]
    InvalidPath(String),
    #[error("{0}")]
    Settings(String),
    #[error("unknown connection: {0}")]
    UnknownConnection(String),
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;

pub(crate) trait IoContext<T> {
    fn at(self, path: &std::path::Path) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: &std::path::Path) -> Result<T> {
        self.map_err(|source| WorkspaceError::Io { path: path.to_owned(), source })
    }
}
