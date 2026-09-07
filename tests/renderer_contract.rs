use std::fs;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin, ExtractionCoverage,
    ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData, FactDraft, FactEvidence,
    IdempotencyKey, LabResult, LabValue, RecordChange, RecordChangeSet, RecordRevision,
    RenderContext, SourceDescriptor, SourceRegion, SubjectId, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(clippy::too_many_lines)]
fn report_bundle_renders_only_structured_analysis_claims() -> TestResult {
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
    let region = SourceRegion {
        locator: "line:1".to_owned(),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: RecordRevision::INITIAL,
        idempotency_key: IdempotencyKey::parse("synthetic-render-lab-001")?,
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
            resulting_candidate_keys: vec![],
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
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-lab-extractor".to_owned(),
                },
            },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Marker".to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::from_str("10")?,
                },
                units: Some("mg/dL".to_owned()),
                reference_range: None,
                reported_flag: None,
                performing_lab: Some("Synthetic Lab Never A Claim".to_owned()),
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    let snapshot = workspace.record().snapshot(analysis.record_revision)?;
    let generated_at = DateTime::parse_from_rfc3339("2026-01-17T08:00:00Z")?.with_timezone(&Utc);

    let bundle =
        workspace
            .renderer()
            .report_bundle(&snapshot, &analysis, RenderContext { generated_at })?;

    assert_eq!(bundle.record_revision, analysis.record_revision);
    assert_eq!(bundle.analysis_id, analysis.id);
    assert_eq!(bundle.generated_at, generated_at);
    assert_eq!(
        bundle.paths(),
        vec![
            "reports/HEALTH_REPORT.html",
            "reports/PHYSICIAN_SUMMARY.md",
            "reports/current/health-summary.md",
            "reports/current/open-loops.md",
        ]
    );
    assert_eq!(bundle.claim_ids, vec![analysis.claims[0].id.clone()]);
    for path in bundle.paths() {
        let rendered = bundle.file(path).expect("bundle path should resolve");
        assert!(rendered.contains(analysis.id.as_str()));
        assert!(rendered.contains("record revision: 1"));
        assert!(rendered.contains("unverified_evidence"));
        assert!(!rendered.contains("Synthetic Lab Never A Claim"));
    }
    for path in [
        "reports/HEALTH_REPORT.html",
        "reports/PHYSICIAN_SUMMARY.md",
        "reports/current/health-summary.md",
    ] {
        let rendered = bundle.file(path).expect("bundle path should resolve");
        assert!(rendered.contains(&analysis.claims[0].statement));
        assert!(rendered.contains(analysis.claims[0].id.as_str()));
        assert!(rendered.contains("low confidence"));
    }

    Ok(())
}
