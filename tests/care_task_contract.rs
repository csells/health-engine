use health_engine::{
    AuthorIdentity, AuthorKind, CareTask, CareTaskKind, CareTaskStatus, CareTaskViewQuery,
    ClinicalTime, EvidenceOrigin, ExtractorIdentity, FactData, FactDraft, FactEvidence, FactKind,
    FactQuery, IdempotencyKey, RecordChange, RecordChangeSet, SubjectId, Workspace,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn care_task_view_preserves_explicit_status_without_inventing_schedule() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let identity = workspace.status()?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let task = |key: &str, status, action: &str| FactDraft {
        clinical_time: ClinicalTime::Undated { source_text: None },
        evidence: FactEvidence::SelfReport {
            reporter: reporter.clone(),
        },
        fact: FactData::CareTask(CareTask {
            task_key: key.to_owned(),
            kind: CareTaskKind::RepeatTest,
            status,
            action: action.to_owned(),
            due: None,
            requested_by: None,
            source_text: action.to_owned(),
        }),
    };
    let proposal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-care-tasks-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-care-task-intake".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![
            RecordChange::AddFact(task(
                "synthetic-task-overdue",
                CareTaskStatus::Overdue,
                "Synthetic explicitly overdue recheck",
            )),
            RecordChange::AddFact(task(
                "synthetic-task-pending",
                CareTaskStatus::Pending,
                "Synthetic pending task without a supplied schedule",
            )),
        ],
    })?;
    workspace.record().apply(proposal)?;

    let history = workspace.record().query(FactQuery {
        kind: Some(FactKind::CareTask),
        include_history: true,
    })?;
    assert_eq!(history.len(), 2);
    let tasks = workspace.record().care_tasks(&CareTaskViewQuery {
        include_closed: false,
    })?;
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().all(|task| task.due.is_none()));
    assert!(
        tasks
            .iter()
            .any(|task| task.status == CareTaskStatus::Overdue)
    );
    assert!(
        tasks
            .iter()
            .any(|task| task.status == CareTaskStatus::Pending)
    );

    Ok(())
}
