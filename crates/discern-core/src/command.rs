use discern_text::Edit;

/// Every user intent is a `Command`; keybindings and menus map onto these.
#[derive(Debug, Clone)]
pub enum Command {
    Edit(Edit),
    Undo,
    Redo,
    RunStatementAtCursor,
    RunAll,
    Cancel,
}
