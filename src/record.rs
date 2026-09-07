use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{OptionalExtension, Row, Transaction, params};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AnalysisId, AuthorIdentity, AuthorKind, CanonicalTest, CareIntervalUnit, CareTask, CareTaskDue,
    CareTaskKind, CareTaskStatus, ClinicalTime, ClinicalTimePoint, ConditionAssertion,
    ConditionAssertionState, CorrectionId, DiagnosticFinding, DiagnosticStudy, DiagnosticStudyKind,
    DiagnosticStudyStatus, DiscrepancyResolutionId, Error, EvidenceAssurance, ExtractionRunId,
    FactData, FactDisposition, FactDraft, FactEvidence, FactId, FactKind, FactQuery,
    FactRestriction, HealthFact, IdempotencyKey, LabComparator, LabResult, LabValue,
    MedicationDose, MedicationEvent, MedicationEventKind, MedicationProductKind, PartialDate,
    ReconciliationId, RecordHazardId, RecordHazardResolutionId, RecordProposalId, RecordRevision,
    RecordSnapshot, ReferenceRange, Result, SourceContentState, SourceId, SubjectId,
    SubjectPreference, SubjectPreferenceCategory, SubjectPreferenceState, VerificationId,
    VitalMeasurement, Workspace, WorkspaceId,
};

const RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_CHANGE_SET_CHANGES: usize = 4_096;
const MAX_EXTRACTION_VECTOR_ITEMS: usize = 1_024;
const MAX_CHANGE_SET_BYTES: usize = 16 * 1024 * 1024;
const LAB_TIMELINE_HEADERS: [&str; 7] = [
    "Date",
    "Test",
    "Value",
    "Units",
    "Reference Range",
    "Flag",
    "Source File",
];

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExtractorIdentity {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceRegion {
    pub locator: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionDomain {
    Laboratory,
    VitalSigns,
    Medications,
    Conditions,
    Encounters,
    DiagnosticStudies,
    CareTasks,
    SubjectPreferences,
    Genomics,
}

impl ExtractionDomain {
    fn as_str(self) -> &'static str {
        match self {
            Self::Laboratory => "laboratory",
            Self::VitalSigns => "vital_signs",
            Self::Medications => "medications",
            Self::Conditions => "conditions",
            Self::Encounters => "encounters",
            Self::DiagnosticStudies => "diagnostic_studies",
            Self::CareTasks => "care_tasks",
            Self::SubjectPreferences => "subject_preferences",
            Self::Genomics => "genomics",
        }
    }

    fn from_stored(value: &str) -> Result<Self> {
        match value {
            "laboratory" => Ok(Self::Laboratory),
            "vital_signs" => Ok(Self::VitalSigns),
            "medications" => Ok(Self::Medications),
            "conditions" => Ok(Self::Conditions),
            "encounters" => Ok(Self::Encounters),
            "diagnostic_studies" => Ok(Self::DiagnosticStudies),
            "care_tasks" => Ok(Self::CareTasks),
            "subject_preferences" => Ok(Self::SubjectPreferences),
            "genomics" => Ok(Self::Genomics),
            _ => Err(Error::InvalidWorkspace(
                "invalid Extraction Domain".to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExtractionCoverage {
    pub regions: Vec<SourceRegion>,
    pub domains: Vec<ExtractionDomain>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExtractionIssue {
    pub code: String,
    pub region: Option<SourceRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExtractionRunDraft {
    pub source_id: SourceId,
    pub coverage: ExtractionCoverage,
    pub omissions: Vec<ExtractionIssue>,
    pub failures: Vec<ExtractionIssue>,
    pub resulting_candidate_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvidenceOrigin {
    DocumentExtraction {
        source_id: SourceId,
    },
    ParserValidatedExtraction {
        source_id: SourceId,
    },
    ExplicitSelfReport {
        reporter: AuthorIdentity,
    },
    ParsedSelfReport {
        source_id: SourceId,
        reporter: AuthorIdentity,
    },
    SourceVerification {
        source_id: SourceId,
    },
    SourceCorrection {
        source_id: SourceId,
        verification_id: VerificationId,
    },
    RecordReview {
        reviewer: AuthorIdentity,
    },
}

impl EvidenceOrigin {
    fn source_id(&self) -> Option<&SourceId> {
        match self {
            Self::DocumentExtraction { source_id }
            | Self::ParserValidatedExtraction { source_id }
            | Self::ParsedSelfReport { source_id, .. }
            | Self::SourceVerification { source_id }
            | Self::SourceCorrection { source_id, .. } => Some(source_id),
            Self::ExplicitSelfReport { .. } | Self::RecordReview { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RecordChangeSet {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub subject_id: SubjectId,
    pub expected_revision: RecordRevision,
    pub idempotency_key: IdempotencyKey,
    pub evidence_origin: EvidenceOrigin,
    pub extractor: ExtractorIdentity,
    pub extraction_run: Option<ExtractionRunDraft>,
    pub changes: Vec<RecordChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabTestMapping {
    pub reported_name: String,
    pub canonical_test: CanonicalTest,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabTimelineImportRequest {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub subject_id: SubjectId,
    pub expected_revision: RecordRevision,
    pub idempotency_key: IdempotencyKey,
    pub source_id: SourceId,
    pub asserted_by: AuthorIdentity,
    pub extractor: ExtractorIdentity,
    pub test_mappings: Vec<LabTestMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabTimelineQuery {
    pub canonical_test_identifier: String,
    pub include_history: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VitalTimelineFormat {
    BloodPressurePulse,
    Weight,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct VitalTimelineImportRequest {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub subject_id: SubjectId,
    pub expected_revision: RecordRevision,
    pub idempotency_key: IdempotencyKey,
    pub source_id: SourceId,
    pub reporter: AuthorIdentity,
    pub extractor: ExtractorIdentity,
    pub format: VitalTimelineFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SubjectPreferenceMigrationRequest {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub subject_id: SubjectId,
    pub expected_revision: RecordRevision,
    pub idempotency_key: IdempotencyKey,
    pub source_id: SourceId,
    pub extractor: ExtractorIdentity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MedicationRegimenQuery {
    pub at: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MedicationRegimenEntry {
    pub identity: String,
    pub product_kind: MedicationProductKind,
    pub name: String,
    pub strength: Option<String>,
    pub dose: Option<MedicationDose>,
    pub route: Option<String>,
    pub schedule: Option<String>,
    pub indication: Option<String>,
    pub source_fact_id: FactId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConditionPictureQuery {
    pub include_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConditionPictureEntry {
    pub identity: String,
    pub name: String,
    pub state: ConditionAssertionState,
    pub body_site: Option<String>,
    pub assertion_text: String,
    pub source_fact_id: FactId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CareTaskViewQuery {
    pub include_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CareTaskViewEntry {
    pub task_key: String,
    pub kind: CareTaskKind,
    pub status: CareTaskStatus,
    pub action: String,
    pub due: Option<CareTaskDue>,
    pub requested_by: Option<String>,
    pub source_text: String,
    pub source_fact_id: FactId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SubjectPreferenceViewQuery {
    pub include_withdrawn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SubjectPreferenceViewEntry {
    pub preference_key: String,
    pub category: SubjectPreferenceCategory,
    pub state: SubjectPreferenceState,
    pub statement: String,
    pub source_text: String,
    pub source_fact_id: FactId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordChange {
    AddFact(FactDraft),
    VerifyFact(VerificationDraft),
    CorrectFact(CorrectionDraft),
    ReconcileFacts(ReconciliationDraft),
    AddRecordHazard(RecordHazardDraft),
    ResolveRecordHazard(RecordHazardResolutionDraft),
    ResolveDiscrepancy(DiscrepancyResolutionDraft),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiscrepancyResolutionDraft {
    pub discrepancy_verification_id: VerificationId,
    pub supporting_verification_id: VerificationId,
    pub rationale: String,
    pub author: AuthorIdentity,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiscrepancyResolution {
    pub id: DiscrepancyResolutionId,
    pub record_revision: RecordRevision,
    pub draft: DiscrepancyResolutionDraft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HazardVerificationState {
    Suspected,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RecordHazardDraft {
    pub affected_fact_ids: Vec<FactId>,
    pub affected_source_ids: Vec<SourceId>,
    pub problematic_statement: String,
    pub danger: String,
    pub corrected_understanding: String,
    pub author: AuthorIdentity,
    pub recorded_at: DateTime<Utc>,
    pub verification_state: HazardVerificationState,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RecordHazard {
    pub id: RecordHazardId,
    pub record_revision: RecordRevision,
    pub draft: RecordHazardDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RecordHazardResolutionDraft {
    pub hazard_id: RecordHazardId,
    pub supporting_verification_id: VerificationId,
    pub rationale: String,
    pub author: AuthorIdentity,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RecordHazardResolution {
    pub id: RecordHazardResolutionId,
    pub record_revision: RecordRevision,
    pub draft: RecordHazardResolutionDraft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationAgreement {
    Equivalent,
    Conflicting,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReconciliationDraft {
    pub fact_ids: Vec<FactId>,
    pub agreement: ReconciliationAgreement,
    pub author: AuthorIdentity,
    pub reconciled_at: DateTime<Utc>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Reconciliation {
    pub id: ReconciliationId,
    pub record_revision: RecordRevision,
    pub draft: ReconciliationDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct VerificationMethod {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    Verified,
    Discrepancy,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct VerificationDraft {
    pub fact_id: FactId,
    pub source_id: SourceId,
    pub source_region: SourceRegion,
    pub verifier: AuthorIdentity,
    pub method: VerificationMethod,
    pub checked_at: DateTime<Utc>,
    pub scope: String,
    pub outcome: VerificationOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Verification {
    pub id: VerificationId,
    pub record_revision: RecordRevision,
    pub draft: VerificationDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CorrectionDraft {
    pub prior_fact_id: FactId,
    pub replacement: FactDraft,
    pub reason: String,
    pub author: AuthorIdentity,
    pub corrected_at: DateTime<Utc>,
    pub supporting_verification_id: VerificationId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Correction {
    pub id: CorrectionId,
    pub replacement_fact_id: FactId,
    pub record_revision: RecordRevision,
    pub draft: CorrectionDraft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProposalImpact {
    pub extraction_runs: u32,
    pub health_facts: u32,
    pub already_present_facts: u32,
    pub verifications: u32,
    pub corrections: u32,
    pub reconciliations: u32,
    pub record_hazards: u32,
    pub record_hazard_resolutions: u32,
    pub discrepancy_resolutions: u32,
}

#[derive(Debug, Clone)]
pub struct RecordProposal {
    id: RecordProposalId,
    change_set: RecordChangeSet,
    extraction_run_id: Option<ExtractionRunId>,
    fact_ids: Vec<FactId>,
    existing_fact_ids: Vec<Option<FactId>>,
    verification_ids: Vec<VerificationId>,
    correction_ids: Vec<CorrectionId>,
    reconciliation_ids: Vec<ReconciliationId>,
    record_hazard_ids: Vec<RecordHazardId>,
    record_hazard_resolution_ids: Vec<RecordHazardResolutionId>,
    discrepancy_resolution_ids: Vec<DiscrepancyResolutionId>,
    impact: ProposalImpact,
}

impl RecordProposal {
    pub fn id(&self) -> &RecordProposalId {
        &self.id
    }

    pub fn expected_revision(&self) -> RecordRevision {
        self.change_set.expected_revision
    }

    pub fn impact(&self) -> ProposalImpact {
        self.impact
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CommitReceipt {
    pub schema_version: u32,
    pub proposal_id: RecordProposalId,
    pub idempotency_key: IdempotencyKey,
    pub previous_revision: RecordRevision,
    pub record_revision: RecordRevision,
    pub accepted_extraction_run_ids: Vec<ExtractionRunId>,
    pub accepted_fact_ids: Vec<String>,
    pub already_present_fact_ids: Vec<String>,
    pub accepted_verification_ids: Vec<VerificationId>,
    pub accepted_correction_ids: Vec<CorrectionId>,
    pub accepted_reconciliation_ids: Vec<ReconciliationId>,
    pub accepted_record_hazard_ids: Vec<RecordHazardId>,
    pub accepted_record_hazard_resolution_ids: Vec<RecordHazardResolutionId>,
    pub accepted_discrepancy_resolution_ids: Vec<DiscrepancyResolutionId>,
    pub newly_stale_analysis_ids: Vec<AnalysisId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExtractionRun {
    pub id: ExtractionRunId,
    pub source_id: SourceId,
    pub extractor: ExtractorIdentity,
    pub coverage: ExtractionCoverage,
    pub omissions: Vec<ExtractionIssue>,
    pub failures: Vec<ExtractionIssue>,
    pub resulting_candidate_keys: Vec<String>,
    pub record_revision: RecordRevision,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationCandidateDisposition {
    Accepted,
    Deduplicated,
    Rejected,
    Quarantined,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationCandidateReason {
    CitedSourceUnresolved,
    CanonicalTestUnmapped,
    InvalidSourceRow,
    SubjectConfirmationRequired,
    Unaccounted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MigrationCandidate {
    pub candidate_key: String,
    pub source_id: SourceId,
    pub source_region: SourceRegion,
    pub domain: ExtractionDomain,
    pub disposition: MigrationCandidateDisposition,
    pub reason: Option<MigrationCandidateReason>,
    pub owner: ExtractorIdentity,
    pub record_revision: RecordRevision,
}

pub struct HealthRecord<'workspace> {
    workspace: &'workspace Workspace,
}

impl<'workspace> HealthRecord<'workspace> {
    pub(crate) fn new(workspace: &'workspace Workspace) -> Self {
        Self { workspace }
    }

    pub fn propose_lab_timeline_import(
        &self,
        request: LabTimelineImportRequest,
    ) -> Result<RecordProposal> {
        let bytes = self
            .workspace
            .sources()
            .read_validated(&request.source_id, MAX_CHANGE_SET_BYTES)?;
        let parsed = parse_lab_timeline(self.workspace, &request, &bytes)?;
        let change_set = RecordChangeSet {
            schema_version: request.schema_version,
            workspace_id: request.workspace_id,
            subject_id: request.subject_id,
            expected_revision: request.expected_revision,
            idempotency_key: request.idempotency_key,
            evidence_origin: EvidenceOrigin::ParserValidatedExtraction {
                source_id: request.source_id.clone(),
            },
            extractor: request.extractor,
            extraction_run: Some(ExtractionRunDraft {
                source_id: request.source_id,
                coverage: ExtractionCoverage {
                    regions: parsed.regions,
                    domains: vec![ExtractionDomain::Laboratory],
                },
                omissions: Vec::new(),
                failures: parsed.failures,
                resulting_candidate_keys: parsed.candidate_keys,
            }),
            changes: parsed.changes,
        };
        self.propose(change_set)
    }

    pub fn propose_vital_timeline_import(
        &self,
        request: VitalTimelineImportRequest,
    ) -> Result<RecordProposal> {
        let bytes = self
            .workspace
            .sources()
            .read_validated(&request.source_id, MAX_CHANGE_SET_BYTES)?;
        let parsed = parse_vital_timeline(&request, &bytes)?;
        self.propose(RecordChangeSet {
            schema_version: request.schema_version,
            workspace_id: request.workspace_id,
            subject_id: request.subject_id,
            expected_revision: request.expected_revision,
            idempotency_key: request.idempotency_key,
            evidence_origin: EvidenceOrigin::ParsedSelfReport {
                source_id: request.source_id.clone(),
                reporter: request.reporter,
            },
            extractor: request.extractor,
            extraction_run: Some(ExtractionRunDraft {
                source_id: request.source_id,
                coverage: ExtractionCoverage {
                    regions: parsed.regions,
                    domains: vec![ExtractionDomain::VitalSigns],
                },
                omissions: Vec::new(),
                failures: Vec::new(),
                resulting_candidate_keys: parsed.candidate_keys,
            }),
            changes: parsed.changes,
        })
    }

    pub fn propose_subject_preference_migration(
        &self,
        request: SubjectPreferenceMigrationRequest,
    ) -> Result<RecordProposal> {
        let bytes = self
            .workspace
            .sources()
            .read_validated(&request.source_id, MAX_CHANGE_SET_BYTES)?;
        let parsed = parse_subject_preference_candidates(&bytes)?;
        self.propose(RecordChangeSet {
            schema_version: request.schema_version,
            workspace_id: request.workspace_id,
            subject_id: request.subject_id,
            expected_revision: request.expected_revision,
            idempotency_key: request.idempotency_key,
            evidence_origin: EvidenceOrigin::DocumentExtraction {
                source_id: request.source_id.clone(),
            },
            extractor: request.extractor,
            extraction_run: Some(ExtractionRunDraft {
                source_id: request.source_id,
                coverage: ExtractionCoverage {
                    regions: parsed.regions,
                    domains: vec![ExtractionDomain::SubjectPreferences],
                },
                omissions: Vec::new(),
                failures: parsed.failures,
                resulting_candidate_keys: parsed.candidate_keys,
            }),
            changes: Vec::new(),
        })
    }

    pub fn propose(&self, change_set: RecordChangeSet) -> Result<RecordProposal> {
        Self::validate_shape(&change_set)?;
        let serialized = serde_json::to_vec(&change_set)?;
        if serialized.len() > MAX_CHANGE_SET_BYTES {
            return Err(Error::InvalidRecordChangeSet("Change Set is too large"));
        }
        let proposal_hash = Sha256::digest(&serialized);
        let proposal_id = RecordProposalId::from_content_hash(format!(
            "proposal-{}",
            lowercase_hex(&proposal_hash)
        ));
        let committed_proposal: Option<String> = self
            .workspace
            .connection()
            .query_row(
                "SELECT proposal_id FROM record_commits WHERE idempotency_key = ?1",
                [change_set.idempotency_key.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        match committed_proposal.as_deref() {
            Some(committed) if committed != proposal_id.as_str() => {
                return Err(Error::IdempotencyConflict);
            }
            Some(_) => {}
            None => self.validate(&change_set)?,
        }
        let ids = derive_proposed_ids(&proposal_hash, &change_set);
        let existing_fact_ids = self.resolve_existing_facts(&change_set, &ids.fact_ids)?;
        let already_present_count = existing_fact_ids.iter().flatten().count();
        let health_fact_count = ids
            .fact_ids
            .len()
            .checked_sub(already_present_count)
            .ok_or(Error::ProposalIntegrity)?;
        let health_facts = u32::try_from(health_fact_count)
            .map_err(|_| Error::InvalidRecordChangeSet("too many Health Facts"))?;
        let already_present_facts = u32::try_from(already_present_count)
            .map_err(|_| Error::InvalidRecordChangeSet("too many Health Facts"))?;
        let extraction_runs = u32::from(change_set.extraction_run.is_some());
        let verifications = u32::try_from(ids.verification_ids.len())
            .map_err(|_| Error::InvalidRecordChangeSet("too many Verifications"))?;
        let corrections = u32::try_from(ids.correction_ids.len())
            .map_err(|_| Error::InvalidRecordChangeSet("too many Corrections"))?;
        let reconciliations = u32::try_from(ids.reconciliation_ids.len())
            .map_err(|_| Error::InvalidRecordChangeSet("too many Reconciliations"))?;
        let record_hazards = u32::try_from(ids.record_hazard_ids.len())
            .map_err(|_| Error::InvalidRecordChangeSet("too many Record Hazards"))?;
        let record_hazard_resolutions = u32::try_from(ids.record_hazard_resolution_ids.len())
            .map_err(|_| Error::InvalidRecordChangeSet("too many Record Hazard Resolutions"))?;
        let discrepancy_resolutions = u32::try_from(ids.discrepancy_resolution_ids.len())
            .map_err(|_| Error::InvalidRecordChangeSet("too many Discrepancy Resolutions"))?;
        Ok(RecordProposal {
            id: proposal_id,
            change_set,
            extraction_run_id: ids.extraction_run_id,
            fact_ids: ids.fact_ids,
            existing_fact_ids,
            verification_ids: ids.verification_ids,
            correction_ids: ids.correction_ids,
            reconciliation_ids: ids.reconciliation_ids,
            record_hazard_ids: ids.record_hazard_ids,
            record_hazard_resolution_ids: ids.record_hazard_resolution_ids,
            discrepancy_resolution_ids: ids.discrepancy_resolution_ids,
            impact: ProposalImpact {
                extraction_runs,
                health_facts,
                already_present_facts,
                verifications,
                corrections,
                reconciliations,
                record_hazards,
                record_hazard_resolutions,
                discrepancy_resolutions,
            },
        })
    }

    pub fn query(&self, query: FactQuery) -> Result<Vec<HealthFact>> {
        let current_filter = if query.include_history {
            ""
        } else {
            "AND NOT EXISTS (SELECT 1 FROM corrections c WHERE c.prior_fact_id = f.id) \
             AND NOT EXISTS (SELECT 1 FROM reconciliation_fact_edges rfe \
               JOIN reconciliations r ON r.id = rfe.reconciliation_id \
               WHERE rfe.fact_id = f.id AND r.agreement = 'equivalent' AND rfe.position > 0)"
        };
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, l.test_name, \
             l.numeric_value, l.text_value, l.units, l.reference_range_original, \
             l.reference_range_lower, l.reference_range_upper, l.reported_flag, l.performing_lab, \
             (SELECT v.outcome FROM verifications v WHERE v.fact_id = f.id \
              ORDER BY v.record_revision DESC, v.id DESC LIMIT 1), \
             (SELECT v.id FROM verifications v WHERE v.fact_id = f.id AND v.outcome = 'discrepancy' \
              AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                              WHERE resolution.discrepancy_verification_id = v.id) \
              ORDER BY v.record_revision DESC, v.id DESC LIMIT 1), \
             (SELECT r.id FROM reconciliations r \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = r.id \
              WHERE edge.fact_id = f.id AND r.agreement = 'conflicting' \
              ORDER BY r.record_revision DESC, r.id DESC LIMIT 1), \
             (SELECT h.id FROM record_hazards h \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = h.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = h.id) \
              ORDER BY h.record_revision DESC, h.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator, \
             l.canonical_test_identifier, l.canonical_test_display_name, \
             l.numeric_comparator, l.numeric_original \
             FROM facts f JOIN lab_results l ON l.fact_id = f.id \
             WHERE f.fact_type = 'lab_result' {current_filter} ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        let rows = statement
            .query_map([], StoredLabRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut facts = rows
            .into_iter()
            .map(StoredLabRow::into_fact)
            .collect::<Result<Vec<_>>>()?;
        if query.kind.is_some() && query.kind != Some(FactKind::LabResult) {
            facts.clear();
        }
        if query.kind.is_none() || query.kind == Some(FactKind::VitalMeasurement) {
            facts.extend(self.query_vital_rows(current_filter)?);
        }
        if query.kind.is_none() || query.kind == Some(FactKind::MedicationEvent) {
            facts.extend(self.query_medication_rows(current_filter)?);
        }
        if query.kind.is_none() || query.kind == Some(FactKind::ConditionAssertion) {
            facts.extend(self.query_condition_rows(current_filter)?);
        }
        if query.kind.is_none() || query.kind == Some(FactKind::DiagnosticStudy) {
            facts.extend(self.query_diagnostic_rows(current_filter)?);
        }
        if query.kind.is_none() || query.kind == Some(FactKind::CareTask) {
            facts.extend(self.query_care_task_rows(current_filter)?);
        }
        if query.kind.is_none() || query.kind == Some(FactKind::SubjectPreference) {
            facts.extend(self.query_subject_preference_rows(current_filter)?);
        }
        sort_facts_by_revision(&mut facts);
        for fact in &mut facts {
            fact.restrictions
                .retain(|restriction| !matches!(restriction, FactRestriction::RecordHazard { .. }));
            fact.restrictions.extend(
                self.unresolved_hazard_ids(&fact.id)?
                    .into_iter()
                    .map(|hazard_id| FactRestriction::RecordHazard { hazard_id }),
            );
            if let Some((source_id, _)) = fact.draft.evidence.source() {
                match self.workspace.sources().status(source_id)?.content_state {
                    SourceContentState::Available => {}
                    SourceContentState::Changed => {
                        fact.restrictions
                            .push(FactRestriction::SourceContentChanged {
                                source_id: source_id.clone(),
                            });
                    }
                    SourceContentState::Unavailable => {
                        fact.restrictions
                            .push(FactRestriction::SourceContentUnavailable {
                                source_id: source_id.clone(),
                            });
                    }
                }
            }
            fact.disposition = if fact.restrictions.is_empty() {
                FactDisposition::Active
            } else {
                FactDisposition::Quarantined
            };
        }
        Ok(facts)
    }

    fn query_vital_rows(&self, current_filter: &str) -> Result<Vec<HealthFact>> {
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, \
             v.vital_kind, v.primary_value, v.secondary_value, v.original_primary, \
             v.original_secondary, v.units, v.measured_by, v.note, \
             (SELECT verification.outcome FROM verifications verification \
              WHERE verification.fact_id = f.id \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT verification.id FROM verifications verification \
              WHERE verification.fact_id = f.id AND verification.outcome = 'discrepancy' \
                AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                                WHERE resolution.discrepancy_verification_id = verification.id) \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT reconciliation.id FROM reconciliations reconciliation \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = reconciliation.id \
              WHERE edge.fact_id = f.id AND reconciliation.agreement = 'conflicting' \
              ORDER BY reconciliation.record_revision DESC, reconciliation.id DESC LIMIT 1), \
             (SELECT hazard.id FROM record_hazards hazard \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = hazard.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = hazard.id) \
              ORDER BY hazard.record_revision DESC, hazard.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator \
             FROM facts f JOIN vital_measurements v ON v.fact_id = f.id \
             WHERE f.fact_type = 'vital_measurement' {current_filter} \
             ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        statement
            .query_map([], StoredVitalRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(StoredVitalRow::into_fact)
            .collect()
    }

    fn query_medication_rows(&self, current_filter: &str) -> Result<Vec<HealthFact>> {
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, \
             m.event_kind, m.product_kind, m.name, m.normalized_identity, m.strength, \
             m.dose_value, m.dose_unit, m.dose_original, m.route, m.schedule, m.indication, \
             m.adherence_context, \
             (SELECT verification.outcome FROM verifications verification \
              WHERE verification.fact_id = f.id \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT verification.id FROM verifications verification \
              WHERE verification.fact_id = f.id AND verification.outcome = 'discrepancy' \
                AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                                WHERE resolution.discrepancy_verification_id = verification.id) \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT reconciliation.id FROM reconciliations reconciliation \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = reconciliation.id \
              WHERE edge.fact_id = f.id AND reconciliation.agreement = 'conflicting' \
              ORDER BY reconciliation.record_revision DESC, reconciliation.id DESC LIMIT 1), \
             (SELECT hazard.id FROM record_hazards hazard \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = hazard.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = hazard.id) \
              ORDER BY hazard.record_revision DESC, hazard.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator \
             FROM facts f JOIN medication_events m ON m.fact_id = f.id \
             WHERE f.fact_type = 'medication_event' {current_filter} \
             ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        statement
            .query_map([], StoredMedicationRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(StoredMedicationRow::into_fact)
            .collect()
    }

    fn query_condition_rows(&self, current_filter: &str) -> Result<Vec<HealthFact>> {
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, \
             c.assertion_state, c.name, c.normalized_identity, c.body_site, c.assertion_text, \
             (SELECT verification.outcome FROM verifications verification \
              WHERE verification.fact_id = f.id \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT verification.id FROM verifications verification \
              WHERE verification.fact_id = f.id AND verification.outcome = 'discrepancy' \
                AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                                WHERE resolution.discrepancy_verification_id = verification.id) \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT reconciliation.id FROM reconciliations reconciliation \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = reconciliation.id \
              WHERE edge.fact_id = f.id AND reconciliation.agreement = 'conflicting' \
              ORDER BY reconciliation.record_revision DESC, reconciliation.id DESC LIMIT 1), \
             (SELECT hazard.id FROM record_hazards hazard \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = hazard.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = hazard.id) \
              ORDER BY hazard.record_revision DESC, hazard.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator \
             FROM facts f JOIN condition_assertions c ON c.fact_id = f.id \
             WHERE f.fact_type = 'condition_assertion' {current_filter} \
             ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        statement
            .query_map([], StoredConditionRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(StoredConditionRow::into_fact)
            .collect()
    }

    fn query_diagnostic_rows(&self, current_filter: &str) -> Result<Vec<HealthFact>> {
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, \
             d.study_kind, d.result_status, d.name, d.body_site, d.method, d.impression, \
             d.resulted_time_kind, d.resulted_time_value, d.ordering_provider, \
             d.interpreting_provider, d.performing_organization, \
             (SELECT verification.outcome FROM verifications verification \
              WHERE verification.fact_id = f.id \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT verification.id FROM verifications verification \
              WHERE verification.fact_id = f.id AND verification.outcome = 'discrepancy' \
                AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                                WHERE resolution.discrepancy_verification_id = verification.id) \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT reconciliation.id FROM reconciliations reconciliation \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = reconciliation.id \
              WHERE edge.fact_id = f.id AND reconciliation.agreement = 'conflicting' \
              ORDER BY reconciliation.record_revision DESC, reconciliation.id DESC LIMIT 1), \
             (SELECT hazard.id FROM record_hazards hazard \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = hazard.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = hazard.id) \
              ORDER BY hazard.record_revision DESC, hazard.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator \
             FROM facts f JOIN diagnostic_studies d ON d.fact_id = f.id \
             WHERE f.fact_type = 'diagnostic_study' {current_filter} \
             ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        let rows = statement
            .query_map([], StoredDiagnosticRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|row| {
                let findings = self.load_diagnostic_findings(&row.id)?;
                row.into_fact(findings)
            })
            .collect()
    }

    fn load_diagnostic_findings(&self, fact_id: &str) -> Result<Vec<DiagnosticFinding>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT section, body_site, finding_text FROM diagnostic_study_findings \
             WHERE study_fact_id = ?1 ORDER BY position",
        )?;
        Ok(statement
            .query_map([fact_id], |row| {
                Ok(DiagnosticFinding {
                    section: row.get(0)?,
                    body_site: row.get(1)?,
                    text: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn query_care_task_rows(&self, current_filter: &str) -> Result<Vec<HealthFact>> {
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, \
             t.task_key, t.task_kind, t.task_status, t.action, t.due_kind, t.due_time_kind, \
             t.due_time_value, t.due_interval_value, t.due_interval_unit, t.due_source_text, \
             t.requested_by, t.source_text, \
             (SELECT verification.outcome FROM verifications verification \
              WHERE verification.fact_id = f.id \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT verification.id FROM verifications verification \
              WHERE verification.fact_id = f.id AND verification.outcome = 'discrepancy' \
                AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                                WHERE resolution.discrepancy_verification_id = verification.id) \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT reconciliation.id FROM reconciliations reconciliation \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = reconciliation.id \
              WHERE edge.fact_id = f.id AND reconciliation.agreement = 'conflicting' \
              ORDER BY reconciliation.record_revision DESC, reconciliation.id DESC LIMIT 1), \
             (SELECT hazard.id FROM record_hazards hazard \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = hazard.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = hazard.id) \
              ORDER BY hazard.record_revision DESC, hazard.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator \
             FROM facts f JOIN care_tasks t ON t.fact_id = f.id \
             WHERE f.fact_type = 'care_task' {current_filter} \
             ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        statement
            .query_map([], StoredCareTaskRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(StoredCareTaskRow::into_fact)
            .collect()
    }

    fn query_subject_preference_rows(&self, current_filter: &str) -> Result<Vec<HealthFact>> {
        let sql = format!(
            "SELECT f.id, f.clinical_time_kind, f.clinical_start_value, f.clinical_source_text, \
             f.recorded_at, f.source_id, f.source_region_locator, f.extraction_run_id, \
             f.evidence_assurance, f.author_kind, f.author_identifier, f.record_revision, \
             p.preference_key, p.preference_category, p.preference_state, p.statement, \
             p.source_text, \
             (SELECT verification.outcome FROM verifications verification \
              WHERE verification.fact_id = f.id \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT verification.id FROM verifications verification \
              WHERE verification.fact_id = f.id AND verification.outcome = 'discrepancy' \
                AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions resolution \
                                WHERE resolution.discrepancy_verification_id = verification.id) \
              ORDER BY verification.record_revision DESC, verification.id DESC LIMIT 1), \
             (SELECT reconciliation.id FROM reconciliations reconciliation \
              JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = reconciliation.id \
              WHERE edge.fact_id = f.id AND reconciliation.agreement = 'conflicting' \
              ORDER BY reconciliation.record_revision DESC, reconciliation.id DESC LIMIT 1), \
             (SELECT hazard.id FROM record_hazards hazard \
              JOIN record_hazard_fact_edges edge ON edge.hazard_id = hazard.id \
              WHERE edge.fact_id = f.id \
                AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                                WHERE resolution.hazard_id = hazard.id) \
              ORDER BY hazard.record_revision DESC, hazard.id DESC LIMIT 1), \
             f.clinical_start_kind, f.clinical_end_kind, f.clinical_end_value, \
             f.cited_source_id, f.cited_source_region_locator \
             FROM facts f JOIN subject_preferences p ON p.fact_id = f.id \
             WHERE f.fact_type = 'subject_preference' {current_filter} \
             ORDER BY f.record_revision, f.id"
        );
        let mut statement = self.workspace.connection().prepare(&sql)?;
        statement
            .query_map([], StoredSubjectPreferenceRow::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(StoredSubjectPreferenceRow::into_fact)
            .collect()
    }

    pub fn migration_candidates(&self, source_id: &SourceId) -> Result<Vec<MigrationCandidate>> {
        let runs = load_extraction_runs(self.workspace, source_id)?;
        let mut candidates = Vec::new();
        for run in runs {
            let domain = run
                .coverage
                .domains
                .first()
                .copied()
                .ok_or_else(invalid_fact_store)?;
            for candidate_key in &run.resulting_candidate_keys {
                let failure = run.failures.iter().find(|issue| {
                    issue.region.as_ref().map(|region| region.locator.as_str())
                        == Some(candidate_key.as_str())
                });
                let (disposition, reason) = if let Some(issue) = failure {
                    (
                        MigrationCandidateDisposition::Quarantined,
                        Some(migration_candidate_reason(&issue.code)),
                    )
                } else {
                    let accepted: i64 = self.workspace.connection().query_row(
                        "SELECT EXISTS(SELECT 1 FROM facts \
                         WHERE extraction_run_id = ?1 AND source_id = ?2 \
                           AND source_region_locator = ?3)",
                        params![run.id.as_str(), source_id.as_str(), candidate_key],
                        |row| row.get(0),
                    )?;
                    if accepted != 0 {
                        (MigrationCandidateDisposition::Accepted, None)
                    } else {
                        let existing: i64 = self.workspace.connection().query_row(
                            "SELECT EXISTS(SELECT 1 FROM facts \
                             WHERE source_id = ?1 AND source_region_locator = ?2)",
                            params![source_id.as_str(), candidate_key],
                            |row| row.get(0),
                        )?;
                        if existing != 0 {
                            (MigrationCandidateDisposition::Deduplicated, None)
                        } else {
                            (
                                MigrationCandidateDisposition::Quarantined,
                                Some(MigrationCandidateReason::Unaccounted),
                            )
                        }
                    }
                };
                candidates.push(MigrationCandidate {
                    candidate_key: candidate_key.clone(),
                    source_id: source_id.clone(),
                    source_region: SourceRegion {
                        locator: candidate_key.clone(),
                    },
                    domain,
                    disposition,
                    reason,
                    owner: run.extractor.clone(),
                    record_revision: run.record_revision,
                });
            }
        }
        Ok(candidates)
    }

    pub fn lab_timeline(&self, query: &LabTimelineQuery) -> Result<Vec<HealthFact>> {
        validate_text(
            &query.canonical_test_identifier,
            "invalid Canonical Test identifier",
        )?;
        let mut facts = self.query(FactQuery {
            kind: Some(FactKind::LabResult),
            include_history: query.include_history,
        })?;
        facts.retain(|fact| {
            let FactData::LabResult(lab) = &fact.draft.fact else {
                return false;
            };
            lab.canonical_test
                .as_ref()
                .is_some_and(|canonical| canonical.identifier == query.canonical_test_identifier)
        });
        facts.sort_by(|left, right| {
            clinical_time_order_key(&left.draft.clinical_time)
                .cmp(&clinical_time_order_key(&right.draft.clinical_time))
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        Ok(facts)
    }

    pub fn medication_regimen(
        &self,
        query: &MedicationRegimenQuery,
    ) -> Result<Vec<MedicationRegimenEntry>> {
        let mut facts = self.query(FactQuery {
            kind: Some(FactKind::MedicationEvent),
            include_history: false,
        })?;
        facts.sort_by(|left, right| {
            clinical_time_order_key(&left.draft.clinical_time)
                .cmp(&clinical_time_order_key(&right.draft.clinical_time))
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        let through = query.at.map(|date| (date_order_key(date), u8::MAX));
        let mut active = BTreeMap::<String, MedicationRegimenEntry>::new();
        for fact in facts {
            if fact.disposition != FactDisposition::Active
                || through
                    .is_some_and(|limit| clinical_time_order_key(&fact.draft.clinical_time) > limit)
            {
                continue;
            }
            let FactData::MedicationEvent(event) = &fact.draft.fact else {
                continue;
            };
            let identity = event
                .normalized_identity
                .clone()
                .unwrap_or_else(|| event.name.trim().to_lowercase());
            match event.event {
                MedicationEventKind::RegimenReported | MedicationEventKind::Started => {
                    active.insert(
                        identity.clone(),
                        medication_regimen_entry(identity, event, fact.id),
                    );
                }
                MedicationEventKind::DoseChanged | MedicationEventKind::ScheduleChanged => {
                    if let Some(current) = active.get_mut(&identity) {
                        current.product_kind = event.product_kind;
                        current.name.clone_from(&event.name);
                        if event.strength.is_some() {
                            current.strength.clone_from(&event.strength);
                        }
                        if event.dose.is_some() {
                            current.dose.clone_from(&event.dose);
                        }
                        if event.route.is_some() {
                            current.route.clone_from(&event.route);
                        }
                        if event.schedule.is_some() {
                            current.schedule.clone_from(&event.schedule);
                        }
                        if event.indication.is_some() {
                            current.indication.clone_from(&event.indication);
                        }
                        current.source_fact_id = fact.id;
                    }
                }
                MedicationEventKind::Stopped => {
                    active.remove(&identity);
                }
                MedicationEventKind::MissedDose | MedicationEventKind::AsNeededUse => {}
            }
        }
        Ok(active.into_values().collect())
    }

    pub fn condition_picture(
        &self,
        query: &ConditionPictureQuery,
    ) -> Result<Vec<ConditionPictureEntry>> {
        let mut facts = self.query(FactQuery {
            kind: Some(FactKind::ConditionAssertion),
            include_history: false,
        })?;
        facts.sort_by(|left, right| {
            clinical_time_order_key(&left.draft.clinical_time)
                .cmp(&clinical_time_order_key(&right.draft.clinical_time))
                .then_with(|| {
                    left.record_revision
                        .value()
                        .cmp(&right.record_revision.value())
                })
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        let mut picture = BTreeMap::<String, ConditionPictureEntry>::new();
        for fact in facts {
            if fact.disposition != FactDisposition::Active {
                continue;
            }
            let FactData::ConditionAssertion(assertion) = &fact.draft.fact else {
                continue;
            };
            let identity = assertion
                .normalized_identity
                .clone()
                .unwrap_or_else(|| assertion.name.trim().to_lowercase());
            picture.insert(
                identity.clone(),
                ConditionPictureEntry {
                    identity,
                    name: assertion.name.clone(),
                    state: assertion.state,
                    body_site: assertion.body_site.clone(),
                    assertion_text: assertion.assertion_text.clone(),
                    source_fact_id: fact.id,
                },
            );
        }
        if !query.include_inactive {
            picture.retain(|_, condition| {
                matches!(
                    condition.state,
                    ConditionAssertionState::Suspected | ConditionAssertionState::Confirmed
                )
            });
        }
        Ok(picture.into_values().collect())
    }

    pub fn care_tasks(&self, query: &CareTaskViewQuery) -> Result<Vec<CareTaskViewEntry>> {
        let mut facts = self.query(FactQuery {
            kind: Some(FactKind::CareTask),
            include_history: false,
        })?;
        facts.sort_by(|left, right| {
            clinical_time_order_key(&left.draft.clinical_time)
                .cmp(&clinical_time_order_key(&right.draft.clinical_time))
                .then_with(|| {
                    left.record_revision
                        .value()
                        .cmp(&right.record_revision.value())
                })
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        let mut tasks = BTreeMap::<String, CareTaskViewEntry>::new();
        for fact in facts {
            if fact.disposition != FactDisposition::Active {
                continue;
            }
            let FactData::CareTask(task) = &fact.draft.fact else {
                continue;
            };
            tasks.insert(
                task.task_key.clone(),
                CareTaskViewEntry {
                    task_key: task.task_key.clone(),
                    kind: task.kind,
                    status: task.status,
                    action: task.action.clone(),
                    due: task.due.clone(),
                    requested_by: task.requested_by.clone(),
                    source_text: task.source_text.clone(),
                    source_fact_id: fact.id,
                },
            );
        }
        if !query.include_closed {
            tasks.retain(|_, task| {
                !matches!(
                    task.status,
                    CareTaskStatus::Completed | CareTaskStatus::Cancelled
                )
            });
        }
        Ok(tasks.into_values().collect())
    }

    pub fn subject_preferences(
        &self,
        query: &SubjectPreferenceViewQuery,
    ) -> Result<Vec<SubjectPreferenceViewEntry>> {
        let mut facts = self.query(FactQuery {
            kind: Some(FactKind::SubjectPreference),
            include_history: false,
        })?;
        facts.sort_by(|left, right| {
            clinical_time_order_key(&left.draft.clinical_time)
                .cmp(&clinical_time_order_key(&right.draft.clinical_time))
                .then_with(|| {
                    left.record_revision
                        .value()
                        .cmp(&right.record_revision.value())
                })
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        let mut preferences = BTreeMap::<String, SubjectPreferenceViewEntry>::new();
        for fact in facts {
            if fact.disposition != FactDisposition::Active {
                continue;
            }
            let FactData::SubjectPreference(preference) = &fact.draft.fact else {
                continue;
            };
            preferences.insert(
                preference.preference_key.clone(),
                SubjectPreferenceViewEntry {
                    preference_key: preference.preference_key.clone(),
                    category: preference.category,
                    state: preference.state,
                    statement: preference.statement.clone(),
                    source_text: preference.source_text.clone(),
                    source_fact_id: fact.id,
                },
            );
        }
        if !query.include_withdrawn {
            preferences
                .retain(|_, preference| preference.state != SubjectPreferenceState::Withdrawn);
        }
        Ok(preferences.into_values().collect())
    }

    fn unresolved_hazard_ids(&self, fact_id: &FactId) -> Result<Vec<RecordHazardId>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT h.id FROM record_hazards h \
             JOIN record_hazard_fact_edges edge ON edge.hazard_id = h.id \
             WHERE edge.fact_id = ?1 \
               AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions resolution \
                               WHERE resolution.hazard_id = h.id) \
             ORDER BY h.record_revision, h.id",
        )?;
        Ok(statement
            .query_map([fact_id.as_str()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(RecordHazardId::from_stored)
            .collect())
    }

    pub fn snapshot(&self, revision: RecordRevision) -> Result<RecordSnapshot> {
        let status = self.workspace.status()?;
        if revision != status.record_revision {
            return Err(Error::Unsupported(
                "historical Record Snapshot is not implemented".to_owned(),
            ));
        }
        Ok(RecordSnapshot {
            workspace_id: status.workspace_id,
            subject_id: status.subject_id,
            record_revision: revision,
            facts: self.query(FactQuery::default())?,
        })
    }

    pub fn verifications(&self, fact_id: &FactId) -> Result<Vec<Verification>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT id, source_id, source_region_locator, verifier_kind, verifier_identifier, \
             method_name, method_version, checked_at, scope, outcome, record_revision \
             FROM verifications WHERE fact_id = ?1 ORDER BY record_revision, id",
        )?;
        let rows = statement
            .query_map([fact_id.as_str()], |row| {
                Ok(StoredVerificationRow {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    source_region_locator: row.get(2)?,
                    verifier_kind: row.get(3)?,
                    verifier_identifier: row.get(4)?,
                    method_name: row.get(5)?,
                    method_version: row.get(6)?,
                    checked_at: row.get(7)?,
                    scope: row.get(8)?,
                    outcome: row.get(9)?,
                    record_revision: row.get(10)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|row| row.into_verification(fact_id))
            .collect()
    }

    pub fn discrepancy_resolutions(
        &self,
        discrepancy_verification_id: &VerificationId,
    ) -> Result<Vec<DiscrepancyResolution>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT id, supporting_verification_id, rationale, author_kind, author_identifier, \
             resolved_at, record_revision FROM discrepancy_resolutions \
             WHERE discrepancy_verification_id = ?1 ORDER BY record_revision, id",
        )?;
        let rows = statement
            .query_map([discrepancy_verification_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    supporting_id,
                    rationale,
                    author_kind,
                    author_identifier,
                    resolved_at,
                    revision,
                )| {
                    let resolved_at = DateTime::parse_from_rfc3339(&resolved_at)
                        .map_err(|_| invalid_fact_store())?
                        .with_timezone(&Utc);
                    let revision = u64::try_from(revision).map_err(|_| invalid_fact_store())?;
                    Ok(DiscrepancyResolution {
                        id: DiscrepancyResolutionId::from_stored(id),
                        record_revision: RecordRevision::from_stored(revision),
                        draft: DiscrepancyResolutionDraft {
                            discrepancy_verification_id: discrepancy_verification_id.clone(),
                            supporting_verification_id: VerificationId::from_stored(supporting_id),
                            rationale,
                            author: AuthorIdentity {
                                kind: parse_author_kind(&author_kind)?,
                                identifier: author_identifier,
                            },
                            resolved_at,
                        },
                    })
                },
            )
            .collect()
    }

    pub fn corrections(&self, prior_fact_id: &FactId) -> Result<Vec<Correction>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT id, replacement_fact_id, reason, author_kind, author_identifier, corrected_at, \
             supporting_verification_id, record_revision FROM corrections \
             WHERE prior_fact_id = ?1 ORDER BY record_revision, id",
        )?;
        let rows = statement
            .query_map([prior_fact_id.as_str()], |row| {
                Ok(StoredCorrectionRow {
                    id: row.get(0)?,
                    replacement_fact_id: row.get(1)?,
                    reason: row.get(2)?,
                    author_kind: row.get(3)?,
                    author_identifier: row.get(4)?,
                    corrected_at: row.get(5)?,
                    supporting_verification_id: row.get(6)?,
                    record_revision: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let replacement_facts = self.query(FactQuery {
            kind: None,
            include_history: true,
        })?;
        rows.into_iter()
            .map(|row| row.into_correction(prior_fact_id, &replacement_facts))
            .collect()
    }

    pub fn reconciliations(&self, fact_id: &FactId) -> Result<Vec<Reconciliation>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT r.id, r.agreement, r.author_kind, r.author_identifier, r.reconciled_at, \
             r.rationale, r.record_revision FROM reconciliations r \
             JOIN reconciliation_fact_edges edge ON edge.reconciliation_id = r.id \
             WHERE edge.fact_id = ?1 ORDER BY r.record_revision, r.id",
        )?;
        let rows = statement
            .query_map([fact_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    agreement,
                    author_kind,
                    author_identifier,
                    reconciled_at,
                    rationale,
                    revision,
                )| {
                    let agreement = match agreement.as_str() {
                        "equivalent" => ReconciliationAgreement::Equivalent,
                        "conflicting" => ReconciliationAgreement::Conflicting,
                        _ => return Err(invalid_fact_store()),
                    };
                    let author_kind = parse_author_kind(&author_kind)?;
                    let reconciled_at = DateTime::parse_from_rfc3339(&reconciled_at)
                        .map_err(|_| invalid_fact_store())?
                        .with_timezone(&Utc);
                    let revision = u64::try_from(revision).map_err(|_| invalid_fact_store())?;
                    Ok(Reconciliation {
                        id: ReconciliationId::from_stored(id.clone()),
                        record_revision: RecordRevision::from_stored(revision),
                        draft: ReconciliationDraft {
                            fact_ids: self.reconciliation_fact_ids(&id)?,
                            agreement,
                            author: AuthorIdentity {
                                kind: author_kind,
                                identifier: author_identifier,
                            },
                            reconciled_at,
                            rationale,
                        },
                    })
                },
            )
            .collect()
    }

    fn reconciliation_fact_ids(&self, reconciliation_id: &str) -> Result<Vec<FactId>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT fact_id FROM reconciliation_fact_edges WHERE reconciliation_id = ?1 \
             ORDER BY position",
        )?;
        Ok(statement
            .query_map([reconciliation_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(FactId::from_stored)
            .collect())
    }

    pub fn hazards(&self, fact_id: &FactId) -> Result<Vec<RecordHazard>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT h.id, h.problematic_statement, h.danger, h.corrected_understanding, \
             h.author_kind, h.author_identifier, h.recorded_at, h.verification_state, \
             h.record_revision FROM record_hazards h \
             JOIN record_hazard_fact_edges edge ON edge.hazard_id = h.id \
             WHERE edge.fact_id = ?1 ORDER BY h.record_revision, h.id",
        )?;
        let rows = statement
            .query_map([fact_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    problematic_statement,
                    danger,
                    corrected_understanding,
                    author_kind,
                    author_identifier,
                    recorded_at,
                    verification_state,
                    revision,
                )| {
                    let author_kind = parse_author_kind(&author_kind)?;
                    let recorded_at = DateTime::parse_from_rfc3339(&recorded_at)
                        .map_err(|_| invalid_fact_store())?
                        .with_timezone(&Utc);
                    let verification_state = match verification_state.as_str() {
                        "suspected" => HazardVerificationState::Suspected,
                        "verified" => HazardVerificationState::Verified,
                        _ => return Err(invalid_fact_store()),
                    };
                    let revision = u64::try_from(revision).map_err(|_| invalid_fact_store())?;
                    Ok(RecordHazard {
                        id: RecordHazardId::from_stored(id.clone()),
                        record_revision: RecordRevision::from_stored(revision),
                        draft: RecordHazardDraft {
                            affected_fact_ids: self.hazard_fact_ids(&id)?,
                            affected_source_ids: self.hazard_source_ids(&id)?,
                            problematic_statement,
                            danger,
                            corrected_understanding,
                            author: AuthorIdentity {
                                kind: author_kind,
                                identifier: author_identifier,
                            },
                            recorded_at,
                            verification_state,
                        },
                    })
                },
            )
            .collect()
    }

    pub fn hazard_resolutions(
        &self,
        hazard_id: &RecordHazardId,
    ) -> Result<Vec<RecordHazardResolution>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT id, supporting_verification_id, rationale, author_kind, author_identifier, \
             resolved_at, record_revision FROM record_hazard_resolutions \
             WHERE hazard_id = ?1 ORDER BY record_revision, id",
        )?;
        let rows = statement
            .query_map([hazard_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    supporting_id,
                    rationale,
                    author_kind,
                    author_identifier,
                    resolved_at,
                    revision,
                )| {
                    let resolved_at = DateTime::parse_from_rfc3339(&resolved_at)
                        .map_err(|_| invalid_fact_store())?
                        .with_timezone(&Utc);
                    let revision = u64::try_from(revision).map_err(|_| invalid_fact_store())?;
                    Ok(RecordHazardResolution {
                        id: RecordHazardResolutionId::from_stored(id),
                        record_revision: RecordRevision::from_stored(revision),
                        draft: RecordHazardResolutionDraft {
                            hazard_id: hazard_id.clone(),
                            supporting_verification_id: VerificationId::from_stored(supporting_id),
                            rationale,
                            author: AuthorIdentity {
                                kind: parse_author_kind(&author_kind)?,
                                identifier: author_identifier,
                            },
                            resolved_at,
                        },
                    })
                },
            )
            .collect()
    }

    fn hazard_fact_ids(&self, hazard_id: &str) -> Result<Vec<FactId>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT fact_id FROM record_hazard_fact_edges WHERE hazard_id = ?1 ORDER BY position",
        )?;
        Ok(statement
            .query_map([hazard_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(FactId::from_stored)
            .collect())
    }

    fn hazard_source_ids(&self, hazard_id: &str) -> Result<Vec<SourceId>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT source_id FROM record_hazard_source_edges WHERE hazard_id = ?1 \
             ORDER BY position",
        )?;
        Ok(statement
            .query_map([hazard_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(SourceId::from_stored)
            .collect())
    }

    pub fn apply(&self, proposal: RecordProposal) -> Result<CommitReceipt> {
        let serialized = serde_json::to_vec(&proposal.change_set)?;
        let expected_id = RecordProposalId::from_content_hash(format!(
            "proposal-{}",
            lowercase_hex(&Sha256::digest(&serialized))
        ));
        if proposal.id != expected_id {
            return Err(Error::ProposalIntegrity);
        }

        let transaction = self.workspace.connection().unchecked_transaction()?;
        if let Some((stored_proposal, receipt_json)) = transaction
            .query_row(
                "SELECT proposal_id, receipt_json FROM record_commits WHERE idempotency_key = ?1",
                [proposal.change_set.idempotency_key.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if stored_proposal != proposal.id.as_str() {
                return Err(Error::IdempotencyConflict);
            }
            return serde_json::from_str(&receipt_json).map_err(Error::from);
        }

        if let Some(source_id) = proposal.change_set.evidence_origin.source_id() {
            self.require_available_source(source_id)?;
        }
        let current_revision = current_revision(&transaction)?;
        if current_revision != proposal.change_set.expected_revision {
            return Err(Error::RecordRevisionConflict);
        }
        let next_value = current_revision
            .value()
            .checked_add(1)
            .ok_or(Error::InvalidRecordChangeSet("record revision overflow"))?;
        let next_revision = RecordRevision::from_stored(next_value);
        insert_extraction_run(&transaction, &proposal, next_revision)?;
        insert_facts(&transaction, &proposal, next_revision, self.workspace.now())?;
        insert_verifications(&transaction, &proposal, next_revision)?;
        insert_corrections(&transaction, &proposal, next_revision)?;
        insert_reconciliations(&transaction, &proposal, next_revision)?;
        insert_record_hazards(&transaction, &proposal, next_revision)?;
        insert_record_hazard_resolutions(&transaction, &proposal, next_revision)?;
        insert_discrepancy_resolutions(&transaction, &proposal, next_revision)?;
        let newly_stale_analysis_ids =
            crate::analysis::mark_fresh_analyses_stale(&transaction, next_revision)?;
        let changed = transaction.execute(
            "UPDATE workspace SET record_revision = ?1 WHERE singleton = 1 AND record_revision = ?2",
            params![
                i64::try_from(next_value)
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?,
                i64::try_from(current_revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
        if changed != 1 {
            return Err(Error::RecordRevisionConflict);
        }
        let receipt = CommitReceipt {
            schema_version: RECORD_SCHEMA_VERSION,
            proposal_id: proposal.id.clone(),
            idempotency_key: proposal.change_set.idempotency_key.clone(),
            previous_revision: current_revision,
            record_revision: next_revision,
            accepted_extraction_run_ids: proposal.extraction_run_id.into_iter().collect(),
            accepted_fact_ids: proposal
                .fact_ids
                .iter()
                .zip(&proposal.existing_fact_ids)
                .filter(|(_, existing)| existing.is_none())
                .map(|(id, _)| id.as_str().to_owned())
                .collect(),
            already_present_fact_ids: proposal
                .existing_fact_ids
                .iter()
                .flatten()
                .map(|id| id.as_str().to_owned())
                .collect(),
            accepted_verification_ids: proposal.verification_ids,
            accepted_correction_ids: proposal.correction_ids,
            accepted_reconciliation_ids: proposal.reconciliation_ids,
            accepted_record_hazard_ids: proposal.record_hazard_ids,
            accepted_record_hazard_resolution_ids: proposal.record_hazard_resolution_ids,
            accepted_discrepancy_resolution_ids: proposal.discrepancy_resolution_ids,
            newly_stale_analysis_ids,
        };
        transaction.execute(
            "INSERT INTO record_commits \
             (idempotency_key, proposal_id, expected_revision, record_revision, receipt_json) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                receipt.idempotency_key.as_str(),
                receipt.proposal_id.as_str(),
                i64::try_from(receipt.previous_revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?,
                i64::try_from(receipt.record_revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?,
                serde_json::to_string(&receipt)?
            ],
        )?;
        transaction.commit()?;
        Ok(receipt)
    }

    fn validate(&self, change_set: &RecordChangeSet) -> Result<()> {
        if change_set.schema_version != RECORD_SCHEMA_VERSION {
            return Err(Error::InvalidRecordChangeSet("unsupported schema version"));
        }
        let status = self.workspace.status()?;
        if change_set.workspace_id != status.workspace_id
            || change_set.subject_id != status.subject_id
        {
            return Err(Error::InvalidRecordChangeSet("identity mismatch"));
        }
        if change_set.expected_revision != status.record_revision {
            return Err(Error::RecordRevisionConflict);
        }
        validate_text(&change_set.extractor.name, "invalid extractor")?;
        validate_text(&change_set.extractor.version, "invalid extractor")?;
        if let Some(run) = &change_set.extraction_run {
            if change_set.evidence_origin.source_id() != Some(&run.source_id) {
                return Err(Error::InvalidRecordChangeSet("Source mismatch"));
            }
            self.require_available_source(&run.source_id)?;
            if run.coverage.regions.is_empty() || run.coverage.domains.is_empty() {
                return Err(Error::InvalidRecordChangeSet("empty extraction coverage"));
            }
            for region in &run.coverage.regions {
                validate_text(&region.locator, "invalid Source region")?;
            }
            for issue in run.omissions.iter().chain(&run.failures) {
                validate_text(&issue.code, "invalid extraction issue")?;
                if let Some(region) = &issue.region {
                    validate_text(&region.locator, "invalid Source region")?;
                }
            }
            for key in &run.resulting_candidate_keys {
                validate_text(key, "invalid candidate key")?;
            }
        }
        let mut correction_targets = HashSet::new();
        for change in &change_set.changes {
            match change {
                RecordChange::AddFact(addition) => self.validate_addition(change_set, addition)?,
                RecordChange::VerifyFact(verification) => {
                    self.validate_verification(change_set, verification)?;
                }
                RecordChange::CorrectFact(correction) => {
                    if !correction_targets.insert(correction.prior_fact_id.as_str()) {
                        return Err(Error::InvalidRecordChangeSet("duplicate Correction target"));
                    }
                    self.validate_correction(change_set, correction)?;
                }
                RecordChange::ReconcileFacts(reconciliation) => {
                    self.validate_reconciliation(change_set, reconciliation)?;
                }
                RecordChange::AddRecordHazard(hazard) => {
                    self.validate_record_hazard(change_set, hazard)?;
                }
                RecordChange::ResolveRecordHazard(resolution) => {
                    self.validate_record_hazard_resolution(change_set, resolution)?;
                }
                RecordChange::ResolveDiscrepancy(resolution) => {
                    self.validate_discrepancy_resolution(change_set, resolution)?;
                }
            }
        }
        Ok(())
    }

    fn validate_shape(change_set: &RecordChangeSet) -> Result<()> {
        if change_set.changes.len() > MAX_CHANGE_SET_CHANGES {
            return Err(Error::InvalidRecordChangeSet("too many Record Changes"));
        }
        if let Some(run) = &change_set.extraction_run
            && (run.coverage.regions.len() > MAX_EXTRACTION_VECTOR_ITEMS
                || run.coverage.domains.len() > MAX_EXTRACTION_VECTOR_ITEMS
                || run.omissions.len() > MAX_EXTRACTION_VECTOR_ITEMS
                || run.failures.len() > MAX_EXTRACTION_VECTOR_ITEMS
                || run.resulting_candidate_keys.len() > MAX_EXTRACTION_VECTOR_ITEMS)
        {
            return Err(Error::InvalidRecordChangeSet(
                "Extraction Run exceeds item limit",
            ));
        }
        Ok(())
    }

    fn resolve_existing_facts(
        &self,
        change_set: &RecordChangeSet,
        proposed_fact_ids: &[FactId],
    ) -> Result<Vec<Option<FactId>>> {
        let mut proposed_ids = proposed_fact_ids.iter();
        let mut resolved = Vec::with_capacity(proposed_fact_ids.len());
        let mut additions_in_proposal: Vec<(&FactDraft, &FactId)> = Vec::new();
        for change in &change_set.changes {
            match change {
                RecordChange::AddFact(draft) => {
                    let proposed_id = proposed_ids.next().ok_or(Error::ProposalIntegrity)?;
                    if let Some((_, existing_id)) = additions_in_proposal
                        .iter()
                        .find(|(candidate, _)| same_source_assertion(candidate, draft))
                    {
                        resolved.push(Some((*existing_id).clone()));
                        continue;
                    }
                    let existing_id = self.find_existing_fact(draft)?;
                    if existing_id.is_none() {
                        additions_in_proposal.push((draft, proposed_id));
                    }
                    resolved.push(existing_id);
                }
                RecordChange::CorrectFact(_) => {
                    proposed_ids.next().ok_or(Error::ProposalIntegrity)?;
                    resolved.push(None);
                }
                RecordChange::VerifyFact(_)
                | RecordChange::ReconcileFacts(_)
                | RecordChange::AddRecordHazard(_)
                | RecordChange::ResolveRecordHazard(_)
                | RecordChange::ResolveDiscrepancy(_) => {}
            }
        }
        if proposed_ids.next().is_some() {
            return Err(Error::ProposalIntegrity);
        }
        Ok(resolved)
    }

    fn find_existing_fact(&self, draft: &FactDraft) -> Result<Option<FactId>> {
        let mut statement = self
            .workspace
            .connection()
            .prepare("SELECT id, canonical_payload FROM facts ORDER BY record_revision, id")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, payload) in rows {
            let stored: FactDraft = serde_json::from_str(&payload)
                .map_err(|_| Error::InvalidWorkspace("invalid stored Health Fact".to_owned()))?;
            if same_source_assertion(&stored, draft) {
                return Ok(Some(FactId::from_stored(id)));
            }
        }
        Ok(None)
    }

    fn validate_addition(&self, change_set: &RecordChangeSet, addition: &FactDraft) -> Result<()> {
        validate_clinical_time(&addition.clinical_time)?;
        self.validate_addition_evidence(change_set, &addition.evidence)?;
        validate_text(&addition.evidence.author().identifier, "invalid author")?;
        match &addition.fact {
            FactData::LabResult(lab) => {
                validate_text(&lab.test_name, "invalid lab test name")?;
                if let Some(canonical_test) = &lab.canonical_test {
                    validate_text(
                        &canonical_test.identifier,
                        "invalid Canonical Test identifier",
                    )?;
                    validate_text(
                        &canonical_test.display_name,
                        "invalid Canonical Test display name",
                    )?;
                }
                match &lab.value {
                    LabValue::QualifiedNumeric { original, .. }
                    | LabValue::Text { value: original } => {
                        validate_text(original, "invalid lab value")?;
                    }
                    LabValue::Numeric { .. } => {}
                }
                if let Some(range) = &lab.reference_range {
                    validate_text(&range.original, "invalid reference range")?;
                    if range
                        .lower
                        .zip(range.upper)
                        .is_some_and(|(lower, upper)| lower > upper)
                    {
                        return Err(Error::InvalidRecordChangeSet("invalid reference range"));
                    }
                }
            }
            FactData::VitalMeasurement(vital) => validate_vital_measurement(vital)?,
            FactData::MedicationEvent(event) => validate_medication_event(event)?,
            FactData::ConditionAssertion(assertion) => {
                validate_condition_assertion(assertion)?;
            }
            FactData::DiagnosticStudy(study) => validate_diagnostic_study(study)?,
            FactData::CareTask(task) => validate_care_task(task)?,
            FactData::SubjectPreference(preference) => {
                validate_subject_preference(preference, &addition.evidence)?;
            }
        }
        Ok(())
    }

    fn validate_addition_evidence(
        &self,
        change_set: &RecordChangeSet,
        evidence: &FactEvidence,
    ) -> Result<()> {
        match (&change_set.evidence_origin, evidence) {
            (
                EvidenceOrigin::DocumentExtraction { source_id }
                | EvidenceOrigin::ParserValidatedExtraction { source_id },
                FactEvidence::Source {
                    source_id: fact_source_id,
                    source_region,
                    ..
                },
            ) => {
                let run =
                    change_set
                        .extraction_run
                        .as_ref()
                        .ok_or(Error::InvalidRecordChangeSet(
                            "source Fact requires Extraction Run",
                        ))?;
                if source_id != fact_source_id || source_id != &run.source_id {
                    return Err(Error::InvalidRecordChangeSet("Health Fact Source mismatch"));
                }
                validate_text(&source_region.locator, "invalid Source region")?;
            }
            (
                EvidenceOrigin::ParserValidatedExtraction { source_id },
                FactEvidence::ParsedSource {
                    parsed_source_id,
                    parsed_region,
                    cited_source_id,
                    cited_region,
                    ..
                },
            ) => {
                let run =
                    change_set
                        .extraction_run
                        .as_ref()
                        .ok_or(Error::InvalidRecordChangeSet(
                            "parsed Fact requires Extraction Run",
                        ))?;
                if source_id != parsed_source_id || source_id != &run.source_id {
                    return Err(Error::InvalidRecordChangeSet("Health Fact Source mismatch"));
                }
                validate_text(&parsed_region.locator, "invalid parsed Source region")?;
                validate_text(&cited_region.locator, "invalid cited Source region")?;
                self.require_available_source(cited_source_id)?;
            }
            (
                EvidenceOrigin::ExplicitSelfReport { reporter },
                FactEvidence::SelfReport {
                    reporter: fact_reporter,
                },
            ) if change_set.extraction_run.is_none() && reporter == fact_reporter => {}
            (
                EvidenceOrigin::ParsedSelfReport {
                    source_id,
                    reporter,
                },
                FactEvidence::SourcedSelfReport {
                    source_id: fact_source_id,
                    source_region,
                    reporter: fact_reporter,
                },
            ) => {
                let run =
                    change_set
                        .extraction_run
                        .as_ref()
                        .ok_or(Error::InvalidRecordChangeSet(
                            "parsed self-report requires Extraction Run",
                        ))?;
                if source_id != fact_source_id
                    || source_id != &run.source_id
                    || reporter != fact_reporter
                {
                    return Err(Error::InvalidRecordChangeSet("Health Fact Source mismatch"));
                }
                validate_text(&source_region.locator, "invalid Source region")?;
            }
            _ => {
                return Err(Error::InvalidRecordChangeSet(
                    "Health Fact evidence does not match origin",
                ));
            }
        }
        Ok(())
    }

    fn validate_verification(
        &self,
        change_set: &RecordChangeSet,
        verification: &VerificationDraft,
    ) -> Result<()> {
        if change_set.extraction_run.is_some()
            || change_set.evidence_origin.source_id() != Some(&verification.source_id)
            || !matches!(
                change_set.evidence_origin,
                EvidenceOrigin::SourceVerification { .. }
            )
        {
            return Err(Error::InvalidRecordChangeSet("invalid Verification origin"));
        }
        validate_text(&verification.source_region.locator, "invalid Source region")?;
        validate_text(&verification.verifier.identifier, "invalid verifier")?;
        validate_text(&verification.method.name, "invalid Verification method")?;
        validate_text(&verification.method.version, "invalid Verification method")?;
        validate_text(&verification.scope, "invalid Verification scope")?;
        let stored_evidence: Option<(Option<String>, Option<String>)> = self
            .workspace
            .connection()
            .query_row(
                "SELECT source_id, source_region_locator FROM facts WHERE id = ?1",
                [verification.fact_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((Some(stored_source), Some(stored_region))) = stored_evidence else {
            return Err(Error::FactNotFound("Verification target".to_owned()));
        };
        if stored_source != verification.source_id.as_str()
            || stored_region != verification.source_region.locator
        {
            return Err(Error::InvalidRecordChangeSet(
                "Verification evidence location mismatch",
            ));
        }
        self.require_available_source(&verification.source_id)?;
        Ok(())
    }

    fn validate_reconciliation(
        &self,
        change_set: &RecordChangeSet,
        reconciliation: &ReconciliationDraft,
    ) -> Result<()> {
        let EvidenceOrigin::RecordReview { reviewer } = &change_set.evidence_origin else {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Reconciliation origin",
            ));
        };
        if change_set.extraction_run.is_some() || reviewer != &reconciliation.author {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Reconciliation origin",
            ));
        }
        validate_text(
            &reconciliation.author.identifier,
            "invalid Reconciliation author",
        )?;
        validate_text(
            &reconciliation.rationale,
            "invalid Reconciliation rationale",
        )?;
        if reconciliation.fact_ids.len() < 2 {
            return Err(Error::InvalidRecordChangeSet(
                "Reconciliation requires at least two Facts",
            ));
        }
        let unique_ids = reconciliation
            .fact_ids
            .iter()
            .map(FactId::as_str)
            .collect::<HashSet<_>>();
        if unique_ids.len() != reconciliation.fact_ids.len() {
            return Err(Error::InvalidRecordChangeSet(
                "Reconciliation Facts must be distinct",
            ));
        }
        let history = self.query(FactQuery {
            kind: None,
            include_history: true,
        })?;
        let facts = reconciliation
            .fact_ids
            .iter()
            .map(|fact_id| {
                history
                    .iter()
                    .find(|fact| fact.id == *fact_id)
                    .ok_or_else(|| Error::FactNotFound("Reconciliation target".to_owned()))
            })
            .collect::<Result<Vec<_>>>()?;
        for fact in &facts {
            let is_current: Option<i64> = self
                .workspace
                .connection()
                .query_row(
                    "SELECT 1 FROM facts f WHERE f.id = ?1 AND NOT EXISTS \
                     (SELECT 1 FROM corrections c WHERE c.prior_fact_id = f.id)",
                    [fact.id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if is_current.is_none() {
                return Err(Error::InvalidRecordChangeSet(
                    "Reconciliation requires current Facts",
                ));
            }
            let already_reconciled: Option<i64> = self
                .workspace
                .connection()
                .query_row(
                    "SELECT 1 FROM reconciliation_fact_edges WHERE fact_id = ?1 LIMIT 1",
                    [fact.id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if already_reconciled.is_some() {
                return Err(Error::InvalidRecordChangeSet("Fact is already reconciled"));
            }
        }
        if reconciliation.agreement == ReconciliationAgreement::Equivalent {
            let source_ids = facts
                .iter()
                .filter_map(|fact| fact.draft.evidence.source().map(|(source_id, _)| source_id))
                .collect::<Vec<_>>();
            let distinct_sources = source_ids
                .iter()
                .map(|source_id| source_id.as_str())
                .collect::<HashSet<_>>();
            if source_ids.len() != facts.len()
                || distinct_sources.len() != facts.len()
                || facts
                    .iter()
                    .skip(1)
                    .any(|fact| !same_observation_assertion(&facts[0].draft, &fact.draft))
            {
                return Err(Error::InvalidRecordChangeSet(
                    "equivalent Reconciliation requires matching cross-Source assertions",
                ));
            }
        }
        Ok(())
    }

    fn validate_correction(
        &self,
        change_set: &RecordChangeSet,
        correction: &CorrectionDraft,
    ) -> Result<()> {
        let EvidenceOrigin::SourceCorrection {
            source_id,
            verification_id,
        } = &change_set.evidence_origin
        else {
            return Err(Error::InvalidRecordChangeSet("invalid Correction origin"));
        };
        let Some((replacement_source_id, replacement_region)) =
            correction.replacement.evidence.source()
        else {
            return Err(Error::InvalidRecordChangeSet("invalid Correction origin"));
        };
        if change_set.extraction_run.is_some()
            || source_id != replacement_source_id
            || verification_id != &correction.supporting_verification_id
        {
            return Err(Error::InvalidRecordChangeSet("invalid Correction origin"));
        }
        validate_text(&correction.reason, "invalid Correction reason")?;
        validate_text(&correction.author.identifier, "invalid Correction author")?;
        validate_text(&replacement_region.locator, "invalid Source region")?;
        validate_text(
            &correction.replacement.evidence.author().identifier,
            "invalid author",
        )?;
        match &correction.replacement.fact {
            FactData::LabResult(lab) => {
                validate_text(&lab.test_name, "invalid lab test name")?;
                if let LabValue::Text { value } = &lab.value {
                    validate_text(value, "invalid lab value")?;
                }
            }
            FactData::VitalMeasurement(vital) => validate_vital_measurement(vital)?,
            FactData::MedicationEvent(event) => validate_medication_event(event)?,
            FactData::ConditionAssertion(assertion) => {
                validate_condition_assertion(assertion)?;
            }
            FactData::DiagnosticStudy(study) => validate_diagnostic_study(study)?,
            FactData::CareTask(task) => validate_care_task(task)?,
            FactData::SubjectPreference(preference) => {
                validate_subject_preference(preference, &correction.replacement.evidence)?;
            }
        }
        let valid_support: Option<i64> = self
            .workspace
            .connection()
            .query_row(
                "SELECT 1 FROM verifications v JOIN facts f ON f.id = v.fact_id \
                 WHERE v.id = ?1 AND v.fact_id = ?2 AND v.source_id = ?3 \
                   AND v.outcome = 'verified' AND f.source_id = ?3 \
                   AND NOT EXISTS (SELECT 1 FROM corrections c WHERE c.prior_fact_id = f.id)",
                params![
                    correction.supporting_verification_id.as_str(),
                    correction.prior_fact_id.as_str(),
                    source_id.as_str()
                ],
                |row| row.get(0),
            )
            .optional()?;
        if valid_support.is_none() {
            return Err(Error::InvalidRecordChangeSet(
                "Correction lacks verified support",
            ));
        }
        self.require_available_source(source_id)?;
        Ok(())
    }

    fn validate_record_hazard(
        &self,
        change_set: &RecordChangeSet,
        hazard: &RecordHazardDraft,
    ) -> Result<()> {
        let EvidenceOrigin::RecordReview { reviewer } = &change_set.evidence_origin else {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Record Hazard origin",
            ));
        };
        if change_set.extraction_run.is_some() || reviewer != &hazard.author {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Record Hazard origin",
            ));
        }
        validate_text(&hazard.author.identifier, "invalid Record Hazard author")?;
        validate_text(
            &hazard.problematic_statement,
            "invalid problematic statement",
        )?;
        validate_text(&hazard.danger, "invalid Record Hazard danger")?;
        validate_text(
            &hazard.corrected_understanding,
            "invalid corrected understanding",
        )?;
        if hazard.affected_fact_ids.is_empty() || hazard.affected_source_ids.is_empty() {
            return Err(Error::InvalidRecordChangeSet(
                "Record Hazard requires affected Facts and Sources",
            ));
        }
        let fact_ids = hazard
            .affected_fact_ids
            .iter()
            .map(FactId::as_str)
            .collect::<HashSet<_>>();
        let source_ids = hazard
            .affected_source_ids
            .iter()
            .map(SourceId::as_str)
            .collect::<HashSet<_>>();
        if fact_ids.len() != hazard.affected_fact_ids.len()
            || source_ids.len() != hazard.affected_source_ids.len()
        {
            return Err(Error::InvalidRecordChangeSet(
                "Record Hazard references must be distinct",
            ));
        }
        let history = self.query(FactQuery {
            kind: None,
            include_history: true,
        })?;
        for fact_id in &hazard.affected_fact_ids {
            let fact = history
                .iter()
                .find(|fact| fact.id == *fact_id)
                .ok_or_else(|| Error::FactNotFound("Record Hazard target".to_owned()))?;
            let Some((source_id, _)) = fact.draft.evidence.source() else {
                return Err(Error::InvalidRecordChangeSet(
                    "Record Hazard Fact lacks affected Source",
                ));
            };
            if !source_ids.contains(source_id.as_str()) {
                return Err(Error::InvalidRecordChangeSet(
                    "Record Hazard Source mismatch",
                ));
            }
        }
        for source_id in &hazard.affected_source_ids {
            let exists: Option<i64> = self
                .workspace
                .connection()
                .query_row(
                    "SELECT 1 FROM sources WHERE id = ?1",
                    [source_id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(Error::SourceNotFound);
            }
        }
        Ok(())
    }

    fn validate_record_hazard_resolution(
        &self,
        change_set: &RecordChangeSet,
        resolution: &RecordHazardResolutionDraft,
    ) -> Result<()> {
        let EvidenceOrigin::RecordReview { reviewer } = &change_set.evidence_origin else {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Record Hazard Resolution origin",
            ));
        };
        if change_set.extraction_run.is_some() || reviewer != &resolution.author {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Record Hazard Resolution origin",
            ));
        }
        validate_text(
            &resolution.author.identifier,
            "invalid Record Hazard Resolution author",
        )?;
        validate_text(
            &resolution.rationale,
            "invalid Record Hazard Resolution rationale",
        )?;
        let support: Option<(i64, i64)> = self
            .workspace
            .connection()
            .query_row(
                "SELECT h.record_revision, v.record_revision \
                 FROM record_hazards h JOIN verifications v ON v.id = ?2 \
                 WHERE h.id = ?1 AND v.outcome = 'verified' \
                   AND EXISTS (SELECT 1 FROM record_hazard_fact_edges fact_edge \
                               WHERE fact_edge.hazard_id = h.id AND fact_edge.fact_id = v.fact_id) \
                   AND EXISTS (SELECT 1 FROM record_hazard_source_edges source_edge \
                               WHERE source_edge.hazard_id = h.id \
                                 AND source_edge.source_id = v.source_id) \
                   AND NOT EXISTS (SELECT 1 FROM record_hazard_resolutions existing \
                                   WHERE existing.hazard_id = h.id)",
                params![
                    resolution.hazard_id.as_str(),
                    resolution.supporting_verification_id.as_str()
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((hazard_revision, support_revision)) = support else {
            return Err(Error::InvalidRecordChangeSet(
                "Record Hazard Resolution lacks valid support",
            ));
        };
        if support_revision <= hazard_revision {
            return Err(Error::InvalidRecordChangeSet(
                "Record Hazard Resolution support predates Hazard",
            ));
        }
        Ok(())
    }

    fn validate_discrepancy_resolution(
        &self,
        change_set: &RecordChangeSet,
        resolution: &DiscrepancyResolutionDraft,
    ) -> Result<()> {
        let EvidenceOrigin::RecordReview { reviewer } = &change_set.evidence_origin else {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Discrepancy Resolution origin",
            ));
        };
        if change_set.extraction_run.is_some() || reviewer != &resolution.author {
            return Err(Error::InvalidRecordChangeSet(
                "invalid Discrepancy Resolution origin",
            ));
        }
        validate_text(
            &resolution.author.identifier,
            "invalid Discrepancy Resolution author",
        )?;
        validate_text(
            &resolution.rationale,
            "invalid Discrepancy Resolution rationale",
        )?;
        let support: Option<(String, String, i64, String, String, i64)> = self
            .workspace
            .connection()
            .query_row(
                "SELECT d.fact_id, d.source_id, d.record_revision, s.fact_id, s.source_id, \
                 s.record_revision FROM verifications d JOIN verifications s \
                 WHERE d.id = ?1 AND d.outcome = 'discrepancy' \
                   AND s.id = ?2 AND s.outcome = 'verified' \
                   AND NOT EXISTS (SELECT 1 FROM discrepancy_resolutions r \
                                   WHERE r.discrepancy_verification_id = d.id)",
                params![
                    resolution.discrepancy_verification_id.as_str(),
                    resolution.supporting_verification_id.as_str()
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            fact_id,
            source_id,
            discrepancy_revision,
            support_fact,
            support_source,
            support_revision,
        )) = support
        else {
            return Err(Error::InvalidRecordChangeSet(
                "Discrepancy Resolution lacks valid support",
            ));
        };
        if fact_id != support_fact
            || source_id != support_source
            || support_revision <= discrepancy_revision
        {
            return Err(Error::InvalidRecordChangeSet(
                "Discrepancy Resolution support mismatch",
            ));
        }
        Ok(())
    }

    fn require_available_source(&self, source_id: &SourceId) -> Result<()> {
        match self.workspace.sources().status(source_id)?.content_state {
            SourceContentState::Available => Ok(()),
            SourceContentState::Changed => Err(Error::SourceContentChanged),
            SourceContentState::Unavailable => Err(Error::SourceContentUnavailable),
        }
    }
}

fn medication_regimen_entry(
    identity: String,
    event: &MedicationEvent,
    source_fact_id: FactId,
) -> MedicationRegimenEntry {
    MedicationRegimenEntry {
        identity,
        product_kind: event.product_kind,
        name: event.name.clone(),
        strength: event.strength.clone(),
        dose: event.dose.clone(),
        route: event.route.clone(),
        schedule: event.schedule.clone(),
        indication: event.indication.clone(),
        source_fact_id,
    }
}

fn sort_facts_by_revision(facts: &mut [HealthFact]) {
    facts.sort_by(|left, right| {
        left.record_revision
            .value()
            .cmp(&right.record_revision.value())
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
}

fn migration_candidate_reason(code: &str) -> MigrationCandidateReason {
    match code {
        "cited_source_unresolved" => MigrationCandidateReason::CitedSourceUnresolved,
        "canonical_test_unmapped" => MigrationCandidateReason::CanonicalTestUnmapped,
        "invalid_source_row" => MigrationCandidateReason::InvalidSourceRow,
        "subject_confirmation_required" => MigrationCandidateReason::SubjectConfirmationRequired,
        _ => MigrationCandidateReason::Unaccounted,
    }
}

pub(crate) fn clinical_time_order_key(value: &ClinicalTime) -> (i64, u8) {
    match value {
        ClinicalTime::Undated { .. } => (i64::MIN, 0),
        ClinicalTime::Instant { value, .. } => (value.timestamp_millis(), 3),
        ClinicalTime::Date { value, .. } => (date_order_key(*value), 2),
        ClinicalTime::PartialDate { value, .. } => partial_date_order_key(value),
        ClinicalTime::Interval { start, .. } => clinical_point_order_key(start),
    }
}

fn clinical_point_order_key(value: &ClinicalTimePoint) -> (i64, u8) {
    match value {
        ClinicalTimePoint::Instant { value } => (value.timestamp_millis(), 3),
        ClinicalTimePoint::Date { value } => (date_order_key(*value), 2),
        ClinicalTimePoint::PartialDate { value } => partial_date_order_key(value),
    }
}

fn partial_date_order_key(value: &PartialDate) -> (i64, u8) {
    match value {
        PartialDate::Year { year } => (
            NaiveDate::from_ymd_opt(*year, 1, 1).map_or(i64::MIN, date_order_key),
            0,
        ),
        PartialDate::Month { year, month } => (
            NaiveDate::from_ymd_opt(*year, u32::from(*month), 1).map_or(i64::MIN, date_order_key),
            1,
        ),
    }
}

fn date_order_key(value: NaiveDate) -> i64 {
    value
        .and_hms_opt(0, 0, 0)
        .map_or(i64::MIN, |value| value.and_utc().timestamp_millis())
}

struct ProposedIds {
    extraction_run_id: Option<ExtractionRunId>,
    fact_ids: Vec<FactId>,
    verification_ids: Vec<VerificationId>,
    correction_ids: Vec<CorrectionId>,
    reconciliation_ids: Vec<ReconciliationId>,
    record_hazard_ids: Vec<RecordHazardId>,
    record_hazard_resolution_ids: Vec<RecordHazardResolutionId>,
    discrepancy_resolution_ids: Vec<DiscrepancyResolutionId>,
}

fn derive_proposed_ids(proposal_hash: &[u8], change_set: &RecordChangeSet) -> ProposedIds {
    let extraction_run_id = change_set.extraction_run.as_ref().map(|_| {
        ExtractionRunId::from_content_hash(derived_id(
            "extraction-run",
            proposal_hash,
            b"extraction-run",
            0,
        ))
    });
    let fact_count = count_changes(change_set, |change| {
        matches!(
            change,
            RecordChange::AddFact(_) | RecordChange::CorrectFact(_)
        )
    });
    let verification_count = count_changes(change_set, |change| {
        matches!(change, RecordChange::VerifyFact(_))
    });
    let correction_count = count_changes(change_set, |change| {
        matches!(change, RecordChange::CorrectFact(_))
    });
    let reconciliation_count = count_changes(change_set, |change| {
        matches!(change, RecordChange::ReconcileFacts(_))
    });
    let record_hazard_count = count_changes(change_set, |change| {
        matches!(change, RecordChange::AddRecordHazard(_))
    });
    let record_hazard_resolution_count = count_changes(change_set, |change| {
        matches!(change, RecordChange::ResolveRecordHazard(_))
    });
    let discrepancy_resolution_count = count_changes(change_set, |change| {
        matches!(change, RecordChange::ResolveDiscrepancy(_))
    });
    ProposedIds {
        extraction_run_id,
        fact_ids: (0..fact_count)
            .map(|index| {
                FactId::from_content_hash(derived_id("fact", proposal_hash, b"fact", index))
            })
            .collect(),
        verification_ids: (0..verification_count)
            .map(|index| {
                VerificationId::from_content_hash(derived_id(
                    "verification",
                    proposal_hash,
                    b"verification",
                    index,
                ))
            })
            .collect(),
        correction_ids: (0..correction_count)
            .map(|index| {
                CorrectionId::from_content_hash(derived_id(
                    "correction",
                    proposal_hash,
                    b"correction",
                    index,
                ))
            })
            .collect(),
        reconciliation_ids: (0..reconciliation_count)
            .map(|index| {
                ReconciliationId::from_content_hash(derived_id(
                    "reconciliation",
                    proposal_hash,
                    b"reconciliation",
                    index,
                ))
            })
            .collect(),
        record_hazard_ids: (0..record_hazard_count)
            .map(|index| {
                RecordHazardId::from_content_hash(derived_id(
                    "record-hazard",
                    proposal_hash,
                    b"record-hazard",
                    index,
                ))
            })
            .collect(),
        record_hazard_resolution_ids: (0..record_hazard_resolution_count)
            .map(|index| {
                RecordHazardResolutionId::from_content_hash(derived_id(
                    "record-hazard-resolution",
                    proposal_hash,
                    b"record-hazard-resolution",
                    index,
                ))
            })
            .collect(),
        discrepancy_resolution_ids: (0..discrepancy_resolution_count)
            .map(|index| {
                DiscrepancyResolutionId::from_content_hash(derived_id(
                    "discrepancy-resolution",
                    proposal_hash,
                    b"discrepancy-resolution",
                    index,
                ))
            })
            .collect(),
    }
}

fn count_changes(change_set: &RecordChangeSet, predicate: impl Fn(&RecordChange) -> bool) -> usize {
    change_set
        .changes
        .iter()
        .filter(|change| predicate(change))
        .count()
}

fn derived_id(prefix: &str, proposal_hash: &[u8], kind: &[u8], index: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(proposal_hash);
    hasher.update(b":");
    hasher.update(kind);
    hasher.update(b":");
    hasher.update(index.to_string().as_bytes());
    format!("{prefix}-{}", lowercase_hex(&hasher.finalize()))
}

fn validate_text(value: &str, message: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(Error::InvalidRecordChangeSet(message));
    }
    Ok(())
}

fn validate_vital_measurement(value: &VitalMeasurement) -> Result<()> {
    let (measurements, originals, units, measured_by, note) = match value {
        VitalMeasurement::BloodPressure {
            systolic,
            diastolic,
            original_systolic,
            original_diastolic,
            units,
            measured_by,
            note,
        } => (
            vec![*systolic, *diastolic],
            vec![original_systolic, original_diastolic],
            units,
            measured_by,
            note,
        ),
        VitalMeasurement::Pulse {
            value,
            original,
            units,
            measured_by,
            note,
        } => (vec![*value], vec![original], units, measured_by, note),
        VitalMeasurement::Weight {
            value,
            original,
            units,
            note,
        } => (vec![*value], vec![original], units, &None, note),
    };
    if measurements
        .iter()
        .any(|measurement| *measurement <= Decimal::ZERO)
    {
        return Err(Error::InvalidRecordChangeSet("invalid vital measurement"));
    }
    for original in originals {
        validate_text(original, "invalid original vital value")?;
    }
    for text in [units, measured_by, note].into_iter().flatten() {
        validate_text(text, "invalid vital context")?;
    }
    Ok(())
}

fn validate_medication_event(value: &MedicationEvent) -> Result<()> {
    validate_text(&value.name, "invalid medication name")?;
    if let Some(identity) = &value.normalized_identity {
        validate_text(identity, "invalid medication identity")?;
    }
    if let Some(strength) = &value.strength {
        validate_text(strength, "invalid medication strength")?;
    }
    if let Some(dose) = &value.dose {
        if dose.value <= Decimal::ZERO {
            return Err(Error::InvalidRecordChangeSet("invalid medication dose"));
        }
        validate_text(&dose.unit, "invalid medication dose unit")?;
        validate_text(&dose.original, "invalid original medication dose")?;
    }
    if value.event == MedicationEventKind::DoseChanged && value.dose.is_none() {
        return Err(Error::InvalidRecordChangeSet(
            "dose change requires medication dose",
        ));
    }
    if value.event == MedicationEventKind::ScheduleChanged && value.schedule.is_none() {
        return Err(Error::InvalidRecordChangeSet(
            "schedule change requires medication schedule",
        ));
    }
    for text in [
        &value.route,
        &value.schedule,
        &value.indication,
        &value.adherence_context,
    ]
    .into_iter()
    .flatten()
    {
        validate_text(text, "invalid medication context")?;
    }
    Ok(())
}

fn validate_condition_assertion(value: &ConditionAssertion) -> Result<()> {
    validate_text(&value.name, "invalid condition name")?;
    validate_text(&value.assertion_text, "invalid condition assertion text")?;
    for text in [&value.normalized_identity, &value.body_site]
        .into_iter()
        .flatten()
    {
        validate_text(text, "invalid condition context")?;
    }
    Ok(())
}

fn validate_diagnostic_study(value: &DiagnosticStudy) -> Result<()> {
    validate_text(&value.name, "invalid Diagnostic Study name")?;
    for text in [
        &value.body_site,
        &value.method,
        &value.ordering_provider,
        &value.interpreting_provider,
        &value.performing_organization,
    ]
    .into_iter()
    .flatten()
    {
        validate_text(text, "invalid Diagnostic Study context")?;
    }
    if value.findings.len() > MAX_EXTRACTION_VECTOR_ITEMS {
        return Err(Error::InvalidRecordChangeSet(
            "too many Diagnostic Study findings",
        ));
    }
    for finding in &value.findings {
        validate_narrative(&finding.text, "invalid Diagnostic Study finding")?;
        for text in [&finding.section, &finding.body_site].into_iter().flatten() {
            validate_text(text, "invalid Diagnostic Study finding context")?;
        }
    }
    if let Some(impression) = &value.impression {
        validate_narrative(impression, "invalid Diagnostic Study impression")?;
    }
    if let Some(resulted_at) = &value.resulted_at {
        validate_clinical_point(resulted_at)?;
    }
    let has_result = !value.findings.is_empty() || value.impression.is_some();
    match value.status {
        DiagnosticStudyStatus::Ordered
        | DiagnosticStudyStatus::Scheduled
        | DiagnosticStudyStatus::Performed
        | DiagnosticStudyStatus::Cancelled
            if has_result || value.resulted_at.is_some() =>
        {
            Err(Error::InvalidRecordChangeSet(
                "Diagnostic Study state cannot carry results",
            ))
        }
        DiagnosticStudyStatus::Preliminary
        | DiagnosticStudyStatus::Final
        | DiagnosticStudyStatus::Amended
            if !has_result =>
        {
            Err(Error::InvalidRecordChangeSet(
                "resulted Diagnostic Study requires result",
            ))
        }
        _ => Ok(()),
    }
}

fn validate_narrative(value: &str, message: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err(Error::InvalidRecordChangeSet(message));
    }
    Ok(())
}

fn validate_care_task(value: &CareTask) -> Result<()> {
    validate_text(&value.task_key, "invalid Care Task key")?;
    validate_narrative(&value.action, "invalid Care Task action")?;
    validate_narrative(&value.source_text, "invalid Care Task source text")?;
    if let Some(requested_by) = &value.requested_by {
        validate_text(requested_by, "invalid Care Task requester")?;
    }
    match &value.due {
        Some(CareTaskDue::On { time }) => validate_clinical_point(time),
        Some(CareTaskDue::Interval {
            value, source_text, ..
        }) => {
            if *value == 0 {
                return Err(Error::InvalidRecordChangeSet(
                    "Care Task interval must be positive",
                ));
            }
            validate_text(source_text, "invalid Care Task interval")
        }
        None => Ok(()),
    }
}

fn validate_subject_preference(value: &SubjectPreference, evidence: &FactEvidence) -> Result<()> {
    let subject_attributed = matches!(
        evidence,
        FactEvidence::SelfReport {
            reporter: AuthorIdentity {
                kind: AuthorKind::Subject,
                ..
            }
        } | FactEvidence::SourcedSelfReport {
            reporter: AuthorIdentity {
                kind: AuthorKind::Subject,
                ..
            },
            ..
        }
    );
    if !subject_attributed {
        return Err(Error::InvalidRecordChangeSet(
            "Subject Preference requires subject attribution",
        ));
    }
    validate_text(&value.preference_key, "invalid Subject Preference key")?;
    validate_narrative(&value.statement, "invalid Subject Preference statement")?;
    validate_narrative(&value.source_text, "invalid Subject Preference source text")
}

struct ParsedLabTimeline {
    changes: Vec<RecordChange>,
    regions: Vec<SourceRegion>,
    candidate_keys: Vec<String>,
    failures: Vec<ExtractionIssue>,
}

struct ParsedVitalTimeline {
    changes: Vec<RecordChange>,
    regions: Vec<SourceRegion>,
    candidate_keys: Vec<String>,
}

struct ParsedSubjectPreferenceCandidates {
    regions: Vec<SourceRegion>,
    candidate_keys: Vec<String>,
    failures: Vec<ExtractionIssue>,
}

fn parse_subject_preference_candidates(bytes: &[u8]) -> Result<ParsedSubjectPreferenceCandidates> {
    let document = std::str::from_utf8(bytes)
        .map_err(|_| Error::InvalidRecordChangeSet("invalid preference migration source"))?;
    let lines = document.lines().collect::<Vec<_>>();
    let mut regions = Vec::new();
    let mut candidate_columns = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            candidate_columns.clear();
            continue;
        }
        if lines
            .get(index + 1)
            .is_some_and(|next| markdown_table_separator(next.trim()))
        {
            candidate_columns = subject_preference_candidate_columns(trimmed);
            continue;
        }
        if markdown_table_separator(trimmed) || candidate_columns.is_empty() {
            continue;
        }
        let cells = markdown_table_cells(trimmed);
        let candidate = candidate_columns.iter().any(|column| {
            cells
                .get(*column)
                .is_some_and(|cell| tracking_preference_candidate(cell, trimmed))
        });
        if !candidate {
            continue;
        }
        regions.push(SourceRegion {
            locator: format!("line:{}", index + 1),
        });
    }
    if regions.len() > MAX_EXTRACTION_VECTOR_ITEMS {
        return Err(Error::InvalidRecordChangeSet(
            "too many Subject Preference candidates",
        ));
    }
    let candidate_keys = regions
        .iter()
        .map(|region| region.locator.clone())
        .collect::<Vec<_>>();
    let failures = regions
        .iter()
        .cloned()
        .map(|region| ExtractionIssue {
            code: "subject_confirmation_required".to_owned(),
            region: Some(region),
        })
        .collect();
    Ok(ParsedSubjectPreferenceCandidates {
        regions,
        candidate_keys,
        failures,
    })
}

fn markdown_table_cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_lowercase())
        .collect()
}

fn markdown_table_separator(line: &str) -> bool {
    let cells = markdown_table_cells(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let value = cell.trim_matches(':');
            value.len() >= 3 && value.bytes().all(|byte| byte == b'-')
        })
}

fn subject_preference_candidate_columns(header: &str) -> Vec<usize> {
    let cells = markdown_table_cells(header);
    let is_item_table = cells
        .iter()
        .any(|cell| cell.contains("item") || cell.contains("topic"));
    if !is_item_table {
        return Vec::new();
    }
    cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            ["description", "detail", "notes", "choice"]
                .iter()
                .any(|name| cell.contains(name))
                .then_some(index)
        })
        .collect()
}

fn tracking_preference_candidate(cell: &str, row: &str) -> bool {
    let tracks_something = cell.contains("track") || monitoring_desire(cell);
    let lower_row = row.to_lowercase();
    let excluded = [
        "recommend",
        "ordered",
        "clinician",
        "doctor",
        "provider",
        "should",
        "must",
        "due",
        "repeat test",
        "follow-up",
        "follow up",
        "medication",
        "supplement",
    ]
    .iter()
    .any(|marker| lower_row.contains(marker));
    tracks_something && !excluded
}

fn monitoring_desire(cell: &str) -> bool {
    let words = cell
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let monitoring = [
        "monitoring",
        "screening",
        "checking",
        "tracking",
        "measuring",
        "logging",
        "recording",
        "watching",
        "testing",
    ];
    let desire = [
        "desire",
        "desired",
        "want",
        "wanted",
        "wants",
        "request",
        "requested",
        "requests",
        "prefer",
        "preferred",
        "prefers",
        "choose",
        "chosen",
        "chooses",
    ];
    words.iter().enumerate().any(|(index, word)| {
        monitoring.contains(word)
            && words[index + 1..]
                .iter()
                .any(|later| desire.contains(later))
    })
}

fn parse_vital_timeline(
    request: &VitalTimelineImportRequest,
    bytes: &[u8],
) -> Result<ParsedVitalTimeline> {
    let expected_headers: &[&str] = match request.format {
        VitalTimelineFormat::BloodPressurePulse => &[
            "date",
            "systolic",
            "diastolic",
            "pulse",
            "measured_by",
            "note",
        ],
        VitalTimelineFormat::Weight => &["date", "weight_lbs", "note"],
    };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|_| Error::InvalidRecordChangeSet("invalid vital timeline CSV"))?;
    if !headers.iter().eq(expected_headers.iter().copied()) {
        return Err(Error::InvalidRecordChangeSet(
            "invalid vital timeline headers",
        ));
    }
    let mut parsed = ParsedVitalTimeline {
        changes: Vec::new(),
        regions: Vec::new(),
        candidate_keys: Vec::new(),
    };
    for (index, row) in reader.records().enumerate() {
        if index >= MAX_EXTRACTION_VECTOR_ITEMS {
            return Err(Error::InvalidRecordChangeSet(
                "vital timeline exceeds row limit",
            ));
        }
        let row = row.map_err(|_| Error::InvalidRecordChangeSet("invalid vital timeline CSV"))?;
        if row.len() != expected_headers.len() {
            return Err(Error::InvalidRecordChangeSet("invalid vital timeline row"));
        }
        let date = NaiveDate::parse_from_str(&row[0], "%Y-%m-%d")
            .map_err(|_| Error::InvalidRecordChangeSet("invalid vital timeline date"))?;
        let row_number = index
            .checked_add(2)
            .ok_or(Error::InvalidRecordChangeSet("vital timeline row overflow"))?;
        let region = SourceRegion {
            locator: format!("row:{row_number}"),
        };
        parsed.candidate_keys.push(region.locator.clone());
        parsed.regions.push(region.clone());
        let measurements = parse_vital_measurements(request.format, &row)?;
        for measurement in measurements {
            parsed.changes.push(RecordChange::AddFact(FactDraft {
                clinical_time: ClinicalTime::Date {
                    value: date,
                    source_text: None,
                },
                evidence: FactEvidence::SourcedSelfReport {
                    source_id: request.source_id.clone(),
                    source_region: region.clone(),
                    reporter: request.reporter.clone(),
                },
                fact: FactData::VitalMeasurement(measurement),
            }));
        }
    }
    Ok(parsed)
}

fn parse_vital_measurements(
    format: VitalTimelineFormat,
    row: &csv::StringRecord,
) -> Result<Vec<VitalMeasurement>> {
    let decimal = |value: &str| {
        Decimal::from_str(value)
            .map_err(|_| Error::InvalidRecordChangeSet("invalid vital measurement"))
    };
    match format {
        VitalTimelineFormat::BloodPressurePulse => {
            let measured_by = optional_source_text(&row[4]);
            let note = optional_source_text(&row[5]);
            let mut measurements = vec![VitalMeasurement::BloodPressure {
                systolic: decimal(&row[1])?,
                diastolic: decimal(&row[2])?,
                original_systolic: row[1].to_owned(),
                original_diastolic: row[2].to_owned(),
                units: None,
                measured_by: measured_by.clone(),
                note: note.clone(),
            }];
            if !row[3].is_empty() {
                measurements.push(VitalMeasurement::Pulse {
                    value: decimal(&row[3])?,
                    original: row[3].to_owned(),
                    units: None,
                    measured_by,
                    note,
                });
            }
            Ok(measurements)
        }
        VitalTimelineFormat::Weight => Ok(vec![VitalMeasurement::Weight {
            value: decimal(&row[1])?,
            original: row[1].to_owned(),
            units: Some("lb".to_owned()),
            note: optional_source_text(&row[2]),
        }]),
    }
}

fn parse_lab_timeline(
    workspace: &Workspace,
    request: &LabTimelineImportRequest,
    bytes: &[u8],
) -> Result<ParsedLabTimeline> {
    let mappings = request
        .test_mappings
        .iter()
        .map(|mapping| (mapping.reported_name.as_str(), &mapping.canonical_test))
        .collect::<BTreeMap<_, _>>();
    if mappings.len() != request.test_mappings.len() {
        return Err(Error::InvalidRecordChangeSet("duplicate lab Test mapping"));
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|_| Error::InvalidRecordChangeSet("invalid lab timeline CSV"))?;
    if !headers.iter().eq(LAB_TIMELINE_HEADERS) {
        return Err(Error::InvalidRecordChangeSet(
            "invalid lab timeline headers",
        ));
    }
    let mut parsed = ParsedLabTimeline {
        changes: Vec::new(),
        regions: Vec::new(),
        candidate_keys: Vec::new(),
        failures: Vec::new(),
    };
    for (index, row) in reader.records().enumerate() {
        if index >= MAX_EXTRACTION_VECTOR_ITEMS {
            return Err(Error::InvalidRecordChangeSet(
                "lab timeline exceeds row limit",
            ));
        }
        let row = row.map_err(|_| Error::InvalidRecordChangeSet("invalid lab timeline CSV"))?;
        if row.len() != LAB_TIMELINE_HEADERS.len() {
            return Err(Error::InvalidRecordChangeSet("invalid lab timeline row"));
        }
        let row_number = index
            .checked_add(2)
            .ok_or(Error::InvalidRecordChangeSet("lab timeline row overflow"))?;
        let region = SourceRegion {
            locator: format!("row:{row_number}"),
        };
        parsed.candidate_keys.push(region.locator.clone());
        parsed.regions.push(region.clone());
        let Some(canonical_test) = mappings.get(&row[1]) else {
            return Err(Error::InvalidRecordChangeSet("unmapped lab Test"));
        };
        let cited_source_id = match workspace.sources().find_by_alias(&row[6]) {
            Ok(Some(source_id)) => source_id,
            Ok(None) | Err(Error::InvalidSourceAlias) => {
                parsed.failures.push(ExtractionIssue {
                    code: "cited_source_unresolved".to_owned(),
                    region: Some(region),
                });
                continue;
            }
            Err(error) => return Err(error),
        };
        parsed.changes.push(RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::parse_from_str(&row[0], "%Y-%m-%d")
                    .map_err(|_| Error::InvalidRecordChangeSet("invalid lab timeline date"))?,
                source_text: None,
            },
            evidence: FactEvidence::ParsedSource {
                parsed_source_id: request.source_id.clone(),
                parsed_region: region,
                cited_source_id,
                cited_region: SourceRegion {
                    locator: "document".to_owned(),
                },
                asserted_by: request.asserted_by.clone(),
            },
            fact: FactData::LabResult(LabResult {
                test_name: row[1].to_owned(),
                canonical_test: Some((*canonical_test).clone()),
                value: parse_lab_timeline_value(&row[2])?,
                units: optional_source_text(&row[3]),
                reference_range: parse_lab_timeline_range(&row[4]),
                reported_flag: optional_source_text(&row[5]),
                performing_lab: None,
            }),
        }));
    }
    Ok(parsed)
}

fn optional_source_text(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_lab_timeline_value(original: &str) -> Result<LabValue> {
    if original.is_empty() {
        return Err(Error::InvalidRecordChangeSet("invalid lab timeline value"));
    }
    for (prefix, comparator) in [
        ("<=", LabComparator::LessThanOrEqual),
        (">=", LabComparator::GreaterThanOrEqual),
        ("<", LabComparator::LessThan),
        (">", LabComparator::GreaterThan),
    ] {
        if let Some(value) = original.strip_prefix(prefix) {
            return Ok(LabValue::QualifiedNumeric {
                comparator,
                value: Decimal::from_str(value)
                    .map_err(|_| Error::InvalidRecordChangeSet("invalid qualified lab value"))?,
                original: original.to_owned(),
            });
        }
    }
    match Decimal::from_str(original) {
        Ok(value) => Ok(LabValue::Numeric { value }),
        Err(_) => Ok(LabValue::Text {
            value: original.to_owned(),
        }),
    }
}

fn parse_lab_timeline_range(original: &str) -> Option<ReferenceRange> {
    if original.is_empty() {
        return None;
    }
    let (lower, upper) = if let Some(value) = original
        .strip_prefix("<=")
        .or_else(|| original.strip_prefix('<'))
    {
        (None, Decimal::from_str(value).map(Some).unwrap_or(None))
    } else if let Some(value) = original
        .strip_prefix(">=")
        .or_else(|| original.strip_prefix('>'))
    {
        (Decimal::from_str(value).map(Some).unwrap_or(None), None)
    } else if let Some((lower, upper)) = original.split_once(" - ") {
        (Decimal::from_str(lower).ok(), Decimal::from_str(upper).ok())
    } else {
        (None, None)
    };
    Some(ReferenceRange {
        original: original.to_owned(),
        lower,
        upper,
    })
}

fn validate_clinical_time(clinical_time: &ClinicalTime) -> Result<()> {
    let source_text = match clinical_time {
        ClinicalTime::Undated { source_text }
        | ClinicalTime::Instant { source_text, .. }
        | ClinicalTime::Date { source_text, .. }
        | ClinicalTime::PartialDate { source_text, .. }
        | ClinicalTime::Interval { source_text, .. } => source_text,
    };
    if let Some(source_text) = source_text {
        validate_text(source_text, "invalid Clinical Time source text")?;
    }
    if let ClinicalTime::Interval { start, end, .. } = clinical_time {
        let reversed = match (start, end) {
            (
                ClinicalTimePoint::Instant { value: start },
                ClinicalTimePoint::Instant { value: end },
            ) => start > end,
            (ClinicalTimePoint::Date { value: start }, ClinicalTimePoint::Date { value: end }) => {
                start > end
            }
            (
                ClinicalTimePoint::PartialDate { value: start },
                ClinicalTimePoint::PartialDate { value: end },
            ) => partial_date_sort_key(start)? > partial_date_sort_key(end)?,
            _ => false,
        };
        if reversed {
            return Err(Error::InvalidRecordChangeSet(
                "Clinical Time interval is reversed",
            ));
        }
    }
    match clinical_time {
        ClinicalTime::PartialDate { value, .. } => {
            partial_date_sort_key(value)?;
        }
        ClinicalTime::Interval { start, end, .. } => {
            validate_clinical_point(start)?;
            validate_clinical_point(end)?;
        }
        ClinicalTime::Undated { .. } | ClinicalTime::Instant { .. } | ClinicalTime::Date { .. } => {
        }
    }
    Ok(())
}

fn validate_clinical_point(point: &ClinicalTimePoint) -> Result<()> {
    if let ClinicalTimePoint::PartialDate { value } = point {
        partial_date_sort_key(value)?;
    }
    Ok(())
}

fn partial_date_sort_key(value: &PartialDate) -> Result<(i32, u8)> {
    match value {
        PartialDate::Year { year } if (1..=9999).contains(year) => Ok((*year, 0)),
        PartialDate::Month { year, month }
            if (1..=9999).contains(year) && (1..=12).contains(month) =>
        {
            Ok((*year, *month))
        }
        PartialDate::Year { .. } | PartialDate::Month { .. } => Err(Error::InvalidRecordChangeSet(
            "Clinical Time partial date is invalid",
        )),
    }
}

fn clinical_point_parts(point: &ClinicalTimePoint) -> (&'static str, String) {
    match point {
        ClinicalTimePoint::Instant { value } => (
            "instant",
            value.to_rfc3339_opts(SecondsFormat::AutoSi, false),
        ),
        ClinicalTimePoint::Date { value } => ("date", value.format("%Y-%m-%d").to_string()),
        ClinicalTimePoint::PartialDate { value } => partial_date_parts(value),
    }
}

fn partial_date_parts(value: &PartialDate) -> (&'static str, String) {
    match value {
        PartialDate::Year { year } => ("partial_year", format!("{year:04}")),
        PartialDate::Month { year, month } => ("partial_month", format!("{year:04}-{month:02}")),
    }
}

struct StoredClinicalTime<'a> {
    time_kind: &'static str,
    start_kind: &'static str,
    start_value: String,
    end_kind: Option<&'static str>,
    end_value: Option<String>,
    source_text: Option<&'a str>,
}

fn stored_clinical_time(value: &ClinicalTime) -> StoredClinicalTime<'_> {
    match value {
        ClinicalTime::Undated { source_text } => StoredClinicalTime {
            time_kind: "undated",
            start_kind: "undated",
            start_value: String::new(),
            end_kind: None,
            end_value: None,
            source_text: source_text.as_deref(),
        },
        ClinicalTime::Instant { value, source_text } => StoredClinicalTime {
            time_kind: "instant",
            start_kind: "instant",
            start_value: value.to_rfc3339_opts(SecondsFormat::AutoSi, false),
            end_kind: None,
            end_value: None,
            source_text: source_text.as_deref(),
        },
        ClinicalTime::Date { value, source_text } => StoredClinicalTime {
            time_kind: "date",
            start_kind: "date",
            start_value: value.format("%Y-%m-%d").to_string(),
            end_kind: None,
            end_value: None,
            source_text: source_text.as_deref(),
        },
        ClinicalTime::PartialDate { value, source_text } => {
            let (start_kind, start_value) = partial_date_parts(value);
            StoredClinicalTime {
                time_kind: "partial_date",
                start_kind,
                start_value,
                end_kind: None,
                end_value: None,
                source_text: source_text.as_deref(),
            }
        }
        ClinicalTime::Interval {
            start,
            end,
            source_text,
        } => {
            let (start_kind, start_value) = clinical_point_parts(start);
            let (end_kind, end_value) = clinical_point_parts(end);
            StoredClinicalTime {
                time_kind: "interval",
                start_kind,
                start_value,
                end_kind: Some(end_kind),
                end_value: Some(end_value),
                source_text: source_text.as_deref(),
            }
        }
    }
}

fn same_source_assertion(left: &FactDraft, right: &FactDraft) -> bool {
    let (Some((left_source, left_region)), Some((right_source, right_region))) =
        (left.evidence.source(), right.evidence.source())
    else {
        return false;
    };
    left_source == right_source
        && left_region == right_region
        && same_observation_assertion(left, right)
}

fn same_observation_assertion(left: &FactDraft, right: &FactDraft) -> bool {
    left.clinical_time == right.clinical_time && left.fact == right.fact
}

fn addition_assurance(origin: &EvidenceOrigin) -> Result<EvidenceAssurance> {
    match origin {
        EvidenceOrigin::DocumentExtraction { .. } => Ok(EvidenceAssurance::Unverified),
        EvidenceOrigin::ParserValidatedExtraction { .. } => Ok(EvidenceAssurance::ParserValidated),
        EvidenceOrigin::ExplicitSelfReport { .. } | EvidenceOrigin::ParsedSelfReport { .. } => {
            Ok(EvidenceAssurance::ExplicitSelfReport)
        }
        EvidenceOrigin::SourceVerification { .. }
        | EvidenceOrigin::SourceCorrection { .. }
        | EvidenceOrigin::RecordReview { .. } => Err(Error::ProposalIntegrity),
    }
}

fn current_revision(transaction: &Transaction<'_>) -> Result<RecordRevision> {
    let stored: i64 = transaction.query_row(
        "SELECT record_revision FROM workspace WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    let value = u64::try_from(stored)
        .map_err(|_| Error::InvalidWorkspace("invalid record revision".to_owned()))?;
    Ok(RecordRevision::from_stored(value))
}

fn insert_extraction_run(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let Some(run) = &proposal.change_set.extraction_run else {
        return Ok(());
    };
    let run_id = proposal
        .extraction_run_id
        .as_ref()
        .ok_or(Error::ProposalIntegrity)?;
    transaction.execute(
        "INSERT INTO extraction_runs \
         (id, source_id, extractor_name, extractor_version, record_revision) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            run_id.as_str(),
            run.source_id.as_str(),
            proposal.change_set.extractor.name,
            proposal.change_set.extractor.version,
            i64::try_from(revision.value())
                .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
        ],
    )?;
    for (position, region) in run.coverage.regions.iter().enumerate() {
        transaction.execute(
            "INSERT INTO extraction_run_regions (extraction_run_id, position, locator) \
             VALUES (?1, ?2, ?3)",
            params![
                run_id.as_str(),
                i64::try_from(position)
                    .map_err(|_| Error::InvalidRecordChangeSet("too many regions"))?,
                region.locator
            ],
        )?;
    }
    for (position, domain) in run.coverage.domains.iter().enumerate() {
        transaction.execute(
            "INSERT INTO extraction_run_domains (extraction_run_id, position, domain) \
             VALUES (?1, ?2, ?3)",
            params![
                run_id.as_str(),
                i64::try_from(position)
                    .map_err(|_| Error::InvalidRecordChangeSet("too many domains"))?,
                domain.as_str()
            ],
        )?;
    }
    insert_issues(transaction, run_id, "omission", &run.omissions)?;
    insert_issues(transaction, run_id, "failure", &run.failures)?;
    for (position, key) in run.resulting_candidate_keys.iter().enumerate() {
        transaction.execute(
            "INSERT INTO extraction_run_candidates (extraction_run_id, position, candidate_key) \
             VALUES (?1, ?2, ?3)",
            params![
                run_id.as_str(),
                i64::try_from(position)
                    .map_err(|_| Error::InvalidRecordChangeSet("too many candidates"))?,
                key
            ],
        )?;
    }
    Ok(())
}

fn insert_facts(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
    recorded_at: DateTime<Utc>,
) -> Result<()> {
    let mut fact_changes = Vec::new();
    for change in &proposal.change_set.changes {
        match change {
            RecordChange::AddFact(draft) => fact_changes.push((
                draft,
                proposal.extraction_run_id.as_ref(),
                addition_assurance(&proposal.change_set.evidence_origin)?,
            )),
            RecordChange::CorrectFact(correction) => fact_changes.push((
                &correction.replacement,
                None,
                EvidenceAssurance::SourceVerified,
            )),
            RecordChange::VerifyFact(_)
            | RecordChange::ReconcileFacts(_)
            | RecordChange::AddRecordHazard(_)
            | RecordChange::ResolveRecordHazard(_)
            | RecordChange::ResolveDiscrepancy(_) => {}
        }
    }
    if fact_changes.is_empty() {
        return Ok(());
    }
    let recorded_at = recorded_at.to_rfc3339_opts(SecondsFormat::Millis, true);
    for ((fact_id, existing_fact_id), (draft, extraction_run_id, assurance)) in proposal
        .fact_ids
        .iter()
        .zip(&proposal.existing_fact_ids)
        .zip(fact_changes)
    {
        if existing_fact_id.is_some() {
            continue;
        }
        validate_fact_insertion_assurance(
            &proposal.change_set.evidence_origin,
            assurance,
            extraction_run_id,
        )?;
        let clinical = stored_clinical_time(&draft.clinical_time);
        let (source_id, source_region) =
            draft
                .evidence
                .source()
                .map_or((None, None), |(source_id, source_region)| {
                    (
                        Some(source_id.as_str()),
                        Some(source_region.locator.as_str()),
                    )
                });
        let (cited_source_id, cited_source_region) =
            draft
                .evidence
                .cited_source()
                .map_or((None, None), |(source_id, source_region)| {
                    (
                        Some(source_id.as_str()),
                        Some(source_region.locator.as_str()),
                    )
                });
        let author = draft.evidence.author();
        transaction.execute(
            "INSERT INTO facts \
             (id, fact_type, clinical_time_kind, clinical_start_kind, clinical_start_value, \
              clinical_end_kind, clinical_end_value, clinical_source_text, recorded_at, \
              source_id, source_region_locator, cited_source_id, cited_source_region_locator, \
              extraction_run_id, evidence_assurance, author_kind, author_identifier, \
              canonical_payload, record_revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                fact_id.as_str(),
                draft.fact.kind(),
                clinical.time_kind,
                clinical.start_kind,
                clinical.start_value,
                clinical.end_kind,
                clinical.end_value,
                clinical.source_text,
                recorded_at,
                source_id,
                source_region,
                cited_source_id,
                cited_source_region,
                extraction_run_id.map(ExtractionRunId::as_str),
                evidence_assurance_name(assurance),
                author_kind_name(author.kind),
                author.identifier,
                serde_json::to_string(draft)?,
                i64::try_from(revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
        insert_fact_projection(transaction, fact_id, &draft.fact)?;
    }
    Ok(())
}

fn insert_fact_projection(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    fact: &FactData,
) -> Result<()> {
    match fact {
        FactData::LabResult(lab) => insert_lab_result(transaction, fact_id, lab),
        FactData::VitalMeasurement(vital) => insert_vital_measurement(transaction, fact_id, vital),
        FactData::MedicationEvent(event) => insert_medication_event(transaction, fact_id, event),
        FactData::ConditionAssertion(assertion) => {
            insert_condition_assertion(transaction, fact_id, assertion)
        }
        FactData::DiagnosticStudy(study) => insert_diagnostic_study(transaction, fact_id, study),
        FactData::CareTask(task) => insert_care_task(transaction, fact_id, task),
        FactData::SubjectPreference(preference) => {
            insert_subject_preference(transaction, fact_id, preference)
        }
    }
}

fn validate_fact_insertion_assurance(
    origin: &EvidenceOrigin,
    assurance: EvidenceAssurance,
    extraction_run_id: Option<&ExtractionRunId>,
) -> Result<()> {
    let source_assurance_without_run = matches!(
        assurance,
        EvidenceAssurance::Unverified | EvidenceAssurance::ParserValidated
    ) && extraction_run_id.is_none();
    let self_report_with_run = matches!(origin, EvidenceOrigin::ExplicitSelfReport { .. })
        && assurance == EvidenceAssurance::ExplicitSelfReport
        && extraction_run_id.is_some();
    if source_assurance_without_run || self_report_with_run {
        return Err(Error::ProposalIntegrity);
    }
    Ok(())
}

fn insert_verifications(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let verifications = proposal
        .change_set
        .changes
        .iter()
        .filter_map(|change| match change {
            RecordChange::VerifyFact(verification) => Some(verification),
            RecordChange::AddFact(_)
            | RecordChange::CorrectFact(_)
            | RecordChange::ReconcileFacts(_)
            | RecordChange::AddRecordHazard(_)
            | RecordChange::ResolveRecordHazard(_)
            | RecordChange::ResolveDiscrepancy(_) => None,
        });
    for (verification_id, draft) in proposal.verification_ids.iter().zip(verifications) {
        let outcome = match draft.outcome {
            VerificationOutcome::Verified => "verified",
            VerificationOutcome::Discrepancy => "discrepancy",
        };
        transaction.execute(
            "INSERT INTO verifications \
             (id, fact_id, source_id, source_region_locator, verifier_kind, verifier_identifier, \
              method_name, method_version, checked_at, scope, outcome, record_revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                verification_id.as_str(),
                draft.fact_id.as_str(),
                draft.source_id.as_str(),
                draft.source_region.locator,
                author_kind_name(draft.verifier.kind),
                draft.verifier.identifier,
                draft.method.name,
                draft.method.version,
                draft
                    .checked_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                draft.scope,
                outcome,
                i64::try_from(revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
    }
    Ok(())
}

fn insert_corrections(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let mut fact_ids = proposal.fact_ids.iter();
    let mut existing_fact_ids = proposal.existing_fact_ids.iter();
    let mut correction_ids = proposal.correction_ids.iter();
    for change in &proposal.change_set.changes {
        match change {
            RecordChange::AddFact(_) => {
                fact_ids.next().ok_or(Error::ProposalIntegrity)?;
                existing_fact_ids.next().ok_or(Error::ProposalIntegrity)?;
            }
            RecordChange::VerifyFact(_)
            | RecordChange::ReconcileFacts(_)
            | RecordChange::AddRecordHazard(_)
            | RecordChange::ResolveRecordHazard(_)
            | RecordChange::ResolveDiscrepancy(_) => {}
            RecordChange::CorrectFact(draft) => {
                let replacement_fact_id = fact_ids.next().ok_or(Error::ProposalIntegrity)?;
                if existing_fact_ids
                    .next()
                    .ok_or(Error::ProposalIntegrity)?
                    .is_some()
                {
                    return Err(Error::ProposalIntegrity);
                }
                let correction_id = correction_ids.next().ok_or(Error::ProposalIntegrity)?;
                transaction.execute(
                    "INSERT INTO corrections \
                     (id, prior_fact_id, replacement_fact_id, reason, author_kind, author_identifier, \
                      corrected_at, supporting_verification_id, record_revision) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        correction_id.as_str(),
                        draft.prior_fact_id.as_str(),
                        replacement_fact_id.as_str(),
                        draft.reason,
                        author_kind_name(draft.author.kind),
                        draft.author.identifier,
                        draft
                            .corrected_at
                            .to_rfc3339_opts(SecondsFormat::Millis, true),
                        draft.supporting_verification_id.as_str(),
                        i64::try_from(revision.value()).map_err(|_| {
                            Error::InvalidRecordChangeSet("record revision overflow")
                        })?
                    ],
                )?;
            }
        }
    }
    if correction_ids.next().is_some() {
        return Err(Error::ProposalIntegrity);
    }
    if fact_ids.next().is_some() || existing_fact_ids.next().is_some() {
        return Err(Error::ProposalIntegrity);
    }
    Ok(())
}

fn insert_reconciliations(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let reconciliations = proposal
        .change_set
        .changes
        .iter()
        .filter_map(|change| match change {
            RecordChange::ReconcileFacts(reconciliation) => Some(reconciliation),
            RecordChange::AddFact(_)
            | RecordChange::VerifyFact(_)
            | RecordChange::CorrectFact(_)
            | RecordChange::AddRecordHazard(_)
            | RecordChange::ResolveRecordHazard(_)
            | RecordChange::ResolveDiscrepancy(_) => None,
        });
    for (reconciliation_id, draft) in proposal.reconciliation_ids.iter().zip(reconciliations) {
        let agreement = match draft.agreement {
            ReconciliationAgreement::Equivalent => "equivalent",
            ReconciliationAgreement::Conflicting => "conflicting",
        };
        transaction.execute(
            "INSERT INTO reconciliations \
             (id, agreement, author_kind, author_identifier, reconciled_at, rationale, record_revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                reconciliation_id.as_str(),
                agreement,
                author_kind_name(draft.author.kind),
                draft.author.identifier,
                draft
                    .reconciled_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                draft.rationale,
                i64::try_from(revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
        for (position, fact_id) in draft.fact_ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO reconciliation_fact_edges (reconciliation_id, position, fact_id) \
                 VALUES (?1, ?2, ?3)",
                params![
                    reconciliation_id.as_str(),
                    i64::try_from(position).map_err(|_| {
                        Error::InvalidRecordChangeSet("too many Reconciliation Facts")
                    })?,
                    fact_id.as_str()
                ],
            )?;
        }
    }
    Ok(())
}

fn insert_record_hazards(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let hazards = proposal
        .change_set
        .changes
        .iter()
        .filter_map(|change| match change {
            RecordChange::AddRecordHazard(hazard) => Some(hazard),
            RecordChange::AddFact(_)
            | RecordChange::VerifyFact(_)
            | RecordChange::CorrectFact(_)
            | RecordChange::ReconcileFacts(_)
            | RecordChange::ResolveRecordHazard(_)
            | RecordChange::ResolveDiscrepancy(_) => None,
        });
    for (hazard_id, draft) in proposal.record_hazard_ids.iter().zip(hazards) {
        let verification_state = match draft.verification_state {
            HazardVerificationState::Suspected => "suspected",
            HazardVerificationState::Verified => "verified",
        };
        transaction.execute(
            "INSERT INTO record_hazards \
             (id, problematic_statement, danger, corrected_understanding, author_kind, \
              author_identifier, recorded_at, verification_state, record_revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                hazard_id.as_str(),
                draft.problematic_statement,
                draft.danger,
                draft.corrected_understanding,
                author_kind_name(draft.author.kind),
                draft.author.identifier,
                draft
                    .recorded_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                verification_state,
                i64::try_from(revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
        for (position, fact_id) in draft.affected_fact_ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO record_hazard_fact_edges (hazard_id, position, fact_id) \
                 VALUES (?1, ?2, ?3)",
                params![
                    hazard_id.as_str(),
                    i64::try_from(position)
                        .map_err(|_| Error::InvalidRecordChangeSet("too many Hazard Facts"))?,
                    fact_id.as_str()
                ],
            )?;
        }
        for (position, source_id) in draft.affected_source_ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO record_hazard_source_edges (hazard_id, position, source_id) \
                 VALUES (?1, ?2, ?3)",
                params![
                    hazard_id.as_str(),
                    i64::try_from(position)
                        .map_err(|_| Error::InvalidRecordChangeSet("too many Hazard Sources"))?,
                    source_id.as_str()
                ],
            )?;
        }
    }
    Ok(())
}

fn insert_record_hazard_resolutions(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let resolutions = proposal
        .change_set
        .changes
        .iter()
        .filter_map(|change| match change {
            RecordChange::ResolveRecordHazard(resolution) => Some(resolution),
            RecordChange::AddFact(_)
            | RecordChange::VerifyFact(_)
            | RecordChange::CorrectFact(_)
            | RecordChange::ReconcileFacts(_)
            | RecordChange::AddRecordHazard(_)
            | RecordChange::ResolveDiscrepancy(_) => None,
        });
    for (resolution_id, draft) in proposal
        .record_hazard_resolution_ids
        .iter()
        .zip(resolutions)
    {
        transaction.execute(
            "INSERT INTO record_hazard_resolutions \
             (id, hazard_id, supporting_verification_id, rationale, author_kind, \
              author_identifier, resolved_at, record_revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                resolution_id.as_str(),
                draft.hazard_id.as_str(),
                draft.supporting_verification_id.as_str(),
                draft.rationale,
                author_kind_name(draft.author.kind),
                draft.author.identifier,
                draft
                    .resolved_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                i64::try_from(revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
    }
    Ok(())
}

fn insert_discrepancy_resolutions(
    transaction: &Transaction<'_>,
    proposal: &RecordProposal,
    revision: RecordRevision,
) -> Result<()> {
    let resolutions = proposal
        .change_set
        .changes
        .iter()
        .filter_map(|change| match change {
            RecordChange::ResolveDiscrepancy(resolution) => Some(resolution),
            RecordChange::AddFact(_)
            | RecordChange::VerifyFact(_)
            | RecordChange::CorrectFact(_)
            | RecordChange::ReconcileFacts(_)
            | RecordChange::AddRecordHazard(_)
            | RecordChange::ResolveRecordHazard(_) => None,
        });
    for (resolution_id, draft) in proposal.discrepancy_resolution_ids.iter().zip(resolutions) {
        transaction.execute(
            "INSERT INTO discrepancy_resolutions \
             (id, discrepancy_verification_id, supporting_verification_id, rationale, author_kind, \
              author_identifier, resolved_at, record_revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                resolution_id.as_str(),
                draft.discrepancy_verification_id.as_str(),
                draft.supporting_verification_id.as_str(),
                draft.rationale,
                author_kind_name(draft.author.kind),
                draft.author.identifier,
                draft
                    .resolved_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                i64::try_from(revision.value())
                    .map_err(|_| Error::InvalidRecordChangeSet("record revision overflow"))?
            ],
        )?;
    }
    Ok(())
}

fn insert_lab_result(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    lab: &LabResult,
) -> Result<()> {
    let (numeric_value, numeric_comparator, numeric_original, text_value) = match &lab.value {
        LabValue::Numeric { value } => (Some(value.to_string()), None, None, None),
        LabValue::QualifiedNumeric {
            comparator,
            value,
            original,
        } => (
            Some(value.to_string()),
            Some(lab_comparator_name(*comparator)),
            Some(original.as_str()),
            None,
        ),
        LabValue::Text { value } => (None, None, None, Some(value.as_str())),
    };
    let (canonical_identifier, canonical_display_name) =
        lab.canonical_test
            .as_ref()
            .map_or((None, None), |canonical| {
                (
                    Some(canonical.identifier.as_str()),
                    Some(canonical.display_name.as_str()),
                )
            });
    let (range_original, range_lower, range_upper) =
        lab.reference_range
            .as_ref()
            .map_or((None, None, None), |range| {
                (
                    Some(range.original.as_str()),
                    range.lower.map(|value| value.to_string()),
                    range.upper.map(|value| value.to_string()),
                )
            });
    transaction.execute(
        "INSERT INTO lab_results \
         (fact_id, test_name, canonical_test_identifier, canonical_test_display_name, \
          numeric_value, numeric_comparator, numeric_original, text_value, units, \
          reference_range_original, reference_range_lower, reference_range_upper, reported_flag, \
          performing_lab) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            fact_id.as_str(),
            lab.test_name,
            canonical_identifier,
            canonical_display_name,
            numeric_value,
            numeric_comparator,
            numeric_original,
            text_value,
            lab.units,
            range_original,
            range_lower,
            range_upper,
            lab.reported_flag,
            lab.performing_lab
        ],
    )?;
    Ok(())
}

fn insert_vital_measurement(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    vital: &VitalMeasurement,
) -> Result<()> {
    let (kind, primary, secondary, original_primary, original_secondary, units, measured_by, note) =
        match vital {
            VitalMeasurement::BloodPressure {
                systolic,
                diastolic,
                original_systolic,
                original_diastolic,
                units,
                measured_by,
                note,
            } => (
                "blood_pressure",
                systolic.to_string(),
                Some(diastolic.to_string()),
                original_systolic.as_str(),
                Some(original_diastolic.as_str()),
                units.as_deref(),
                measured_by.as_deref(),
                note.as_deref(),
            ),
            VitalMeasurement::Pulse {
                value,
                original,
                units,
                measured_by,
                note,
            } => (
                "pulse",
                value.to_string(),
                None,
                original.as_str(),
                None,
                units.as_deref(),
                measured_by.as_deref(),
                note.as_deref(),
            ),
            VitalMeasurement::Weight {
                value,
                original,
                units,
                note,
            } => (
                "weight",
                value.to_string(),
                None,
                original.as_str(),
                None,
                units.as_deref(),
                None,
                note.as_deref(),
            ),
        };
    transaction.execute(
        "INSERT INTO vital_measurements \
         (fact_id, vital_kind, primary_value, secondary_value, original_primary, \
          original_secondary, units, measured_by, note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            fact_id.as_str(),
            kind,
            primary,
            secondary,
            original_primary,
            original_secondary,
            units,
            measured_by,
            note
        ],
    )?;
    Ok(())
}

fn insert_medication_event(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    event: &MedicationEvent,
) -> Result<()> {
    let (dose_value, dose_unit, dose_original) =
        event.dose.as_ref().map_or((None, None, None), |dose| {
            (
                Some(dose.value.to_string()),
                Some(dose.unit.as_str()),
                Some(dose.original.as_str()),
            )
        });
    transaction.execute(
        "INSERT INTO medication_events \
         (fact_id, event_kind, product_kind, name, normalized_identity, strength, dose_value, \
          dose_unit, dose_original, route, schedule, indication, adherence_context) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            fact_id.as_str(),
            medication_event_kind_name(event.event),
            medication_product_kind_name(event.product_kind),
            event.name,
            event.normalized_identity,
            event.strength,
            dose_value,
            dose_unit,
            dose_original,
            event.route,
            event.schedule,
            event.indication,
            event.adherence_context,
        ],
    )?;
    Ok(())
}

fn insert_condition_assertion(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    assertion: &ConditionAssertion,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO condition_assertions \
         (fact_id, assertion_state, name, normalized_identity, body_site, assertion_text) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            fact_id.as_str(),
            condition_assertion_state_name(assertion.state),
            assertion.name,
            assertion.normalized_identity,
            assertion.body_site,
            assertion.assertion_text,
        ],
    )?;
    Ok(())
}

fn insert_diagnostic_study(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    study: &DiagnosticStudy,
) -> Result<()> {
    let (resulted_kind, resulted_value) =
        study.resulted_at.as_ref().map_or((None, None), |point| {
            let (kind, value) = clinical_point_parts(point);
            (Some(kind), Some(value))
        });
    transaction.execute(
        "INSERT INTO diagnostic_studies \
         (fact_id, study_kind, result_status, name, body_site, method, impression, \
          resulted_time_kind, resulted_time_value, ordering_provider, interpreting_provider, \
          performing_organization) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            fact_id.as_str(),
            diagnostic_study_kind_name(study.kind),
            diagnostic_study_status_name(study.status),
            study.name,
            study.body_site,
            study.method,
            study.impression,
            resulted_kind,
            resulted_value,
            study.ordering_provider,
            study.interpreting_provider,
            study.performing_organization,
        ],
    )?;
    for (position, finding) in study.findings.iter().enumerate() {
        transaction.execute(
            "INSERT INTO diagnostic_study_findings \
             (study_fact_id, position, section, body_site, finding_text) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                fact_id.as_str(),
                i64::try_from(position).map_err(|_| {
                    Error::InvalidRecordChangeSet("too many Diagnostic Study findings")
                })?,
                finding.section,
                finding.body_site,
                finding.text,
            ],
        )?;
    }
    Ok(())
}

fn insert_care_task(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    task: &CareTask,
) -> Result<()> {
    let mut due_kind = None;
    let mut due_time_kind = None;
    let mut due_time_value = None;
    let mut due_interval_value = None;
    let mut due_interval_unit = None;
    let mut due_source_text = None;
    match &task.due {
        Some(CareTaskDue::On { time }) => {
            let (kind, value) = clinical_point_parts(time);
            due_kind = Some("on");
            due_time_kind = Some(kind);
            due_time_value = Some(value);
        }
        Some(CareTaskDue::Interval {
            value,
            unit,
            source_text,
        }) => {
            due_kind = Some("interval");
            due_interval_value = Some(i64::from(*value));
            due_interval_unit = Some(care_interval_unit_name(*unit));
            due_source_text = Some(source_text.as_str());
        }
        None => {}
    }
    transaction.execute(
        "INSERT INTO care_tasks \
         (fact_id, task_key, task_kind, task_status, action, due_kind, due_time_kind, \
          due_time_value, due_interval_value, due_interval_unit, due_source_text, requested_by, \
          source_text) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            fact_id.as_str(),
            task.task_key,
            care_task_kind_name(task.kind),
            care_task_status_name(task.status),
            task.action,
            due_kind,
            due_time_kind,
            due_time_value,
            due_interval_value,
            due_interval_unit,
            due_source_text,
            task.requested_by,
            task.source_text,
        ],
    )?;
    Ok(())
}

fn insert_subject_preference(
    transaction: &Transaction<'_>,
    fact_id: &FactId,
    preference: &SubjectPreference,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO subject_preferences \
         (fact_id, preference_key, preference_category, preference_state, statement, source_text) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            fact_id.as_str(),
            preference.preference_key,
            subject_preference_category_name(preference.category),
            subject_preference_state_name(preference.state),
            preference.statement,
            preference.source_text,
        ],
    )?;
    Ok(())
}

fn subject_preference_category_name(value: SubjectPreferenceCategory) -> &'static str {
    match value {
        SubjectPreferenceCategory::Tracking => "tracking",
        SubjectPreferenceCategory::Presentation => "presentation",
        SubjectPreferenceCategory::PersonalRoutine => "personal_routine",
    }
}

fn parse_subject_preference_category(value: &str) -> Result<SubjectPreferenceCategory> {
    match value {
        "tracking" => Ok(SubjectPreferenceCategory::Tracking),
        "presentation" => Ok(SubjectPreferenceCategory::Presentation),
        "personal_routine" => Ok(SubjectPreferenceCategory::PersonalRoutine),
        _ => Err(invalid_fact_store()),
    }
}

fn subject_preference_state_name(value: SubjectPreferenceState) -> &'static str {
    match value {
        SubjectPreferenceState::Active => "active",
        SubjectPreferenceState::Withdrawn => "withdrawn",
    }
}

fn parse_subject_preference_state(value: &str) -> Result<SubjectPreferenceState> {
    match value {
        "active" => Ok(SubjectPreferenceState::Active),
        "withdrawn" => Ok(SubjectPreferenceState::Withdrawn),
        _ => Err(invalid_fact_store()),
    }
}

fn care_task_kind_name(value: CareTaskKind) -> &'static str {
    match value {
        CareTaskKind::RepeatTest => "repeat_test",
        CareTaskKind::Referral => "referral",
        CareTaskKind::Appointment => "appointment",
        CareTaskKind::AwaitedResult => "awaited_result",
        CareTaskKind::Other => "other",
    }
}

fn parse_care_task_kind(value: &str) -> Result<CareTaskKind> {
    match value {
        "repeat_test" => Ok(CareTaskKind::RepeatTest),
        "referral" => Ok(CareTaskKind::Referral),
        "appointment" => Ok(CareTaskKind::Appointment),
        "awaited_result" => Ok(CareTaskKind::AwaitedResult),
        "other" => Ok(CareTaskKind::Other),
        _ => Err(invalid_fact_store()),
    }
}

fn care_task_status_name(value: CareTaskStatus) -> &'static str {
    match value {
        CareTaskStatus::Pending => "pending",
        CareTaskStatus::Due => "due",
        CareTaskStatus::Overdue => "overdue",
        CareTaskStatus::AwaitingResult => "awaiting_result",
        CareTaskStatus::Completed => "completed",
        CareTaskStatus::Cancelled => "cancelled",
    }
}

fn parse_care_task_status(value: &str) -> Result<CareTaskStatus> {
    match value {
        "pending" => Ok(CareTaskStatus::Pending),
        "due" => Ok(CareTaskStatus::Due),
        "overdue" => Ok(CareTaskStatus::Overdue),
        "awaiting_result" => Ok(CareTaskStatus::AwaitingResult),
        "completed" => Ok(CareTaskStatus::Completed),
        "cancelled" => Ok(CareTaskStatus::Cancelled),
        _ => Err(invalid_fact_store()),
    }
}

fn care_interval_unit_name(value: CareIntervalUnit) -> &'static str {
    match value {
        CareIntervalUnit::Days => "days",
        CareIntervalUnit::Weeks => "weeks",
        CareIntervalUnit::Months => "months",
        CareIntervalUnit::Years => "years",
    }
}

fn parse_care_interval_unit(value: &str) -> Result<CareIntervalUnit> {
    match value {
        "days" => Ok(CareIntervalUnit::Days),
        "weeks" => Ok(CareIntervalUnit::Weeks),
        "months" => Ok(CareIntervalUnit::Months),
        "years" => Ok(CareIntervalUnit::Years),
        _ => Err(invalid_fact_store()),
    }
}

fn diagnostic_study_kind_name(value: DiagnosticStudyKind) -> &'static str {
    match value {
        DiagnosticStudyKind::Imaging => "imaging",
        DiagnosticStudyKind::Pathology => "pathology",
        DiagnosticStudyKind::Procedure => "procedure",
        DiagnosticStudyKind::Other => "other",
    }
}

fn parse_diagnostic_study_kind(value: &str) -> Result<DiagnosticStudyKind> {
    match value {
        "imaging" => Ok(DiagnosticStudyKind::Imaging),
        "pathology" => Ok(DiagnosticStudyKind::Pathology),
        "procedure" => Ok(DiagnosticStudyKind::Procedure),
        "other" => Ok(DiagnosticStudyKind::Other),
        _ => Err(invalid_fact_store()),
    }
}

fn diagnostic_study_status_name(value: DiagnosticStudyStatus) -> &'static str {
    match value {
        DiagnosticStudyStatus::Ordered => "ordered",
        DiagnosticStudyStatus::Scheduled => "scheduled",
        DiagnosticStudyStatus::Performed => "performed",
        DiagnosticStudyStatus::Preliminary => "preliminary",
        DiagnosticStudyStatus::Final => "final",
        DiagnosticStudyStatus::Amended => "amended",
        DiagnosticStudyStatus::Cancelled => "cancelled",
    }
}

fn parse_diagnostic_study_status(value: &str) -> Result<DiagnosticStudyStatus> {
    match value {
        "ordered" => Ok(DiagnosticStudyStatus::Ordered),
        "scheduled" => Ok(DiagnosticStudyStatus::Scheduled),
        "performed" => Ok(DiagnosticStudyStatus::Performed),
        "preliminary" => Ok(DiagnosticStudyStatus::Preliminary),
        "final" => Ok(DiagnosticStudyStatus::Final),
        "amended" => Ok(DiagnosticStudyStatus::Amended),
        "cancelled" => Ok(DiagnosticStudyStatus::Cancelled),
        _ => Err(invalid_fact_store()),
    }
}

fn condition_assertion_state_name(value: ConditionAssertionState) -> &'static str {
    match value {
        ConditionAssertionState::Suspected => "suspected",
        ConditionAssertionState::Confirmed => "confirmed",
        ConditionAssertionState::RuledOut => "ruled_out",
        ConditionAssertionState::Inactive => "inactive",
        ConditionAssertionState::Resolved => "resolved",
    }
}

fn parse_condition_assertion_state(value: &str) -> Result<ConditionAssertionState> {
    match value {
        "suspected" => Ok(ConditionAssertionState::Suspected),
        "confirmed" => Ok(ConditionAssertionState::Confirmed),
        "ruled_out" => Ok(ConditionAssertionState::RuledOut),
        "inactive" => Ok(ConditionAssertionState::Inactive),
        "resolved" => Ok(ConditionAssertionState::Resolved),
        _ => Err(invalid_fact_store()),
    }
}

fn medication_event_kind_name(value: MedicationEventKind) -> &'static str {
    match value {
        MedicationEventKind::RegimenReported => "regimen_reported",
        MedicationEventKind::Started => "started",
        MedicationEventKind::Stopped => "stopped",
        MedicationEventKind::DoseChanged => "dose_changed",
        MedicationEventKind::ScheduleChanged => "schedule_changed",
        MedicationEventKind::MissedDose => "missed_dose",
        MedicationEventKind::AsNeededUse => "as_needed_use",
    }
}

fn parse_medication_event_kind(value: &str) -> Result<MedicationEventKind> {
    match value {
        "regimen_reported" => Ok(MedicationEventKind::RegimenReported),
        "started" => Ok(MedicationEventKind::Started),
        "stopped" => Ok(MedicationEventKind::Stopped),
        "dose_changed" => Ok(MedicationEventKind::DoseChanged),
        "schedule_changed" => Ok(MedicationEventKind::ScheduleChanged),
        "missed_dose" => Ok(MedicationEventKind::MissedDose),
        "as_needed_use" => Ok(MedicationEventKind::AsNeededUse),
        _ => Err(invalid_fact_store()),
    }
}

fn medication_product_kind_name(value: MedicationProductKind) -> &'static str {
    match value {
        MedicationProductKind::Medication => "medication",
        MedicationProductKind::Supplement => "supplement",
    }
}

fn parse_medication_product_kind(value: &str) -> Result<MedicationProductKind> {
    match value {
        "medication" => Ok(MedicationProductKind::Medication),
        "supplement" => Ok(MedicationProductKind::Supplement),
        _ => Err(invalid_fact_store()),
    }
}

fn lab_comparator_name(value: LabComparator) -> &'static str {
    match value {
        LabComparator::LessThan => "less_than",
        LabComparator::LessThanOrEqual => "less_than_or_equal",
        LabComparator::GreaterThan => "greater_than",
        LabComparator::GreaterThanOrEqual => "greater_than_or_equal",
    }
}

fn parse_lab_comparator(value: &str) -> Result<LabComparator> {
    match value {
        "less_than" => Ok(LabComparator::LessThan),
        "less_than_or_equal" => Ok(LabComparator::LessThanOrEqual),
        "greater_than" => Ok(LabComparator::GreaterThan),
        "greater_than_or_equal" => Ok(LabComparator::GreaterThanOrEqual),
        _ => Err(invalid_fact_store()),
    }
}

fn evidence_assurance_name(value: EvidenceAssurance) -> &'static str {
    match value {
        EvidenceAssurance::ParserValidated => "parser_validated",
        EvidenceAssurance::Unverified => "unverified",
        EvidenceAssurance::SourceVerified => "source_verified",
        EvidenceAssurance::ExplicitSelfReport => "explicit_self_report",
    }
}

fn author_kind_name(value: AuthorKind) -> &'static str {
    match value {
        AuthorKind::Subject => "subject",
        AuthorKind::Caregiver => "caregiver",
        AuthorKind::Agent => "agent",
        AuthorKind::Application => "application",
        AuthorKind::Clinician => "clinician",
    }
}

fn parse_author_kind(value: &str) -> Result<AuthorKind> {
    match value {
        "subject" => Ok(AuthorKind::Subject),
        "caregiver" => Ok(AuthorKind::Caregiver),
        "agent" => Ok(AuthorKind::Agent),
        "application" => Ok(AuthorKind::Application),
        "clinician" => Ok(AuthorKind::Clinician),
        _ => Err(invalid_fact_store()),
    }
}

struct StoredSubjectPreferenceRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    preference_key: String,
    preference_category: String,
    preference_state: String,
    statement: String,
    source_text: String,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
}

impl StoredSubjectPreferenceRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            preference_key: row.get(12)?,
            preference_category: row.get(13)?,
            preference_state: row.get(14)?,
            statement: row.get(15)?,
            source_text: row.get(16)?,
            current_verification: row.get(17)?,
            unresolved_discrepancy_id: row.get(18)?,
            conflicting_reconciliation_id: row.get(19)?,
            record_hazard_id: row.get(20)?,
            clinical_start_kind: row.get(21)?,
            clinical_end_kind: row.get(22)?,
            clinical_end_value: row.get(23)?,
            cited_source_id: row.get(24)?,
            cited_source_region_locator: row.get(25)?,
        })
    }

    fn into_fact(self) -> Result<HealthFact> {
        let clinical_time = parse_stored_clinical_time_values(
            &self.clinical_time_kind,
            &self.clinical_start_kind,
            &self.clinical_start_value,
            self.clinical_end_kind.as_deref(),
            self.clinical_end_value.as_deref(),
            self.clinical_source_text,
        )?;
        let evidence_assurance = parse_evidence_assurance(&self.evidence_assurance)?;
        let current_assurance =
            parse_current_assurance(self.current_verification.as_deref(), evidence_assurance)?;
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            AuthorIdentity {
                kind: parse_author_kind(&self.author_kind)?,
                identifier: self.author_identifier,
            },
            evidence_assurance,
        )?;
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::SubjectPreference(SubjectPreference {
                    preference_key: self.preference_key,
                    category: parse_subject_preference_category(&self.preference_category)?,
                    state: parse_subject_preference_state(&self.preference_state)?,
                    statement: self.statement,
                    source_text: self.source_text,
                }),
            },
        })
    }
}

struct StoredCareTaskRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    task_key: String,
    task_kind: String,
    task_status: String,
    action: String,
    due_kind: Option<String>,
    due_time_kind: Option<String>,
    due_time_value: Option<String>,
    due_interval_value: Option<i64>,
    due_interval_unit: Option<String>,
    due_source_text: Option<String>,
    requested_by: Option<String>,
    source_text: String,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
}

impl StoredCareTaskRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            task_key: row.get(12)?,
            task_kind: row.get(13)?,
            task_status: row.get(14)?,
            action: row.get(15)?,
            due_kind: row.get(16)?,
            due_time_kind: row.get(17)?,
            due_time_value: row.get(18)?,
            due_interval_value: row.get(19)?,
            due_interval_unit: row.get(20)?,
            due_source_text: row.get(21)?,
            requested_by: row.get(22)?,
            source_text: row.get(23)?,
            current_verification: row.get(24)?,
            unresolved_discrepancy_id: row.get(25)?,
            conflicting_reconciliation_id: row.get(26)?,
            record_hazard_id: row.get(27)?,
            clinical_start_kind: row.get(28)?,
            clinical_end_kind: row.get(29)?,
            clinical_end_value: row.get(30)?,
            cited_source_id: row.get(31)?,
            cited_source_region_locator: row.get(32)?,
        })
    }

    fn into_fact(self) -> Result<HealthFact> {
        let clinical_time = parse_stored_clinical_time_values(
            &self.clinical_time_kind,
            &self.clinical_start_kind,
            &self.clinical_start_value,
            self.clinical_end_kind.as_deref(),
            self.clinical_end_value.as_deref(),
            self.clinical_source_text,
        )?;
        let evidence_assurance = parse_evidence_assurance(&self.evidence_assurance)?;
        let current_assurance =
            parse_current_assurance(self.current_verification.as_deref(), evidence_assurance)?;
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            AuthorIdentity {
                kind: parse_author_kind(&self.author_kind)?,
                identifier: self.author_identifier,
            },
            evidence_assurance,
        )?;
        let due = match (
            self.due_kind.as_deref(),
            self.due_time_kind,
            self.due_time_value,
            self.due_interval_value,
            self.due_interval_unit,
            self.due_source_text,
        ) {
            (Some("on"), Some(kind), Some(value), None, None, None) => Some(CareTaskDue::On {
                time: parse_clinical_point(&kind, &value)?,
            }),
            (Some("interval"), None, None, Some(value), Some(unit), Some(source_text)) => {
                Some(CareTaskDue::Interval {
                    value: u32::try_from(value).map_err(|_| invalid_fact_store())?,
                    unit: parse_care_interval_unit(&unit)?,
                    source_text,
                })
            }
            (None, None, None, None, None, None) => None,
            _ => return Err(invalid_fact_store()),
        };
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::CareTask(CareTask {
                    task_key: self.task_key,
                    kind: parse_care_task_kind(&self.task_kind)?,
                    status: parse_care_task_status(&self.task_status)?,
                    action: self.action,
                    due,
                    requested_by: self.requested_by,
                    source_text: self.source_text,
                }),
            },
        })
    }
}

struct StoredDiagnosticRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    study_kind: String,
    result_status: String,
    name: String,
    body_site: Option<String>,
    method: Option<String>,
    impression: Option<String>,
    resulted_time_kind: Option<String>,
    resulted_time_value: Option<String>,
    ordering_provider: Option<String>,
    interpreting_provider: Option<String>,
    performing_organization: Option<String>,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
}

impl StoredDiagnosticRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            study_kind: row.get(12)?,
            result_status: row.get(13)?,
            name: row.get(14)?,
            body_site: row.get(15)?,
            method: row.get(16)?,
            impression: row.get(17)?,
            resulted_time_kind: row.get(18)?,
            resulted_time_value: row.get(19)?,
            ordering_provider: row.get(20)?,
            interpreting_provider: row.get(21)?,
            performing_organization: row.get(22)?,
            current_verification: row.get(23)?,
            unresolved_discrepancy_id: row.get(24)?,
            conflicting_reconciliation_id: row.get(25)?,
            record_hazard_id: row.get(26)?,
            clinical_start_kind: row.get(27)?,
            clinical_end_kind: row.get(28)?,
            clinical_end_value: row.get(29)?,
            cited_source_id: row.get(30)?,
            cited_source_region_locator: row.get(31)?,
        })
    }

    fn into_fact(self, findings: Vec<DiagnosticFinding>) -> Result<HealthFact> {
        let clinical_time = parse_stored_clinical_time_values(
            &self.clinical_time_kind,
            &self.clinical_start_kind,
            &self.clinical_start_value,
            self.clinical_end_kind.as_deref(),
            self.clinical_end_value.as_deref(),
            self.clinical_source_text,
        )?;
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let evidence_assurance = parse_evidence_assurance(&self.evidence_assurance)?;
        let current_assurance =
            parse_current_assurance(self.current_verification.as_deref(), evidence_assurance)?;
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            AuthorIdentity {
                kind: parse_author_kind(&self.author_kind)?,
                identifier: self.author_identifier,
            },
            evidence_assurance,
        )?;
        let resulted_at = match (self.resulted_time_kind, self.resulted_time_value) {
            (Some(kind), Some(value)) => Some(parse_clinical_point(&kind, &value)?),
            (None, None) => None,
            _ => return Err(invalid_fact_store()),
        };
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::DiagnosticStudy(DiagnosticStudy {
                    kind: parse_diagnostic_study_kind(&self.study_kind)?,
                    status: parse_diagnostic_study_status(&self.result_status)?,
                    name: self.name,
                    body_site: self.body_site,
                    method: self.method,
                    findings,
                    impression: self.impression,
                    resulted_at,
                    ordering_provider: self.ordering_provider,
                    interpreting_provider: self.interpreting_provider,
                    performing_organization: self.performing_organization,
                }),
            },
        })
    }
}

