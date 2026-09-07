use rusqlite::{OptionalExtension, Transaction, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

use crate::{
    AnalysisId, AuthorIdentity, AuthorKind, CommitReceipt, Error, ExtractionRunId, FactId,
    RecordRevision, Result, SourceId, SubjectId, Workspace, WorkspaceId,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpungementReason {
    WrongSubject,
    AccidentalSensitiveIngestion,
    PrivacyRequest,
}

impl ExpungementReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::WrongSubject => "wrong_subject",
            Self::AccidentalSensitiveIngestion => "accidental_sensitive_ingestion",
            Self::PrivacyRequest => "privacy_request",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "wrong_subject" => Ok(Self::WrongSubject),
            "accidental_sensitive_ingestion" => Ok(Self::AccidentalSensitiveIngestion),
            "privacy_request" => Ok(Self::PrivacyRequest),
            _ => Err(Error::InvalidWorkspace(
                "invalid Expungement tombstone".to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExpungementRequest {
    pub reason: ExpungementReason,
    pub fact_ids: Vec<FactId>,
    pub source_ids: Vec<SourceId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalRemediationCode {
    SourceFiles,
    Snapshots,
    Backups,
}

#[derive(Debug, Clone)]
pub struct ExpungementPlan {
    id: String,
    request: ExpungementRequest,
    workspace_id: WorkspaceId,
    subject_id: SubjectId,
    record_revision: RecordRevision,
    pub affected_fact_ids: Vec<FactId>,
    pub affected_source_aliases: Vec<String>,
    source_ids: Vec<SourceId>,
    extraction_run_ids: Vec<ExtractionRunId>,
    reconciliation_ids: Vec<String>,
    hazard_ids: Vec<String>,
    analysis_ids: Vec<AnalysisId>,
    commit_keys: Vec<String>,
}

impl ExpungementPlan {
    pub fn confirmation_challenge(&self) -> String {
        format!("EXPUNGE {}", self.id)
    }
}

#[derive(Debug, Clone)]
pub struct ExpungementAuthorization {
    plan_id: String,
    workspace_id: WorkspaceId,
    subject_id: SubjectId,
    record_revision: RecordRevision,
    authorized_by: AuthorIdentity,
}

impl ExpungementAuthorization {
    pub fn confirm(
        plan: &ExpungementPlan,
        response: &str,
        authorized_by: &AuthorIdentity,
    ) -> Result<Self> {
        if response != plan.confirmation_challenge()
            || authorized_by.kind == AuthorKind::Agent
            || !valid_operator_identifier(&authorized_by.identifier)
        {
            return Err(Error::ExpungementAuthorization);
        }
        Ok(Self {
            plan_id: plan.id.clone(),
            workspace_id: plan.workspace_id.clone(),
            subject_id: plan.subject_id.clone(),
            record_revision: plan.record_revision,
            authorized_by: authorized_by.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExpungementReceipt {
    pub reason: ExpungementReason,
    pub record_revision: RecordRevision,
    pub removed_fact_count: u64,
    pub external_source_aliases: Vec<String>,
    pub external_remediation: Vec<ExternalRemediationCode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExpungementTombstone {
    pub id: String,
    pub reason: ExpungementReason,
}

pub struct ExpungementManager<'workspace> {
    workspace: &'workspace Workspace,
}

impl<'workspace> ExpungementManager<'workspace> {
    pub(crate) fn new(workspace: &'workspace Workspace) -> Self {
        Self { workspace }
    }

    pub fn plan(&self, request: ExpungementRequest) -> Result<ExpungementPlan> {
        if request.fact_ids.is_empty() && request.source_ids.is_empty() {
            return Err(Error::InvalidRecordChangeSet(
                "Expungement requires at least one Fact or Source",
            ));
        }
        let status = self.workspace.status()?;
        let all_facts = self.load_fact_provenance()?;
        let requested_source_ids = request
            .source_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        if requested_source_ids.len() != request.source_ids.len() {
            return Err(Error::InvalidRecordChangeSet(
                "Expungement Source targets must be distinct",
            ));
        }
        self.require_sources(&request.source_ids)?;
        let mut requested_fact_ids = request.fact_ids.clone();
        requested_fact_ids.extend(
            all_facts
                .iter()
                .filter(|fact| {
                    fact.source_id
                        .as_ref()
                        .is_some_and(|id| requested_source_ids.contains(id))
                })
                .map(|fact| FactId::from_stored(fact.id.clone())),
        );
        let affected = self.expand_fact_closure(&requested_fact_ids, &all_facts)?;
        let mut source_ids = requested_source_ids;
        source_ids.extend(
            all_facts
                .iter()
                .filter(|fact| affected.contains(&fact.id))
                .filter_map(|fact| fact.source_id.clone()),
        );
        let affected_fact_ids = affected
            .iter()
            .cloned()
            .map(FactId::from_stored)
            .collect::<Vec<_>>();
        let source_ids = source_ids
            .into_iter()
            .map(SourceId::from_stored)
            .collect::<Vec<_>>();
        let extraction_run_ids = self.extraction_run_ids(&source_ids)?;
        let affected_source_aliases = self.source_aliases(&source_ids)?;
        let reconciliation_ids = self.relationship_ids(
            "SELECT DISTINCT reconciliation_id FROM reconciliation_fact_edges WHERE fact_id = ?1",
            &affected_fact_ids,
        )?;
        let hazard_ids = self.relationship_ids(
            "SELECT DISTINCT hazard_id FROM record_hazard_fact_edges WHERE fact_id = ?1",
            &affected_fact_ids,
        )?;
        let analysis_ids = self.analysis_ids(&affected_fact_ids)?;
        let commit_keys = self.commit_keys(&affected_fact_ids, &extraction_run_ids)?;
        let seed = serde_json::json!({
            "workspace_id": status.workspace_id,
            "subject_id": status.subject_id,
            "record_revision": status.record_revision,
            "reason": request.reason,
            "fact_ids": affected_fact_ids,
            "source_ids": source_ids,
            "extraction_run_ids": extraction_run_ids,
            "reconciliation_ids": reconciliation_ids,
            "hazard_ids": hazard_ids,
            "analysis_ids": analysis_ids,
            "commit_keys": commit_keys,
        });
        let id = format!(
            "expungement-plan-{}",
            lowercase_hex(&Sha256::digest(serde_json::to_vec(&seed)?))
        );
        Ok(ExpungementPlan {
            id,
            request,
            workspace_id: status.workspace_id,
            subject_id: status.subject_id,
            record_revision: status.record_revision,
            affected_fact_ids,
            affected_source_aliases,
            source_ids,
            extraction_run_ids,
            reconciliation_ids,
            hazard_ids,
            analysis_ids,
            commit_keys,
        })
    }

    pub fn execute(
        &self,
        plan: ExpungementPlan,
        authorization: &ExpungementAuthorization,
    ) -> Result<ExpungementReceipt> {
        let current = self.workspace.status()?;
        if authorization.plan_id != plan.id
            || authorization.workspace_id != plan.workspace_id
            || authorization.subject_id != plan.subject_id
            || authorization.record_revision != plan.record_revision
            || authorization.authorized_by.kind == AuthorKind::Agent
            || !valid_operator_identifier(&authorization.authorized_by.identifier)
        {
            return Err(Error::ExpungementAuthorization);
        }
        if current.workspace_id != plan.workspace_id
            || current.subject_id != plan.subject_id
            || current.record_revision != plan.record_revision
        {
            return Err(Error::ExpungementPlanStale);
        }
        let current_plan = self.plan(plan.request.clone())?;
        if current_plan.id != plan.id {
            return Err(Error::ExpungementPlanStale);
        }
        let next_revision = RecordRevision::from_stored(
            current
                .record_revision
                .value()
                .checked_add(1)
                .ok_or(Error::ExpungementPlanStale)?,
        );
        let transaction = self.workspace.connection().unchecked_transaction()?;
        let guard_changed = transaction.execute(
            "UPDATE ledger_mutation_guard SET expungement_enabled = 1 \
             WHERE singleton = 1 AND expungement_enabled = 0",
            [],
        )?;
        if guard_changed != 1 {
            return Err(Error::InvalidWorkspace(
                "invalid ledger mutation guard".to_owned(),
            ));
        }
        Self::delete_analyses(&transaction, &plan.analysis_ids)?;
        Self::delete_relationships(&transaction, &plan)?;
        Self::delete_provenance(&transaction, &plan)?;
        transaction.execute(
            "UPDATE workspace SET record_revision = ?1 WHERE singleton = 1 AND record_revision = ?2",
            params![
                i64::try_from(next_revision.value()).map_err(|_| Error::ExpungementPlanStale)?,
                i64::try_from(current.record_revision.value())
                    .map_err(|_| Error::ExpungementPlanStale)?
            ],
        )?;
        transaction.execute(
            "INSERT INTO expungement_tombstones (id, reason_code) VALUES (?1, ?2)",
            params![Uuid::now_v7().to_string(), plan.request.reason.as_str()],
        )?;
        transaction.execute(
            "UPDATE ledger_mutation_guard SET expungement_enabled = 0 WHERE singleton = 1",
            [],
        )?;
        transaction.execute(
            "UPDATE expungement_cleanup_state SET pending = 1 WHERE singleton = 1",
            [],
        )?;
        transaction.commit()?;
        self.workspace.complete_pending_expungement_cleanup()?;
        Ok(ExpungementReceipt {
            reason: plan.request.reason,
            record_revision: next_revision,
            removed_fact_count: u64::try_from(plan.affected_fact_ids.len())
                .map_err(|_| Error::ExpungementPlanStale)?,
            external_source_aliases: plan.affected_source_aliases,
            external_remediation: vec![
                ExternalRemediationCode::SourceFiles,
                ExternalRemediationCode::Snapshots,
                ExternalRemediationCode::Backups,
            ],
        })
    }

    fn delete_analyses(transaction: &Transaction<'_>, analysis_ids: &[AnalysisId]) -> Result<()> {
        for analysis_id in analysis_ids {
            let claim_ids = {
                let mut statement =
                    transaction.prepare("SELECT id FROM analysis_claims WHERE analysis_id = ?1")?;
                statement
                    .query_map([analysis_id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };
            transaction.execute(
                "DELETE FROM analysis_warnings WHERE analysis_id = ?1",
                [analysis_id.as_str()],
            )?;
            for claim_id in claim_ids {
                transaction.execute(
                    "DELETE FROM analysis_claim_verification_edges WHERE claim_id = ?1",
                    [&claim_id],
                )?;
                transaction.execute(
                    "DELETE FROM analysis_claim_fact_edges WHERE claim_id = ?1",
                    [&claim_id],
                )?;
            }
            transaction.execute(
                "DELETE FROM analysis_claims WHERE analysis_id = ?1",
                [analysis_id.as_str()],
            )?;
            transaction.execute("DELETE FROM analyses WHERE id = ?1", [analysis_id.as_str()])?;
        }
        Ok(())
    }

    fn delete_relationships(transaction: &Transaction<'_>, plan: &ExpungementPlan) -> Result<()> {
        for hazard_id in &plan.hazard_ids {
            transaction.execute(
                "DELETE FROM record_hazard_resolutions WHERE hazard_id = ?1",
                [hazard_id],
            )?;
            transaction.execute(
                "DELETE FROM record_hazard_source_edges WHERE hazard_id = ?1",
                [hazard_id],
            )?;
            transaction.execute(
                "DELETE FROM record_hazard_fact_edges WHERE hazard_id = ?1",
                [hazard_id],
            )?;
            transaction.execute("DELETE FROM record_hazards WHERE id = ?1", [hazard_id])?;
        }
        for reconciliation_id in &plan.reconciliation_ids {
            transaction.execute(
                "DELETE FROM reconciliation_fact_edges WHERE reconciliation_id = ?1",
                [reconciliation_id],
            )?;
            transaction.execute(
                "DELETE FROM reconciliations WHERE id = ?1",
                [reconciliation_id],
            )?;
        }
        Ok(())
    }

    fn delete_provenance(transaction: &Transaction<'_>, plan: &ExpungementPlan) -> Result<()> {
        for fact_id in &plan.affected_fact_ids {
            transaction.execute(
                "DELETE FROM corrections WHERE prior_fact_id = ?1 OR replacement_fact_id = ?1",
                [fact_id.as_str()],
            )?;
            transaction.execute(
                "DELETE FROM discrepancy_resolutions WHERE discrepancy_verification_id IN \
                 (SELECT id FROM verifications WHERE fact_id = ?1) \
                 OR supporting_verification_id IN \
                 (SELECT id FROM verifications WHERE fact_id = ?1)",
                [fact_id.as_str()],
            )?;
            transaction.execute(
                "DELETE FROM verifications WHERE fact_id = ?1",
                [fact_id.as_str()],
            )?;
            transaction.execute(
                "DELETE FROM diagnostic_study_findings WHERE study_fact_id = ?1",
                [fact_id.as_str()],
            )?;
            for table in [
                "lab_results",
                "vital_measurements",
                "medication_events",
                "condition_assertions",
                "diagnostic_studies",
                "care_tasks",
                "subject_preferences",
            ] {
                transaction.execute(
                    &format!("DELETE FROM {table} WHERE fact_id = ?1"),
                    [fact_id.as_str()],
                )?;
            }
            transaction.execute("DELETE FROM facts WHERE id = ?1", [fact_id.as_str()])?;
        }
        for run_id in &plan.extraction_run_ids {
            for table in [
                "extraction_run_candidates",
                "extraction_run_issues",
                "extraction_run_domains",
                "extraction_run_regions",
            ] {
                transaction.execute(
                    &format!("DELETE FROM {table} WHERE extraction_run_id = ?1"),
                    [run_id.as_str()],
                )?;
            }
            transaction.execute(
                "DELETE FROM extraction_runs WHERE id = ?1",
                [run_id.as_str()],
            )?;
        }
        for commit_key in &plan.commit_keys {
            transaction.execute(
                "DELETE FROM record_commits WHERE idempotency_key = ?1",
                [commit_key],
            )?;
        }
        for source_id in &plan.source_ids {
            transaction.execute(
                "DELETE FROM source_aliases WHERE source_id = ?1",
                [source_id.as_str()],
            )?;
            transaction.execute("DELETE FROM sources WHERE id = ?1", [source_id.as_str()])?;
        }
        Ok(())
    }

    pub fn tombstones(&self) -> Result<Vec<ExpungementTombstone>> {
        let mut statement = self
            .workspace
            .connection()
            .prepare("SELECT id, reason_code FROM expungement_tombstones ORDER BY id")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, reason)| {
                Ok(ExpungementTombstone {
                    id,
                    reason: ExpungementReason::parse(&reason)?,
                })
            })
            .collect()
    }

    fn expand_fact_closure(
        &self,
        requested: &[FactId],
        all_facts: &[StoredFactProvenance],
    ) -> Result<BTreeSet<String>> {
        let all_fact_ids = all_facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<HashSet<_>>();
        let mut affected = requested
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        if affected
            .iter()
            .any(|id| !all_fact_ids.contains(id.as_str()))
        {
            return Err(Error::FactNotFound("Expungement target".to_owned()));
        }
        let corrections = self.load_correction_edges()?;
        loop {
            let before = affected.len();
            let source_ids = related_values(&affected, all_facts, |fact| fact.source_id.as_deref());
            let run_ids = related_values(&affected, all_facts, |fact| {
                fact.extraction_run_id.as_deref()
            });
            for fact in all_facts {
                if fact
                    .source_id
                    .as_deref()
                    .is_some_and(|id| source_ids.contains(id))
                    || fact
                        .extraction_run_id
                        .as_deref()
                        .is_some_and(|id| run_ids.contains(id))
                {
                    affected.insert(fact.id.clone());
                }
            }
            for (prior, replacement) in &corrections {
                if affected.contains(prior) || affected.contains(replacement) {
                    affected.insert(prior.clone());
                    affected.insert(replacement.clone());
                }
            }
            if affected.len() == before {
                return Ok(affected);
            }
        }
    }

    fn load_fact_provenance(&self) -> Result<Vec<StoredFactProvenance>> {
        let mut statement = self.workspace.connection().prepare(
            "SELECT id, source_id, extraction_run_id FROM facts ORDER BY record_revision, id",
        )?;
        Ok(statement
            .query_map([], |row| {
                Ok(StoredFactProvenance {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    extraction_run_id: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn load_correction_edges(&self) -> Result<Vec<(String, String)>> {
        let mut statement = self
            .workspace
            .connection()
            .prepare("SELECT prior_fact_id, replacement_fact_id FROM corrections ORDER BY id")?;
        Ok(statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn source_aliases(&self, source_ids: &[SourceId]) -> Result<Vec<String>> {
        let mut aliases = BTreeSet::new();
        for source_id in source_ids {
            let mut statement = self
                .workspace
                .connection()
                .prepare("SELECT alias FROM source_aliases WHERE source_id = ?1 ORDER BY alias")?;
            aliases.extend(
                statement
                    .query_map([source_id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            );
        }
        Ok(aliases.into_iter().collect())
    }

    fn require_sources(&self, source_ids: &[SourceId]) -> Result<()> {
        for source_id in source_ids {
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

    fn extraction_run_ids(&self, source_ids: &[SourceId]) -> Result<Vec<ExtractionRunId>> {
        let mut ids = BTreeSet::new();
        for source_id in source_ids {
            let mut statement = self.workspace.connection().prepare(
                "SELECT id FROM extraction_runs WHERE source_id = ?1 ORDER BY record_revision, id",
            )?;
            ids.extend(
                statement
                    .query_map([source_id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            );
        }
        Ok(ids.into_iter().map(ExtractionRunId::from_stored).collect())
    }

    fn relationship_ids(&self, sql: &str, fact_ids: &[FactId]) -> Result<Vec<String>> {
        let mut ids = BTreeSet::new();
        for fact_id in fact_ids {
            let mut statement = self.workspace.connection().prepare(sql)?;
            ids.extend(
                statement
                    .query_map([fact_id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            );
        }
        Ok(ids.into_iter().collect())
    }

    fn analysis_ids(&self, fact_ids: &[FactId]) -> Result<Vec<AnalysisId>> {
        let mut ids = BTreeSet::new();
        for fact_id in fact_ids {
            let mut statement = self.workspace.connection().prepare(
                "SELECT DISTINCT c.analysis_id FROM analysis_claim_fact_edges edge \
                 JOIN analysis_claims c ON c.id = edge.claim_id WHERE edge.fact_id = ?1 \
                 UNION SELECT analysis_id FROM analysis_warnings WHERE related_fact_id = ?1",
            )?;
            ids.extend(
                statement
                    .query_map([fact_id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            );
        }
        Ok(ids.into_iter().map(AnalysisId::from_stored).collect())
    }

    fn commit_keys(&self, fact_ids: &[FactId], run_ids: &[ExtractionRunId]) -> Result<Vec<String>> {
        let fact_ids = fact_ids.iter().map(FactId::as_str).collect::<HashSet<_>>();
        let run_ids = run_ids
            .iter()
            .map(ExtractionRunId::as_str)
            .collect::<HashSet<_>>();
        let mut statement = self.workspace.connection().prepare(
            "SELECT idempotency_key, receipt_json FROM record_commits ORDER BY record_revision",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .filter_map(|(key, json)| {
                let receipt = match serde_json::from_str::<CommitReceipt>(&json) {
                    Ok(receipt) => receipt,
                    Err(error) => return Some(Err(Error::Json(error))),
                };
                let affected = receipt
                    .accepted_fact_ids
                    .iter()
                    .any(|id| fact_ids.contains(id.as_str()))
                    || receipt
                        .accepted_extraction_run_ids
                        .iter()
                        .any(|id| run_ids.contains(id.as_str()));
                affected.then_some(Ok(key))
            })
            .collect()
    }
}

fn valid_operator_identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

struct StoredFactProvenance {
    id: String,
    source_id: Option<String>,
    extraction_run_id: Option<String>,
}

fn related_values<'a>(
    affected: &BTreeSet<String>,
    facts: &'a [StoredFactProvenance],
    accessor: impl Fn(&'a StoredFactProvenance) -> Option<&'a str>,
) -> HashSet<&'a str> {
    facts
        .iter()
        .filter(|fact| affected.contains(&fact.id))
        .filter_map(accessor)
        .collect()
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
