use std::fs;

use health_engine::{
    AuthorIdentity, AuthorKind, EvidenceAssurance, ExtractorIdentity, FactData, FactEvidence,
    FactKind, FactQuery, IdempotencyKey, SourceDescriptor, SubjectId, VitalMeasurement,
    VitalTimelineFormat, VitalTimelineImportRequest, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn vital_timeline_preserves_self_report_attribution_and_context() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-vitals.csv"),
        concat!(
            "date,systolic,diastolic,pulse,measured_by,note\n",
            "2026-01-15,120,80,60,self,synthetic seated context\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-vitals.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };

    let proposal =
        workspace
            .record()
            .propose_vital_timeline_import(VitalTimelineImportRequest {
                schema_version: 1,
                workspace_id: identity.workspace_id,
                subject_id: identity.subject_id,
                expected_revision: identity.record_revision,
                idempotency_key: IdempotencyKey::parse("synthetic-vital-timeline-001")?,
                source_id: source.source_id.clone(),
                reporter: reporter.clone(),
                extractor: ExtractorIdentity {
                    name: "health-engine-vital-timeline".to_owned(),
                    version: "1.0.0".to_owned(),
                },
                format: VitalTimelineFormat::BloodPressurePulse,
            })?;

    assert_eq!(proposal.impact().health_facts, 2);
    workspace.record().apply(proposal)?;
    let facts = workspace.record().query(FactQuery {
        kind: Some(FactKind::VitalMeasurement),
        include_history: false,
    })?;
    assert_eq!(facts.len(), 2);
    assert!(
        facts
            .iter()
            .all(|fact| fact.original_assurance == EvidenceAssurance::ExplicitSelfReport)
    );
    for fact in &facts {
        let FactEvidence::SourcedSelfReport {
            source_id,
            source_region,
            reporter: fact_reporter,
        } = &fact.draft.evidence
        else {
            panic!("expected sourced self-report");
        };
        assert_eq!(source_id, &source.source_id);
        assert_eq!(source_region.locator, "row:2");
        assert_eq!(fact_reporter, &reporter);
    }
    let pressure_fact = facts
        .iter()
        .find(|fact| {
            matches!(
                fact.draft.fact,
                FactData::VitalMeasurement(VitalMeasurement::BloodPressure { .. })
            )
        })
        .expect("blood pressure fact");
    let FactData::VitalMeasurement(VitalMeasurement::BloodPressure {
        systolic,
        diastolic,
        measured_by,
        note,
        ..
    }) = &pressure_fact.draft.fact
    else {
        unreachable!("selected blood pressure fact");
    };
    assert_eq!(*systolic, Decimal::new(120, 0));
    assert_eq!(*diastolic, Decimal::new(80, 0));
    assert_eq!(measured_by.as_deref(), Some("self"));
    assert_eq!(note.as_deref(), Some("synthetic seated context"));
    assert!(facts.iter().any(|fact| matches!(
        fact.draft.fact,
        FactData::VitalMeasurement(VitalMeasurement::Pulse { .. })
    )));
    let encoded = serde_json::to_value(&pressure_fact.draft.fact)?;
    assert_eq!(encoded["type"], "vital_measurement");
    assert_eq!(encoded["measurement_type"], "blood_pressure");

    Ok(())
}

#[test]
fn empty_pulse_cell_does_not_invent_a_measurement() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-pressure-only.csv"),
        concat!(
            "date,systolic,diastolic,pulse,measured_by,note\n",
            "2026-01-15,120,80,,self,synthetic pressure-only context\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-pressure-only.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let proposal =
        workspace
            .record()
            .propose_vital_timeline_import(VitalTimelineImportRequest {
                schema_version: 1,
                workspace_id: identity.workspace_id,
                subject_id: identity.subject_id,
                expected_revision: identity.record_revision,
                idempotency_key: IdempotencyKey::parse("synthetic-pressure-only-001")?,
                source_id: source.source_id,
                reporter: AuthorIdentity {
                    kind: AuthorKind::Subject,
                    identifier: "subject-synthetic-001".to_owned(),
                },
                extractor: ExtractorIdentity {
                    name: "health-engine-vital-timeline".to_owned(),
                    version: "1.0.0".to_owned(),
                },
                format: VitalTimelineFormat::BloodPressurePulse,
            })?;

    assert_eq!(proposal.impact().health_facts, 1);
    workspace.record().apply(proposal)?;
    let facts = workspace.record().query(FactQuery {
        kind: Some(FactKind::VitalMeasurement),
        include_history: false,
    })?;
    assert!(matches!(
        facts.as_slice(),
        [health_engine::HealthFact {
            draft: health_engine::FactDraft {
                fact: FactData::VitalMeasurement(VitalMeasurement::BloodPressure { .. }),
                ..
            },
            ..
        }]
    ));

    Ok(())
}
