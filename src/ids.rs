use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct SubjectId(String);

impl SubjectId {
    pub fn parse(value: &str) -> Result<Self> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(Error::InvalidIdentifier { kind: "subject" });
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    pub(crate) fn generate() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct SourceId(String);

impl SourceId {
    pub fn parse(value: &str) -> Result<Self> {
        let parsed =
            Uuid::parse_str(value).map_err(|_| Error::InvalidIdentifier { kind: "source" })?;
        if parsed.to_string() != value {
            return Err(Error::InvalidIdentifier { kind: "source" });
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn from_content_digest(digest: &[u8]) -> Self {
        Self(Uuid::new_v5(&Uuid::NAMESPACE_URL, digest).to_string())
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct ExtractionRunId(String);

impl ExtractionRunId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct RecordProposalId(String);

impl RecordProposalId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn parse(value: &str) -> Result<Self> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(Error::InvalidIdentifier {
                kind: "idempotency_key",
            });
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct FactId(String);

impl FactId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct VerificationId(String);

impl VerificationId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct CorrectionId(String);

impl CorrectionId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct ReconciliationId(String);

impl ReconciliationId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct RecordHazardId(String);

impl RecordHazardId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct RecordHazardResolutionId(String);

impl RecordHazardResolutionId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct DiscrepancyResolutionId(String);

impl DiscrepancyResolutionId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct AnalysisId(String);

impl AnalysisId {
    pub fn parse(value: &str) -> Result<Self> {
        let Some(digest) = value.strip_prefix("analysis-") else {
            return Err(Error::InvalidIdentifier { kind: "analysis" });
        };
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Error::InvalidIdentifier { kind: "analysis" });
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct ClaimId(String);

impl ClaimId {
    pub(crate) fn from_content_hash(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct RecordRevision(u64);

impl RecordRevision {
    pub const INITIAL: Self = Self(0);

    pub(crate) fn from_stored(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}
