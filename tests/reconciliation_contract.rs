use std::fs;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin, ExtractionCoverage,
    ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData, FactDraft, FactEvidence,
    FactQuery, IdempotencyKey, LabResult, LabValue, ReconciliationAgreement, ReconciliationDraft,
    RecordChange, RecordChangeSet, SourceDescriptor, SourceId, SourceRegion, SubjectId, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn add_synthetic_lab(
    workspace: &Workspace,
    source_id: SourceId,
    idempotency_key: &str,
    value: i64,
) -> TestResult {
    let identity = workspace.status()?;
    let source_region = SourceRegion {
        locator: "line:1".to_owned(),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse(idempotency_key)?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-lab-extractor".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source_id.clone(),
            coverage: ExtractionCoverage {
                regions: vec![source_region.clone()],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec!["synthetic-marker".to_owned()],
        }),
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: Some("2026-01-15".to_owned()),
            },
            evidence: FactEvidence::Source {
                source_id,
                source_region,
                asserted_by: AuthorIdentity {
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-lab-extractor".to_owned(),
                },
            },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Marker".to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::new(value, 0),
                },
                units: Some("mg/dL".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: Some("Synthetic Lab".to_owned()),
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    Ok(())
}

#[test]
fn equivalent_cross_source_facts_present_once_without_erasing_history() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-a\n",
    )?;
    fs::write(
        root.path().join("sources/lab-b.txt"),
        b"synthetic-source-b\n",
    )?;
    let source_a = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let source_b = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-b.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    add_synthetic_lab(
        &workspace,
        source_a.source_id,
        "synthetic-reconciliation-a-001",
        10,
    )?;
    add_synthetic_lab(
        &workspace,
        source_b.source_id,
        "synthetic-reconciliation-b-001",
        10,
    )?;
    let before = workspace.record().query(FactQuery::default())?;
    assert_eq!(before.len(), 2);
    let representative_id = before[0].id.clone();
    let fact_ids = before
        .iter()
        .map(|fact| fact.id.clone())
        .collect::<Vec<_>>();
    let reviewer = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-record-reviewer".to_owned(),
    };
    let reconciliation = ReconciliationDraft {
        fact_ids: fact_ids.clone(),
        agreement: ReconciliationAgreement::Equivalent,
        author: reviewer.clone(),
        reconciled_at: DateTime::parse_from_rfc3339("2026-01-18T12:00:00Z")?.with_timezone(&Utc),
        rationale: "same_observation_reported_by_two_sources".to_owned(),
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-reconciliation-001")?,
        evidence_origin: EvidenceOrigin::RecordReview { reviewer },
        extractor: ExtractorIdentity {
            name: "synthetic-record-reviewer".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::ReconcileFacts(reconciliation.clone())],
    })?;

    assert_eq!(proposal.impact().reconciliations, 1);
    let receipt = workspace.record().apply(proposal)?;

    assert_eq!(receipt.accepted_reconciliation_ids.len(), 1);
    let current = workspace.record().query(FactQuery::default())?;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, representative_id);
    let history = workspace.record().query(FactQuery {
        kind: None,
        include_history: true,
    })?;
    assert_eq!(history.len(), 2);
    assert_eq!(
        history
            .iter()
            .map(|fact| fact.id.clone())
            .collect::<Vec<_>>(),
        fact_ids
    );
    let relationships = workspace.record().reconciliations(&representative_id)?;
    assert_eq!(relationships.len(), 1);
    assert_eq!(relationships[0].draft, reconciliation);
    assert_eq!(relationships[0].id, receipt.accepted_reconciliation_ids[0]);

    Ok(())
}

#[test]
fn conflicting_cross_source_facts_remain_visible_but_cannot_drive_analysis() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-a\n",
    )?;
    fs::write(
        root.path().join("sources/lab-b.txt"),
        b"synthetic-source-b\n",
    )?;
    let source_a = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let source_b = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-b.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    add_synthetic_lab(
        &workspace,
        source_a.source_id,
        "synthetic-conflict-a-001",
        10,
    )?;
    add_synthetic_lab(
        &workspace,
        source_b.source_id,
        "synthetic-conflict-b-001",
        12,
    )?;
    let facts = workspace.record().query(FactQuery::default())?;
    let fact_ids = facts.iter().map(|fact| fact.id.clone()).collect::<Vec<_>>();
    let reviewer = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-record-reviewer".to_owned(),
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-conflict-001")?,
        evidence_origin: EvidenceOrigin::RecordReview {
            reviewer: reviewer.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-record-reviewer".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::ReconcileFacts(ReconciliationDraft {
            fact_ids: fact_ids.clone(),
            agreement: ReconciliationAgreement::Conflicting,
            author: reviewer,
            reconciled_at: DateTime::parse_from_rfc3339("2026-01-18T12:00:00Z")?
                .with_timezone(&Utc),
            rationale: "sources_disagree_on_value".to_owned(),
        })],
    })?;
    workspace.record().apply(proposal)?;

    let current = workspace.record().query(FactQuery::default())?;
    assert_eq!(current.len(), 2);
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    assert!(analysis.claims.is_empty());
    assert_eq!(analysis.warnings.len(), 1);
    assert_eq!(analysis.warnings[0].code, "conflicting_evidence");
    assert_eq!(analysis.warnings[0].related_fact_ids, fact_ids);

    Ok(())
}
