use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("workspace already exists at {0}")]
    WorkspaceExists(PathBuf),

    #[error("no Health Engine workspace found at {0}")]
    WorkspaceNotFound(PathBuf),

    #[error("workspace database is invalid: {0}")]
    InvalidWorkspace(String),

    #[error("invalid health fact: {0}")]
    InvalidFact(String),

    #[error("fact not found: {0}")]
    FactNotFound(String),

    #[error("analysis not found: {0}")]
    AnalysisNotFound(String),

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceExists(_) => "workspace_exists",
            Self::WorkspaceNotFound(_) => "workspace_not_found",
            Self::InvalidWorkspace(_) => "invalid_workspace",
            Self::InvalidFact(_) => "invalid_fact",
            Self::FactNotFound(_) => "fact_not_found",
            Self::AnalysisNotFound(_) => "analysis_not_found",
            Self::Unsupported(_) => "unsupported",
            Self::Database(_) => "database_error",
            Self::Json(_) => "json_error",
            Self::Io(_) => "io_error",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
