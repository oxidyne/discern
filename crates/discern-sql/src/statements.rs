use crate::{Dialect, TokenKind, tokenize};
use std::ops::Range;

/// Splits a script into statement spans (without the trailing `;`), skipping
/// semicolons inside strings/comments and dropping empty statements.
pub fn split_statements(src: &str, dialect: Dialect) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0;
    for t in tokenize(src, dialect) {
        match t.kind {
            TokenKind::Semicolon => {
                if let Some(s) = start.take() { out.push(s..end); }
            }
            TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment => {}
            _ => { start.get_or_insert(t.span.start); end = t.span.end; }
        }
    }
    if let Some(s) = start { out.push(s..end); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_semicolons_outside_strings() {
        let s = "select ';'; select 2;; -- end\n";
        let parts: Vec<_> = split_statements(s, Dialect::Generic).into_iter().map(|r| &s[r]).collect();
        assert_eq!(parts, ["select ';'", "select 2"]);
    }
}