struct StoredConditionRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    assertion_state: String,
    name: String,
    normalized_identity: Option<String>,
    body_site: Option<String>,
    assertion_text: String,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
}

impl StoredConditionRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            assertion_state: row.get(12)?,
            name: row.get(13)?,
            normalized_identity: row.get(14)?,
            body_site: row.get(15)?,
            assertion_text: row.get(16)?,
            current_verification: row.get(17)?,
            unresolved_discrepancy_id: row.get(18)?,
            conflicting_reconciliation_id: row.get(19)?,
            record_hazard_id: row.get(20)?,
            clinical_start_kind: row.get(21)?,
            clinical_end_kind: row.get(22)?,
            clinical_end_value: row.get(23)?,
            cited_source_id: row.get(24)?,
            cited_source_region_locator: row.get(25)?,
        })
    }

    fn into_fact(self) -> Result<HealthFact> {
        let clinical_time = parse_stored_clinical_time_values(
            &self.clinical_time_kind,
            &self.clinical_start_kind,
            &self.clinical_start_value,
            self.clinical_end_kind.as_deref(),
            self.clinical_end_value.as_deref(),
            self.clinical_source_text,
        )?;
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let evidence_assurance = parse_evidence_assurance(&self.evidence_assurance)?;
        let current_assurance =
            parse_current_assurance(self.current_verification.as_deref(), evidence_assurance)?;
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            AuthorIdentity {
                kind: parse_author_kind(&self.author_kind)?,
                identifier: self.author_identifier,
            },
            evidence_assurance,
        )?;
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::ConditionAssertion(ConditionAssertion {
                    state: parse_condition_assertion_state(&self.assertion_state)?,
                    name: self.name,
                    normalized_identity: self.normalized_identity,
                    body_site: self.body_site,
                    assertion_text: self.assertion_text,
                }),
            },
        })
    }
}

