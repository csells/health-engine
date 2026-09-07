use std::fs;

use chrono::NaiveDate;
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, EvidenceAssurance, EvidenceOrigin,
    ExtractionCoverage, ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData,
    FactDraft, FactEvidence, FactQuery, IdempotencyKey, LabResult, LabValue, RecordChange,
    RecordChangeSet, SourceDescriptor, SourceRegion, SubjectId, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn self_report_requires_attribution_without_inventing_a_source() -> TestResult {
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
        idempotency_key: IdempotencyKey::parse("synthetic-self-report-001")?,
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
            evidence: FactEvidence::SelfReport {
                reporter: reporter.clone(),
            },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Home Marker".to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::new(10, 0),
                },
                units: Some("units".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;

    let facts = workspace.record().query(FactQuery::default())?;
    assert_eq!(facts.len(), 1);
    assert_eq!(
        facts[0].original_assurance,
        EvidenceAssurance::ExplicitSelfReport
    );
    assert_eq!(
        facts[0].current_assurance,
        EvidenceAssurance::ExplicitSelfReport
    );
    assert_eq!(
        facts[0].draft.evidence,
        FactEvidence::SelfReport { reporter }
    );
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    assert_eq!(analysis.claims.len(), 1);
    assert!(analysis.claims[0].statement.starts_with("Self-reported: "));

    Ok(())
}

#[test]
fn parser_validated_source_retains_parser_assurance() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab.csv"),
        b"marker,value\nsynthetic,10\n",
    )?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let region = SourceRegion {
        locator: "row:2".to_owned(),
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-parser-validated-001")?,
        evidence_origin: EvidenceOrigin::ParserValidatedExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-csv-parser".to_owned(),
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
                source_id: source.source_id,
                source_region: region,
                asserted_by: AuthorIdentity {
                    kind: AuthorKind::Application,
                    identifier: "synthetic-csv-parser".to_owned(),
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
    assert_eq!(fact.original_assurance, EvidenceAssurance::ParserValidated);
    assert_eq!(fact.current_assurance, EvidenceAssurance::ParserValidated);
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    assert_eq!(analysis.claims.len(), 1);
    assert!(analysis.warnings.is_empty());

    Ok(())
}
