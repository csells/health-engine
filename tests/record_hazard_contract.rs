use std::fs;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin, ExtractionCoverage,
    ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData, FactDisposition, FactDraft,
    FactEvidence, FactQuery, FactRestriction, HazardVerificationState, IdempotencyKey, LabResult,
    LabValue, RecordChange, RecordChangeSet, RecordHazardDraft, RecordHazardResolutionDraft,
    SourceDescriptor, SourceRegion, SubjectId, VerificationDraft, VerificationMethod,
    VerificationOutcome, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(clippy::too_many_lines)]
fn open_record_hazard_follows_fact_and_excludes_it_from_analysis() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(root.path().join("sources/chart.txt"), b"synthetic chart\n")?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/chart.txt".to_owned(),
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
        idempotency_key: IdempotencyKey::parse("synthetic-hazard-fact-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-chart-extractor".to_owned(),
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
                    identifier: "synthetic-chart-extractor".to_owned(),
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
    let reviewer = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-record-reviewer".to_owned(),
    };
    let hazard = RecordHazardDraft {
        affected_fact_ids: vec![fact.id.clone()],
        affected_source_ids: vec![source.source_id],
        problematic_statement: "synthetic chart statement".to_owned(),
        danger: "could propagate a synthetic error".to_owned(),
        corrected_understanding: "statement requires independent confirmation".to_owned(),
        author: reviewer.clone(),
        recorded_at: DateTime::parse_from_rfc3339("2026-01-19T12:00:00Z")?.with_timezone(&Utc),
        verification_state: HazardVerificationState::Suspected,
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-hazard-001")?,
        evidence_origin: EvidenceOrigin::RecordReview { reviewer },
        extractor: ExtractorIdentity {
            name: "synthetic-record-reviewer".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddRecordHazard(hazard.clone())],
    })?;

    assert_eq!(proposal.impact().record_hazards, 1);
    let receipt = workspace.record().apply(proposal)?;
    let hazard_id = receipt.accepted_record_hazard_ids[0].clone();

    let current = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(current.id, fact.id);
    assert_eq!(current.disposition, FactDisposition::Quarantined);
    assert_eq!(
        current.restrictions,
        vec![FactRestriction::RecordHazard {
            hazard_id: hazard_id.clone(),
        }]
    );
    let hazards = workspace.record().hazards(&fact.id)?;
    assert_eq!(hazards.len(), 1);
    assert_eq!(hazards[0].id, hazard_id);
    assert_eq!(hazards[0].draft, hazard);
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    assert!(analysis.claims.is_empty());
    assert_eq!(analysis.warnings.len(), 1);
    assert_eq!(analysis.warnings[0].code, "record_hazard");
    assert_eq!(analysis.warnings[0].related_fact_ids, vec![fact.id.clone()]);

    let (fact_source_id, fact_source_region) =
        fact.draft.evidence.source().expect("source-backed fact");
    let identity = workspace.status()?;
    let verification = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-hazard-verification-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: fact_source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(VerificationDraft {
            fact_id: fact.id.clone(),
            source_id: fact_source_id.clone(),
            source_region: fact_source_region.clone(),
            verifier: AuthorIdentity {
                kind: AuthorKind::Agent,
                identifier: "synthetic-source-verifier".to_owned(),
            },
            method: VerificationMethod {
                name: "exact-location-source-comparison".to_owned(),
                version: "1.0.0".to_owned(),
            },
            checked_at: DateTime::parse_from_rfc3339("2026-01-20T12:00:00Z")?.with_timezone(&Utc),
            scope: "single_lab_result".to_owned(),
            outcome: VerificationOutcome::Verified,
        })],
    })?;
    let verification_receipt = workspace.record().apply(verification)?;
    assert_eq!(
        workspace
            .record()
            .query(FactQuery::default())?
            .remove(0)
            .disposition,
        FactDisposition::Quarantined
    );
    let reviewer = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-record-reviewer".to_owned(),
    };
    let resolution = RecordHazardResolutionDraft {
        hazard_id: hazard_id.clone(),
        supporting_verification_id: verification_receipt.accepted_verification_ids[0].clone(),
        rationale: "independent_source_review_addressed_hazard".to_owned(),
        author: reviewer.clone(),
        resolved_at: DateTime::parse_from_rfc3339("2026-01-20T12:05:00Z")?.with_timezone(&Utc),
    };
    let identity = workspace.status()?;
    let resolution_proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-hazard-resolution-001")?,
        evidence_origin: EvidenceOrigin::RecordReview { reviewer },
        extractor: ExtractorIdentity {
            name: "synthetic-record-reviewer".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::ResolveRecordHazard(resolution.clone())],
    })?;
    let resolution_receipt = workspace.record().apply(resolution_proposal)?;

    assert_eq!(
        resolution_receipt
            .accepted_record_hazard_resolution_ids
            .len(),
        1
    );
    let resolved = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(resolved.disposition, FactDisposition::Active);
    assert!(resolved.restrictions.is_empty());
    let resolutions = workspace.record().hazard_resolutions(&hazard_id)?;
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0].draft, resolution);
    assert_eq!(
        workspace.analyze(AnalysisOptions::default())?.claims.len(),
        1
    );

    let reviewer = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-record-reviewer".to_owned(),
    };
    let make_hazard = |problematic_statement: &str, danger: &str| RecordHazardDraft {
        affected_fact_ids: vec![fact.id.clone()],
        affected_source_ids: vec![fact_source_id.clone()],
        problematic_statement: problematic_statement.to_owned(),
        danger: danger.to_owned(),
        corrected_understanding: "requires separate source confirmation".to_owned(),
        author: reviewer.clone(),
        recorded_at: DateTime::parse_from_rfc3339("2026-01-21T12:00:00Z")
            .expect("valid synthetic time")
            .with_timezone(&Utc),
        verification_state: HazardVerificationState::Suspected,
    };
    let identity = workspace.status()?;
    let multiple_hazards = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-multiple-hazards-001")?,
        evidence_origin: EvidenceOrigin::RecordReview {
            reviewer: reviewer.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-record-reviewer".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![
            RecordChange::AddRecordHazard(make_hazard(
                "first independent synthetic hazard",
                "first distinct propagation risk",
            )),
            RecordChange::AddRecordHazard(make_hazard(
                "second independent synthetic hazard",
                "second distinct propagation risk",
            )),
        ],
    })?;
    let multiple_receipt = workspace.record().apply(multiple_hazards)?;
    assert_eq!(multiple_receipt.accepted_record_hazard_ids.len(), 2);
    let restricted = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(restricted.restrictions.len(), 2);
    for hazard_id in &multiple_receipt.accepted_record_hazard_ids {
        assert!(
            restricted
                .restrictions
                .contains(&FactRestriction::RecordHazard {
                    hazard_id: hazard_id.clone(),
                })
        );
    }
    let analysis = workspace.analyze(AnalysisOptions::default())?;
    assert!(analysis.claims.is_empty());
    assert_eq!(
        analysis
            .warnings
            .iter()
            .filter(|warning| warning.code == "record_hazard")
            .count(),
        2
    );

    Ok(())
}
