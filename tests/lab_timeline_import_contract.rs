use std::fs;

use chrono::NaiveDate;
use health_engine::{
    AnalysisOptions, AuthorIdentity, AuthorKind, CanonicalTest, ClinicalTime, EvidenceAssurance,
    ExtractorIdentity, FactData, FactEvidence, FactQuery, IdempotencyKey, LabComparator,
    LabTestMapping, LabTimelineImportRequest, LabTimelineQuery, LabValue,
    MigrationCandidateDisposition, MigrationCandidateReason, SourceDescriptor, SubjectId,
    Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn lab_timeline_import_preserves_qualified_value_and_cited_source() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-panel.pdf"),
        b"synthetic PDF fixture without health information",
    )?;
    fs::write(
        root.path().join("sources/synthetic-labs.csv"),
        concat!(
            "Date,Test,Value,Units,Reference Range,Flag,Source File\n",
            "2026-01-15,\"Synthetic, Qualified Marker\",\"<5.0\",mg/dL,\"<=10.0\",H,",
            "sources/synthetic-panel.pdf\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let cited_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-panel.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    })?;
    let parsed_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-labs.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;

    let proposal = workspace
        .record()
        .propose_lab_timeline_import(LabTimelineImportRequest {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-lab-timeline-import-001")?,
            source_id: parsed_source.source_id.clone(),
            asserted_by: AuthorIdentity {
                kind: AuthorKind::Application,
                identifier: "synthetic-migration-adapter".to_owned(),
            },
            extractor: ExtractorIdentity {
                name: "health-engine-lab-timeline".to_owned(),
                version: "1.0.0".to_owned(),
            },
            test_mappings: vec![LabTestMapping {
                reported_name: "Synthetic, Qualified Marker".to_owned(),
                canonical_test: CanonicalTest {
                    identifier: "synthetic-qualified-marker".to_owned(),
                    display_name: "Synthetic Qualified Marker".to_owned(),
                },
            }],
        })?;

    assert_eq!(proposal.impact().health_facts, 1);
    assert!(workspace.record().query(FactQuery::default())?.is_empty());

    workspace.record().apply(proposal)?;
    let facts = workspace.record().query(FactQuery::default())?;
    let fact = &facts[0];
    assert_eq!(fact.original_assurance, EvidenceAssurance::ParserValidated);
    let FactEvidence::ParsedSource {
        parsed_source_id,
        parsed_region,
        cited_source_id,
        cited_region,
        ..
    } = &fact.draft.evidence
    else {
        panic!("expected parsed Source evidence");
    };
    assert_eq!(parsed_source_id, &parsed_source.source_id);
    assert_eq!(parsed_region.locator, "row:2");
    assert_eq!(cited_source_id, &cited_source.source_id);
    assert_eq!(cited_region.locator, "document");

    let FactData::LabResult(lab) = &fact.draft.fact else {
        panic!("expected lab result");
    };
    assert_eq!(lab.test_name, "Synthetic, Qualified Marker");
    assert_eq!(
        lab.canonical_test,
        Some(CanonicalTest {
            identifier: "synthetic-qualified-marker".to_owned(),
            display_name: "Synthetic Qualified Marker".to_owned(),
        })
    );
    assert_eq!(
        lab.value,
        LabValue::QualifiedNumeric {
            comparator: LabComparator::LessThan,
            value: Decimal::new(50, 1),
            original: "<5.0".to_owned(),
        }
    );
    assert_eq!(
        lab.reference_range
            .as_ref()
            .map(|range| range.original.as_str()),
        Some("<=10.0")
    );

    Ok(())
}

#[test]
fn unresolved_cited_source_is_a_durable_quarantined_candidate() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-unresolved-labs.csv"),
        concat!(
            "Date,Test,Value,Units,Reference Range,Flag,Source File\n",
            "2026-01-15,Synthetic Marker,5.0,mg/dL,,,sources/unresolved-synthetic.pdf\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let parsed_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-unresolved-labs.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let extractor = ExtractorIdentity {
        name: "health-engine-lab-timeline".to_owned(),
        version: "1.0.0".to_owned(),
    };

    let proposal = workspace
        .record()
        .propose_lab_timeline_import(LabTimelineImportRequest {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-lab-unresolved-source-001")?,
            source_id: parsed_source.source_id.clone(),
            asserted_by: AuthorIdentity {
                kind: AuthorKind::Application,
                identifier: "synthetic-migration-adapter".to_owned(),
            },
            extractor: extractor.clone(),
            test_mappings: vec![LabTestMapping {
                reported_name: "Synthetic Marker".to_owned(),
                canonical_test: CanonicalTest {
                    identifier: "synthetic-marker".to_owned(),
                    display_name: "Synthetic Marker".to_owned(),
                },
            }],
        })?;

    assert_eq!(proposal.impact().health_facts, 0);
    workspace.record().apply(proposal)?;
    assert!(workspace.record().query(FactQuery::default())?.is_empty());

    let candidates = workspace
        .record()
        .migration_candidates(&parsed_source.source_id)?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].disposition,
        MigrationCandidateDisposition::Quarantined
    );
    assert_eq!(
        candidates[0].reason,
        Some(MigrationCandidateReason::CitedSourceUnresolved)
    );
    assert_eq!(candidates[0].source_region.locator, "row:2");
    assert_eq!(candidates[0].owner, extractor);

    Ok(())
}

