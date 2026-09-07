use std::fs;

use chrono::NaiveDate;
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin,
    ExpungementAuthorization, ExpungementReason, ExpungementRequest, ExternalRemediationCode,
    ExtractionCoverage, ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData,
    FactDraft, FactEvidence, FactQuery, IdempotencyKey, LabResult, LabValue, RecordChange,
    RecordChangeSet, SourceDescriptor, SourceRegion, SubjectId, VitalMeasurement, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn add_self_report(workspace: &Workspace, key: &str, test_name: &str) -> TestResult {
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse(key)?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-intake-adapter".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: Some("2026-01-15".to_owned()),
            },
            evidence: FactEvidence::SelfReport { reporter },
            fact: FactData::LabResult(LabResult {
                test_name: test_name.to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::new(10, 0),
                },
                units: None,
                reference_range: None,
                reported_flag: None,
                performing_lab: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn authorized_expungement_removes_dependency_closure_but_not_external_source() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let source_path = root.path().join("sources/wrong-subject.txt");
    fs::write(&source_path, b"synthetic wrong-subject source\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/wrong-subject.txt".to_owned(),
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
        idempotency_key: IdempotencyKey::parse("synthetic-expungement-fact-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-lab-extractor".to_owned(),
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
                source_text: Some("2026-01-15".to_owned()),
            },
            evidence: FactEvidence::Source {
                source_id: source.source_id.clone(),
                source_region: region,
                asserted_by: AuthorIdentity {
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-lab-extractor".to_owned(),
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
    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    let identity = workspace.status()?;
    let factless_run = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-expungement-factless-run-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-lab-extractor".to_owned(),
            version: "1.0.1".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source.source_id.clone(),
            coverage: ExtractionCoverage {
                regions: vec![SourceRegion {
                    locator: "line:2".to_owned(),
                }],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec![],
        }),
        changes: vec![],
    })?;
    workspace.record().apply(factless_run)?;
    let unrelated_path = root.path().join("sources/unrelated.txt");
    fs::write(&unrelated_path, b"synthetic unrelated source\n")?;
    let unrelated_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/unrelated.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    add_self_report(
        &workspace,
        "synthetic-expungement-unrelated-fact-001",
        "Synthetic Unrelated Marker",
    )?;
    let unrelated_fact = workspace
        .record()
        .query(FactQuery::default())?
        .into_iter()
        .find(|candidate| candidate.id != fact.id)
        .expect("unrelated synthetic Fact");
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    let before_revision = workspace.status()?.record_revision;

    let plan = workspace.expungements().plan(ExpungementRequest {
        reason: ExpungementReason::WrongSubject,
        fact_ids: vec![fact.id.clone()],
        source_ids: vec![],
    })?;

    assert_eq!(workspace.status()?.record_revision, before_revision);
    assert_eq!(workspace.record().query(FactQuery::default())?.len(), 2);
    assert_eq!(plan.affected_fact_ids, vec![fact.id]);
    assert_eq!(
        plan.affected_source_aliases,
        vec!["sources/wrong-subject.txt"]
    );
    let operator = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let agent = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-autonomous-agent".to_owned(),
    };
    let agent_error =
        ExpungementAuthorization::confirm(&plan, &plan.confirmation_challenge(), &agent)
            .expect_err("Agent identity must never authorize Expungement");
    assert_eq!(agent_error.code(), "expungement_authorization");
    let authorization_error =
        ExpungementAuthorization::confirm(&plan, "EXPUNGE wrong-plan", &operator)
            .expect_err("authorization must bind the exact plan");
    assert_eq!(authorization_error.code(), "expungement_authorization");
    assert_eq!(workspace.record().query(FactQuery::default())?.len(), 2);
    fs::create_dir_all(root.path().join("reports/current"))?;
    fs::write(
        root.path().join("reports/current/health-summary.md"),
        b"synthetic derived report\n",
    )?;
    fs::create_dir_all(root.path().join(".health-engine/staging"))?;
    fs::write(
        root.path().join(".health-engine/staging/proposal.tmp"),
        b"synthetic staged content\n",
    )?;
    let challenge = plan.confirmation_challenge();
    let authorization = ExpungementAuthorization::confirm(&plan, &challenge, &operator)?;

    let receipt = workspace.expungements().execute(plan, &authorization)?;

    assert_eq!(receipt.reason, ExpungementReason::WrongSubject);
    assert!(receipt.removed_fact_count >= 1);
    assert_eq!(
        receipt.external_source_aliases,
        vec!["sources/wrong-subject.txt"]
    );
    assert_eq!(
        receipt.external_remediation,
        vec![
            ExternalRemediationCode::SourceFiles,
            ExternalRemediationCode::Snapshots,
            ExternalRemediationCode::Backups,
        ]
    );
    assert_eq!(
        workspace.record().query(FactQuery::default())?,
        vec![unrelated_fact]
    );
    assert_eq!(
        workspace
            .analyses()
            .show(&analysis.id)
            .expect_err("dependent Analysis must be removed")
            .code(),
        "analysis_not_found"
    );
    assert_eq!(
        workspace
            .sources()
            .status(&source.source_id)
            .expect_err("Source provenance must be removed")
            .code(),
        "source_not_found"
    );
    assert!(source_path.is_file());
    assert_eq!(
        workspace
            .sources()
            .status(&unrelated_source.source_id)?
            .content_state,
        health_engine::SourceContentState::Available
    );
    assert!(unrelated_path.is_file());
    assert!(
        !root
            .path()
            .join("reports/current/health-summary.md")
            .exists()
    );
    assert!(!root.path().join(".health-engine/staging").exists());
    let tombstones = workspace.expungements().tombstones()?;
    assert_eq!(tombstones.len(), 1);
    assert_eq!(tombstones[0].reason, ExpungementReason::WrongSubject);

    Ok(())
}

