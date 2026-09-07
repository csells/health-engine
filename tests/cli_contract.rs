use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn cli_reports_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_health-engine"))
        .arg("--version")
        .output()
        .expect("health-engine executable should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"health-engine 0.1.0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_rejects_oversized_stdin_without_echoing_payload() {
    let root = tempfile::tempdir().expect("temporary Workspace root");
    let sentinel = "SYNTHETIC-SENSITIVE-SENTINEL";
    let padding = "x".repeat(16 * 1024 * 1024);
    let input = format!(
        r#"{{"schema_version":1,"subject_id":"subject-synthetic-001","padding":"{padding}{sentinel}"}}"#
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_health-engine"))
        .args([
            "--workspace",
            root.path().to_str().expect("UTF-8 temporary path"),
            "--json",
            "workspace",
            "init",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("health-engine executable should start");
    let write_result = child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input.as_bytes());
    if let Err(error) = write_result {
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }

    let output = child.wait_with_output().expect("CLI should exit");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"{\"error\":\"input_too_large\"}\n");
    assert!(!String::from_utf8_lossy(&output.stderr).contains(sentinel));
}
