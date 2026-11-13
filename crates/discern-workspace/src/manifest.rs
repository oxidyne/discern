use crate::layout::FORMAT_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    pub name: String,
    pub created_at: u64,
}

impl Manifest {
    pub(crate) fn new(name: String) -> Self {
        Self { format: FORMAT_VERSION, id: crate::fsutil::new_id(), name, created_at: crate::fsutil::unix_now() }
    }
}
