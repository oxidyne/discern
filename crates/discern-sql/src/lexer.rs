use crate::Dialect;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    Keyword,
    Ident,
    QuotedIdent,
    String,
    Number,
    Punct,
    Semicolon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Range<usize>,
}

/// Single-pass, allocation-light lexer. Never fails: unknown bytes become `Punct`.
pub fn tokenize(src: &str, dialect: Dialect) -> Vec<Token> {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(src.len() / 4);
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let c = b[i];
        let kind = match c {
            _ if c.is_ascii_whitespace() => {
                while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
                TokenKind::Whitespace
            }
            b'-' if b.get(i + 1) == Some(&b'-') => {
                while i < b.len() && b[i] != b'\n' { i += 1; }
                TokenKind::LineComment
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) { i += 1; }
                i = (i + 2).min(b.len());
                TokenKind::BlockComment
            }
            b'\'' => { i = skip_quoted(b, i, b'\''); TokenKind::String }
            b'"' | b'`' => { i = skip_quoted(b, i, c); TokenKind::QuotedIdent }
            b';' => { i += 1; TokenKind::Semicolon }
            b'0'..=b'9' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') { i += 1; }
                TokenKind::Number
            }
            _ if c == b'_' || c.is_ascii_alphabetic() || c >= 0x80 => {
                while i < b.len() && (b[i] == b'_' || b[i].is_ascii_alphanumeric() || b[i] >= 0x80) { i += 1; }
                if dialect.is_keyword(&src[start..i]) { TokenKind::Keyword } else { TokenKind::Ident }
            }
            _ => { i += 1; TokenKind::Punct }
        };
        out.push(Token { kind, span: start..i });
    }
    out
}

/// Skips a quoted run; a doubled quote is an escape. Unterminated runs end at EOF.
fn skip_quoted(b: &[u8], mut i: usize, q: u8) -> usize {
    i += 1;
    while i < b.len() {
        if b[i] == q {
            if b.get(i + 1) == Some(&q) { i += 2; continue; }
            return i + 1;
        }
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(s: &str) -> Vec<TokenKind> {
        tokenize(s, Dialect::Generic).into_iter().map(|t| t.kind).filter(|k| *k != TokenKind::Whitespace).collect()
    }

    #[test]
    fn basic_select() {
        use TokenKind::*;
        assert_eq!(kinds("SELECT a, 'x''y' FROM \"t\" -- c"),
            [Keyword, Ident, Punct, String, Keyword, QuotedIdent, LineComment]);
    }

    #[test]
    fn spans_cover_input() {
        let s = "select /* é */ 1;";
        let t = tokenize(s, Dialect::Generic);
        assert_eq!(t.iter().map(|t| t.span.len()).sum::<usize>(), s.len());
    }
}
