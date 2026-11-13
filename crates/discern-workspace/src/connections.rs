use serde::{Deserialize, Serialize};

/// A saved data source. Secrets never live here: `password_ref` is a key
/// into the OS keychain (resolved by the app layer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedConnection {
    pub id: String,
    pub name: String,
    /// Driver id, e.g. `postgres`, `mysql`, `sqlite`.
    pub driver: String,
    /// URL without credentials.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_ref: Option<String>,
}

impl SavedConnection {
    pub fn new(name: impl Into<String>, driver: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: crate::fsutil::new_id(),
            name: name.into(),
            driver: driver.into(),
            url: url.into(),
            user: None,
            password_ref: None,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ConnectionsFile {
    #[serde(default)]
    pub connections: Vec<SavedConnection>,
}
