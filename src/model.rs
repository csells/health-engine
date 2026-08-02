use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SubjectDraft {
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceReference {
    File {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<SourceLocation>,
    },
    SelfReport {
        description: String,
    },
}

impl SourceReference {
    pub fn file_path(&self) -> Option<&str> {
        match self {
            Self::File { path, .. } => Some(path),
            Self::SelfReport { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Author {
    pub kind: AuthorKind,
    pub identifier: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorKind {
    Subject,
    Caregiver,
    Agent,
    Application,
    Clinician,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct FactDraft {
    pub effective_date: NaiveDate,
    pub source: SourceReference,
    pub author: Author,
    pub extraction_confidence: f32,
    pub fact: FactData,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactData {
    LabResult(LabResult),
}

impl FactData {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::LabResult(_) => "lab_result",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LabResult {
    pub test_name: String,
    pub value: LabValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_range: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_flag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performing_lab: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabValue {
    Numeric { value: Decimal },
    Text { value: String },
}

impl fmt::Display for LabValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric { value } => write!(formatter, "{value}"),
            Self::Text { value } => formatter.write_str(value),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Discrepancy,
}

impl VerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Verified => "verified",
            Self::Discrepancy => "discrepancy",
        }
    }
}

impl std::str::FromStr for VerificationStatus {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "unverified" => Ok(Self::Unverified),
            "verified" => Ok(Self::Verified),
            "discrepancy" => Ok(Self::Discrepancy),
            _ => Err(format!("unknown verification status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct StoredFact {
    pub id: String,
    pub effective_date: NaiveDate,
    pub recorded_at: DateTime<Utc>,
    pub source: SourceReference,
    pub author: Author,
    pub extraction_confidence: f32,
    pub verification_status: VerificationStatus,
    pub is_current: bool,
    pub fact: FactData,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CorrectionDraft {
    pub fact_id: String,
    pub reason: String,
    pub replacement: FactDraft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    Verified,
    Discrepancy,
}

impl VerificationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Discrepancy => "discrepancy",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct VerificationDraft {
    pub fact_id: String,
    pub outcome: VerificationOutcome,
    pub reviewer: Author,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FactQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<VerificationStatus>,
    #[serde(default)]
    pub include_history: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceCategory {
    Unknown,
    Conflicting,
    Low,
    Moderate,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceRating {
    pub source: String,
    pub rating: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Confidence {
    pub category: ConfidenceCategory,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_ratings: Vec<SourceRating>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub kind: String,
    pub statement: String,
    pub confidence: Confidence,
    pub evidence_fact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Guidance {
    pub id: String,
    pub recommendation: String,
    pub confidence: Confidence,
    pub evidence_fact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AnalysisWarning {
    pub code: String,
    pub message: String,
    pub related_fact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Analysis {
    pub schema_version: u32,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub engine_version: String,
    pub subject_id: String,
    pub dependency_fingerprint: String,
    pub findings: Vec<Finding>,
    pub guidance: Vec<Guidance>,
    pub exclusions: Vec<String>,
    pub warnings: Vec<AnalysisWarning>,
}
