use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use health_engine::{SubjectId, Workspace};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn run_cli(arguments: &[&str], input: &Value) -> std::io::Result<Output> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_health-engine"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin should be present")
        .write_all(serde_json::to_string(input)?.as_bytes())?;
    child.wait_with_output()
}

#[test]
fn cli_registers_and_reports_available_source() -> TestResult {
    let root = tempfile::tempdir()?;
    Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-v1\n",
    )?;
    let workspace = root
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let common = ["--workspace", workspace, "--json", "source"];

    let registered = run_cli(
        &[common[0], common[1], common[2], common[3], "register"],
        &json!({
            "schema_version": 1,
            "alias": "sources/lab-a.txt",
            "media_type": "text/plain"
        }),
    )?;
    assert!(
        registered.status.success(),
        "register stderr: {}",
        String::from_utf8_lossy(&registered.stderr)
    );
    assert!(registered.stderr.is_empty());
    let registered_status: Value = serde_json::from_slice(&registered.stdout)?;
    let source_id = registered_status["source_id"]
        .as_str()
        .expect("Source ID should be a string");

    let queried = run_cli(
        &[common[0], common[1], common[2], common[3], "status"],
        &json!({"schema_version": 1, "source_id": source_id}),
    )?;
    assert!(
        queried.status.success(),
        "status stderr: {}",
        String::from_utf8_lossy(&queried.stderr)
    );
    assert!(queried.stderr.is_empty());
    let queried_status: Value = serde_json::from_slice(&queried.stdout)?;

    assert_eq!(queried_status, registered_status);
    assert_eq!(queried_status["schema_version"], 1);
    assert_eq!(queried_status["kind"], "source_status");
    assert_eq!(queried_status["content_state"], "available");
    assert_eq!(queried_status["size_bytes"], 20);
    assert_eq!(queried_status["media_type"], "text/plain");
    assert_eq!(queried_status["aliases"], json!(["sources/lab-a.txt"]));

    for output in [
        &registered.stdout,
        &registered.stderr,
        &queried.stdout,
        &queried.stderr,
    ] {
        let text = String::from_utf8_lossy(output);
        assert!(!text.contains("synthetic-source-v1"));
        assert!(!text.contains("sha256"));
        assert!(!text.contains("digest"));
        assert!(!text.contains(workspace));
    }

    Ok(())
}
