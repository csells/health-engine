use std::fs;
use std::str::FromStr;
use std::sync::{Arc, Barrier};
use std::thread;

use chrono::{DateTime, NaiveDate, Utc};
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, ClinicalTime, CorrectionDraft, EvidenceAssurance,
    EvidenceOrigin, ExtractionCoverage, ExtractionDomain, ExtractionRunDraft, ExtractorIdentity,
    FactData, FactDisposition, FactDraft, FactEvidence, FactQuery, FactRestriction, IdempotencyKey,
    LabResult, LabValue, RecordChange, RecordChangeSet, RecordProposal, RecordRevision,
    ReferenceRange, SourceDescriptor, SourceRegion, SubjectId, VerificationDraft,
    VerificationMethod, VerificationOutcome, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn prepare_lab_proposal()
-> Result<(tempfile::TempDir, Workspace, RecordProposal, FactDraft), Box<dyn std::error::Error>> {
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
    let fact = FactDraft {
        clinical_time: ClinicalTime::Date {
            value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
            source_text: Some("2026-01-15".to_owned()),
        },
        evidence: FactEvidence::Source {
            source_id: source.source_id.clone(),
            source_region: source_region.clone(),
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
            reference_range: Some(ReferenceRange {
                original: "5-15".to_owned(),
                lower: Some(Decimal::from_str("5")?),
                upper: Some(Decimal::from_str("15")?),
            }),
            reported_flag: Some("normal".to_owned()),
            performing_lab: Some("Synthetic Lab".to_owned()),
        }),
    };
    let change_set = RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: RecordRevision::INITIAL,
        idempotency_key: IdempotencyKey::parse("synthetic-lab-command-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-lab-extractor".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source.source_id,
            coverage: ExtractionCoverage {
                regions: vec![source_region],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec![],
        }),
        changes: vec![RecordChange::AddFact(fact.clone())],
    };
    let proposal = workspace.record().propose(change_set)?;
    Ok((root, workspace, proposal, fact))
}

#[test]
fn record_proposal_previews_lab_without_mutating_the_record() -> TestResult {
    let (_root, workspace, proposal, _fact) = prepare_lab_proposal()?;

    assert_eq!(proposal.expected_revision(), RecordRevision::INITIAL);
    assert_eq!(proposal.impact().extraction_runs, 1);
    assert_eq!(proposal.impact().health_facts, 1);
    assert_eq!(workspace.status()?.record_revision, RecordRevision::INITIAL);
    assert!(workspace.record().query(FactQuery::default())?.is_empty());

    Ok(())
}

#[test]
fn record_apply_commits_extraction_run_and_lab_atomically() -> TestResult {
    let (_root, workspace, proposal, expected_fact) = prepare_lab_proposal()?;

    let receipt = workspace.record().apply(proposal)?;

    assert_eq!(receipt.previous_revision, RecordRevision::INITIAL);
    assert_eq!(receipt.record_revision.value(), 1);
    assert_eq!(receipt.accepted_extraction_run_ids.len(), 1);
    assert_eq!(receipt.accepted_fact_ids.len(), 1);
    assert!(receipt.newly_stale_analysis_ids.is_empty());
    let facts = workspace.record().query(FactQuery::default())?;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].draft, expected_fact);
    assert_eq!(facts[0].record_revision.value(), 1);
    assert_eq!(
        facts[0].extraction_run_id.as_ref(),
        Some(&receipt.accepted_extraction_run_ids[0])
    );
    assert_eq!(receipt.accepted_fact_ids[0], facts[0].id.as_str());
    assert_eq!(workspace.status()?.record_revision.value(), 1);

    Ok(())
}

