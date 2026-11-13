use ropey::Rope;
use std::ops::Range;

/// A replacement of `range` (byte offsets) with `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<usize>,
    pub text: String,
}

/// Rope-backed text buffer. Cloning is O(1) (structural sharing), which lets
/// background threads (parsing, completion) work on a snapshot.
#[derive(Debug, Clone, Default)]
pub struct Buffer {
    rope: Rope,
    revision: u64,
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Self { rope: Rope::from_str(text), revision: 0 }
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Monotonic counter bumped on every edit; used to discard stale async results.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn slice(&self, range: Range<usize>) -> String {
        let r = &self.rope;
        r.byte_slice(range).to_string()
    }

    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    pub fn line_of_byte(&self, byte: usize) -> usize {
        self.rope.byte_to_line(byte)
    }

    /// Applies `edit` and returns the inverse edit (for undo).
    pub fn apply(&mut self, edit: &Edit) -> Edit {
        let removed = self.slice(edit.range.clone());
        let start = self.rope.byte_to_char(edit.range.start);
        let end = self.rope.byte_to_char(edit.range.end);
        self.rope.remove(start..end);
        self.rope.insert(start, &edit.text);
        self.revision += 1;
        Edit { range: edit.range.start..edit.range.start + edit.text.len(), text: removed }
    }
}
