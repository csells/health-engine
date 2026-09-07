use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cli_exposes_versioned_record_change_set_schema_without_a_workspace() -> TestResult {
    for resource in [
        "record-change-set",
        "fact-query",
        "analysis-options",
        "analysis",
        "commit-receipt",
        "report-bundle",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_health-engine"))
            .args(["--json", "schema", "show", resource])
            .output()?;

        assert!(
            output.status.success(),
            "{resource} schema stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let response: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(response["schema_version"], 1);
        assert_eq!(response["kind"], "json_schema");
        assert_eq!(response["name"], resource);
        assert!(response["schema"]["$schema"].is_string());
        if resource == "record-change-set" {
            for property in [
                "schema_version",
                "workspace_id",
                "subject_id",
                "expected_revision",
                "idempotency_key",
                "evidence_origin",
                "extractor",
                "changes",
            ] {
                assert!(
                    response["schema"]["properties"][property].is_object(),
                    "missing schema property {property}"
                );
            }
        }
    }

    Ok(())
}
