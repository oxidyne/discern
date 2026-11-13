use discern_sql::{Dialect, Token, split_statements, tokenize};
use discern_text::{Buffer, Edit, History, Selection};
use std::ops::Range;

/// One editor tab: text + selection + history + cached lexical analysis.
#[derive(Debug)]
pub struct Document {
    pub buffer: Buffer,
    pub selection: Selection,
    pub dialect: Dialect,
    history: History,
    tokens: Vec<Token>,
    tokens_rev: u64,
}

impl Document {
    pub fn new(text: &str, dialect: Dialect) -> Self {
        let buffer = Buffer::new(text);
        let tokens = tokenize(text, dialect);
        Self { tokens_rev: buffer.revision(), buffer, selection: Selection::default(), dialect, history: History::default(), tokens }
    }

    pub fn edit(&mut self, edit: &Edit) {
        self.history.apply(&mut self.buffer, edit);
    }

    pub fn undo(&mut self) { self.history.undo(&mut self.buffer); }
    pub fn redo(&mut self) { self.history.redo(&mut self.buffer); }

    /// Tokens for the current revision, re-lexed lazily.
    /// TODO: make incremental (relex only the dirty window).
    pub fn tokens(&mut self) -> &[Token] {
        if self.tokens_rev != self.buffer.revision() {
            self.tokens = tokenize(&self.buffer.to_string(), self.dialect);
            self.tokens_rev = self.buffer.revision();
        }
        &self.tokens
    }

    /// Span of the statement containing (or immediately preceding) `offset`.
    pub fn statement_at(&self, offset: usize) -> Option<Range<usize>> {
        let text = self.buffer.to_string();
        let stmts = split_statements(&text, self.dialect);
        stmts.iter().find(|r| r.start <= offset && offset <= r.end + 1).cloned()
            .or_else(|| stmts.into_iter().rev().find(|r| r.end <= offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_statement_under_cursor() {
        let d = Document::new("select 1;\nselect 2;", Dialect::Generic);
        assert_eq!(d.statement_at(12), Some(10..18));
    }
}
