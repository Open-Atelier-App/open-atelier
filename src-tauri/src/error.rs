use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    WorkspaceNotFound,
    PathNotAllowed,
    IoError,
    DbError,
    ExtractionFailed,
    EmbeddingFailed,
    ProviderError,
    ProviderUnauthorized,
    ProviderRateLimited,
    ProviderUnavailable,
    KeychainError,
    AppleModelUnavailable,
    FileTooLarge,
    Unsupported,
    Cancelled,
    Internal,
}

#[derive(Debug, Serialize, Deserialize, Error, Clone)]
#[error("{message}")]
pub struct AtelierError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
}

impl AtelierError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), detail: None }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, msg)
    }

    pub fn db(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::DbError, msg)
    }

    pub fn io(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::IoError, msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::WorkspaceNotFound, msg)
    }

    pub fn path_not_allowed(path: impl Into<String>) -> Self {
        Self::new(ErrorCode::PathNotAllowed, format!("Path not allowed: {}", path.into()))
    }
}

impl From<rusqlite::Error> for AtelierError {
    fn from(e: rusqlite::Error) -> Self {
        Self::db(e.to_string())
    }
}

impl From<std::io::Error> for AtelierError {
    fn from(e: std::io::Error) -> Self {
        Self::io(e.to_string())
    }
}

impl From<serde_json::Error> for AtelierError {
    fn from(e: serde_json::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<anyhow::Error> for AtelierError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AtelierError>;
