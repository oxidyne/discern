//! Per-user list of recently opened workspaces (start screen).

use crate::{
    error::Result,
    fsutil::{read_json_opt, unix_now, write_json},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_RECENT: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub name: String,
    pub last_opened: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentFile {
    #[serde(default)]
    entries: Vec<RecentEntry>,
}

#[derive(Debug)]
pub struct RecentWorkspaces {
    file: PathBuf,
}

impl RecentWorkspaces {
    pub fn at(file: impl Into<PathBuf>) -> Self { Self { file: file.into() } }

    pub fn default_location() -> Option<Self> {
        Some(Self::at(config_dir()?.join("recent.json")))
    }

    pub fn list(&self) -> Result<Vec<RecentEntry>> {
        Ok(read_json_opt::<RecentFile>(&self.file)?.unwrap_or_default().entries)
    }

    pub fn record(&self, path: &Path, name: &str) -> Result<()> {
        let mut entries = self.list()?;
        entries.retain(|e| e.path != path);
        entries.insert(0, RecentEntry { path: path.to_owned(), name: name.to_owned(), last_opened: unix_now() });
        entries.truncate(MAX_RECENT);
        write_json(&self.file, &RecentFile { entries })
    }

    pub fn forget(&self, path: &Path) -> Result<()> {
        let mut entries = self.list()?;
        entries.retain(|e| e.path != path);
        write_json(&self.file, &RecentFile { entries })
    }
}

/// Default parent folder proposed for new workspaces, per platform:
/// `~/Documents/Discern` on macOS and Linux, `%USERPROFILE%\Documents\Discern`
/// on Windows. Falls back to `~/Discern` when there is no `Documents` folder.
pub fn default_workspaces_dir() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)?;
    let documents = home.join("Documents");
    Some(if documents.is_dir() { documents } else { home }.join("Discern"))
}

/// Per-user config folder: `~/Library/Application Support/discern` on macOS,
/// `$XDG_CONFIG_HOME/discern` (or `~/.config/discern`) on Linux,
/// `%APPDATA%\discern` on Windows.
pub fn config_dir() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else if cfg!(windows) {
        env("APPDATA")?
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
    };
    Some(base.join("discern"))
}
