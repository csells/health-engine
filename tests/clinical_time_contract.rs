use chrono::{DateTime, FixedOffset, NaiveDate};
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, ClinicalTimePoint, EvidenceOrigin, ExtractorIdentity,
    FactData, FactDraft, FactEvidence, FactQuery, HealthFact, IdempotencyKey, LabResult, LabValue,
    PartialDate, RecordChange, RecordChangeSet, SubjectId, Workspace,
};
use rust_decimal::Decimal;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn record_timed_fact(
    workspace: &Workspace,
    idempotency_key: &str,
    clinical_time: ClinicalTime,
) -> Result<HealthFact, Box<dyn std::error::Error>> {
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let identity = workspace.status()?;
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse(idempotency_key)?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-self-report".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time,
            evidence: FactEvidence::SelfReport { reporter },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Timed Marker".to_owned(),
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
    Ok(workspace.record().query(FactQuery::default())?.remove(0))
}

#[test]
fn exact_instant_preserves_its_known_source_offset() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let clinical_time = ClinicalTime::Instant {
        value: DateTime::<FixedOffset>::parse_from_rfc3339("2026-01-15T07:30:00-08:00")?,
        source_text: Some("synthetic local collection time".to_owned()),
    };
    let fact = record_timed_fact(
        &workspace,
        "synthetic-clinical-instant-001",
        clinical_time.clone(),
    )?;

    assert_eq!(fact.draft.clinical_time, clinical_time);

    Ok(())
}

#[test]
fn interval_preserves_both_source_endpoints() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let clinical_time = ClinicalTime::Interval {
        start: ClinicalTimePoint::Date {
            value: NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid synthetic date"),
        },
        end: ClinicalTimePoint::Date {
            value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
        },
        source_text: Some("synthetic two-week interval".to_owned()),
    };

    let fact = record_timed_fact(
        &workspace,
        "synthetic-clinical-interval-001",
        clinical_time.clone(),
    )?;

    assert_eq!(fact.draft.clinical_time, clinical_time);

    Ok(())
}

#[test]
fn year_precision_preserves_unknown_month_and_day() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let clinical_time = ClinicalTime::PartialDate {
        value: PartialDate::Year { year: 2026 },
        source_text: Some("synthetic year-only history".to_owned()),
    };

    let fact = record_timed_fact(
        &workspace,
        "synthetic-clinical-partial-year-001",
        clinical_time.clone(),
    )?;

    assert_eq!(fact.draft.clinical_time, clinical_time);

    Ok(())
}

#[test]
fn undated_source_does_not_invent_clinical_time_from_recording_time() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let clinical_time = ClinicalTime::Undated {
        source_text: Some("synthetic source supplied no date".to_owned()),
    };

    let fact = record_timed_fact(
        &workspace,
        "synthetic-clinical-undated-001",
        clinical_time.clone(),
    )?;

    assert_eq!(fact.draft.clinical_time, clinical_time);
    assert_ne!(
        fact.recorded_at.to_rfc3339(),
        "synthetic source supplied no date"
    );

    Ok(())
}
