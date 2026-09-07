use std::fs;

use chrono::NaiveDate;
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, EvidenceAssurance, EvidenceOrigin,
    ExtractionCoverage, ExtractionDomain, ExtractionIssue, ExtractionRunDraft, ExtractorIdentity,
    FactData, FactDraft, FactEvidence, FactQuery, IdempotencyKey, LabResult, LabValue,
    RecordChange, RecordChangeSet, RecordRevision, SourceDescriptor, SourceRegion, SubjectId,
    Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(clippy::too_many_lines)]
fn extraction_run_is_proposed_without_mutation_then_committed_atomically() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-v1\n",
    )?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let identity = workspace.status()?;
    let change_set = RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id.clone(),
        subject_id: identity.subject_id.clone(),
        expected_revision: RecordRevision::INITIAL,
        idempotency_key: IdempotencyKey::parse("synthetic-extraction-run-001")?,
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
                regions: vec![SourceRegion {
                    locator: "page:1#table:results".to_owned(),
                }],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![ExtractionIssue {
                code: "reference_notes_not_examined".to_owned(),
                region: Some(SourceRegion {
                    locator: "page:2#section:notes".to_owned(),
                }),
            }],
            failures: vec![],
            resulting_candidate_keys: vec![],
        }),
        changes: vec![],
    };

    let proposal = workspace.record().propose(change_set)?;

    assert_eq!(proposal.expected_revision(), RecordRevision::INITIAL);
    assert_eq!(proposal.impact().extraction_runs, 1);
    assert_eq!(proposal.impact().health_facts, 0);
    assert_eq!(workspace.status()?.record_revision, RecordRevision::INITIAL);
    assert!(
        workspace
            .sources()
            .extraction_runs(&source.source_id)?
            .is_empty()
    );

    let receipt = workspace.record().apply(proposal)?;

    assert_eq!(receipt.previous_revision, RecordRevision::INITIAL);
    assert_eq!(receipt.record_revision.value(), 1);
    assert_eq!(receipt.accepted_extraction_run_ids.len(), 1);
    assert!(receipt.accepted_fact_ids.is_empty());
    assert!(receipt.newly_stale_analysis_ids.is_empty());
    assert_eq!(workspace.status()?.record_revision.value(), 1);
    let runs = workspace.sources().extraction_runs(&source.source_id)?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].source_id, source.source_id);
    assert_eq!(runs[0].extractor.name, "synthetic-lab-extractor");
    assert_eq!(runs[0].extractor.version, "1.0.0");
    assert_eq!(runs[0].coverage.regions[0].locator, "page:1#table:results");
    assert_eq!(runs[0].coverage.domains, vec![ExtractionDomain::Laboratory]);
    assert_eq!(runs[0].omissions[0].code, "reference_notes_not_examined");
    assert!(runs[0].failures.is_empty());
    assert!(runs[0].resulting_candidate_keys.is_empty());
    assert_eq!(runs[0].record_revision.value(), 1);

    let identity = workspace.status()?;
    let Err(oversized) = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-oversized-extraction-001")?,
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
                regions: (0..1_025)
                    .map(|index| SourceRegion {
                        locator: format!("line:{index}"),
                    })
                    .collect(),
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec![],
        }),
        changes: vec![],
    }) else {
        panic!("Extraction Run vectors must have a practical bound");
    };
    assert_eq!(oversized.code(), "invalid_record_change_set");
    assert_eq!(
        workspace.status()?.record_revision,
        identity.record_revision
    );

    Ok(())
}

#[test]
fn changed_source_cannot_produce_a_record_proposal() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let alias = root.path().join("sources/lab-a.txt");
    fs::write(&alias, b"synthetic-source-v1\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    fs::write(alias, b"synthetic-source-v2\n")?;
    let identity = workspace.status()?;

    let error = workspace
        .record()
        .propose(RecordChangeSet {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: RecordRevision::INITIAL,
            idempotency_key: IdempotencyKey::parse("synthetic-changed-source-001")?,
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
        })
        .expect_err("changed evidence must not become a proposal");

    assert_eq!(error.code(), "source_content_changed");
    assert_eq!(workspace.status()?.record_revision, RecordRevision::INITIAL);
    assert!(
        workspace
            .sources()
            .extraction_runs(&source.source_id)?
            .is_empty()
    );

    Ok(())
}

#[test]
fn document_extraction_is_derived_as_unverified() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-v1\n",
    )?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let identity = workspace.status()?;
    let source_region = SourceRegion {
        locator: "line:1".to_owned(),
    };

    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-document-cannot-self-verify-001")?,
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
                source_id: source.source_id,
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
                    value: Decimal::new(10, 0),
                },
                units: Some("mg/dL".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: Some("Synthetic Lab".to_owned()),
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;

    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(fact.original_assurance, EvidenceAssurance::Unverified);
    assert_eq!(fact.current_assurance, EvidenceAssurance::Unverified);

    Ok(())
}

#[test]
fn source_drift_after_proposal_prevents_every_record_effect() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let source_path = root.path().join("sources/lab-a.txt");
    fs::write(&source_path, b"synthetic-source-v1\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let identity = workspace.status()?;
    let source_region = SourceRegion {
        locator: "line:1".to_owned(),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-source-race-001")?,
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
                source_id: source.source_id.clone(),
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
                    value: Decimal::new(10, 0),
                },
                units: Some("mg/dL".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: Some("Synthetic Lab".to_owned()),
            }),
        })],
    })?;

    fs::write(source_path, b"synthetic-source-v2\n")?;
    let error = workspace
        .record()
        .apply(proposal)
        .expect_err("drifted evidence must invalidate its proposal");

    assert_eq!(error.code(), "source_content_changed");
    assert_eq!(workspace.status()?.record_revision, RecordRevision::INITIAL);
    assert!(workspace.record().query(FactQuery::default())?.is_empty());
    assert!(
        workspace
            .sources()
            .extraction_runs(&source.source_id)?
            .is_empty()
    );

    Ok(())
}
