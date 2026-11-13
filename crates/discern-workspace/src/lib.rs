//! Persistent workspace (DataGrip-style project): connections, scripts,
//! session and caches in one folder, designed to open instantly.
//!
//! UI-agnostic and synchronous; the app layer decides when to call it
//! (e.g. save the session from a debounced background task).

mod cache;
mod connections;
mod error;
mod fsutil;
mod layout;
mod manifest;
mod recent;
mod scripts;
mod session;
mod settings;
mod store;

pub use cache::{ColumnInfo, SchemaInfo, SchemaSnapshot, TableInfo, TableKind};
pub use connections::SavedConnection;
pub use error::{Result, WorkspaceError};
pub use layout::{FORMAT_VERSION, Paths};
pub use manifest::Manifest;
pub use recent::{RecentEntry, RecentWorkspaces, config_dir, default_workspaces_dir};
pub use settings::{DEFINITIONS, Def, definition, Kind, Parsed, Problem, Settings, SettingsStore, Severity, Val, parse_settings};
pub use scripts::Entry;
pub use session::{Session, TabSource, TabState};
pub use store::WorkspaceStore;
