use std::fs;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AnalysisFreshness, AnalysisOptions, AuthorIdentity, AuthorKind, ClaimKind, ClinicalTime,
    ConfidenceLevel, DiscrepancyResolutionDraft, EvidenceOrigin, ExtractionCoverage,
    ExtractionDomain, ExtractionRunDraft, ExtractorIdentity, FactData, FactDisposition, FactDraft,
    FactEvidence, FactQuery, FactRestriction, IdempotencyKey, LabResult, LabValue, RecordChange,
    RecordChangeSet, RecordRevision, SourceDescriptor, SourceRegion, SubjectId, VerificationDraft,
    VerificationMethod, VerificationOutcome, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(clippy::too_many_lines)]
fn verification_and_discrepancy_change_analysis_evidence_use() -> TestResult {
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
        idempotency_key: IdempotencyKey::parse("synthetic-analysis-lab-001")?,
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
                source_id: source.source_id.clone(),
                source_region: region.clone(),
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
                performing_lab: Some("Synthetic Lab".to_owned()),
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;
    let fact = workspace.record().query(FactQuery::default())?.remove(0);

    let initial = workspace.analyze(AnalysisOptions::default())?;

    assert_eq!(initial.record_revision.value(), 1);
    assert_eq!(initial.freshness, AnalysisFreshness::Fresh);
    assert_eq!(initial.claims.len(), 1);
    assert_eq!(initial.claims[0].kind, ClaimKind::Finding);
    assert_eq!(initial.claims[0].confidence, ConfidenceLevel::Low);
    assert_eq!(initial.claims[0].evidence_fact_ids, vec![fact.id.clone()]);
    assert!(initial.claims[0].evidence_verification_ids.is_empty());
    assert_eq!(initial.warnings.len(), 1);
    assert_eq!(initial.warnings[0].code, "unverified_evidence");
    assert_eq!(
        workspace.analyses().status(&initial.id)?,
        AnalysisFreshness::Fresh
    );

    let identity = workspace.status()?;
    let verification_proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-analysis-verification-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(VerificationDraft {
            fact_id: fact.id.clone(),
            source_id: source.source_id,
            source_region: region,
            verifier: AuthorIdentity {
                kind: AuthorKind::Agent,
                identifier: "synthetic-source-verifier".to_owned(),
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
    let receipt = workspace.record().apply(verification_proposal)?;

    assert_eq!(receipt.newly_stale_analysis_ids, vec![initial.id.clone()]);
    assert_eq!(
        workspace.analyses().status(&initial.id)?,
        AnalysisFreshness::Stale
    );
    let current = workspace.analyze(AnalysisOptions::default())?;
    assert_ne!(current.id, initial.id);
    assert_eq!(current.record_revision.value(), 2);
    assert_eq!(current.freshness, AnalysisFreshness::Fresh);
    assert_eq!(current.claims.len(), 1);
    assert_eq!(current.claims[0].confidence, ConfidenceLevel::High);
    assert_eq!(current.claims[0].evidence_fact_ids, vec![fact.id.clone()]);
    assert_eq!(current.claims[0].evidence_verification_ids.len(), 1);
    assert!(current.warnings.is_empty());

    let identity = workspace.status()?;
    let fact_id = fact.id.clone();
    let (fact_source_id, fact_source_region) =
        fact.draft.evidence.source().expect("source-backed fact");
    let fact_source_id = fact_source_id.clone();
    let fact_source_region = fact_source_region.clone();
    let discrepancy_proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-analysis-discrepancy-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: fact_source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(VerificationDraft {
            fact_id: fact_id.clone(),
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
            checked_at: DateTime::parse_from_rfc3339("2026-01-17T12:00:00Z")?.with_timezone(&Utc),
            scope: "single_lab_result".to_owned(),
            outcome: VerificationOutcome::Discrepancy,
        })],
    })?;
    workspace.record().apply(discrepancy_proposal)?;

    let quarantined_fact = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(quarantined_fact.disposition, FactDisposition::Quarantined);
    assert_eq!(
        quarantined_fact.restrictions,
        vec![FactRestriction::SourceDiscrepancy {
            verification_id: verification_history_id(&workspace, &fact.id)?,
        }]
    );
    let quarantined = workspace.analyze(AnalysisOptions::default())?;

    assert!(quarantined.claims.is_empty());
    assert_eq!(quarantined.warnings.len(), 1);
    assert_eq!(quarantined.warnings[0].code, "quarantined_evidence");
    assert_eq!(quarantined.warnings[0].related_fact_ids, vec![fact_id]);
    let verification_history = workspace.record().verifications(&fact.id)?;
    assert_eq!(verification_history.len(), 2);
    assert_eq!(
        verification_history[1].draft.outcome,
        VerificationOutcome::Discrepancy
    );
    let discrepancy_id = verification_history[1].id.clone();
    let identity = workspace.status()?;
    let follow_up = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-analysis-follow-up-check-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: fact_source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.1".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(VerificationDraft {
            fact_id: fact.id.clone(),
            source_id: fact_source_id,
            source_region: fact_source_region,
            verifier: AuthorIdentity {
                kind: AuthorKind::Agent,
                identifier: "synthetic-source-verifier".to_owned(),
            },
            method: VerificationMethod {
                name: "exact-location-source-comparison".to_owned(),
                version: "1.0.1".to_owned(),
            },
            checked_at: DateTime::parse_from_rfc3339("2026-01-18T12:00:00Z")?.with_timezone(&Utc),
            scope: "single_lab_result".to_owned(),
            outcome: VerificationOutcome::Verified,
        })],
    })?;
    let follow_up_receipt = workspace.record().apply(follow_up)?;

    let still_quarantined = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(still_quarantined.disposition, FactDisposition::Quarantined);
    assert!(
        still_quarantined
            .restrictions
            .contains(&FactRestriction::SourceDiscrepancy {
                verification_id: discrepancy_id.clone(),
            })
    );
    assert!(
        workspace
            .analyze(AnalysisOptions::default())?
            .claims
            .is_empty()
    );

    let reviewer = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-record-reviewer".to_owned(),
    };
    let identity = workspace.status()?;
    let resolution = DiscrepancyResolutionDraft {
        discrepancy_verification_id: discrepancy_id.clone(),
        supporting_verification_id: follow_up_receipt.accepted_verification_ids[0].clone(),
        rationale: "repeat_source_check_resolved_transcription_ambiguity".to_owned(),
        author: reviewer.clone(),
        resolved_at: DateTime::parse_from_rfc3339("2026-01-18T12:05:00Z")?.with_timezone(&Utc),
    };
    let resolution_proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-discrepancy-resolution-001")?,
        evidence_origin: EvidenceOrigin::RecordReview { reviewer },
        extractor: ExtractorIdentity {
            name: "synthetic-record-reviewer".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::ResolveDiscrepancy(resolution.clone())],
    })?;
    let resolution_receipt = workspace.record().apply(resolution_proposal)?;

    assert_eq!(
        resolution_receipt.accepted_discrepancy_resolution_ids.len(),
        1
    );
    let resolved_fact = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(resolved_fact.disposition, FactDisposition::Active);
    assert!(resolved_fact.restrictions.is_empty());
    assert_eq!(
        resolved_fact.current_assurance,
        health_engine::EvidenceAssurance::SourceVerified
    );
    let resolutions = workspace
        .record()
        .discrepancy_resolutions(&discrepancy_id)?;
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0].draft, resolution);
    let resolved_analysis = workspace.analyze(AnalysisOptions::default())?;
    assert_eq!(resolved_analysis.claims.len(), 1);
    assert!(resolved_analysis.warnings.is_empty());

    Ok(())
}

fn verification_history_id(
    workspace: &Workspace,
    fact_id: &health_engine::FactId,
) -> Result<health_engine::VerificationId, Box<dyn std::error::Error>> {
    Ok(workspace
        .record()
        .verifications(fact_id)?
        .last()
        .expect("discrepancy Verification")
        .id
        .clone())
}
