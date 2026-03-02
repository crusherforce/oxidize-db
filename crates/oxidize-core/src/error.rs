use thiserror::Error;

#[derive(Debug, Error)]
pub enum OxidizeError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Corrupt database: {0}")]
    CorruptDatabase(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Key not found")]
    KeyNotFound,

    #[error("Schema error: {0}")]
    SchemaError(String),
}