#[test]
fn later_run_reports_the_same_source_assertion_as_already_present() -> TestResult {
    let (_root, workspace, proposal, mut fact) = prepare_lab_proposal()?;
    let first_receipt = workspace.record().apply(proposal)?;
    let (source_id, source_region) = fact.evidence.source().expect("source-backed fact");
    let source_id = source_id.clone();
    let source_region = source_region.clone();
    let FactEvidence::Source { asserted_by, .. } = &mut fact.evidence else {
        unreachable!("prepared Fact is source-backed");
    };
    asserted_by.identifier = "independent-synthetic-extractor".to_owned();
    let identity = workspace.status()?;
    let second_proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-lab-command-002")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "independent-synthetic-extractor".to_owned(),
            version: "1.0.1".to_owned(),
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
        changes: vec![RecordChange::AddFact(fact.clone())],
    })?;
    assert_eq!(second_proposal.impact().health_facts, 0);
    assert_eq!(second_proposal.impact().already_present_facts, 1);

    let second_receipt = workspace.record().apply(second_proposal)?;

    assert_eq!(second_receipt.accepted_extraction_run_ids.len(), 1);
    assert!(second_receipt.accepted_fact_ids.is_empty());
    assert_eq!(
        second_receipt.already_present_fact_ids,
        first_receipt.accepted_fact_ids
    );
    assert_eq!(
        workspace
            .record()
            .query(FactQuery {
                kind: None,
                include_history: true,
            })?
            .len(),
        1
    );
    assert_eq!(workspace.sources().extraction_runs(&source_id)?.len(), 2);
    assert_eq!(first_receipt.accepted_fact_ids.len(), 1);

    Ok(())
}

#[test]
fn stale_concurrent_proposal_cannot_partially_commit() -> TestResult {
    let (root, workspace_a, proposal_a, fact_a) = prepare_lab_proposal()?;
    let workspace_b = Workspace::open(root.path())?;
    let identity = workspace_b.status()?;
    let (source_id, source_region) = fact_a.evidence.source().expect("source-backed fact");
    let mut fact_b = fact_a.clone();
    match &mut fact_b.fact {
        FactData::LabResult(lab) => {
            lab.value = LabValue::Numeric {
                value: Decimal::from_str("11")?,
            };
        }
        FactData::VitalMeasurement(_)
        | FactData::MedicationEvent(_)
        | FactData::ConditionAssertion(_)
        | FactData::DiagnosticStudy(_)
        | FactData::CareTask(_)
        | FactData::SubjectPreference(_) => {
            unreachable!("lab fixture")
        }
    }
    let proposal_b = workspace_b.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-concurrent-command-002")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-lab-extractor".to_owned(),
            version: "1.0.1".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source_id.clone(),
            coverage: ExtractionCoverage {
                regions: vec![source_region.clone()],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec!["synthetic-marker-second".to_owned()],
        }),
        changes: vec![RecordChange::AddFact(fact_b)],
    })?;

    workspace_a.record().apply(proposal_a)?;
    let error = workspace_b
        .record()
        .apply(proposal_b)
        .expect_err("stale proposal must not commit");

    assert_eq!(error.code(), "record_revision_conflict");
    assert_eq!(workspace_b.status()?.record_revision.value(), 1);
    assert_eq!(
        workspace_b
            .record()
            .query(FactQuery {
                kind: None,
                include_history: true,
            })?
            .len(),
        1
    );
    assert_eq!(workspace_b.sources().extraction_runs(source_id)?.len(), 1);

    Ok(())
}

