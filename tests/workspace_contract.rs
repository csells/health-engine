use health_engine::{RecordRevision, SubjectId, Workspace};
use rusqlite::Connection;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn workspace_init_reopens_one_subject() -> TestResult {
    let root = tempfile::tempdir()?;
    let subject_id = SubjectId::parse("subject-synthetic-001")?;

    let created = Workspace::init(root.path(), subject_id.clone())?;
    let initial = created.status()?;

    assert_eq!(initial.schema_version, 1);
    assert_eq!(initial.subject_id, subject_id);
    assert_eq!(initial.record_revision, RecordRevision::INITIAL);
    assert!(!initial.workspace_id.as_str().is_empty());

    drop(created);

    let reopened = Workspace::open(root.path())?;
    assert_eq!(reopened.status()?, initial);

    Ok(())
}

#[cfg(unix)]
#[test]
fn workspace_engine_state_is_owner_only() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    drop(workspace);

    let engine_mode = std::fs::metadata(root.path().join(".health-engine"))?
        .permissions()
        .mode()
        & 0o777;
    let database_mode = std::fs::metadata(root.path().join(".health-engine/health.sqlite3"))?
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(engine_mode, 0o700);
    assert_eq!(database_mode, 0o600);

    Ok(())
}

#[test]
fn reopening_workspace_removes_abandoned_private_staging() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    drop(workspace);
    let staging = root.path().join(".health-engine/staging");
    std::fs::create_dir(&staging)?;
    std::fs::write(staging.join("abandoned.tmp"), b"synthetic staged content\n")?;

    let reopened = Workspace::open(root.path())?;

    assert_eq!(
        reopened.status()?.subject_id,
        SubjectId::parse("subject-synthetic-001")?
    );
    assert!(!staging.exists());

    Ok(())
}

#[test]
fn reopening_workspace_completes_interrupted_expungement_cleanup() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    drop(workspace);
    let report = root.path().join("reports/current/health-summary.md");
    std::fs::create_dir_all(report.parent().expect("report has parent"))?;
    std::fs::write(&report, b"synthetic derived report\n")?;
    let staging = root.path().join(".health-engine/staging");
    std::fs::create_dir(&staging)?;
    std::fs::write(staging.join("proposal.tmp"), b"synthetic staged content\n")?;
    let database_path = root.path().join(".health-engine/health.sqlite3");
    let connection = Connection::open(&database_path)?;
    connection.execute(
        "UPDATE expungement_cleanup_state SET pending = 1 WHERE singleton = 1",
        [],
    )?;
    drop(connection);

    let reopened = Workspace::open(root.path())?;

    assert!(!report.exists());
    assert!(!staging.exists());
    let pending: i64 = Connection::open(database_path)?.query_row(
        "SELECT pending FROM expungement_cleanup_state WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(pending, 0);
    assert_eq!(
        reopened.status()?.subject_id,
        SubjectId::parse("subject-synthetic-001")?
    );

    Ok(())
}
