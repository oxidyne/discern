//! SQL language services: lexer, statement splitting, dialects.
//!
//! Pure functions over `&str`; no I/O and no UI. Designed so that later
//! stages (incremental parser, completion, formatter) plug in as modules.

mod dialect;
mod lexer;
mod statements;

pub use dialect::Dialect;
pub use lexer::{Token, TokenKind, tokenize};
pub use statements::split_statements;
