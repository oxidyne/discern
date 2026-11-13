//! On-disk layout of a workspace folder.
//!
//! ```text
//! <root>/
//!   discern.workspace.json      manifest (tiny, always read first)
//!   connections.json            data sources (no secrets)
//!   scripts/                    the user's .sql files, plain and git-friendly
//!   .discern/                   machine-local state, safe to gitignore
//!     session.json              open tabs, cursors, dock layout
//!     drafts/<id>.sql           unsaved console buffers
//!     cache/schema/<conn>.json  introspection snapshots (instant tree on open)
//! ```
//!
//! Opening a workspace reads only the manifest, connections and session —
//! three small files. Scripts are listed lazily, schema caches and drafts
//! are loaded on demand, so open time does not grow with workspace size.

use std::path::{Path, PathBuf};

pub const FORMAT_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "discern.workspace.json";
pub const CONNECTIONS_FILE: &str = "connections.json";
pub const SCRIPTS_DIR: &str = "scripts";
pub const STATE_DIR: &str = ".discern";

#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self { Self { root: root.into() } }
    pub fn root(&self) -> &Path { &self.root }
    pub fn manifest(&self) -> PathBuf { self.root.join(MANIFEST_FILE) }
    pub fn connections(&self) -> PathBuf { self.root.join(CONNECTIONS_FILE) }
    pub fn scripts(&self) -> PathBuf { self.root.join(SCRIPTS_DIR) }
    pub fn state(&self) -> PathBuf { self.root.join(STATE_DIR) }
    pub fn session(&self) -> PathBuf { self.state().join("session.json") }
    pub fn drafts(&self) -> PathBuf { self.state().join("drafts") }
    pub fn schema_cache(&self) -> PathBuf { self.state().join("cache").join("schema") }
}
