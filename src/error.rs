use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("workspace already exists at {0}")]
    WorkspaceExists(PathBuf),

    #[error("no Health Engine workspace found at {0}")]
    WorkspaceNotFound(PathBuf),

    #[error("workspace database is invalid: {0}")]
    InvalidWorkspace(String),

    #[error("invalid {kind} identifier")]
    InvalidIdentifier { kind: &'static str },

    #[error("invalid Source alias")]
    InvalidSourceAlias,

    #[error("Source alias already identifies different content")]
    SourceAliasConflict,

    #[error("Source not found")]
    SourceNotFound,

    #[error("Source content has changed")]
    SourceContentChanged,

    #[error("Source content is unavailable")]
    SourceContentUnavailable,

    #[error("invalid Record Change Set: {0}")]
    InvalidRecordChangeSet(&'static str),

    #[error("record revision does not match")]
    RecordRevisionConflict,

    #[error("idempotency key identifies different content")]
    IdempotencyConflict,

    #[error("Record Proposal integrity check failed")]
    ProposalIntegrity,

    #[error("Expungement authorization is invalid")]
    ExpungementAuthorization,

    #[error("Expungement plan is stale")]
    ExpungementPlanStale,

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

    #[error("input exceeds the configured size limit")]
    InputTooLarge,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceExists(_) => "workspace_exists",
            Self::WorkspaceNotFound(_) => "workspace_not_found",
            Self::InvalidWorkspace(_) => "invalid_workspace",
            Self::InvalidIdentifier { .. } => "invalid_identifier",
            Self::InvalidSourceAlias => "invalid_source_alias",
            Self::SourceAliasConflict => "source_alias_conflict",
            Self::SourceNotFound => "source_not_found",
            Self::SourceContentChanged => "source_content_changed",
            Self::SourceContentUnavailable => "source_content_unavailable",
            Self::InvalidRecordChangeSet(_) => "invalid_record_change_set",
            Self::RecordRevisionConflict => "record_revision_conflict",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::ProposalIntegrity => "proposal_integrity",
            Self::ExpungementAuthorization => "expungement_authorization",
            Self::ExpungementPlanStale => "expungement_plan_stale",
            Self::InvalidFact(_) => "invalid_fact",
            Self::FactNotFound(_) => "fact_not_found",
            Self::AnalysisNotFound(_) => "analysis_not_found",
            Self::Unsupported(_) => "unsupported",
            Self::Database(_) => "database_error",
            Self::Json(_) => "json_error",
            Self::InputTooLarge => "input_too_large",
            Self::Io(_) => "io_error",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
