use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ExtractionRunId, FactId, ReconciliationId, RecordHazardId, RecordRevision, SourceId,
    SourceRegion, SubjectId, VerificationId, WorkspaceId,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorKind {
    Subject,
    Caregiver,
    Agent,
    Application,
    Clinician,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AuthorIdentity {
    pub kind: AuthorKind,
    pub identifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "precision", rename_all = "snake_case")]
pub enum PartialDate {
    Year { year: i32 },
    Month { year: i32, month: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "precision", rename_all = "snake_case")]
pub enum ClinicalTimePoint {
    Instant { value: DateTime<FixedOffset> },
    Date { value: NaiveDate },
    PartialDate { value: PartialDate },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "precision", rename_all = "snake_case")]
pub enum ClinicalTime {
    Undated {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_text: Option<String>,
    },
    Instant {
        value: DateTime<FixedOffset>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_text: Option<String>,
    },
    Date {
        value: NaiveDate,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_text: Option<String>,
    },
    PartialDate {
        value: PartialDate,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_text: Option<String>,
    },
    Interval {
        start: ClinicalTimePoint,
        end: ClinicalTimePoint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_text: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAssurance {
    ParserValidated,
    Unverified,
    SourceVerified,
    ExplicitSelfReport,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactDisposition {
    Active,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactRestriction {
    SourceDiscrepancy { verification_id: VerificationId },
    SourceContentChanged { source_id: SourceId },
    SourceContentUnavailable { source_id: SourceId },
    ConflictingReconciliation { reconciliation_id: ReconciliationId },
    RecordHazard { hazard_id: RecordHazardId },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactEvidence {
    Source {
        source_id: SourceId,
        source_region: SourceRegion,
        asserted_by: AuthorIdentity,
    },
    SelfReport {
        reporter: AuthorIdentity,
    },
    ParsedSource {
        parsed_source_id: SourceId,
        parsed_region: SourceRegion,
        cited_source_id: SourceId,
        cited_region: SourceRegion,
        asserted_by: AuthorIdentity,
    },
    SourcedSelfReport {
        source_id: SourceId,
        source_region: SourceRegion,
        reporter: AuthorIdentity,
    },
}

impl FactEvidence {
    pub fn source(&self) -> Option<(&SourceId, &SourceRegion)> {
        match self {
            Self::Source {
                source_id,
                source_region,
                ..
            }
            | Self::SourcedSelfReport {
                source_id,
                source_region,
                ..
            } => Some((source_id, source_region)),
            Self::ParsedSource {
                parsed_source_id,
                parsed_region,
                ..
            } => Some((parsed_source_id, parsed_region)),
            Self::SelfReport { .. } => None,
        }
    }

    pub fn author(&self) -> &AuthorIdentity {
        match self {
            Self::Source { asserted_by, .. } | Self::ParsedSource { asserted_by, .. } => {
                asserted_by
            }
            Self::SelfReport { reporter } | Self::SourcedSelfReport { reporter, .. } => reporter,
        }
    }

    pub fn cited_source(&self) -> Option<(&SourceId, &SourceRegion)> {
        match self {
            Self::ParsedSource {
                cited_source_id,
                cited_region,
                ..
            } => Some((cited_source_id, cited_region)),
            Self::Source { .. } | Self::SelfReport { .. } | Self::SourcedSelfReport { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReferenceRange {
    pub original: String,
    pub lower: Option<Decimal>,
    pub upper: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabValue {
    Numeric {
        value: Decimal,
    },
    QualifiedNumeric {
        comparator: LabComparator,
        value: Decimal,
        original: String,
    },
    Text {
        value: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LabComparator {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CanonicalTest {
    pub identifier: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabResult {
    pub test_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_test: Option<CanonicalTest>,
    pub value: LabValue,
    pub units: Option<String>,
    pub reference_range: Option<ReferenceRange>,
    pub reported_flag: Option<String>,
    pub performing_lab: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "measurement_type", rename_all = "snake_case")]
pub enum VitalMeasurement {
    BloodPressure {
        systolic: Decimal,
        diastolic: Decimal,
        original_systolic: String,
        original_diastolic: String,
        units: Option<String>,
        measured_by: Option<String>,
        note: Option<String>,
    },
    Pulse {
        value: Decimal,
        original: String,
        units: Option<String>,
        measured_by: Option<String>,
        note: Option<String>,
    },
    Weight {
        value: Decimal,
        original: String,
        units: Option<String>,
        note: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MedicationProductKind {
    Medication,
    Supplement,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MedicationEventKind {
    RegimenReported,
    Started,
    Stopped,
    DoseChanged,
    ScheduleChanged,
    MissedDose,
    AsNeededUse,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MedicationDose {
    pub value: Decimal,
    pub unit: String,
    pub original: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MedicationEvent {
    pub event: MedicationEventKind,
    pub product_kind: MedicationProductKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dose: Option<MedicationDose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indication: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adherence_context: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionAssertionState {
    Suspected,
    Confirmed,
    RuledOut,
    Inactive,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConditionAssertion {
    pub state: ConditionAssertionState,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_site: Option<String>,
    pub assertion_text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStudyKind {
    Imaging,
    Pathology,
    Procedure,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStudyStatus {
    Ordered,
    Scheduled,
    Performed,
    Preliminary,
    Final,
    Amended,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiagnosticFinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_site: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiagnosticStudy {
    pub kind: DiagnosticStudyKind,
    pub status: DiagnosticStudyStatus,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_site: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub findings: Vec<DiagnosticFinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulted_at: Option<ClinicalTimePoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreting_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performing_organization: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CareTaskKind {
    RepeatTest,
    Referral,
    Appointment,
    AwaitedResult,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CareTaskStatus {
    Pending,
    Due,
    Overdue,
    AwaitingResult,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CareIntervalUnit {
    Days,
    Weeks,
    Months,
    Years,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CareTaskDue {
    On {
        time: ClinicalTimePoint,
    },
    Interval {
        value: u32,
        unit: CareIntervalUnit,
        source_text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CareTask {
    pub task_key: String,
    pub kind: CareTaskKind,
    pub status: CareTaskStatus,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<CareTaskDue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<String>,
    pub source_text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubjectPreferenceCategory {
    Tracking,
    Presentation,
    PersonalRoutine,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubjectPreferenceState {
    Active,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SubjectPreference {
    pub preference_key: String,
    pub category: SubjectPreferenceCategory,
    pub state: SubjectPreferenceState,
    pub statement: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactData {
    LabResult(LabResult),
    VitalMeasurement(VitalMeasurement),
    MedicationEvent(MedicationEvent),
    ConditionAssertion(ConditionAssertion),
    DiagnosticStudy(DiagnosticStudy),
    CareTask(CareTask),
    SubjectPreference(SubjectPreference),
}

impl FactData {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::LabResult(_) => "lab_result",
            Self::VitalMeasurement(_) => "vital_measurement",
            Self::MedicationEvent(_) => "medication_event",
            Self::ConditionAssertion(_) => "condition_assertion",
            Self::DiagnosticStudy(_) => "diagnostic_study",
            Self::CareTask(_) => "care_task",
            Self::SubjectPreference(_) => "subject_preference",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FactDraft {
    pub clinical_time: ClinicalTime,
    pub evidence: FactEvidence,
    pub fact: FactData,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    LabResult,
    VitalMeasurement,
    MedicationEvent,
    ConditionAssertion,
    DiagnosticStudy,
    CareTask,
    SubjectPreference,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FactQuery {
    pub kind: Option<FactKind>,
    pub include_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct HealthFact {
    pub id: FactId,
    pub extraction_run_id: Option<ExtractionRunId>,
    pub record_revision: RecordRevision,
    pub recorded_at: DateTime<Utc>,
    pub original_assurance: EvidenceAssurance,
    pub current_assurance: EvidenceAssurance,
    pub disposition: FactDisposition,
    pub restrictions: Vec<FactRestriction>,
    #[serde(flatten)]
    pub draft: FactDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RecordSnapshot {
    pub workspace_id: WorkspaceId,
    pub subject_id: SubjectId,
    pub record_revision: RecordRevision,
    pub facts: Vec<HealthFact>,
}
