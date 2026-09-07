use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use health_engine::{
    AuthorIdentity, AuthorKind, ExtractorIdentity, FactData, FactKind, FactQuery, IdempotencyKey,
    SourceDescriptor, SubjectId, VitalMeasurement, VitalTimelineFormat, VitalTimelineImportRequest,
    Workspace,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cli_imports_weight_timeline_without_losing_source_literal() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/synthetic-weight.csv"),
        "date,weight_lbs,note\n2026-01-15,150.0,synthetic morning context\n",
    )?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/synthetic-weight.csv".to_owned(),
        media_type: "text/csv".to_owned(),
    })?;
    let identity = workspace.status()?;
    let request = VitalTimelineImportRequest {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: identity.record_revision,
        idempotency_key: IdempotencyKey::parse("synthetic-cli-weight-timeline-001")?,
        source_id: source.source_id,
        reporter: AuthorIdentity {
            kind: AuthorKind::Subject,
            identifier: "subject-synthetic-001".to_owned(),
        },
        extractor: ExtractorIdentity {
            name: "health-engine-vital-timeline".to_owned(),
            version: "1.0.0".to_owned(),
        },
        format: VitalTimelineFormat::Weight,
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_health-engine"))
        .args([
            "--workspace",
            root.path().to_str().expect("UTF-8 temporary path"),
            "--json",
            "record",
            "import-vital-timeline",
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
    let facts = workspace.record().query(FactQuery {
        kind: Some(FactKind::VitalMeasurement),
        include_history: false,
    })?;
    let FactData::VitalMeasurement(VitalMeasurement::Weight {
        original, units, ..
    }) = &facts[0].draft.fact
    else {
        panic!("expected weight fact");
    };
    assert_eq!(original, "150.0");
    assert_eq!(units.as_deref(), Some("lb"));

    Ok(())
}
