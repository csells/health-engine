use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin, ExtractionCoverage, ExtractionDomain,
    ExtractionRunDraft, ExtractorIdentity, FactData, FactDraft, FactEvidence, FactKind, FactQuery,
    IdempotencyKey, MedicationDose, MedicationEvent, MedicationEventKind, MedicationProductKind,
    MedicationRegimenQuery, RecordChange, RecordChangeSet, SourceDescriptor, SourceRegion,
    SubjectId, Workspace,
};
use rust_decimal::Decimal;
use std::{fs, str::FromStr};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(clippy::too_many_lines)]
fn medication_regimen_is_reconstructed_from_immutable_events() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-medications.md"),
        "# Synthetic medication history\n",
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-medications.md".to_owned(),
        media_type: "text/markdown".to_owned(),
    })?;
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let event = |date, kind, dose: Option<&str>, locator: &str| FactDraft {
        clinical_time: ClinicalTime::Date {
            value: date,
            source_text: Some(date.to_string()),
        },
        evidence: FactEvidence::SourcedSelfReport {
            source_id: source.source_id.clone(),
            source_region: SourceRegion {
                locator: locator.to_owned(),
            },
            reporter: reporter.clone(),
        },
        fact: FactData::MedicationEvent(MedicationEvent {
            event: kind,
            product_kind: MedicationProductKind::Medication,
            name: "Synthetic Therapy".to_owned(),
            normalized_identity: Some("synthetic-therapy".to_owned()),
            strength: None,
            dose: dose.map(|value| MedicationDose {
                value: Decimal::from_str(value).expect("synthetic dose"),
                unit: "mg".to_owned(),
                original: format!("{value} mg"),
            }),
            route: Some("oral".to_owned()),
            schedule: Some("once daily".to_owned()),
            indication: Some("synthetic indication".to_owned()),
            adherence_context: None,
        }),
    };
    let events = vec![
        event(
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("date"),
            MedicationEventKind::Started,
            Some("10"),
            "line:2",
        ),
        event(
            chrono::NaiveDate::from_ymd_opt(2025, 6, 1).expect("date"),
            MedicationEventKind::DoseChanged,
            Some("20"),
            "line:3",
        ),
        event(
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
            MedicationEventKind::Stopped,
            None,
            "line:4",
        ),
    ];
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-medication-events-001")?,
        evidence_origin: EvidenceOrigin::ParsedSelfReport {
            source_id: source.source_id.clone(),
            reporter,
        },
        extractor: ExtractorIdentity {
            name: "synthetic-medication-extractor".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source.source_id,
            coverage: ExtractionCoverage {
                regions: vec![
                    SourceRegion {
                        locator: "line:2".to_owned(),
                    },
                    SourceRegion {
                        locator: "line:3".to_owned(),
                    },
                    SourceRegion {
                        locator: "line:4".to_owned(),
                    },
                ],
                domains: vec![ExtractionDomain::Medications],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec!["synthetic-therapy".to_owned()],
        }),
        changes: events.into_iter().map(RecordChange::AddFact).collect(),
    })?;
    workspace.record().apply(proposal)?;

    let history = workspace.record().query(FactQuery {
        kind: Some(FactKind::MedicationEvent),
        include_history: true,
    })?;
    assert_eq!(history.len(), 3);
    let midyear = workspace
        .record()
        .medication_regimen(&MedicationRegimenQuery {
            at: Some(chrono::NaiveDate::from_ymd_opt(2025, 7, 1).expect("date")),
        })?;
    assert_eq!(midyear.len(), 1);
    assert_eq!(
        midyear[0].dose.as_ref().map(|dose| dose.value),
        Some(Decimal::from_str("20")?)
    );
    let current = workspace
        .record()
        .medication_regimen(&MedicationRegimenQuery { at: None })?;
    assert!(current.is_empty());

    Ok(())
}

#[test]
fn current_regimen_report_does_not_invent_a_start_date() -> TestResult {
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
        idempotency_key: IdempotencyKey::parse("synthetic-current-regimen-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-current-regimen".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::PartialDate {
                value: health_engine::PartialDate::Month {
                    year: 2026,
                    month: 6,
                },
                source_text: Some("synthetic month".to_owned()),
            },
            evidence: FactEvidence::SelfReport { reporter },
            fact: FactData::MedicationEvent(MedicationEvent {
                event: MedicationEventKind::RegimenReported,
                product_kind: MedicationProductKind::Supplement,
                name: "Synthetic Compound".to_owned(),
                normalized_identity: Some("synthetic-compound".to_owned()),
                strength: Some("synthetic multi-component strength".to_owned()),
                dose: Some(MedicationDose {
                    value: Decimal::ONE,
                    unit: "capsule".to_owned(),
                    original: "1 capsule".to_owned(),
                }),
                route: None,
                schedule: Some("every morning".to_owned()),
                indication: None,
                adherence_context: None,
            }),
        })],
    })?;
    workspace.record().apply(proposal)?;

    let facts = workspace.record().query(FactQuery {
        kind: Some(FactKind::MedicationEvent),
        include_history: false,
    })?;
    let FactData::MedicationEvent(event) = &facts[0].draft.fact else {
        panic!("expected medication event");
    };
    assert_eq!(event.event, MedicationEventKind::RegimenReported);
    assert_eq!(
        event.strength.as_deref(),
        Some("synthetic multi-component strength")
    );
    let regimen = workspace
        .record()
        .medication_regimen(&MedicationRegimenQuery { at: None })?;
    assert_eq!(regimen.len(), 1);
    assert_eq!(
        regimen[0].strength.as_deref(),
        Some("synthetic multi-component strength")
    );

    Ok(())
}
