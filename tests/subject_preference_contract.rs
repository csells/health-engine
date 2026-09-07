use std::fs;

use chrono::NaiveDate;
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, Error, EvidenceOrigin, ExtractorIdentity, FactData,
    FactDraft, FactEvidence, FactKind, FactQuery, IdempotencyKey, MigrationCandidateDisposition,
    MigrationCandidateReason, RecordChange, RecordChangeSet, SourceDescriptor, SubjectId,
    SubjectPreference, SubjectPreferenceCategory, SubjectPreferenceMigrationRequest,
    SubjectPreferenceState, SubjectPreferenceViewQuery, Workspace,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn assert_preference_views(workspace: &Workspace) -> TestResult {
    let history = workspace.record().query(FactQuery {
        kind: Some(FactKind::SubjectPreference),
        include_history: true,
    })?;
    assert_eq!(history.len(), 3);

    let active = workspace
        .record()
        .subject_preferences(&SubjectPreferenceViewQuery {
            include_withdrawn: false,
        })?;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].preference_key, "synthetic-presentation");
    assert_eq!(active[0].state, SubjectPreferenceState::Active);

    let complete = workspace
        .record()
        .subject_preferences(&SubjectPreferenceViewQuery {
            include_withdrawn: true,
        })?;
    assert_eq!(complete.len(), 2);
    assert!(complete.iter().any(|preference| {
        preference.preference_key == "synthetic-routine"
            && preference.state == SubjectPreferenceState::Withdrawn
    }));
    Ok(())
}

#[test]
fn current_preferences_derive_from_explicit_assertion_history() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let reporter = AuthorIdentity {
        kind: AuthorKind::Subject,
        identifier: "subject-synthetic-001".to_owned(),
    };
    let preference = |key: &str, category, state, statement: &str, date| FactDraft {
        clinical_time: ClinicalTime::Date {
            value: date,
            source_text: None,
        },
        evidence: FactEvidence::SelfReport {
            reporter: reporter.clone(),
        },
        fact: FactData::SubjectPreference(SubjectPreference {
            preference_key: key.to_owned(),
            category,
            state,
            statement: statement.to_owned(),
            source_text: statement.to_owned(),
        }),
    };

    let identity = workspace.status()?;
    let initial = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-subject-preferences-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-preference-intake".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![
            RecordChange::AddFact(preference(
                "synthetic-routine",
                SubjectPreferenceCategory::PersonalRoutine,
                SubjectPreferenceState::Active,
                "Synthetic subject chooses a morning routine",
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
            )),
            RecordChange::AddFact(preference(
                "synthetic-presentation",
                SubjectPreferenceCategory::Presentation,
                SubjectPreferenceState::Active,
                "Synthetic subject prefers concise summaries",
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
            )),
        ],
    })?;
    workspace.record().apply(initial)?;

    let identity = workspace.status()?;
    let withdrawal = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-subject-preferences-002")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: reporter.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-preference-intake".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(preference(
            "synthetic-routine",
            SubjectPreferenceCategory::PersonalRoutine,
            SubjectPreferenceState::Withdrawn,
            "Synthetic subject stopped the morning routine",
            NaiveDate::from_ymd_opt(2026, 2, 1).expect("valid date"),
        ))],
    })?;
    workspace.record().apply(withdrawal)?;

    assert_preference_views(&workspace)
}

#[test]
fn subject_preference_rejects_agent_authored_advice() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let identity = workspace.status()?;
    let agent = AuthorIdentity {
        kind: AuthorKind::Agent,
        identifier: "synthetic-agent".to_owned(),
    };
    let result = workspace.record().propose(RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-agent-preference-001")?,
        evidence_origin: EvidenceOrigin::ExplicitSelfReport {
            reporter: agent.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-agent".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: None,
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Undated { source_text: None },
            evidence: FactEvidence::SelfReport { reporter: agent },
            fact: FactData::SubjectPreference(SubjectPreference {
                preference_key: "synthetic-advice".to_owned(),
                category: SubjectPreferenceCategory::PersonalRoutine,
                state: SubjectPreferenceState::Active,
                statement: "Synthetic agent recommendation".to_owned(),
                source_text: "Synthetic agent recommendation".to_owned(),
            }),
        })],
    });

    assert!(matches!(result, Err(Error::InvalidRecordChangeSet(_))));
    Ok(())
}

#[test]
fn living_markdown_yields_confirmation_candidate_not_preference_fact() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::write(
        root.path().join("synthetic-preferences.md"),
        concat!(
            "| | **Item** | **Detail** |\n",
            "| --- | --- | --- |\n",
            "| ✓ | Synthetic metric | **Monitoring** is **wanted** |\n",
            "| ✓ | Synthetic follow-up | Clinician ordered tracking |\n",
            "| ✓ | Synthetic routine | Agent recommends tracking |\n",
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "synthetic-preferences.md".to_owned(),
        media_type: "text/markdown".to_owned(),
    })?;
    let identity = workspace.status()?;
    let proposal = workspace.record().propose_subject_preference_migration(
        SubjectPreferenceMigrationRequest {
            schema_version: 1,
            workspace_id: identity.workspace_id,
            subject_id: identity.subject_id,
            expected_revision: identity.record_revision,
            idempotency_key: IdempotencyKey::parse("synthetic-preference-migration-001")?,
            source_id: source.source_id.clone(),
            extractor: ExtractorIdentity {
                name: "synthetic-preference-migration".to_owned(),
                version: "1.0.0".to_owned(),
            },
        },
    )?;
    assert_eq!(proposal.impact().health_facts, 0);
    workspace.record().apply(proposal)?;
    assert!(workspace.record().query(FactQuery::default())?.is_empty());

    let candidates = workspace.record().migration_candidates(&source.source_id)?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].disposition,
        MigrationCandidateDisposition::Quarantined
    );
    assert_eq!(
        candidates[0].reason,
        Some(MigrationCandidateReason::SubjectConfirmationRequired)
    );
    assert_eq!(candidates[0].source_region.locator, "line:3");

    Ok(())
}
