//! Dock blocks. Each block is a `Panel` entity placed in the dock layout.
//!
//! Placeholders for now: each one gets replaced by its real implementation
//! (own module) one at a time.

mod connections;
mod editor;
mod files;
mod placeholder;

pub use connections::ConnectionsBlock;
pub use editor::EditorBlock;
pub use files::{FilesBlock, FilesEvent};
pub use placeholder::Placeholder;

use gpui_kit::component::dock::{BasePanelView, panel_handle};
use gpui_kit::{App, AppContext as _};
use std::sync::Arc;

/// Stable ids: they are persisted in saved layouts, never rename them.
pub fn results(cx: &mut App) -> Arc<dyn BasePanelView> {
    panel_handle(cx.new(|cx| Placeholder::new("discern.results", "Results", cx)))
}

pub fn snippets(cx: &mut App) -> Arc<dyn BasePanelView> {
    panel_handle(cx.new(|cx| Placeholder::new("discern.snippets", "Snippets", cx)))
}

pub fn history(cx: &mut App) -> Arc<dyn BasePanelView> {
    panel_handle(cx.new(|cx| Placeholder::new("discern.history", "History", cx)))
}

pub fn inspector(cx: &mut App) -> Arc<dyn BasePanelView> {
    panel_handle(cx.new(|cx| Placeholder::new("discern.inspector", "Inspector", cx)))
}
