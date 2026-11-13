use crate::{Buffer, Edit};

/// Linear undo/redo stack storing inverse edits.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

impl History {
    /// Applies `edit` to `buffer` and records it.
    pub fn apply(&mut self, buffer: &mut Buffer, edit: &Edit) {
        let inverse = buffer.apply(edit);
        self.undo.push(inverse);
        self.redo.clear();
    }

    pub fn undo(&mut self, buffer: &mut Buffer) -> Option<Edit> {
        let edit = self.undo.pop()?;
        let inverse = buffer.apply(&edit);
        self.redo.push(inverse);
        Some(edit)
    }

    pub fn redo(&mut self, buffer: &mut Buffer) -> Option<Edit> {
        let edit = self.redo.pop()?;
        let inverse = buffer.apply(&edit);
        self.undo.push(inverse);
        Some(edit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut buf = Buffer::new("select 1");
        let mut h = History::default();
        h.apply(&mut buf, &Edit { range: 7..8, text: "42".into() });
        assert_eq!(buf.to_string(), "select 42");
        h.undo(&mut buf);
        assert_eq!(buf.to_string(), "select 1");
        h.redo(&mut buf);
        assert_eq!(buf.to_string(), "select 42");
    }
}
