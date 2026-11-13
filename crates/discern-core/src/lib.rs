//! Application core: UI-agnostic state and logic.
//!
//! The UI crate renders `Document`/`Workspace` state and sends `Command`s;
//! it holds no business logic. This makes the core fully unit-testable
//! without opening a window.

mod command;
mod document;
mod workspace;

pub use command::Command;
pub use document::Document;
pub use workspace::{DocumentId, Workspace};
