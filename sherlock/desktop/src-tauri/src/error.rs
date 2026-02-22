use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("task join error: {0}")]
    Join(String),
}

pub type AppResult<T> = Result<T, AppError>;
