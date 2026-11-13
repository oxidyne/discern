//! Database abstraction. Drivers (Postgres, MySQL, SQLite, ...) implement
//! [`Driver`] in their own feature-gated modules; nothing above this crate
//! knows which engine is in use.

mod error;
mod types;

pub use error::{DbError, DbResult};
pub use types::{Column, ConnectionConfig, RowBatch, Value};

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;

/// Rows are streamed in batches so huge results never sit fully in memory
/// and the UI can render the first page immediately.
pub type RowStream = Pin<Box<dyn Stream<Item = DbResult<RowBatch>> + Send>>;

#[async_trait]
pub trait Driver: Send + Sync {
    async fn connect(&self, config: &ConnectionConfig) -> DbResult<Box<dyn Connection>>;
}

#[async_trait]
pub trait Connection: Send + Sync {
    /// Column metadata is returned up front; rows follow through the stream.
    async fn execute(&self, sql: &str) -> DbResult<(Vec<Column>, RowStream)>;
    /// Cancels the in-flight statement, if any.
    async fn cancel(&self) -> DbResult<()>;
}