#[test]
fn canonical_lab_timeline_is_filtered_and_clinically_ordered() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-timeline.pdf"),
        b"synthetic PDF fixture without health information",
    )?;
    fs::write(
        root.path().join("sources/synthetic-timeline.csv"),
        concat!(
            "Date,Test,Value,Units,Reference Range,Flag,Source File\n",
            "2026-03-01,Synthetic Marker,3.0,mg/dL,,,sources/synthetic-timeline.pdf\n",
            "2026-01-01,Synthetic Marker,1.0,mg/dL,,,sources/synthetic-timeline.pdf\n",
            "2026-02-01,Other Synthetic Marker,2.0,mg/dL,,,sources/synthetic-timeline.pdf\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-timeline.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    })?;
    let parsed_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-timeline.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let proposal = workspace
        .record()
        .propose_lab_timeline_import(LabTimelineImportRequest {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-canonical-lab-timeline-001")?,
            source_id: parsed_source.source_id,
            asserted_by: AuthorIdentity {
                kind: AuthorKind::Application,
                identifier: "synthetic-migration-adapter".to_owned(),
            },
            extractor: ExtractorIdentity {
                name: "health-engine-lab-timeline".to_owned(),
                version: "1.0.0".to_owned(),
            },
            test_mappings: vec![
                LabTestMapping {
                    reported_name: "Synthetic Marker".to_owned(),
                    canonical_test: CanonicalTest {
                        identifier: "synthetic-marker".to_owned(),
                        display_name: "Synthetic Marker".to_owned(),
                    },
                },
                LabTestMapping {
                    reported_name: "Other Synthetic Marker".to_owned(),
                    canonical_test: CanonicalTest {
                        identifier: "other-synthetic-marker".to_owned(),
                        display_name: "Other Synthetic Marker".to_owned(),
                    },
                },
            ],
        })?;
    workspace.record().apply(proposal)?;

    let timeline = workspace.record().lab_timeline(&LabTimelineQuery {
        canonical_test_identifier: "synthetic-marker".to_owned(),
        include_history: false,
    })?;

    assert_eq!(timeline.len(), 2);
    let ClinicalTime::Date { value: first, .. } = timeline[0].draft.clinical_time else {
        panic!("expected exact synthetic date");
    };
    let ClinicalTime::Date { value: second, .. } = timeline[1].draft.clinical_time else {
        panic!("expected exact synthetic date");
    };
    assert_eq!(first, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    assert_eq!(second, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());

    Ok(())
}

#[test]
fn comparable_canonical_results_produce_trend_and_reversal_claims() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-trend.pdf"),
        b"synthetic PDF fixture without health information",
    )?;
    fs::write(
        root.path().join("sources/synthetic-trend.csv"),
        concat!(
            "Date,Test,Value,Units,Reference Range,Flag,Source File\n",
            "2026-01-01,Synthetic Trend Marker,1.0,mg/dL,,,sources/synthetic-trend.pdf\n",
            "2026-02-01,Synthetic Trend Marker,3.0,mg/dL,,,sources/synthetic-trend.pdf\n",
            "2026-03-01,Synthetic Trend Marker,2.0,mg/dL,,,sources/synthetic-trend.pdf\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-trend.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    })?;
    let parsed_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-trend.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let proposal = workspace
        .record()
        .propose_lab_timeline_import(LabTimelineImportRequest {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-lab-trend-analysis-001")?,
            source_id: parsed_source.source_id,
            asserted_by: AuthorIdentity {
                kind: AuthorKind::Application,
                identifier: "synthetic-migration-adapter".to_owned(),
            },
            extractor: ExtractorIdentity {
                name: "health-engine-lab-timeline".to_owned(),
                version: "1.0.0".to_owned(),
            },
            test_mappings: vec![LabTestMapping {
                reported_name: "Synthetic Trend Marker".to_owned(),
                canonical_test: CanonicalTest {
                    identifier: "synthetic-trend-marker".to_owned(),
                    display_name: "Synthetic Trend Marker".to_owned(),
                },
            }],
        })?;
    workspace.record().apply(proposal)?;

    let analysis = workspace.analyze(AnalysisOptions::default())?;
    let trend = analysis
        .claims
        .iter()
        .find(|claim| claim.key == "lab_trend:synthetic-trend-marker")
        .expect("trend claim");
    assert_eq!(trend.evidence_fact_ids.len(), 2);
    assert!(trend.statement.contains("decreased"));
    let reversal = analysis
        .claims
        .iter()
        .find(|claim| claim.key == "lab_reversal:synthetic-trend-marker")
        .expect("reversal claim");
    assert_eq!(reversal.evidence_fact_ids.len(), 3);

    Ok(())
}