#[test]
fn stale_expungement_plan_cannot_mutate_the_workspace() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    add_self_report(
        &workspace,
        "synthetic-stale-plan-a-001",
        "Synthetic Marker A",
    )?;
    let first_fact = workspace.record().query(FactQuery::default())?.remove(0);
    let plan = workspace.expungements().plan(ExpungementRequest {
        reason: ExpungementReason::PrivacyRequest,
        fact_ids: vec![first_fact.id],
        source_ids: vec![],
    })?;
    let operator = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let authorization =
        ExpungementAuthorization::confirm(&plan, &plan.confirmation_challenge(), &operator)?;
    add_self_report(
        &workspace,
        "synthetic-stale-plan-b-001",
        "Synthetic Marker B",
    )?;

    let error = workspace
        .expungements()
        .execute(plan, &authorization)
        .expect_err("record change must stale destructive authorization");

    assert_eq!(error.code(), "expungement_plan_stale");
    assert_eq!(workspace.record().query(FactQuery::default())?.len(), 2);
    assert!(workspace.expungements().tombstones()?.is_empty());

    Ok(())
}

#[test]
fn authorized_expungement_can_target_factless_source_provenance() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let source_path = root.path().join("sources/accidental.txt");
    fs::write(&source_path, b"synthetic accidental source\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/accidental.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let identity = workspace.status()?;
    let run = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-factless-source-run-001")?,
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
                regions: vec![SourceRegion {
                    locator: "line:1".to_owned(),
                }],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec![],
        }),
        changes: vec![],
    })?;
    workspace.record().apply(run)?;

    let plan = workspace.expungements().plan(ExpungementRequest {
        reason: ExpungementReason::AccidentalSensitiveIngestion,
        fact_ids: vec![],
        source_ids: vec![source.source_id.clone()],
    })?;
    let operator = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let authorization =
        ExpungementAuthorization::confirm(&plan, &plan.confirmation_challenge(), &operator)?;
    let receipt = workspace.expungements().execute(plan, &authorization)?;

    assert_eq!(receipt.removed_fact_count, 0);
    assert_eq!(
        receipt.external_source_aliases,
        vec!["sources/accidental.txt"]
    );
    assert_eq!(
        workspace
            .sources()
            .status(&source.source_id)
            .expect_err("factless Source provenance must be removed")
            .code(),
        "source_not_found"
    );
    assert!(source_path.is_file());

    Ok(())
}

#[test]
fn authorized_expungement_removes_non_lab_fact_projection() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-expunge-vital-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-vital-intake".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: Some("2026-01-15".to_owned()),
            },
            evidence: FactEvidence::SelfReport {
                reporter: reporter.clone(),
            },
            fact: FactData::VitalMeasurement(VitalMeasurement::Weight {
                value: Decimal::new(150, 0),
                original: "150".to_owned(),
                units: Some("lb".to_owned()),
                note: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    let plan = workspace.expungements().plan(ExpungementRequest {
        reason: ExpungementReason::PrivacyRequest,
        fact_ids: vec![fact.id],
        source_ids: vec![],
    })?;
    let authorization =
        ExpungementAuthorization::confirm(&plan, &plan.confirmation_challenge(), &reporter)?;

    workspace.expungements().execute(plan, &authorization)?;

    assert!(workspace.record().query(FactQuery::default())?.is_empty());
    Ok(())
}
