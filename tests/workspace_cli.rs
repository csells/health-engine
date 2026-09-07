use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn run_cli(arguments: &[&str], input: Option<&str>) -> std::io::Result<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_health-engine"));
    command
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(document) = input {
        child
            .stdin
            .take()
            .expect("piped stdin should be present")
            .write_all(document.as_bytes())?;
    }
    child.wait_with_output()
}

#[test]
fn cli_initializes_and_reports_workspace_status() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = root
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let common = ["--workspace", workspace, "--json", "workspace"];

    let initialized = run_cli(
        &[common[0], common[1], common[2], common[3], "init"],
        Some(r#"{"schema_version":1,"subject_id":"subject-synthetic-001"}"#),
    )?;
    assert!(
        initialized.status.success(),
        "init stderr: {}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    assert!(initialized.stderr.is_empty());
    let initialized_status: Value = serde_json::from_slice(&initialized.stdout)?;
    assert_eq!(initialized_status["schema_version"], 1);
    assert_eq!(initialized_status["kind"], "workspace_status");
    assert_eq!(initialized_status["subject_id"], "subject-synthetic-001");
    assert_eq!(initialized_status["record_revision"], 0);
    assert!(
        initialized_status["workspace_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    let reopened = run_cli(
        &[common[0], common[1], common[2], common[3], "status"],
        None,
    )?;
    assert!(
        reopened.status.success(),
        "status stderr: {}",
        String::from_utf8_lossy(&reopened.stderr)
    );
    assert!(reopened.stderr.is_empty());
    let reopened_status: Value = serde_json::from_slice(&reopened.stdout)?;
    assert_eq!(reopened_status, initialized_status);

    Ok(())
}