struct StoredMedicationRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    event_kind: String,
    product_kind: String,
    name: String,
    normalized_identity: Option<String>,
    strength: Option<String>,
    dose_value: Option<String>,
    dose_unit: Option<String>,
    dose_original: Option<String>,
    route: Option<String>,
    schedule: Option<String>,
    indication: Option<String>,
    adherence_context: Option<String>,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
}

impl StoredMedicationRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            event_kind: row.get(12)?,
            product_kind: row.get(13)?,
            name: row.get(14)?,
            normalized_identity: row.get(15)?,
            strength: row.get(16)?,
            dose_value: row.get(17)?,
            dose_unit: row.get(18)?,
            dose_original: row.get(19)?,
            route: row.get(20)?,
            schedule: row.get(21)?,
            indication: row.get(22)?,
            adherence_context: row.get(23)?,
            current_verification: row.get(24)?,
            unresolved_discrepancy_id: row.get(25)?,
            conflicting_reconciliation_id: row.get(26)?,
            record_hazard_id: row.get(27)?,
            clinical_start_kind: row.get(28)?,
            clinical_end_kind: row.get(29)?,
            clinical_end_value: row.get(30)?,
            cited_source_id: row.get(31)?,
            cited_source_region_locator: row.get(32)?,
        })
    }

    fn into_fact(self) -> Result<HealthFact> {
        let clinical_time = parse_stored_clinical_time_values(
            &self.clinical_time_kind,
            &self.clinical_start_kind,
            &self.clinical_start_value,
            self.clinical_end_kind.as_deref(),
            self.clinical_end_value.as_deref(),
            self.clinical_source_text,
        )?;
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let evidence_assurance = parse_evidence_assurance(&self.evidence_assurance)?;
        let current_assurance =
            parse_current_assurance(self.current_verification.as_deref(), evidence_assurance)?;
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            AuthorIdentity {
                kind: parse_author_kind(&self.author_kind)?,
                identifier: self.author_identifier,
            },
            evidence_assurance,
        )?;
        let dose = match (self.dose_value, self.dose_unit, self.dose_original) {
            (Some(value), Some(unit), Some(original)) => Some(MedicationDose {
                value: Decimal::from_str(&value).map_err(|_| invalid_fact_store())?,
                unit,
                original,
            }),
            (None, None, None) => None,
            _ => return Err(invalid_fact_store()),
        };
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::MedicationEvent(MedicationEvent {
                    event: parse_medication_event_kind(&self.event_kind)?,
                    product_kind: parse_medication_product_kind(&self.product_kind)?,
                    name: self.name,
                    normalized_identity: self.normalized_identity,
                    strength: self.strength,
                    dose,
                    route: self.route,
                    schedule: self.schedule,
                    indication: self.indication,
                    adherence_context: self.adherence_context,
                }),
            },
        })
    }
}

