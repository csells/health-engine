use chrono::NaiveDate;
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, ConditionAssertion, ConditionAssertionState,
    ConditionPictureQuery, EvidenceOrigin, ExtractorIdentity, FactData, FactDraft, FactEvidence,
    FactKind, FactQuery, IdempotencyKey, RecordChange, RecordChangeSet, SubjectId, Workspace,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn current_condition_picture_is_derived_from_assertion_history() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let assertion = |date, state, source_text: &str| FactDraft {
        clinical_time: ClinicalTime::Date {
            value: date,
            source_text: Some(date.to_string()),
        },
        evidence: FactEvidence::SelfReport {
            reporter: reporter.clone(),
        },
        fact: FactData::ConditionAssertion(ConditionAssertion {
            state,
            name: "Synthetic Condition".to_owned(),
            normalized_identity: Some("synthetic-condition".to_owned()),
            body_site: None,
            assertion_text: source_text.to_owned(),
        }),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-condition-history-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-condition-intake".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![
            RecordChange::AddFact(assertion(
                NaiveDate::from_ymd_opt(2025, 1, 1).expect("date"),
                ConditionAssertionState::Confirmed,
                "synthetic confirmed assertion",
            )),
            RecordChange::AddFact(assertion(
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
                ConditionAssertionState::Resolved,
                "synthetic resolved assertion",
            )),
        ],
    })?;
    workspace.record().apply(proposal)?;

    let history = workspace.record().query(FactQuery {
        kind: Some(FactKind::ConditionAssertion),
        include_history: true,
    })?;
    assert_eq!(history.len(), 2);
    let active = workspace
        .record()
        .condition_picture(&ConditionPictureQuery {
            include_inactive: false,
        })?;
    assert!(active.is_empty());
    let complete = workspace
        .record()
        .condition_picture(&ConditionPictureQuery {
            include_inactive: true,
        })?;
    assert_eq!(complete.len(), 1);
    assert_eq!(complete[0].state, ConditionAssertionState::Resolved);
    assert_eq!(complete[0].assertion_text, "synthetic resolved assertion");

    Ok(())
}
