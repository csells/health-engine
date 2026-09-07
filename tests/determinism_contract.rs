use std::fs;
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, Clock, EvidenceOrigin,
    ExtractionCoverage, ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData,
    FactDraft, FactEvidence, FactQuery, IdempotencyKey, LabResult, LabValue, RecordChange,
    RecordChangeSet, RecordRevision, SourceDescriptor, SourceRegion, SubjectId, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

#[test]
fn injected_clock_controls_engine_assigned_fact_and_analysis_times() -> TestResult {
    let root = tempfile::tempdir()?;
    let fixed = DateTime::parse_from_rfc3339("2026-01-18T09:30:00Z")?.with_timezone(&Utc);
    let workspace = Workspace::init_with_clock(
        root.path(),
        SubjectId::parse("subject-synthetic-001")?,
        Arc::new(FixedClock(fixed)),
    )?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(root.path().join("sources/lab.txt"), b"synthetic\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let status = workspace.status()?;
    let region = SourceRegion {
        locator: "line:1".to_owned(),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: status.workspace_id,
        subject_id: status.subject_id,
        expected_revision: RecordRevision::INITIAL,
        idempotency_key: IdempotencyKey::parse("synthetic-clock-001")?,
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
            resulting_candidate_keys: vec![],
        }),
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: None,
            },
            evidence: FactEvidence::Source {
                source_id: source.source_id,
                source_region: region,
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
                units: None,
                reference_range: None,
                reported_flag: None,
                performing_lab: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;

    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    assert_eq!(fact.recorded_at, fixed);
    assert_eq!(analysis.created_at, fixed);
    let repeated = workspace.analyze(AnalysisOptions::default())?;
    assert_eq!(repeated.id, analysis.id);

    Ok(())
}

#[test]
fn replay_from_the_same_blank_workspace_is_deterministic() -> TestResult {
    let first_root = tempfile::tempdir()?;
    let second_root = tempfile::tempdir()?;
    let fixed = DateTime::parse_from_rfc3339("2026-01-18T09:30:00Z")?.with_timezone(&Utc);
    let blank = Workspace::init_with_clock(
        first_root.path(),
        SubjectId::parse("subject-synthetic-001")?,
        Arc::new(FixedClock(fixed)),
    )?;
    drop(blank);
    fs::create_dir(second_root.path().join(".health-engine"))?;
    fs::copy(
        first_root.path().join(".health-engine/health.sqlite3"),
        second_root.path().join(".health-engine/health.sqlite3"),
    )?;
    for root in [first_root.path(), second_root.path()] {
        fs::create_dir(root.join("sources"))?;
        fs::write(root.join("sources/lab.txt"), b"synthetic replay source\n")?;
    }
    let first = Workspace::open_with_clock(first_root.path(), Arc::new(FixedClock(fixed)))?;
    let second = Workspace::open_with_clock(second_root.path(), Arc::new(FixedClock(fixed)))?;
    let first_source = first.sources().register(SourceDescriptor {
        alias: "sources/lab.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let second_source = second.sources().register(SourceDescriptor {
        alias: "sources/lab.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;

    assert_eq!(first_source.source_id, second_source.source_id);

    for (workspace, source_id) in [
        (&first, first_source.source_id),
        (&second, second_source.source_id),
    ] {
        let status = workspace.status()?;
        let region = SourceRegion {
            locator: "line:1".to_owned(),
        };
        let proposal = workspace.record().propose(RecordChangeSet {
            schema_version: 1,
            workspace_id: status.workspace_id,
            subject_id: status.subject_id,
            expected_revision: RecordRevision::INITIAL,
            idempotency_key: IdempotencyKey::parse("synthetic-replay-001")?,
            evidence_origin: EvidenceOrigin::ParserValidatedExtraction {
                source_id: source_id.clone(),
            },
            extractor: ExtractorIdentity {
                name: "synthetic-parser".to_owned(),
                version: "1.0.0".to_owned(),
            },
            extraction_run: Some(ExtractionRunDraft {
                source_id: source_id.clone(),
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
                    source_id,
                    source_region: region,
                    asserted_by: AuthorIdentity {
                        kind: AuthorKind::Application,
                        identifier: "synthetic-parser".to_owned(),
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
    }

    assert_eq!(
        first.record().query(FactQuery::default())?,
        second.record().query(FactQuery::default())?
    );
    assert_eq!(
        first.analyze(AnalysisOptions::default())?,
        second.analyze(AnalysisOptions::default())?
    );

    Ok(())
}
