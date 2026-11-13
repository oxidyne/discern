use serde::{Deserialize, Serialize};

/// Everything needed to put the window back exactly as it was.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub tabs: Vec<TabState>,
    /// Index into `tabs`.
    #[serde(default)]
    pub active_tab: Option<usize>,
    /// Connection each console/script is attached to is stored per tab;
    /// this is the one selected in the explorer.
    #[serde(default)]
    pub active_connection: Option<String>,
    /// Opaque dock layout owned by the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<serde_json::Value>,
    /// Explorer directories currently expanded (workspace-relative).
    #[serde(default)]
    pub expanded_dirs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabState {
    pub source: TabSource,
    #[serde(default)]
    pub connection: Option<String>,
    /// Byte offset of the cursor.
    #[serde(default)]
    pub cursor: usize,
    #[serde(default)]
    pub scroll_line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TabSource {
    /// A script file, relative to the workspace root.
    File(String),
    /// An unsaved console buffer stored in `.discern/drafts`.
    Draft(String),
    /// The user-level `settings.json`.
    Settings,
}
