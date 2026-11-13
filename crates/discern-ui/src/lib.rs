#![allow(missing_debug_implementations)] // GPUI views hold non-Debug handles

//! GPUI presentation layer (built on gpui-kit). Depends on `discern-core`,
//! never the reverse.
//!
//! - `main_page` : window content = title bar + dock area + status bar
//! - `blocks`    : panels hosted by the dock (explorer, editor, results, ...)

pub mod blocks;
pub mod main_page;
pub mod skin;

pub use main_page::MainPage;
