use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::{DateTime, SecondsFormat, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Analysis, AnalysisFreshness, AnalysisId, ClaimId, ConfidenceLevel, Error, RecordRevision,
    RecordSnapshot, Result, Workspace,
};

const HEALTH_REPORT: &str = "reports/HEALTH_REPORT.html";
const PHYSICIAN_SUMMARY: &str = "reports/PHYSICIAN_SUMMARY.md";
const HEALTH_SUMMARY: &str = "reports/current/health-summary.md";
const OPEN_LOOPS: &str = "reports/current/open-loops.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenderContext {
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReportBundle {
    pub schema_version: u32,
    pub record_revision: RecordRevision,
    pub analysis_id: AnalysisId,
    pub generated_at: DateTime<Utc>,
    pub claim_ids: Vec<ClaimId>,
    files: BTreeMap<String, String>,
}

impl ReportBundle {
    pub fn paths(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }

    pub fn file(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }
}

pub struct Renderer<'workspace> {
    workspace: &'workspace Workspace,
}

impl<'workspace> Renderer<'workspace> {
    pub(crate) fn new(workspace: &'workspace Workspace) -> Self {
        Self { workspace }
    }

    pub fn report_bundle(
        &self,
        snapshot: &RecordSnapshot,
        analysis: &Analysis,
        context: RenderContext,
    ) -> Result<ReportBundle> {
        if snapshot.record_revision != analysis.record_revision
            || snapshot.workspace_id != self.workspace.status()?.workspace_id
            || self.workspace.analyses().status(&analysis.id)? != AnalysisFreshness::Fresh
        {
            return Err(Error::Unsupported(
                "Report Bundle requires matching fresh inputs".to_owned(),
            ));
        }
        for claim in &analysis.claims {
            if claim
                .evidence_fact_ids
                .iter()
                .any(|fact_id| !snapshot.facts.iter().any(|fact| &fact.id == fact_id))
            {
                return Err(Error::InvalidWorkspace(
                    "Analysis dependency is absent from Record Snapshot".to_owned(),
                ));
            }
        }

        let metadata = markdown_metadata(analysis, context.generated_at);
        let claims = markdown_claims(analysis);
        let warnings = markdown_warnings(analysis);
        let mut files = BTreeMap::new();
        files.insert(
            HEALTH_REPORT.to_owned(),
            html_report(analysis, context.generated_at),
        );
        files.insert(
            PHYSICIAN_SUMMARY.to_owned(),
            format!(
                "# Physician Summary\n\n{metadata}\n\n## Findings\n\n{claims}\n## Warnings\n\n{warnings}"
            ),
        );
        files.insert(
            HEALTH_SUMMARY.to_owned(),
            format!(
                "# Health Summary\n\n{metadata}\n\n## Findings\n\n{claims}\n## Warnings\n\n{warnings}"
            ),
        );
        files.insert(
            OPEN_LOOPS.to_owned(),
            format!(
                "# Open Loops\n\n{metadata}\n\nNo structured open-loop claims in this Analysis.\n\n\
                 ## Warnings\n\n{warnings}"
            ),
        );
        Ok(ReportBundle {
            schema_version: 1,
            record_revision: analysis.record_revision,
            analysis_id: analysis.id.clone(),
            generated_at: context.generated_at,
            claim_ids: analysis
                .claims
                .iter()
                .map(|claim| claim.id.clone())
                .collect(),
            files,
        })
    }
}

fn markdown_metadata(analysis: &Analysis, generated_at: DateTime<Utc>) -> String {
    format!(
        "analysis: {}  \nrecord revision: {}  \ngenerated: {}",
        analysis.id.as_str(),
        analysis.record_revision.value(),
        generated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

fn markdown_claims(analysis: &Analysis) -> String {
    let mut output = String::new();
    for claim in &analysis.claims {
        writeln!(
            &mut output,
            "- {} ({} confidence; `{}`)",
            claim.statement,
            confidence_label(claim.confidence),
            claim.id.as_str()
        )
        .expect("writing to a String cannot fail");
    }
    output
}

fn markdown_warnings(analysis: &Analysis) -> String {
    let mut output = String::new();
    if analysis.warnings.is_empty() {
        output.push_str("None.\n");
        return output;
    }
    for warning in &analysis.warnings {
        let fact_ids = warning
            .related_fact_ids
            .iter()
            .map(crate::FactId::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(&mut output, "- {} (facts: {fact_ids})", warning.code)
            .expect("writing to a String cannot fail");
    }
    output
}

fn html_report(analysis: &Analysis, generated_at: DateTime<Utc>) -> String {
    let mut claims = String::new();
    for claim in &analysis.claims {
        write!(
            &mut claims,
            "<li>{} <strong>{} confidence</strong> <code>{}</code></li>",
            escape_html(&claim.statement),
            confidence_label(claim.confidence),
            escape_html(claim.id.as_str())
        )
        .expect("writing to a String cannot fail");
    }
    let mut warnings = String::new();
    for warning in &analysis.warnings {
        let fact_ids = warning
            .related_fact_ids
            .iter()
            .map(|id| escape_html(id.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            &mut warnings,
            "<li><code>{}</code> (facts: {fact_ids})</li>",
            escape_html(&warning.code)
        )
        .expect("writing to a String cannot fail");
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Health Report</title></head>\
         <body><h1>Health Report</h1><p>analysis: {}</p><p>record revision: {}</p>\
         <p>generated: {}</p><h2>Findings</h2><ul>{claims}</ul>\
         <h2>Warnings</h2><ul>{warnings}</ul></body></html>\n",
        escape_html(analysis.id.as_str()),
        analysis.record_revision.value(),
        generated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

fn confidence_label(confidence: ConfidenceLevel) -> &'static str {
    match confidence {
        ConfidenceLevel::Low => "low",
        ConfidenceLevel::High => "high",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