#[test]
fn simultaneous_writers_produce_one_commit_and_one_revision_conflict() -> TestResult {
    let (root, workspace, _unused_proposal, template) = prepare_lab_proposal()?;
    drop(workspace);
    let workspace_path = root.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for index in 0..2 {
        let workspace_path = workspace_path.clone();
        let barrier = Arc::clone(&barrier);
        let mut fact = template.clone();
        match &mut fact.fact {
            FactData::LabResult(lab) => {
                lab.value = LabValue::Numeric {
                    value: Decimal::new(10 + index, 0),
                };
            }
            FactData::VitalMeasurement(_)
            | FactData::MedicationEvent(_)
            | FactData::ConditionAssertion(_)
            | FactData::DiagnosticStudy(_)
            | FactData::CareTask(_)
            | FactData::SubjectPreference(_) => {
                unreachable!("lab fixture")
            }
        }
        handles.push(thread::spawn(move || -> Result<u64, String> {
            let workspace = Workspace::open(&workspace_path).map_err(|error| error.code())?;
            let identity = workspace.status().map_err(|error| error.code())?;
            let (source_id, source_region) = fact
                .evidence
                .source()
                .expect("prepared Fact is source-backed");
            let proposal = workspace
                .record()
                .propose(RecordChangeSet {
                    schema_version: 1,
                    workspace_id: identity.workspace_id,
                    subject_id: identity.subject_id,
                    expected_revision: identity.record_revision,
                    idempotency_key: IdempotencyKey::parse(&format!(
                        "synthetic-simultaneous-writer-{index}"
                    ))
                    .map_err(|error| error.code())?,
                    evidence_origin: EvidenceOrigin::DocumentExtraction {
                        source_id: source_id.clone(),
                    },
                    extractor: ExtractorIdentity {
                        name: format!("synthetic-writer-{index}"),
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
                        resulting_candidate_keys: vec![format!("synthetic-writer-{index}")],
                    }),
                    changes: vec![RecordChange::AddFact(fact)],
                })
                .map_err(|error| error.code())?;
            barrier.wait();
            workspace
                .record()
                .apply(proposal)
                .map(|receipt| receipt.record_revision.value())
                .map_err(|error| error.code().to_owned())
        }));
    }
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread must not panic"))
        .collect::<Vec<_>>();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| result
                .as_ref()
                .is_err_and(|code| code == "record_revision_conflict"))
            .count(),
        1,
        "writer outcomes: {results:?}"
    );
    let workspace = Workspace::open(root.path())?;
    assert_eq!(workspace.status()?.record_revision.value(), 1);
    assert_eq!(
        workspace
            .record()
            .query(FactQuery {
                kind: None,
                include_history: true,
            })?
            .len(),
        1
    );

    Ok(())
}

#[test]
fn source_verification_is_append_only_and_updates_current_assurance() -> TestResult {
    let (root, workspace, proposal, _expected_fact) = prepare_lab_proposal()?;
    workspace.record().apply(proposal)?;
    let before = workspace.record().query(FactQuery::default())?;
    let fact = before.first().expect("committed lab fact");
    assert_eq!(fact.current_assurance, EvidenceAssurance::Unverified);
    assert!(workspace.record().verifications(&fact.id)?.is_empty());
    let identity = workspace.status()?;
    let (source_id, source_region) = fact.draft.evidence.source().expect("source-backed fact");
    let source_id = source_id.clone();
    let verification = VerificationDraft {
        fact_id: fact.id.clone(),
        source_id: source_id.clone(),
        source_region: source_region.clone(),
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
    };
    let change_set = RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-lab-verification-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(verification.clone())],
    };

    let proposal = workspace.record().propose(change_set)?;
    assert_eq!(proposal.impact().verifications, 1);
    assert_eq!(workspace.status()?.record_revision.value(), 1);
    assert!(workspace.record().verifications(&fact.id)?.is_empty());
    let receipt = workspace.record().apply(proposal)?;

    assert_eq!(receipt.record_revision.value(), 2);
    assert_eq!(receipt.accepted_verification_ids.len(), 1);
    let current = workspace.record().query(FactQuery::default())?;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].original_assurance, EvidenceAssurance::Unverified);
    assert_eq!(
        current[0].current_assurance,
        EvidenceAssurance::SourceVerified
    );
    let history = workspace.record().verifications(&current[0].id)?;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].draft, verification);
    assert_eq!(history[0].record_revision.value(), 2);

    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-v2\n",
    )?;
    let drifted = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(drifted.disposition, FactDisposition::Quarantined);
    assert_eq!(
        drifted.restrictions,
        vec![FactRestriction::SourceContentChanged {
            source_id: source_id.clone(),
        }]
    );
    let drifted_analysis = workspace.analyze(AnalysisOptions::default())?;
    assert!(drifted_analysis.claims.is_empty());
    assert_eq!(drifted_analysis.warnings.len(), 1);
    assert_eq!(drifted_analysis.warnings[0].code, "source_content_changed");
    assert_eq!(
        drifted_analysis.warnings[0].related_fact_ids,
        vec![fact.id.clone()]
    );

    fs::remove_file(root.path().join("sources/lab-a.txt"))?;
    let unavailable = workspace.record().query(FactQuery::default())?.remove(0);
    assert_eq!(unavailable.disposition, FactDisposition::Quarantined);
    assert_eq!(
        unavailable.restrictions,
        vec![FactRestriction::SourceContentUnavailable {
            source_id: source_id.clone(),
        }]
    );
    let unavailable_analysis = workspace.analyze(AnalysisOptions::default())?;
    assert!(unavailable_analysis.claims.is_empty());
    assert_eq!(unavailable_analysis.warnings.len(), 1);
    assert_eq!(
        unavailable_analysis.warnings[0].code,
        "source_content_unavailable"
    );

    Ok(())
}

