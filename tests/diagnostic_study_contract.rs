use chrono::NaiveDate;
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, ClinicalTimePoint, DiagnosticFinding,
    DiagnosticStudy, DiagnosticStudyKind, DiagnosticStudyStatus, EvidenceAssurance, EvidenceOrigin,
    ExtractionCoverage, ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData,
    FactDraft, FactEvidence, FactKind, FactQuery, IdempotencyKey, RecordChange, RecordChangeSet,
    SourceDescriptor, SourceRegion, SubjectId, Workspace,
};
use std::fs;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn final_study_keeps_findings_distinct_from_impression() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-study.pdf"),
        b"synthetic diagnostic source\n",
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-study.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    })?;
    let identity = workspace.status()?;
    let region = SourceRegion {
        locator: "page:1".to_owned(),
    };
    let expected = DiagnosticStudy {
        kind: DiagnosticStudyKind::Imaging,
        status: DiagnosticStudyStatus::Final,
        name: "Synthetic Imaging Study".to_owned(),
        body_site: Some("synthetic site".to_owned()),
        method: Some("synthetic method".to_owned()),
        findings: vec![DiagnosticFinding {
            section: Some("Synthetic organ".to_owned()),
            body_site: Some("synthetic site".to_owned()),
            text: "Synthetic observed finding".to_owned(),
        }],
        impression: Some("Synthetic clinician impression".to_owned()),
        resulted_at: Some(ClinicalTimePoint::Date {
            value: NaiveDate::from_ymd_opt(2026, 1, 16).expect("date"),
        }),
        ordering_provider: Some("Synthetic Ordering Clinician".to_owned()),
        interpreting_provider: Some("Synthetic Interpreting Clinician".to_owned()),
        performing_organization: Some("Synthetic Imaging Center".to_owned()),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-diagnostic-study-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-study-extractor".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source.source_id.clone(),
            coverage: ExtractionCoverage {
                regions: vec![region.clone()],
                domains: vec![ExtractionDomain::DiagnosticStudies],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec![region.locator.clone()],
        }),
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("date"),
                source_text: Some("synthetic performed date".to_owned()),
            },
            evidence: FactEvidence::Source {
                source_id: source.source_id,
                source_region: region,
                asserted_by: AuthorIdentity {
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-study-extractor".to_owned(),
                },
            },
            fact: FactData::DiagnosticStudy(expected.clone()),
        })],
    })?;
    workspace.record().apply(proposal)?;

    let facts = workspace.record().query(FactQuery {
        kind: Some(FactKind::DiagnosticStudy),
        include_history: false,
    })?;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].original_assurance, EvidenceAssurance::Unverified);
    let FactData::DiagnosticStudy(study) = &facts[0].draft.fact else {
        panic!("expected Diagnostic Study");
    };
    assert_eq!(study, &expected);
    assert_ne!(study.findings[0].text, study.impression.as_deref().unwrap());

    Ok(())
}

#[test]
fn ordered_study_cannot_claim_results() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let error = workspace
        .record()
        .propose(RecordChangeSet {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-ordered-study-001")?,
            evidence_origin: EvidenceOrigin::ExplicitSelfReport {
                reporter: reporter.clone(),
            },
            extractor: ExtractorIdentity {
                name: "synthetic-study-intake".to_owned(),
                version: "1.0.0".to_owned(),
            },
            extraction_run: None,
            changes: vec![RecordChange::AddFact(FactDraft {
                clinical_time: ClinicalTime::Undated { source_text: None },
                evidence: FactEvidence::SelfReport { reporter },
                fact: FactData::DiagnosticStudy(DiagnosticStudy {
                    kind: DiagnosticStudyKind::Imaging,
                    status: DiagnosticStudyStatus::Ordered,
                    name: "Synthetic Ordered Study".to_owned(),
                    body_site: None,
                    method: None,
                    findings: vec![DiagnosticFinding {
                        section: None,
                        body_site: None,
                        text: "Synthetic result that cannot exist yet".to_owned(),
                    }],
                    impression: None,
                    resulted_at: None,
                    ordering_provider: None,
                    interpreting_provider: None,
                    performing_organization: None,
                }),
            })],
        })
        .expect_err("an ordered study must not carry results");

    assert_eq!(error.code(), "invalid_record_change_set");
    assert!(workspace.record().query(FactQuery::default())?.is_empty());
    Ok(())
}
