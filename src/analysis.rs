use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{OptionalExtension, Transaction, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::record::clinical_time_order_key;
use crate::{
    AnalysisId, CareTask, CareTaskStatus, ClaimId, ConditionAssertion, ConditionAssertionState,
    DiagnosticStudy, DiagnosticStudyStatus, Error, EvidenceAssurance, FactData, FactId, FactQuery,
    FactRestriction, HealthFact, LabValue, MedicationEvent, MedicationEventKind,
    ReconciliationAgreement, RecordRevision, Result, SubjectPreference, SubjectPreferenceState,
    VerificationId, VerificationOutcome, VitalMeasurement, Workspace,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Finding,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    Low,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AnalysisClaim {
    pub id: ClaimId,
    pub kind: ClaimKind,
    pub key: String,
    pub statement: String,
    pub confidence: ConfidenceLevel,
    pub evidence_fact_ids: Vec<FactId>,
    pub evidence_verification_ids: Vec<VerificationId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AnalysisWarning {
    pub code: String,
    pub related_fact_ids: Vec<FactId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Analysis {
    pub schema_version: u32,
    pub id: AnalysisId,
    pub record_revision: RecordRevision,
    pub created_at: DateTime<Utc>,
    pub engine_version: String,
    pub freshness: AnalysisFreshness,
    pub claims: Vec<AnalysisClaim>,
    pub warnings: Vec<AnalysisWarning>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AnalysisOptions {
    pub include_unverified: bool,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            include_unverified: true,
        }
    }
}

pub struct Analyses<'workspace> {
    workspace: &'workspace Workspace,
}

impl<'workspace> Analyses<'workspace> {
    pub(crate) fn new(workspace: &'workspace Workspace) -> Self {
        Self { workspace }
    }

    pub fn status(&self, analysis_id: &AnalysisId) -> Result<AnalysisFreshness> {
        let stored: Option<String> = self
            .workspace
            .connection()
            .query_row(
                "SELECT freshness FROM analyses WHERE id = ?1",
                [analysis_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        match stored.as_deref() {
            Some("fresh") => Ok(AnalysisFreshness::Fresh),
            Some("stale") => Ok(AnalysisFreshness::Stale),
            Some(_) => Err(Error::InvalidWorkspace(
                "invalid Analysis freshness".to_owned(),
            )),
            None => Err(Error::AnalysisNotFound(analysis_id.as_str().to_owned())),
        }
    }

    pub fn show(&self, analysis_id: &AnalysisId) -> Result<Analysis> {
        let metadata: Option<(i64, String, String, String)> = self
            .workspace
            .connection()
            .query_row(
                "SELECT record_revision, created_at, engine_version, freshness \
                 FROM analyses WHERE id = ?1",
                [analysis_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let (stored_revision, created_at, engine_version, freshness) =
            metadata.ok_or_else(|| Error::AnalysisNotFound(analysis_id.as_str().to_owned()))?;
        let revision = u64::try_from(stored_revision)
            .map_err(|_| Error::InvalidWorkspace("invalid Analysis revision".to_owned()))?;
        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|_| Error::InvalidWorkspace("invalid Analysis time".to_owned()))?
            .with_timezone(&Utc);
        let freshness = parse_freshness(&freshness)?;
        Ok(Analysis {
            schema_version: 1,
            id: analysis_id.clone(),
            record_revision: RecordRevision::from_stored(revision),
            created_at,
            engine_version,
            freshness,
            claims: self.load_claims(analysis_id)?,
            warnings: self.load_warnings(analysis_id)?,
        })
    }

    fn load_claims(&self, analysis_id: &AnalysisId) -> Result<Vec<AnalysisClaim>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT id, claim_kind, claim_key, statement, confidence FROM analysis_claims \
             WHERE analysis_id = ?1 ORDER BY position",
        )?;
        let rows = statement
            .query_map([analysis_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, kind, key, statement, confidence)| {
                let claim_id = ClaimId::from_stored(id);
                Ok(AnalysisClaim {
                    evidence_fact_ids: load_fact_edges(self.workspace, &claim_id)?,
                    evidence_verification_ids: load_verification_edges(self.workspace, &claim_id)?,
                    id: claim_id,
                    kind: match kind.as_str() {
                        "finding" => ClaimKind::Finding,
                        _ => return Err(Error::InvalidWorkspace("invalid Claim kind".to_owned())),
                    },
                    key,
                    statement,
                    confidence: parse_confidence(&confidence)?,
                })
            })
            .collect()
    }

    fn load_warnings(&self, analysis_id: &AnalysisId) -> Result<Vec<AnalysisWarning>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT position, code, related_fact_id FROM analysis_warnings \
             WHERE analysis_id = ?1 ORDER BY position, fact_position",
        )?;
        let rows = statement
            .query_map([analysis_id.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut warnings: Vec<AnalysisWarning> = Vec::new();
        let mut current_position = None;
        for (position, code, fact_id) in rows {
            if current_position == Some(position) {
                let warning = warnings.last_mut().ok_or_else(|| {
                    Error::InvalidWorkspace("invalid Analysis warning".to_owned())
                })?;
                if warning.code != code {
                    return Err(Error::InvalidWorkspace(
                        "invalid Analysis warning".to_owned(),
                    ));
                }
                warning.related_fact_ids.push(FactId::from_stored(fact_id));
            } else {
                warnings.push(AnalysisWarning {
                    code,
                    related_fact_ids: vec![FactId::from_stored(fact_id)],
                });
                current_position = Some(position);
            }
        }
        Ok(warnings)
    }
}

fn evidence_restrictions(
    workspace: &Workspace,
    facts: &[HealthFact],
) -> Result<(HashSet<String>, Vec<AnalysisWarning>)> {
    let mut restricted_fact_ids = HashSet::new();
    let mut seen_conflicts = HashSet::new();
    let mut seen_hazards = HashSet::new();
    let mut warnings = Vec::new();
    for fact in facts {
        for reconciliation in workspace.record().reconciliations(&fact.id)? {
            if reconciliation.draft.agreement == ReconciliationAgreement::Conflicting
                && seen_conflicts.insert(reconciliation.id.as_str().to_owned())
            {
                restricted_fact_ids.extend(
                    reconciliation
                        .draft
                        .fact_ids
                        .iter()
                        .map(|id| id.as_str().to_owned()),
                );
                warnings.push(AnalysisWarning {
                    code: "conflicting_evidence".to_owned(),
                    related_fact_ids: reconciliation.draft.fact_ids,
                });
            }
        }
        for restriction in &fact.restrictions {
            let FactRestriction::RecordHazard { hazard_id } = restriction else {
                continue;
            };
            if seen_hazards.insert(hazard_id.as_str().to_owned()) {
                let hazard = workspace
                    .record()
                    .hazards(&fact.id)?
                    .into_iter()
                    .find(|hazard| hazard.id == *hazard_id)
                    .ok_or_else(|| {
                        Error::InvalidWorkspace("missing Record Hazard restriction".to_owned())
                    })?;
                restricted_fact_ids.extend(
                    hazard
                        .draft
                        .affected_fact_ids
                        .iter()
                        .map(|id| id.as_str().to_owned()),
                );
                warnings.push(AnalysisWarning {
                    code: "record_hazard".to_owned(),
                    related_fact_ids: hazard.draft.affected_fact_ids,
                });
            }
        }
    }
    Ok((restricted_fact_ids, warnings))
}

pub(crate) fn run(workspace: &Workspace, options: AnalysisOptions) -> Result<Analysis> {
    let status = workspace.status()?;
    let facts = workspace.record().query(FactQuery::default())?;
    let mut claims = Vec::new();
    let (restricted_fact_ids, mut warnings) = evidence_restrictions(workspace, &facts)?;
    for fact in &facts {
        if restricted_fact_ids.contains(fact.id.as_str()) {
            continue;
        }
        let (claim, warning) = analyze_fact(workspace, fact, options)?;
        if let Some(claim) = claim {
            claims.push(claim);
        }
        if let Some(warning) = warning {
            warnings.push(warning);
        }
    }
    let (timeline_claims, timeline_warnings) =
        analyze_lab_timelines(workspace, &facts, &restricted_fact_ids, options)?;
    claims.extend(timeline_claims);
    warnings.extend(timeline_warnings);
    let engine_version = env!("CARGO_PKG_VERSION").to_owned();
    let analysis_seed = json!({
        "schema_version": 1,
        "record_revision": status.record_revision,
        "engine_version": engine_version,
        "claims": claims,
        "warnings": warnings,
    });
    let analysis_id =
        AnalysisId::from_content_hash(format!("analysis-{}", digest_json(&analysis_seed)?));
    let analysis = Analysis {
        schema_version: 1,
        id: analysis_id,
        record_revision: status.record_revision,
        created_at: workspace.now(),
        engine_version,
        freshness: AnalysisFreshness::Fresh,
        claims,
        warnings,
    };
    persist(workspace, &analysis)?;
    Ok(analysis)
}

fn analyze_lab_timelines(
    workspace: &Workspace,
    facts: &[HealthFact],
    restricted_fact_ids: &HashSet<String>,
    options: AnalysisOptions,
) -> Result<(Vec<AnalysisClaim>, Vec<AnalysisWarning>)> {
    let mut grouped: BTreeMap<String, Vec<(&HealthFact, &crate::LabResult)>> = BTreeMap::new();
    for fact in facts {
        if restricted_fact_ids.contains(fact.id.as_str())
            || !fact.restrictions.is_empty()
            || (fact.current_assurance == EvidenceAssurance::Unverified
                && !options.include_unverified)
        {
            continue;
        }
        let FactData::LabResult(lab) = &fact.draft.fact else {
            continue;
        };
        let Some(canonical) = &lab.canonical_test else {
            continue;
        };
        grouped
            .entry(canonical.identifier.clone())
            .or_default()
            .push((fact, lab));
    }

    let mut claims = Vec::new();
    let mut warnings = Vec::new();
    for (canonical_id, timeline) in grouped {
        let (group_claims, warning) = analyze_lab_group(workspace, &canonical_id, timeline)?;
        claims.extend(group_claims);
        warnings.extend(warning);
    }
    Ok((claims, warnings))
}

fn analyze_lab_group<'fact>(
    workspace: &Workspace,
    canonical_id: &str,
    mut timeline: Vec<(&'fact HealthFact, &'fact crate::LabResult)>,
) -> Result<(Vec<AnalysisClaim>, Option<AnalysisWarning>)> {
    timeline.sort_by(|(left, _), (right, _)| {
        clinical_time_order_key(&left.draft.clinical_time)
            .cmp(&clinical_time_order_key(&right.draft.clinical_time))
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    let numeric = timeline
        .iter()
        .filter_map(|(fact, lab)| match &lab.value {
            LabValue::Numeric { value } => Some((*fact, *lab, *value)),
            LabValue::QualifiedNumeric { .. } | LabValue::Text { .. } => None,
        })
        .collect::<Vec<_>>();
    if numeric.len() < 2 {
        return Ok((Vec::new(), None));
    }
    let units = numeric
        .iter()
        .map(|(_, lab, _)| lab.units.clone())
        .collect::<HashSet<_>>();
    if units.len() != 1 {
        return Ok((
            Vec::new(),
            Some(AnalysisWarning {
                code: "lab_units_not_comparable".to_owned(),
                related_fact_ids: numeric.iter().map(|(fact, _, _)| fact.id.clone()).collect(),
            }),
        ));
    }
    let previous = &numeric[numeric.len() - 2];
    let latest = &numeric[numeric.len() - 1];
    let direction = match latest.2.cmp(&previous.2) {
        std::cmp::Ordering::Greater => "increased",
        std::cmp::Ordering::Less => "decreased",
        std::cmp::Ordering::Equal => "was unchanged",
    };
    let display_name = latest
        .1
        .canonical_test
        .as_ref()
        .map_or(latest.1.test_name.as_str(), |test| {
            test.display_name.as_str()
        });
    let unit_suffix = latest
        .1
        .units
        .as_ref()
        .map_or_else(String::new, |units| format!(" {units}"));
    let statement = format!(
        "{display_name} {direction} from {} to {}{unit_suffix}",
        previous.2, latest.2
    );
    let trend = timeline_claim(
        workspace,
        format!("lab_trend:{canonical_id}"),
        statement,
        vec![previous.0, latest.0],
        vec![previous.0.id.clone(), latest.0.id.clone()],
    )?;
    let mut claims = vec![trend];
    if numeric.len() >= 3 {
        let before_previous = &numeric[numeric.len() - 3];
        let first_direction = previous.2.cmp(&before_previous.2);
        let second_direction = latest.2.cmp(&previous.2);
        if first_direction != std::cmp::Ordering::Equal
            && second_direction != std::cmp::Ordering::Equal
            && first_direction != second_direction
        {
            claims.push(timeline_claim(
                workspace,
                format!("lab_reversal:{canonical_id}"),
                format!("{display_name} reversed its latest measured direction"),
                vec![before_previous.0, previous.0, latest.0],
                vec![
                    before_previous.0.id.clone(),
                    previous.0.id.clone(),
                    latest.0.id.clone(),
                ],
            )?);
        }
    }
    Ok((claims, None))
}

fn timeline_claim(
    workspace: &Workspace,
    key: String,
    statement: String,
    facts: Vec<&HealthFact>,
    evidence_fact_ids: Vec<FactId>,
) -> Result<AnalysisClaim> {
    let confidence = if facts.iter().any(|fact| {
        matches!(
            fact.current_assurance,
            EvidenceAssurance::ExplicitSelfReport | EvidenceAssurance::Unverified
        )
    }) {
        ConfidenceLevel::Low
    } else {
        ConfidenceLevel::High
    };
    let mut evidence_verification_ids = Vec::new();
    for fact in facts {
        if let Some(verification) = workspace
            .record()
            .verifications(&fact.id)?
            .into_iter()
            .rev()
            .find(|verification| verification.draft.outcome == VerificationOutcome::Verified)
        {
            evidence_verification_ids.push(verification.id);
        }
    }
    let seed = json!({
        "kind": "finding",
        "key": key,
        "statement": statement,
        "confidence": confidence,
        "fact_ids": evidence_fact_ids,
        "verification_ids": evidence_verification_ids,
    });
    Ok(AnalysisClaim {
        id: ClaimId::from_content_hash(format!("claim-{}", digest_json(&seed)?)),
        kind: ClaimKind::Finding,
        key,
        statement,
        confidence,
        evidence_fact_ids,
        evidence_verification_ids,
    })
}

fn analyze_fact(
    workspace: &Workspace,
    fact: &HealthFact,
    options: AnalysisOptions,
) -> Result<(Option<AnalysisClaim>, Option<AnalysisWarning>)> {
    let restriction_warning = fact.restrictions.iter().find_map(|restriction| {
        let code = match restriction {
            FactRestriction::SourceContentChanged { .. } => "source_content_changed",
            FactRestriction::SourceContentUnavailable { .. } => "source_content_unavailable",
            FactRestriction::SourceDiscrepancy { .. } => "quarantined_evidence",
            FactRestriction::ConflictingReconciliation { .. }
            | FactRestriction::RecordHazard { .. } => return None,
        };
        Some(AnalysisWarning {
            code: code.to_owned(),
            related_fact_ids: vec![fact.id.clone()],
        })
    });
    if restriction_warning.is_some()
        || (fact.current_assurance == EvidenceAssurance::Unverified && !options.include_unverified)
    {
        return Ok((None, restriction_warning));
    }
    let evidence_verification_ids = workspace
        .record()
        .verifications(&fact.id)?
        .iter()
        .rev()
        .find(|verification| verification.draft.outcome == VerificationOutcome::Verified)
        .map(|verification| vec![verification.id.clone()])
        .unwrap_or_default();
    let confidence = confidence_for_assurance(fact.current_assurance);
    let (key_prefix, test_name, value, units) = match &fact.draft.fact {
        FactData::LabResult(lab) => {
            let value = match &lab.value {
                LabValue::Numeric { value } => value.to_string(),
                LabValue::QualifiedNumeric { original, .. } => original.clone(),
                LabValue::Text { value } => value.clone(),
            };
            (
                "lab_observation",
                lab.test_name.clone(),
                value,
                lab.units.clone(),
            )
        }
        FactData::VitalMeasurement(vital) => vital_claim_parts(vital),
        FactData::MedicationEvent(event) => medication_claim_parts(event),
        FactData::ConditionAssertion(assertion) => condition_claim_parts(assertion),
        FactData::DiagnosticStudy(study) => diagnostic_study_claim_parts(study),
        FactData::CareTask(task) => care_task_claim_parts(task),
        FactData::SubjectPreference(preference) => subject_preference_claim_parts(preference),
    };
    let mut statement = units.map_or_else(
        || format!("{test_name}: {value}"),
        |units| format!("{test_name}: {value} {units}"),
    );
    if fact.current_assurance == EvidenceAssurance::ExplicitSelfReport {
        statement = format!("Self-reported: {statement}");
    }
    let key = format!("{key_prefix}:{}", fact.id.as_str());
    let claim_seed = json!({
        "kind": "finding",
        "key": key,
        "statement": statement,
        "confidence": confidence,
        "fact_id": fact.id.as_str(),
        "verification_ids": evidence_verification_ids,
    });
    let claim_id = ClaimId::from_content_hash(format!("claim-{}", digest_json(&claim_seed)?));
    let warning =
        (fact.current_assurance == EvidenceAssurance::Unverified).then(|| AnalysisWarning {
            code: "unverified_evidence".to_owned(),
            related_fact_ids: vec![fact.id.clone()],
        });
    Ok((
        Some(AnalysisClaim {
            id: claim_id,
            kind: ClaimKind::Finding,
            key,
            statement,
            confidence,
            evidence_fact_ids: vec![fact.id.clone()],
            evidence_verification_ids,
        }),
        warning,
    ))
}

fn care_task_claim_parts(value: &CareTask) -> (&'static str, String, String, Option<String>) {
    let status = match value.status {
        CareTaskStatus::Pending => "pending",
        CareTaskStatus::Due => "due",
        CareTaskStatus::Overdue => "overdue",
        CareTaskStatus::AwaitingResult => "awaiting result",
        CareTaskStatus::Completed => "completed",
        CareTaskStatus::Cancelled => "cancelled",
    };
    (
        "care_task",
        value.action.clone(),
        format!("{status}: {}", value.source_text),
        None,
    )
}

fn subject_preference_claim_parts(
    value: &SubjectPreference,
) -> (&'static str, String, String, Option<String>) {
    let state = match value.state {
        SubjectPreferenceState::Active => "active",
        SubjectPreferenceState::Withdrawn => "withdrawn",
    };
    (
        "subject_preference",
        value.statement.clone(),
        format!("{state}: {}", value.source_text),
        None,
    )
}

fn diagnostic_study_claim_parts(
    value: &DiagnosticStudy,
) -> (&'static str, String, String, Option<String>) {
    let status = match value.status {
        DiagnosticStudyStatus::Ordered => "ordered",
        DiagnosticStudyStatus::Scheduled => "scheduled",
        DiagnosticStudyStatus::Performed => "performed",
        DiagnosticStudyStatus::Preliminary => "preliminary",
        DiagnosticStudyStatus::Final => "final",
        DiagnosticStudyStatus::Amended => "amended",
        DiagnosticStudyStatus::Cancelled => "cancelled",
    };
    let result = value.impression.clone().unwrap_or_else(|| {
        value
            .findings
            .iter()
            .map(|finding| finding.text.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    });
    let detail = if result.is_empty() {
        status.to_owned()
    } else {
        format!("{status}: {result}")
    };
    ("diagnostic_study", value.name.clone(), detail, None)
}

fn condition_claim_parts(
    value: &ConditionAssertion,
) -> (&'static str, String, String, Option<String>) {
    let state = match value.state {
        ConditionAssertionState::Suspected => "suspected",
        ConditionAssertionState::Confirmed => "confirmed",
        ConditionAssertionState::RuledOut => "ruled out",
        ConditionAssertionState::Inactive => "inactive",
        ConditionAssertionState::Resolved => "resolved",
    };
    (
        "condition_assertion",
        value.name.clone(),
        format!("{state}: {}", value.assertion_text),
        None,
    )
}

fn medication_claim_parts(
    value: &MedicationEvent,
) -> (&'static str, String, String, Option<String>) {
    let event = match value.event {
        MedicationEventKind::RegimenReported => "regimen reported",
        MedicationEventKind::Started => "started",
        MedicationEventKind::Stopped => "stopped",
        MedicationEventKind::DoseChanged => "dose changed",
        MedicationEventKind::ScheduleChanged => "schedule changed",
        MedicationEventKind::MissedDose => "missed dose",
        MedicationEventKind::AsNeededUse => "as-needed use",
    };
    let detail = value.dose.as_ref().map_or_else(
        || event.to_owned(),
        |dose| format!("{event}: {}", dose.original),
    );
    (
        "medication_event",
        value.name.clone(),
        detail,
        value.schedule.clone(),
    )
}

fn vital_claim_parts(value: &VitalMeasurement) -> (&'static str, String, String, Option<String>) {
    match value {
        VitalMeasurement::BloodPressure {
            original_systolic,
            original_diastolic,
            units,
            ..
        } => (
            "vital_observation",
            "Blood pressure".to_owned(),
            format!("{original_systolic}/{original_diastolic}"),
            units.clone(),
        ),
        VitalMeasurement::Pulse {
            original, units, ..
        } => (
            "vital_observation",
            "Pulse".to_owned(),
            original.clone(),
            units.clone(),
        ),
        VitalMeasurement::Weight {
            original, units, ..
        } => (
            "vital_observation",
            "Weight".to_owned(),
            original.clone(),
            units.clone(),
        ),
    }
}

fn confidence_for_assurance(assurance: EvidenceAssurance) -> ConfidenceLevel {
    match assurance {
        EvidenceAssurance::ParserValidated | EvidenceAssurance::SourceVerified => {
            ConfidenceLevel::High
        }
        EvidenceAssurance::ExplicitSelfReport | EvidenceAssurance::Unverified => {
            ConfidenceLevel::Low
        }
    }
}

fn persist(workspace: &Workspace, analysis: &Analysis) -> Result<()> {
    let transaction = workspace.connection().unchecked_transaction()?;
    transaction.execute(
        "INSERT OR IGNORE INTO analyses \
         (id, record_revision, created_at, engine_version, freshness) \
         VALUES (?1, ?2, ?3, ?4, 'fresh')",
        params![
            analysis.id.as_str(),
            i64::try_from(analysis.record_revision.value())
                .map_err(|_| Error::InvalidWorkspace("invalid record revision".to_owned()))?,
            analysis
                .created_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            analysis.engine_version
        ],
    )?;
    for (position, claim) in analysis.claims.iter().enumerate() {
        transaction.execute(
            "INSERT OR IGNORE INTO analysis_claims \
             (id, analysis_id, position, claim_kind, claim_key, statement, confidence) \
             VALUES (?1, ?2, ?3, 'finding', ?4, ?5, ?6)",
            params![
                claim.id.as_str(),
                analysis.id.as_str(),
                i64::try_from(position)
                    .map_err(|_| Error::InvalidWorkspace("too many Analysis claims".to_owned()))?,
                claim.key,
                claim.statement,
                confidence_name(claim.confidence)
            ],
        )?;
        for (edge_position, fact_id) in claim.evidence_fact_ids.iter().enumerate() {
            transaction.execute(
                "INSERT OR IGNORE INTO analysis_claim_fact_edges (claim_id, position, fact_id) \
                 VALUES (?1, ?2, ?3)",
                params![
                    claim.id.as_str(),
                    i64::try_from(edge_position).map_err(|_| Error::InvalidWorkspace(
                        "too many Analysis edges".to_owned()
                    ))?,
                    fact_id.as_str()
                ],
            )?;
        }
        for (edge_position, verification_id) in claim.evidence_verification_ids.iter().enumerate() {
            transaction.execute(
                "INSERT OR IGNORE INTO analysis_claim_verification_edges \
                 (claim_id, position, verification_id) VALUES (?1, ?2, ?3)",
                params![
                    claim.id.as_str(),
                    i64::try_from(edge_position).map_err(|_| Error::InvalidWorkspace(
                        "too many Analysis edges".to_owned()
                    ))?,
                    verification_id.as_str()
                ],
            )?;
        }
    }
    for (position, warning) in analysis.warnings.iter().enumerate() {
        for (fact_position, fact_id) in warning.related_fact_ids.iter().enumerate() {
            transaction.execute(
                "INSERT OR IGNORE INTO analysis_warnings \
                 (analysis_id, position, fact_position, code, related_fact_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    analysis.id.as_str(),
                    i64::try_from(position).map_err(|_| Error::InvalidWorkspace(
                        "too many Analysis warnings".to_owned()
                    ))?,
                    i64::try_from(fact_position).map_err(|_| Error::InvalidWorkspace(
                        "too many Analysis warning edges".to_owned()
                    ))?,
                    warning.code,
                    fact_id.as_str()
                ],
            )?;
        }
    }
    transaction.commit()?;
    Ok(())
}

pub(crate) fn mark_fresh_analyses_stale(
    transaction: &Transaction<'_>,
    next_revision: RecordRevision,
) -> Result<Vec<AnalysisId>> {
    let mut statement = transaction.prepare(
        "SELECT id FROM analyses WHERE freshness = 'fresh' AND record_revision < ?1 ORDER BY id",
    )?;
    let ids = statement
        .query_map(
            [i64::try_from(next_revision.value())
                .map_err(|_| Error::InvalidWorkspace("invalid record revision".to_owned()))?],
            |row| row.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(statement);
    transaction.execute(
        "UPDATE analyses SET freshness = 'stale' \
         WHERE freshness = 'fresh' AND record_revision < ?1",
        [i64::try_from(next_revision.value())
            .map_err(|_| Error::InvalidWorkspace("invalid record revision".to_owned()))?],
    )?;
    Ok(ids.into_iter().map(AnalysisId::from_stored).collect())
}

fn confidence_name(value: ConfidenceLevel) -> &'static str {
    match value {
        ConfidenceLevel::Low => "low",
        ConfidenceLevel::High => "high",
    }
}

fn parse_confidence(value: &str) -> Result<ConfidenceLevel> {
    match value {
        "low" => Ok(ConfidenceLevel::Low),
        "high" => Ok(ConfidenceLevel::High),
        _ => Err(Error::InvalidWorkspace(
            "invalid Analysis confidence".to_owned(),
        )),
    }
}

fn parse_freshness(value: &str) -> Result<AnalysisFreshness> {
    match value {
        "fresh" => Ok(AnalysisFreshness::Fresh),
        "stale" => Ok(AnalysisFreshness::Stale),
        _ => Err(Error::InvalidWorkspace(
            "invalid Analysis freshness".to_owned(),
        )),
    }
}

fn load_fact_edges(workspace: &Workspace, claim_id: &ClaimId) -> Result<Vec<FactId>> {
    let mut statement = workspace.connection().prepare(
        "SELECT fact_id FROM analysis_claim_fact_edges WHERE claim_id = ?1 ORDER BY position",
    )?;
    statement
        .query_map([claim_id.as_str()], |row| row.get::<_, String>(0))?
        .map(|value| Ok(FactId::from_stored(value?)))
        .collect()
}

fn load_verification_edges(
    workspace: &Workspace,
    claim_id: &ClaimId,
) -> Result<Vec<VerificationId>> {
    let mut statement = workspace.connection().prepare(
        "SELECT verification_id FROM analysis_claim_verification_edges \
         WHERE claim_id = ?1 ORDER BY position",
    )?;
    statement
        .query_map([claim_id.as_str()], |row| row.get::<_, String>(0))?
        .map(|value| Ok(VerificationId::from_stored(value?)))
        .collect()
}

fn digest_json(value: &serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    let digest = Sha256::digest(bytes);
    Ok(lowercase_hex(&digest))
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