#[test]
fn source_verification_must_match_the_fact_location() -> TestResult {
    let (_root, workspace, proposal, _expected_fact) = prepare_lab_proposal()?;
    workspace.record().apply(proposal)?;
    let fact = workspace.record().query(FactQuery::default())?.remove(0);
    let (source_id, _) = fact.draft.evidence.source().expect("source-backed fact");
    let source_id = source_id.clone();
    let identity = workspace.status()?;

    let error = workspace
        .record()
        .propose(RecordChangeSet {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-wrong-location-verification-001")?,
            evidence_origin: EvidenceOrigin::SourceVerification {
                source_id: source_id.clone(),
            },
            extractor: ExtractorIdentity {
                name: "synthetic-source-verifier".to_owned(),
                version: "1.0.0".to_owned(),
            },
            extraction_run: None,
            changes: vec![RecordChange::VerifyFact(VerificationDraft {
                fact_id: fact.id.clone(),
                source_id,
                source_region: SourceRegion {
                    locator: "line:999".to_owned(),
                },
                verifier: AuthorIdentity {
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-source-verifier".to_owned(),
                },
                method: VerificationMethod {
                    name: "exact-location-source-comparison".to_owned(),
                    version: "1.0.0".to_owned(),
                },
                checked_at: DateTime::parse_from_rfc3339("2026-01-16T12:00:00Z")?
                    .with_timezone(&Utc),
                scope: "single_lab_result".to_owned(),
                outcome: VerificationOutcome::Verified,
            })],
        })
        .expect_err("a different Source location cannot verify the Fact");

    assert_eq!(error.code(), "invalid_record_change_set");
    assert_eq!(
        workspace.status()?.record_revision,
        identity.record_revision
    );
    assert!(workspace.record().verifications(&fact.id)?.is_empty());

    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn correction_replaces_the_current_lab_without_erasing_history() -> TestResult {
    let (_root, workspace, proposal, _expected_fact) = prepare_lab_proposal()?;
    workspace.record().apply(proposal)?;
    let original = workspace.record().query(FactQuery::default())?.remove(0);
    let (source_id, source_region) = original
        .draft
        .evidence
        .source()
        .expect("source-backed fact");
    let source_id = source_id.clone();
    let identity = workspace.status()?;
    let verification = VerificationDraft {
        fact_id: original.id.clone(),
        source_id: source_id.clone(),
        source_region: source_region.clone(),
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
    };
    let verification_proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-correction-verification-001")?,
        evidence_origin: EvidenceOrigin::SourceVerification {
            source_id: source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::VerifyFact(verification)],
    })?;
    let verification_receipt = workspace.record().apply(verification_proposal)?;
    let verification_id = verification_receipt.accepted_verification_ids[0].clone();
    let mut replacement = original.draft.clone();
    match &mut replacement.fact {
        FactData::LabResult(lab) => {
            lab.value = LabValue::Numeric {
                value: Decimal::from_str("12")?,
            };
        }
        FactData::VitalMeasurement(_)
        | FactData::MedicationEvent(_)
        | FactData::ConditionAssertion(_)
        | FactData::DiagnosticStudy(_)
        | FactData::CareTask(_)
        | FactData::SubjectPreference(_) => {
            unreachable!("lab fixture")
        }
    }
    let correction = CorrectionDraft {
        prior_fact_id: original.id.clone(),
        replacement: replacement.clone(),
        reason: "source_transcription_error".to_owned(),
        author: AuthorIdentity {
            kind: AuthorKind::Agent,
            identifier: "synthetic-source-verifier".to_owned(),
        },
        corrected_at: DateTime::parse_from_rfc3339("2026-01-16T12:05:00Z")?.with_timezone(&Utc),
        supporting_verification_id: verification_id.clone(),
    };
    let identity = workspace.status()?;
    let invalid_error = workspace
        .record()
        .propose(RecordChangeSet {
            schema_version: 1,
            workspace_id: identity.workspace_id.clone(),
            subject_id: identity.subject_id.clone(),
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-duplicate-correction-001")?,
            evidence_origin: EvidenceOrigin::SourceCorrection {
                source_id: source_id.clone(),
                verification_id: verification_id.clone(),
            },
            extractor: ExtractorIdentity {
                name: "synthetic-source-verifier".to_owned(),
                version: "1.0.0".to_owned(),
            },
            extraction_run: None,
            changes: vec![
                RecordChange::CorrectFact(correction.clone()),
                RecordChange::CorrectFact(correction.clone()),
            ],
        })
        .and_then(|proposal| workspace.record().apply(proposal))
        .expect_err("one batch cannot correct the same Fact twice");

    assert_eq!(invalid_error.code(), "invalid_record_change_set");
    assert_eq!(
        workspace.status()?.record_revision,
        identity.record_revision
    );
    assert_eq!(
        workspace
            .record()
            .query(FactQuery {
                kind: None,
                include_history: true,
            })?
            .len(),
        1
    );
    assert!(workspace.record().corrections(&original.id)?.is_empty());

    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-lab-correction-001")?,
        evidence_origin: EvidenceOrigin::SourceCorrection {
            source_id,
            verification_id,
        },
        extractor: ExtractorIdentity {
            name: "synthetic-source-verifier".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::CorrectFact(correction.clone())],
    })?;

    assert_eq!(proposal.impact().corrections, 1);
    assert_eq!(proposal.impact().health_facts, 1);
    let receipt = workspace.record().apply(proposal)?;

    assert_eq!(receipt.record_revision.value(), 3);
    assert_eq!(receipt.accepted_correction_ids.len(), 1);
    assert_eq!(receipt.accepted_fact_ids.len(), 1);
    let current = workspace.record().query(FactQuery::default())?;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].draft, replacement);
    assert_eq!(current[0].id.as_str(), receipt.accepted_fact_ids[0]);
    let history = workspace.record().query(FactQuery {
        kind: None,
        include_history: true,
    })?;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, original.id);
    assert_eq!(history[0].draft, original.draft);
    assert_eq!(history[1].id, current[0].id);
    let corrections = workspace.record().corrections(&history[0].id)?;
    assert_eq!(corrections.len(), 1);
    assert_eq!(corrections[0].draft, correction);
    assert_eq!(corrections[0].replacement_fact_id, history[1].id);

    Ok(())
}
