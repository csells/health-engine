use std::fs;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin, ExtractionCoverage, ExtractionDomain,
    ExtractionRunDraft, ExtractorIdentity, FactData, FactDraft, FactEvidence, FactQuery,
    IdempotencyKey, LabResult, LabValue, RecordChange, RecordChangeSet, SourceDescriptor,
    SourceRegion, SubjectId, VerificationDraft, VerificationMethod, VerificationOutcome, Workspace,
};
use rusqlite::Connection;
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(clippy::too_many_lines)]
fn reopened_workspace_enforces_foreign_keys_during_apply() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(root.path().join("sources/lab.txt"), b"synthetic lab\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let region = SourceRegion {
        locator: "line:1".to_owned(),
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-integrity-fact-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-extractor".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source.source_id.clone(),
            coverage: ExtractionCoverage {
                regions: vec![region.clone()],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec!["synthetic-marker".to_owned()],
        }),
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: None,
            },
            evidence: FactEvidence::Source {
                source_id: source.source_id.clone(),
                source_region: region.clone(),
                asserted_by: AuthorIdentity {
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-extractor".to_owned(),
                },
            },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Marker".to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::new(10, 0),
                },
                units: Some("mg/dL".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    drop(workspace);

    let workspace = Workspace::open(root.path())?;
    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-integrity-verification-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(VerificationDraft {
            fact_id: fact.id.clone(),
            source_id: source.source_id,
            source_region: region,
            verifier: AuthorIdentity {
                kind: AuthorKind::Agent,
                identifier: "synthetic-verifier".to_owned(),
            },
            method: VerificationMethod {
                name: "exact-location-source-comparison".to_owned(),
                version: "1.0.0".to_owned(),
            },
            checked_at: DateTime::parse_from_rfc3339("2026-01-16T12:00:00Z")?.with_timezone(&Utc),
            scope: "single_lab_result".to_owned(),
            outcome: VerificationOutcome::Verified,
        })],
    })?;

    let external = Connection::open(root.path().join(".health-engine/health.sqlite3"))?;
    external.execute(
        "UPDATE ledger_mutation_guard SET expungement_enabled = 1 WHERE singleton = 1",
        [],
    )?;
    external.execute(
        "DELETE FROM lab_results WHERE fact_id = ?1",
        [fact.id.as_str()],
    )?;
    external.execute("DELETE FROM facts WHERE id = ?1", [fact.id.as_str()])?;
    external.execute(
        "UPDATE ledger_mutation_guard SET expungement_enabled = 0 WHERE singleton = 1",
        [],
    )?;
    drop(external);

    let error = workspace
        .record()
        .apply(proposal)
        .expect_err("a relationship whose target disappeared must not commit");

    assert_eq!(error.code(), "database_error");
    assert_eq!(workspace.status()?.record_revision.value(), 1);
    assert!(workspace.record().verifications(&fact.id)?.is_empty());

    Ok(())
}

#[test]
fn database_rejects_rewriting_accepted_fact_history() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-append-only-fact-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-self-report".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: None,
            },
            evidence: FactEvidence::SelfReport { reporter },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Marker".to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::new(10, 0),
                },
                units: Some("mg/dL".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    drop(workspace);

    let external = Connection::open(root.path().join(".health-engine/health.sqlite3"))?;
    let error = external
        .execute(
            "UPDATE lab_results SET numeric_value = '999' WHERE fact_id = ?1",
            [fact.id.as_str()],
        )
        .expect_err("accepted fact projections must be append-only");

    assert!(error.to_string().contains("append-only ledger"));

    Ok(())
}
