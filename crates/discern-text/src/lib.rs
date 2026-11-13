//! Text engine: rope-backed buffer, edits, selections and undo history.
//!
//! No SQL knowledge and no UI dependency. Everything is addressed in byte
//! offsets (UTF-8); conversion to line/column happens at the edges.

mod buffer;
mod history;
mod selection;

pub use buffer::{Buffer, Edit};
pub use history::History;
pub use selection::Selection;