struct StoredVitalRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    vital_kind: String,
    primary_value: String,
    secondary_value: Option<String>,
    original_primary: String,
    original_secondary: Option<String>,
    units: Option<String>,
    measured_by: Option<String>,
    note: Option<String>,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
}

impl StoredVitalRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            vital_kind: row.get(12)?,
            primary_value: row.get(13)?,
            secondary_value: row.get(14)?,
            original_primary: row.get(15)?,
            original_secondary: row.get(16)?,
            units: row.get(17)?,
            measured_by: row.get(18)?,
            note: row.get(19)?,
            current_verification: row.get(20)?,
            unresolved_discrepancy_id: row.get(21)?,
            conflicting_reconciliation_id: row.get(22)?,
            record_hazard_id: row.get(23)?,
            clinical_start_kind: row.get(24)?,
            clinical_end_kind: row.get(25)?,
            clinical_end_value: row.get(26)?,
            cited_source_id: row.get(27)?,
            cited_source_region_locator: row.get(28)?,
        })
    }

    fn into_fact(self) -> Result<HealthFact> {
        let vital = parse_stored_vital(&self)?;
        let clinical_time = parse_stored_clinical_time_values(
            &self.clinical_time_kind,
            &self.clinical_start_kind,
            &self.clinical_start_value,
            self.clinical_end_kind.as_deref(),
            self.clinical_end_value.as_deref(),
            self.clinical_source_text,
        )?;
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let evidence_assurance = parse_evidence_assurance(&self.evidence_assurance)?;
        let current_assurance =
            parse_current_assurance(self.current_verification.as_deref(), evidence_assurance)?;
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let author = AuthorIdentity {
            kind: parse_author_kind(&self.author_kind)?,
            identifier: self.author_identifier,
        };
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            author,
            evidence_assurance,
        )?;
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::VitalMeasurement(vital),
            },
        })
    }
}

