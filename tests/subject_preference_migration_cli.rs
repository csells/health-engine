use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use health_engine::{
    ExtractorIdentity, FactQuery, IdempotencyKey, MigrationCandidateReason, SourceDescriptor,
    SubjectId, SubjectPreferenceMigrationRequest, Workspace,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cli_quarantines_legacy_tracking_choice_for_subject_confirmation() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::write(
        root.path().join("synthetic-preferences.md"),
        concat!(
            "| Topic | Choice |\n",
            "| --- | --- |\n",
            "| Synthetic metric | I prefer to track this |\n",
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "synthetic-preferences.md".to_owned(),
        media_type: "text/markdown".to_owned(),
    })?;
    let identity = workspace.status()?;
    let request = SubjectPreferenceMigrationRequest {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-cli-preference-migration-001")?,
        source_id: source.source_id.clone(),
        extractor: ExtractorIdentity {
            name: "health-engine-subject-preference-migration".to_owned(),
            version: "1.0.0".to_owned(),
        },
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_health-engine"))
        .args([
            "--workspace",
            root.path().to_str().expect("UTF-8 temporary path"),
            "--json",
            "record",
            "import-subject-preferences",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&serde_json::to_vec(&request)?)?;
    let output = child.wait_with_output()?;

    assert!(
        output.status.success(),
        "import stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(workspace.record().query(FactQuery::default())?.is_empty());
    let candidates = workspace.record().migration_candidates(&source.source_id)?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].reason,
        Some(MigrationCandidateReason::SubjectConfirmationRequired)
    );

    Ok(())
}
