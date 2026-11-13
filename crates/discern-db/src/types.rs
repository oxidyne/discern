use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: Arc<str>,
    pub type_name: Arc<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(Arc<str>),
    Bytes(Arc<[u8]>),
}

/// A chunk of rows, row-major. Columnar storage can replace this later
/// without touching drivers' public API beyond this type.
#[derive(Debug, Clone, Default)]
pub struct RowBatch {
    pub rows: Vec<Vec<Value>>,
}
