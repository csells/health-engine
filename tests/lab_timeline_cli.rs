use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use health_engine::{
    AuthorIdentity, AuthorKind, CanonicalTest, ExtractorIdentity, FactQuery, IdempotencyKey,
    LabTestMapping, LabTimelineImportRequest, SourceDescriptor, SubjectId, Workspace,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cli_imports_lab_timeline_from_stdin_and_applies_atomically() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-cli-panel.pdf"),
        b"synthetic PDF fixture without health information",
    )?;
    fs::write(
        root.path().join("sources/synthetic-cli-timeline.csv"),
        concat!(
            "Date,Test,Value,Units,Reference Range,Flag,Source File\n",
            "2026-01-15,Synthetic CLI Marker,5.0,mg/dL,,,sources/synthetic-cli-panel.pdf\n"
        ),
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-cli-panel.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    })?;
    let parsed_source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-cli-timeline.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let request = LabTimelineImportRequest {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-cli-lab-timeline-001")?,
        source_id: parsed_source.source_id,
        asserted_by: AuthorIdentity {
            kind: AuthorKind::Application,
            identifier: "synthetic-cli-migration-adapter".to_owned(),
        },
        extractor: ExtractorIdentity {
            name: "health-engine-lab-timeline".to_owned(),
            version: "1.0.0".to_owned(),
        },
        test_mappings: vec![LabTestMapping {
            reported_name: "Synthetic CLI Marker".to_owned(),
            canonical_test: CanonicalTest {
                identifier: "synthetic-cli-marker".to_owned(),
                display_name: "Synthetic CLI Marker".to_owned(),
            },
        }],
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_health-engine"))
        .args([
            "--workspace",
            root.path().to_str().expect("UTF-8 temporary path"),
            "--json",
            "record",
            "import-lab-timeline",
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
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(receipt["kind"], "commit_receipt");
    assert_eq!(
        receipt["accepted_fact_ids"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(workspace.record().query(FactQuery::default())?.len(), 1);

    Ok(())
}