fn parse_evidence_assurance(value: &str) -> Result<EvidenceAssurance> {
    match value {
        "parser_validated" => Ok(EvidenceAssurance::ParserValidated),
        "unverified" => Ok(EvidenceAssurance::Unverified),
        "source_verified" => Ok(EvidenceAssurance::SourceVerified),
        "explicit_self_report" => Ok(EvidenceAssurance::ExplicitSelfReport),
        _ => Err(invalid_fact_store()),
    }
}

fn parse_current_assurance(
    verification: Option<&str>,
    original: EvidenceAssurance,
) -> Result<EvidenceAssurance> {
    match verification {
        Some("verified") => Ok(EvidenceAssurance::SourceVerified),
        Some("discrepancy") | None => Ok(original),
        Some(_) => Err(invalid_fact_store()),
    }
}

fn parse_stored_vital(row: &StoredVitalRow) -> Result<VitalMeasurement> {
    let primary_value = Decimal::from_str(&row.primary_value).map_err(|_| invalid_fact_store())?;
    match (
        row.vital_kind.as_str(),
        row.secondary_value.as_deref(),
        row.original_secondary.as_deref(),
    ) {
        ("blood_pressure", Some(secondary), Some(original_diastolic)) => {
            Ok(VitalMeasurement::BloodPressure {
                systolic: primary_value,
                diastolic: Decimal::from_str(secondary).map_err(|_| invalid_fact_store())?,
                original_systolic: row.original_primary.clone(),
                original_diastolic: original_diastolic.to_owned(),
                units: row.units.clone(),
                measured_by: row.measured_by.clone(),
                note: row.note.clone(),
            })
        }
        ("pulse", None, None) => Ok(VitalMeasurement::Pulse {
            value: primary_value,
            original: row.original_primary.clone(),
            units: row.units.clone(),
            measured_by: row.measured_by.clone(),
            note: row.note.clone(),
        }),
        ("weight", None, None) if row.measured_by.is_none() => Ok(VitalMeasurement::Weight {
            value: primary_value,
            original: row.original_primary.clone(),
            units: row.units.clone(),
            note: row.note.clone(),
        }),
        _ => Err(invalid_fact_store()),
    }
}

