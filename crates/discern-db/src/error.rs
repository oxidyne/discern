#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("query failed: {0}")]
    Query(String),
    #[error("cancelled")]
    Cancelled,
}

pub type DbResult<T> = Result<T, DbError>;
