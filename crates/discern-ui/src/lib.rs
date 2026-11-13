#![allow(missing_debug_implementations)] // GPUI views hold non-Debug handles

//! GPUI presentation layer (built on gpui-kit). Depends on `discern-core`,
//! never the reverse.
//!
//! - `main_page` : window content = title bar + dock area + status bar
//! - `blocks`    : panels hosted by the dock (explorer, editor, results, ...)

pub mod blocks;
pub mod layout;
pub mod main_page;
pub mod settings;
pub mod skin;

pub use main_page::{MainPage, init, open_main_window};

use discern_workspace::WorkspaceStore;
use std::{cell::RefCell, rc::Rc};

/// The open workspace, shared by every block on the (single) UI thread.
pub type SharedWorkspace = Rc<RefCell<WorkspaceStore>>;
