//! GPUI presentation layer. Depends on `discern-core`, never the reverse.
//!
//! Planned modules:
//! - `editor`  : virtualized text view (paints only visible lines, glyph/line cache)
//! - `grid`    : virtualized results table fed by `RowBatch` streams
//! - `sidebar` : schema/connection tree
//! - `theme`   : color tokens

pub mod root;
