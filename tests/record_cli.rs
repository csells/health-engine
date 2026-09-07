use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::str::FromStr;

use chrono::NaiveDate;
use health_engine::{
    AuthorIdentity, AuthorKind, ClinicalTime, EvidenceOrigin, ExtractionCoverage, ExtractionDomain,
    ExtractionRunDraft, ExtractorIdentity, FactData, FactDraft, FactEvidence, IdempotencyKey,
    LabResult, LabValue, RecordChange, RecordChangeSet, RecordRevision, ReferenceRange,
    SourceDescriptor, SourceRegion, SubjectId, Workspace,
};
use rust_decimal::Decimal;
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
#[allow(clippy::too_many_lines)]
fn cli_proposes_applies_and_queries_a_lab_change_set() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-v1\n",
    )?;
    let source = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let identity = workspace.status()?;
    let region = SourceRegion {
        locator: "line:1".to_owned(),
    };
    let change_set = RecordChangeSet {
        schema_version: 1,
        workspace_id: identity.workspace_id,
        subject_id: identity.subject_id,
        expected_revision: RecordRevision::INITIAL,
        idempotency_key: IdempotencyKey::parse("synthetic-cli-lab-001")?,
        evidence_origin: EvidenceOrigin::DocumentExtraction {
            source_id: source.source_id.clone(),
        },
        extractor: ExtractorIdentity {
            name: "synthetic-lab-extractor".to_owned(),
            version: "1.0.0".to_owned(),
        },
        extraction_run: Some(ExtractionRunDraft {
            source_id: source.source_id.clone(),
            coverage: ExtractionCoverage {
                regions: vec![region.clone()],
                domains: vec![ExtractionDomain::Laboratory],
            },
            omissions: vec![],
            failures: vec![],
            resulting_candidate_keys: vec![],
        }),
        changes: vec![RecordChange::AddFact(FactDraft {
            clinical_time: ClinicalTime::Date {
                value: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid synthetic date"),
                source_text: Some("2026-01-15".to_owned()),
            },
            evidence: FactEvidence::Source {
                source_id: source.source_id,
                source_region: region,
                asserted_by: AuthorIdentity {
                    kind: AuthorKind::Agent,
                    identifier: "synthetic-lab-extractor".to_owned(),
                },
            },
            fact: FactData::LabResult(LabResult {
                test_name: "Synthetic Marker".to_owned(),
                canonical_test: None,
                value: LabValue::Numeric {
                    value: Decimal::from_str("10")?,
                },
                units: Some("mg/dL".to_owned()),
                reference_range: Some(ReferenceRange {
                    original: "5-15".to_owned(),
                    lower: Some(Decimal::from_str("5")?),
                    upper: Some(Decimal::from_str("15")?),
                }),
                reported_flag: Some("normal".to_owned()),
                performing_lab: Some("Synthetic Lab".to_owned()),
            }),
        })],
    };
    let change_set_json = serde_json::to_value(&change_set)?;
    let workspace_path = root
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let common = ["--workspace", workspace_path, "--json", "record"];

    let proposed = run_cli(
        &[common[0], common[1], common[2], common[3], "propose"],
        &change_set_json,
    )?;
    assert!(
        proposed.status.success(),
        "propose stderr: {}",
        String::from_utf8_lossy(&proposed.stderr)
    );
    assert!(proposed.stderr.is_empty());
    let proposal: Value = serde_json::from_slice(&proposed.stdout)?;
    assert_eq!(proposal["schema_version"], 1);
    assert_eq!(proposal["kind"], "record_proposal");
    assert_eq!(proposal["expected_revision"], 0);
    assert_eq!(proposal["impact"]["extraction_runs"], 1);
    assert_eq!(proposal["impact"]["health_facts"], 1);
    assert_eq!(workspace.status()?.record_revision, RecordRevision::INITIAL);

    let apply_request = json!({
        "schema_version": 1,
        "proposal_id": proposal["proposal_id"],
        "change_set": change_set_json
    });
    let applied = run_cli(
        &[common[0], common[1], common[2], common[3], "apply"],
        &apply_request,
    )?;
    assert!(
        applied.status.success(),
        "apply stderr: {}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(applied.stderr.is_empty());
    let receipt: Value = serde_json::from_slice(&applied.stdout)?;
    assert_eq!(receipt["schema_version"], 1);
    assert_eq!(receipt["kind"], "commit_receipt");
    assert_eq!(receipt["previous_revision"], 0);
    assert_eq!(receipt["record_revision"], 1);
    assert_eq!(
        receipt["accepted_fact_ids"].as_array().map(Vec::len),
        Some(1)
    );
    let retried = run_cli(
        &[common[0], common[1], common[2], common[3], "apply"],
        &apply_request,
    )?;
    assert!(
        retried.status.success(),
        "retry stderr: {}",
        String::from_utf8_lossy(&retried.stderr)
    );
    assert!(retried.stderr.is_empty());
    assert_eq!(retried.stdout, applied.stdout);

    let queried = run_cli(
        &[common[0], common[1], common[2], common[3], "query"],
        &json!({
            "schema_version": 1,
            "query": {"kind": "lab_result", "include_history": false}
        }),
    )?;
    assert!(
        queried.status.success(),
        "query stderr: {}",
        String::from_utf8_lossy(&queried.stderr)
    );
    assert!(queried.stderr.is_empty());
    let result: Value = serde_json::from_slice(&queried.stdout)?;
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["kind"], "fact_query");
    assert_eq!(result["record_revision"], 1);
    assert_eq!(result["facts"].as_array().map(Vec::len), Some(1));
    assert_eq!(result["facts"][0]["fact"]["type"], "lab_result");
    assert_eq!(result["facts"][0]["fact"]["test_name"], "Synthetic Marker");

    let analysis_run = run_cli(
        &["--workspace", workspace_path, "--json", "analysis", "run"],
        &json!({
            "schema_version": 1,
            "options": {"include_unverified": true}
        }),
    )?;
    assert!(
        analysis_run.status.success(),
        "analysis run stderr: {}",
        String::from_utf8_lossy(&analysis_run.stderr)
    );
    assert!(analysis_run.stderr.is_empty());
    let analysis: Value = serde_json::from_slice(&analysis_run.stdout)?;
    assert_eq!(analysis["schema_version"], 1);
    assert_eq!(analysis["kind"], "analysis");
    assert_eq!(analysis["record_revision"], 1);
    assert_eq!(analysis["freshness"], "fresh");
    assert_eq!(analysis["claims"].as_array().map(Vec::len), Some(1));

    let analysis_status = run_cli(
        &[
            "--workspace",
            workspace_path,
            "--json",
            "analysis",
            "status",
        ],
        &json!({"schema_version": 1, "analysis_id": analysis["id"]}),
    )?;
    assert!(
        analysis_status.status.success(),
        "analysis status stderr: {}",
        String::from_utf8_lossy(&analysis_status.stderr)
    );
    let status: Value = serde_json::from_slice(&analysis_status.stdout)?;
    assert_eq!(status["kind"], "analysis_status");
    assert_eq!(status["freshness"], "fresh");

    let exported = run_cli(
        &[
            "--workspace",
            workspace_path,
            "--json",
            "analysis",
            "export",
        ],
        &json!({
            "schema_version": 1,
            "analysis_id": analysis["id"],
            "generated_at": "2026-01-17T08:00:00Z"
        }),
    )?;
    assert!(
        exported.status.success(),
        "analysis export stderr: {}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let report: Value = serde_json::from_slice(&exported.stdout)?;
    assert_eq!(report["kind"], "report_bundle");
    assert_eq!(report["record_revision"], 1);
    assert_eq!(report["analysis_id"], analysis["id"]);
    assert_eq!(
        report["files"].as_object().map(serde_json::Map::len),
        Some(4)
    );
    let claim_id = analysis["claims"][0]["id"].as_str().unwrap_or("");
    assert!(
        report["files"]["reports/HEALTH_REPORT.html"]
            .as_str()
            .is_some_and(|html| html.contains(claim_id))
    );

    for output in [
        proposed,
        applied,
        retried,
        queried,
        analysis_run,
        analysis_status,
        exported,
    ] {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stdout.contains(workspace_path));
        assert!(!stderr.contains(workspace_path));
        assert!(!stdout.contains("synthetic-source-v1"));
        assert!(!stderr.contains("synthetic-source-v1"));
        assert!(!stdout.contains("sha256"));
        assert!(!stderr.contains("sha256"));
    }

    Ok(())
}