fn parse_stored_clinical_time_values(
    time_kind: &str,
    start_kind: &str,
    start_value: &str,
    end_kind: Option<&str>,
    end_value: Option<&str>,
    source_text: Option<String>,
) -> Result<ClinicalTime> {
    match (time_kind, start_kind, end_kind, end_value) {
        ("undated", "undated", None, None) if start_value.is_empty() => {
            Ok(ClinicalTime::Undated { source_text })
        }
        ("instant", "instant", None, None) => Ok(ClinicalTime::Instant {
            value: DateTime::parse_from_rfc3339(start_value).map_err(|_| invalid_fact_store())?,
            source_text,
        }),
        ("date", "date", None, None) => Ok(ClinicalTime::Date {
            value: NaiveDate::parse_from_str(start_value, "%Y-%m-%d")
                .map_err(|_| invalid_fact_store())?,
            source_text,
        }),
        ("partial_date", start_kind, None, None) => Ok(ClinicalTime::PartialDate {
            value: parse_partial_date(start_kind, start_value)?,
            source_text,
        }),
        ("interval", start_kind, Some(end_kind), Some(end_value)) => Ok(ClinicalTime::Interval {
            start: parse_clinical_point(start_kind, start_value)?,
            end: parse_clinical_point(end_kind, end_value)?,
            source_text,
        }),
        _ => Err(invalid_fact_store()),
    }
}

struct StoredLabRow {
    id: String,
    clinical_time_kind: String,
    clinical_start_value: String,
    clinical_source_text: Option<String>,
    recorded_at: String,
    source_id: Option<String>,
    source_region_locator: Option<String>,
    extraction_run_id: Option<String>,
    evidence_assurance: String,
    author_kind: String,
    author_identifier: String,
    record_revision: i64,
    test_name: String,
    numeric_value: Option<String>,
    text_value: Option<String>,
    units: Option<String>,
    reference_range_original: Option<String>,
    reference_range_lower: Option<String>,
    reference_range_upper: Option<String>,
    reported_flag: Option<String>,
    performing_lab: Option<String>,
    current_verification: Option<String>,
    unresolved_discrepancy_id: Option<String>,
    conflicting_reconciliation_id: Option<String>,
    record_hazard_id: Option<String>,
    clinical_start_kind: String,
    clinical_end_kind: Option<String>,
    clinical_end_value: Option<String>,
    cited_source_id: Option<String>,
    cited_source_region_locator: Option<String>,
    canonical_test_identifier: Option<String>,
    canonical_test_display_name: Option<String>,
    numeric_comparator: Option<String>,
    numeric_original: Option<String>,
}

impl StoredLabRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            clinical_time_kind: row.get(1)?,
            clinical_start_value: row.get(2)?,
            clinical_source_text: row.get(3)?,
            recorded_at: row.get(4)?,
            source_id: row.get(5)?,
            source_region_locator: row.get(6)?,
            extraction_run_id: row.get(7)?,
            evidence_assurance: row.get(8)?,
            author_kind: row.get(9)?,
            author_identifier: row.get(10)?,
            record_revision: row.get(11)?,
            test_name: row.get(12)?,
            numeric_value: row.get(13)?,
            text_value: row.get(14)?,
            units: row.get(15)?,
            reference_range_original: row.get(16)?,
            reference_range_lower: row.get(17)?,
            reference_range_upper: row.get(18)?,
            reported_flag: row.get(19)?,
            performing_lab: row.get(20)?,
            current_verification: row.get(21)?,
            unresolved_discrepancy_id: row.get(22)?,
            conflicting_reconciliation_id: row.get(23)?,
            record_hazard_id: row.get(24)?,
            clinical_start_kind: row.get(25)?,
            clinical_end_kind: row.get(26)?,
            clinical_end_value: row.get(27)?,
            cited_source_id: row.get(28)?,
            cited_source_region_locator: row.get(29)?,
            canonical_test_identifier: row.get(30)?,
            canonical_test_display_name: row.get(31)?,
            numeric_comparator: row.get(32)?,
            numeric_original: row.get(33)?,
        })
    }

    fn into_fact(self) -> Result<HealthFact> {
        let clinical_time = parse_stored_clinical_time(&self)?;
        let recorded_at = DateTime::parse_from_rfc3339(&self.recorded_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let evidence_assurance = match self.evidence_assurance.as_str() {
            "parser_validated" => EvidenceAssurance::ParserValidated,
            "unverified" => EvidenceAssurance::Unverified,
            "source_verified" => EvidenceAssurance::SourceVerified,
            "explicit_self_report" => EvidenceAssurance::ExplicitSelfReport,
            _ => return Err(invalid_fact_store()),
        };
        let current_assurance = match self.current_verification.as_deref() {
            Some("verified") => EvidenceAssurance::SourceVerified,
            Some("discrepancy") | None => evidence_assurance,
            Some(_) => return Err(invalid_fact_store()),
        };
        let (disposition, restrictions) = fact_restrictions(
            self.unresolved_discrepancy_id,
            self.conflicting_reconciliation_id,
            self.record_hazard_id,
        );
        let author_kind = parse_author_kind(&self.author_kind)?;
        let value = parse_stored_lab_value(
            self.numeric_value,
            self.text_value,
            self.numeric_comparator,
            self.numeric_original,
        )?;
        let canonical_test = parse_stored_canonical_test(
            self.canonical_test_identifier,
            self.canonical_test_display_name,
        )?;
        let reference_range = parse_stored_reference_range(
            self.reference_range_original,
            self.reference_range_lower,
            self.reference_range_upper,
        )?;
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        let author = AuthorIdentity {
            kind: author_kind,
            identifier: self.author_identifier,
        };
        let evidence = parse_stored_evidence(
            self.source_id,
            self.source_region_locator,
            self.cited_source_id,
            self.cited_source_region_locator,
            author,
            evidence_assurance,
        )?;
        Ok(HealthFact {
            id: FactId::from_stored(self.id),
            extraction_run_id: self.extraction_run_id.map(ExtractionRunId::from_stored),
            record_revision: RecordRevision::from_stored(revision),
            recorded_at,
            original_assurance: evidence_assurance,
            current_assurance,
            disposition,
            restrictions,
            draft: FactDraft {
                clinical_time,
                evidence,
                fact: FactData::LabResult(LabResult {
                    test_name: self.test_name,
                    canonical_test,
                    value,
                    units: self.units,
                    reference_range,
                    reported_flag: self.reported_flag,
                    performing_lab: self.performing_lab,
                }),
            },
        })
    }
}

fn parse_stored_clinical_time(row: &StoredLabRow) -> Result<ClinicalTime> {
    let source_text = row.clinical_source_text.clone();
    match (
        row.clinical_time_kind.as_str(),
        row.clinical_start_kind.as_str(),
        row.clinical_end_kind.as_deref(),
        row.clinical_end_value.as_deref(),
    ) {
        ("undated", "undated", None, None) if row.clinical_start_value.is_empty() => {
            Ok(ClinicalTime::Undated { source_text })
        }
        ("instant", "instant", None, None) => Ok(ClinicalTime::Instant {
            value: DateTime::parse_from_rfc3339(&row.clinical_start_value)
                .map_err(|_| invalid_fact_store())?,
            source_text,
        }),
        ("date", "date", None, None) => Ok(ClinicalTime::Date {
            value: NaiveDate::parse_from_str(&row.clinical_start_value, "%Y-%m-%d")
                .map_err(|_| invalid_fact_store())?,
            source_text,
        }),
        ("partial_date", start_kind, None, None) => Ok(ClinicalTime::PartialDate {
            value: parse_partial_date(start_kind, &row.clinical_start_value)?,
            source_text,
        }),
        ("interval", start_kind, Some(end_kind), Some(end_value)) => Ok(ClinicalTime::Interval {
            start: parse_clinical_point(start_kind, &row.clinical_start_value)?,
            end: parse_clinical_point(end_kind, end_value)?,
            source_text,
        }),
        _ => Err(invalid_fact_store()),
    }
}

fn parse_stored_lab_value(
    numeric_value: Option<String>,
    text_value: Option<String>,
    comparator: Option<String>,
    original: Option<String>,
) -> Result<LabValue> {
    match (numeric_value, text_value, comparator, original) {
        (Some(value), None, None, None) => Ok(LabValue::Numeric {
            value: Decimal::from_str(&value).map_err(|_| invalid_fact_store())?,
        }),
        (Some(value), None, Some(comparator), Some(original)) => Ok(LabValue::QualifiedNumeric {
            comparator: parse_lab_comparator(&comparator)?,
            value: Decimal::from_str(&value).map_err(|_| invalid_fact_store())?,
            original,
        }),
        (None, Some(value), None, None) => Ok(LabValue::Text { value }),
        _ => Err(invalid_fact_store()),
    }
}

fn parse_stored_canonical_test(
    identifier: Option<String>,
    display_name: Option<String>,
) -> Result<Option<CanonicalTest>> {
    match (identifier, display_name) {
        (Some(identifier), Some(display_name)) => Ok(Some(CanonicalTest {
            identifier,
            display_name,
        })),
        (None, None) => Ok(None),
        _ => Err(invalid_fact_store()),
    }
}

fn parse_stored_reference_range(
    original: Option<String>,
    lower: Option<String>,
    upper: Option<String>,
) -> Result<Option<ReferenceRange>> {
    match original {
        Some(original) => Ok(Some(ReferenceRange {
            original,
            lower: parse_optional_decimal(lower)?,
            upper: parse_optional_decimal(upper)?,
        })),
        None if lower.is_none() && upper.is_none() => Ok(None),
        None => Err(invalid_fact_store()),
    }
}

fn parse_stored_evidence(
    source_id: Option<String>,
    source_locator: Option<String>,
    cited_source_id: Option<String>,
    cited_locator: Option<String>,
    author: AuthorIdentity,
    assurance: EvidenceAssurance,
) -> Result<FactEvidence> {
    match (source_id, source_locator, cited_source_id, cited_locator) {
        (Some(source_id), Some(locator), None, None)
            if assurance == EvidenceAssurance::ExplicitSelfReport =>
        {
            Ok(FactEvidence::SourcedSelfReport {
                source_id: SourceId::from_stored(source_id),
                source_region: SourceRegion { locator },
                reporter: author,
            })
        }
        (Some(source_id), Some(locator), None, None) => Ok(FactEvidence::Source {
            source_id: SourceId::from_stored(source_id),
            source_region: SourceRegion { locator },
            asserted_by: author,
        }),
        (Some(source_id), Some(locator), Some(cited_source_id), Some(cited_locator)) => {
            Ok(FactEvidence::ParsedSource {
                parsed_source_id: SourceId::from_stored(source_id),
                parsed_region: SourceRegion { locator },
                cited_source_id: SourceId::from_stored(cited_source_id),
                cited_region: SourceRegion {
                    locator: cited_locator,
                },
                asserted_by: author,
            })
        }
        (None, None, None, None) if assurance == EvidenceAssurance::ExplicitSelfReport => {
            Ok(FactEvidence::SelfReport { reporter: author })
        }
        _ => Err(invalid_fact_store()),
    }
}

fn parse_clinical_point(kind: &str, value: &str) -> Result<ClinicalTimePoint> {
    match kind {
        "instant" => Ok(ClinicalTimePoint::Instant {
            value: DateTime::parse_from_rfc3339(value).map_err(|_| invalid_fact_store())?,
        }),
        "date" => Ok(ClinicalTimePoint::Date {
            value: NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| invalid_fact_store())?,
        }),
        "partial_year" | "partial_month" => Ok(ClinicalTimePoint::PartialDate {
            value: parse_partial_date(kind, value)?,
        }),
        _ => Err(invalid_fact_store()),
    }
}

fn parse_partial_date(kind: &str, value: &str) -> Result<PartialDate> {
    let partial = match kind {
        "partial_year" => PartialDate::Year {
            year: value.parse().map_err(|_| invalid_fact_store())?,
        },
        "partial_month" => {
            let (year, month) = value.split_once('-').ok_or_else(invalid_fact_store)?;
            PartialDate::Month {
                year: year.parse().map_err(|_| invalid_fact_store())?,
                month: month.parse().map_err(|_| invalid_fact_store())?,
            }
        }
        _ => return Err(invalid_fact_store()),
    };
    partial_date_sort_key(&partial).map_err(|_| invalid_fact_store())?;
    Ok(partial)
}

fn fact_restrictions(
    discrepancy_id: Option<String>,
    reconciliation_id: Option<String>,
    hazard_id: Option<String>,
) -> (FactDisposition, Vec<FactRestriction>) {
    let mut restrictions = Vec::new();
    if let Some(verification_id) = discrepancy_id {
        restrictions.push(FactRestriction::SourceDiscrepancy {
            verification_id: VerificationId::from_stored(verification_id),
        });
    }
    if let Some(reconciliation_id) = reconciliation_id {
        restrictions.push(FactRestriction::ConflictingReconciliation {
            reconciliation_id: ReconciliationId::from_stored(reconciliation_id),
        });
    }
    if let Some(hazard_id) = hazard_id {
        restrictions.push(FactRestriction::RecordHazard {
            hazard_id: RecordHazardId::from_stored(hazard_id),
        });
    }
    let disposition = if restrictions.is_empty() {
        FactDisposition::Active
    } else {
        FactDisposition::Quarantined
    };
    (disposition, restrictions)
}

struct StoredVerificationRow {
    id: String,
    source_id: String,
    source_region_locator: String,
    verifier_kind: String,
    verifier_identifier: String,
    method_name: String,
    method_version: String,
    checked_at: String,
    scope: String,
    outcome: String,
    record_revision: i64,
}

impl StoredVerificationRow {
    fn into_verification(self, fact_id: &FactId) -> Result<Verification> {
        let verifier_kind = match self.verifier_kind.as_str() {
            "subject" => AuthorKind::Subject,
            "caregiver" => AuthorKind::Caregiver,
            "agent" => AuthorKind::Agent,
            "application" => AuthorKind::Application,
            "clinician" => AuthorKind::Clinician,
            _ => return Err(invalid_fact_store()),
        };
        let outcome = match self.outcome.as_str() {
            "verified" => VerificationOutcome::Verified,
            "discrepancy" => VerificationOutcome::Discrepancy,
            _ => return Err(invalid_fact_store()),
        };
        let checked_at = DateTime::parse_from_rfc3339(&self.checked_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(Verification {
            id: VerificationId::from_stored(self.id),
            record_revision: RecordRevision::from_stored(revision),
            draft: VerificationDraft {
                fact_id: fact_id.clone(),
                source_id: SourceId::from_stored(self.source_id),
                source_region: SourceRegion {
                    locator: self.source_region_locator,
                },
                verifier: AuthorIdentity {
                    kind: verifier_kind,
                    identifier: self.verifier_identifier,
                },
                method: VerificationMethod {
                    name: self.method_name,
                    version: self.method_version,
                },
                checked_at,
                scope: self.scope,
                outcome,
            },
        })
    }
}

struct StoredCorrectionRow {
    id: String,
    replacement_fact_id: String,
    reason: String,
    author_kind: String,
    author_identifier: String,
    corrected_at: String,
    supporting_verification_id: String,
    record_revision: i64,
}

impl StoredCorrectionRow {
    fn into_correction(self, prior_fact_id: &FactId, facts: &[HealthFact]) -> Result<Correction> {
        let replacement = facts
            .iter()
            .find(|fact| fact.id.as_str() == self.replacement_fact_id)
            .ok_or_else(invalid_fact_store)?;
        let author_kind = match self.author_kind.as_str() {
            "subject" => AuthorKind::Subject,
            "caregiver" => AuthorKind::Caregiver,
            "agent" => AuthorKind::Agent,
            "application" => AuthorKind::Application,
            "clinician" => AuthorKind::Clinician,
            _ => return Err(invalid_fact_store()),
        };
        let corrected_at = DateTime::parse_from_rfc3339(&self.corrected_at)
            .map_err(|_| invalid_fact_store())?
            .with_timezone(&Utc);
        let revision = u64::try_from(self.record_revision).map_err(|_| invalid_fact_store())?;
        Ok(Correction {
            id: CorrectionId::from_stored(self.id),
            replacement_fact_id: replacement.id.clone(),
            record_revision: RecordRevision::from_stored(revision),
            draft: CorrectionDraft {
                prior_fact_id: prior_fact_id.clone(),
                replacement: replacement.draft.clone(),
                reason: self.reason,
                author: AuthorIdentity {
                    kind: author_kind,
                    identifier: self.author_identifier,
                },
                corrected_at,
                supporting_verification_id: VerificationId::from_stored(
                    self.supporting_verification_id,
                ),
            },
        })
    }
}

fn parse_optional_decimal(value: Option<String>) -> Result<Option<Decimal>> {
    value
        .map(|value| Decimal::from_str(&value).map_err(|_| invalid_fact_store()))
        .transpose()
}

fn invalid_fact_store() -> Error {
    Error::InvalidWorkspace("invalid stored Health Fact".to_owned())
}

fn insert_issues(
    transaction: &Transaction<'_>,
    run_id: &ExtractionRunId,
    kind: &str,
    issues: &[ExtractionIssue],
) -> Result<()> {
    for (position, issue) in issues.iter().enumerate() {
        transaction.execute(
            "INSERT INTO extraction_run_issues \
             (extraction_run_id, issue_kind, position, code, region_locator) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                run_id.as_str(),
                kind,
                i64::try_from(position)
                    .map_err(|_| Error::InvalidRecordChangeSet("too many extraction issues"))?,
                issue.code,
                issue.region.as_ref().map(|region| region.locator.as_str())
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn load_extraction_runs(
    workspace: &Workspace,
    source_id: &SourceId,
) -> Result<Vec<ExtractionRun>> {
    let mut statement = workspace.connection().prepare(
        "SELECT id, extractor_name, extractor_version, record_revision \
         FROM extraction_runs WHERE source_id = ?1 ORDER BY record_revision, id",
    )?;
    let headers = statement
        .query_map([source_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut runs = Vec::with_capacity(headers.len());
    for (id, extractor_name, extractor_version, stored_revision) in headers {
        let run_id = ExtractionRunId::from_stored(id);
        let revision_value = u64::try_from(stored_revision)
            .map_err(|_| Error::InvalidWorkspace("invalid record revision".to_owned()))?;
        runs.push(ExtractionRun {
            coverage: ExtractionCoverage {
                regions: load_regions(workspace, &run_id)?,
                domains: load_domains(workspace, &run_id)?,
            },
            omissions: load_issues(workspace, &run_id, "omission")?,
            failures: load_issues(workspace, &run_id, "failure")?,
            resulting_candidate_keys: load_candidates(workspace, &run_id)?,
            id: run_id,
            source_id: source_id.clone(),
            extractor: ExtractorIdentity {
                name: extractor_name,
                version: extractor_version,
            },
            record_revision: RecordRevision::from_stored(revision_value),
        });
    }
    Ok(runs)
}

fn load_regions(workspace: &Workspace, run_id: &ExtractionRunId) -> Result<Vec<SourceRegion>> {
    let mut statement = workspace.connection().prepare(
        "SELECT locator FROM extraction_run_regions \
         WHERE extraction_run_id = ?1 ORDER BY position",
    )?;
    statement
        .query_map([run_id.as_str()], |row| {
            Ok(SourceRegion {
                locator: row.get(0)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)
}

fn load_domains(workspace: &Workspace, run_id: &ExtractionRunId) -> Result<Vec<ExtractionDomain>> {
    let mut statement = workspace.connection().prepare(
        "SELECT domain FROM extraction_run_domains \
         WHERE extraction_run_id = ?1 ORDER BY position",
    )?;
    let stored = statement
        .query_map([run_id.as_str()], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    stored
        .iter()
        .map(|value| ExtractionDomain::from_stored(value))
        .collect()
}

fn load_issues(
    workspace: &Workspace,
    run_id: &ExtractionRunId,
    kind: &str,
) -> Result<Vec<ExtractionIssue>> {
    let mut statement = workspace.connection().prepare(
        "SELECT code, region_locator FROM extraction_run_issues \
         WHERE extraction_run_id = ?1 AND issue_kind = ?2 ORDER BY position",
    )?;
    statement
        .query_map(params![run_id.as_str(), kind], |row| {
            let locator: Option<String> = row.get(1)?;
            Ok(ExtractionIssue {
                code: row.get(0)?,
                region: locator.map(|locator| SourceRegion { locator }),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)
}

fn load_candidates(workspace: &Workspace, run_id: &ExtractionRunId) -> Result<Vec<String>> {
    let mut statement = workspace.connection().prepare(
        "SELECT candidate_key FROM extraction_run_candidates \
         WHERE extraction_run_id = ?1 ORDER BY position",
    )?;
    statement
        .query_map([run_id.as_str()], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
