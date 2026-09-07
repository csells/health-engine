use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    Analysis, AnalysisFreshness, AnalysisId, AnalysisOptions, CommitReceipt, Error, FactQuery,
    HealthFact, LabTimelineImportRequest, ProposalImpact, RecordChangeSet, RecordRevision,
    RenderContext, ReportBundle, Result, SourceDescriptor, SourceId, SourceStatus, SubjectId,
    SubjectPreferenceMigrationRequest, VitalTimelineImportRequest, Workspace, WorkspaceId,
};

const MAX_JSON_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    Analysis {
        #[command(subcommand)]
        command: AnalysisCommand,
    },
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum WorkspaceCommand {
    Init,
    Status,
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum SourceCommand {
    Register,
    Status,
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum RecordCommand {
    Propose,
    Apply,
    Query,
    ImportLabTimeline,
    ImportVitalTimeline,
    ImportSubjectPreferences,
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum AnalysisCommand {
    Run,
    Status,
    Export,
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum SchemaCommand {
    Show { resource: SchemaResource },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SchemaResource {
    RecordChangeSet,
    FactQuery,
    AnalysisOptions,
    Analysis,
    CommitReceipt,
    ReportBundle,
}

#[derive(Debug, Deserialize)]
struct WorkspaceInitRequest {
    schema_version: u32,
    subject_id: String,
}

#[derive(Debug, Serialize)]
struct WorkspaceStatusResponse {
    schema_version: u32,
    kind: &'static str,
    workspace_id: WorkspaceId,
    subject_id: SubjectId,
    record_revision: u64,
}

#[derive(Debug, Deserialize)]
struct SourceRegisterRequest {
    schema_version: u32,
    alias: String,
    media_type: String,
}

#[derive(Debug, Deserialize)]
struct SourceStatusRequest {
    schema_version: u32,
    source_id: String,
}

#[derive(Debug, Serialize)]
struct SourceStatusResponse {
    schema_version: u32,
    kind: &'static str,
    #[serde(flatten)]
    status: SourceStatus,
}

#[derive(Debug, Serialize)]
struct RecordProposalResponse {
    schema_version: u32,
    kind: &'static str,
    proposal_id: String,
    expected_revision: RecordRevision,
    impact: ProposalImpact,
}

#[derive(Debug, Deserialize)]
struct RecordApplyRequest {
    schema_version: u32,
    proposal_id: String,
    change_set: RecordChangeSet,
}

#[derive(Debug, Serialize)]
struct CommitReceiptResponse {
    kind: &'static str,
    #[serde(flatten)]
    receipt: CommitReceipt,
}

#[derive(Debug, Deserialize)]
struct RecordQueryRequest {
    schema_version: u32,
    query: FactQuery,
}

#[derive(Debug, Serialize)]
struct FactQueryResponse {
    schema_version: u32,
    kind: &'static str,
    record_revision: RecordRevision,
    facts: Vec<HealthFact>,
}

#[derive(Debug, Deserialize)]
struct AnalysisRunRequest {
    schema_version: u32,
    options: AnalysisOptions,
}

#[derive(Debug, Serialize)]
struct AnalysisResponse {
    kind: &'static str,
    #[serde(flatten)]
    analysis: Analysis,
}

#[derive(Debug, Deserialize)]
struct AnalysisStatusRequest {
    schema_version: u32,
    analysis_id: String,
}

#[derive(Debug, Serialize)]
struct AnalysisStatusResponse {
    schema_version: u32,
    kind: &'static str,
    analysis_id: AnalysisId,
    freshness: AnalysisFreshness,
}

#[derive(Debug, Deserialize)]
struct AnalysisExportRequest {
    schema_version: u32,
    analysis_id: String,
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ReportBundleResponse {
    kind: &'static str,
    #[serde(flatten)]
    bundle: ReportBundle,
}

#[derive(Debug, Serialize)]
struct JsonSchemaResponse {
    schema_version: u32,
    kind: &'static str,
    name: &'static str,
    schema: serde_json::Value,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Command::Schema { command } = cli.command {
        return run_schema(command, cli.json);
    }
    let workspace_path = cli
        .workspace
        .ok_or_else(|| Error::InvalidWorkspace("explicit workspace required".to_owned()))?;

    match cli.command {
        Command::Workspace { command } => run_workspace(command, &workspace_path, cli.json),
        Command::Source { command } => run_source(command, &workspace_path, cli.json),
        Command::Record { command } => run_record(command, &workspace_path, cli.json),
        Command::Analysis { command } => run_analysis(command, &workspace_path, cli.json),
        Command::Schema { .. } => unreachable!("Schema commands return before Workspace lookup"),
    }
}

fn run_workspace(command: WorkspaceCommand, workspace_path: &Path, json: bool) -> Result<()> {
    match command {
        WorkspaceCommand::Init => {
            let request: WorkspaceInitRequest = read_stdin_json()?;
            require_schema(request.schema_version, "workspace request schema")?;
            let status = Workspace::init(workspace_path, SubjectId::parse(&request.subject_id)?)?
                .status()?;
            write_workspace_status(status, json)
        }
        WorkspaceCommand::Status => {
            write_workspace_status(Workspace::open(workspace_path)?.status()?, json)
        }
    }
}

fn run_source(command: SourceCommand, workspace_path: &Path, json: bool) -> Result<()> {
    match command {
        SourceCommand::Register => {
            let request: SourceRegisterRequest = read_stdin_json()?;
            require_schema(request.schema_version, "source request schema")?;
            let workspace = Workspace::open(workspace_path)?;
            let registration = workspace.sources().register(SourceDescriptor {
                alias: request.alias,
                media_type: request.media_type,
            })?;
            let status = workspace.sources().status(&registration.source_id)?;
            write_source_status(status, json)
        }
        SourceCommand::Status => {
            let request: SourceStatusRequest = read_stdin_json()?;
            require_schema(request.schema_version, "source request schema")?;
            let workspace = Workspace::open(workspace_path)?;
            let status = workspace
                .sources()
                .status(&SourceId::parse(&request.source_id)?)?;
            write_source_status(status, json)
        }
    }
}

fn run_record(command: RecordCommand, workspace_path: &Path, json: bool) -> Result<()> {
    match command {
        RecordCommand::Propose => {
            let change_set: RecordChangeSet = read_stdin_json()?;
            let workspace = Workspace::open(workspace_path)?;
            let proposal = workspace.record().propose(change_set)?;
            write_record_proposal(&proposal, json)
        }
        RecordCommand::Apply => {
            let request: RecordApplyRequest = read_stdin_json()?;
            require_schema(request.schema_version, "record apply request schema")?;
            let workspace = Workspace::open(workspace_path)?;
            let proposal = workspace.record().propose(request.change_set)?;
            if proposal.id().as_str() != request.proposal_id {
                return Err(Error::ProposalIntegrity);
            }
            let receipt = workspace.record().apply(proposal)?;
            write_commit_receipt(receipt, json)
        }
        RecordCommand::Query => {
            let request: RecordQueryRequest = read_stdin_json()?;
            require_schema(request.schema_version, "record query request schema")?;
            let workspace = Workspace::open(workspace_path)?;
            let facts = workspace.record().query(request.query)?;
            let record_revision = workspace.status()?.record_revision;
            write_fact_query(record_revision, facts, json)
        }
        RecordCommand::ImportLabTimeline => {
            let request: LabTimelineImportRequest = read_stdin_json()?;
            require_schema(request.schema_version, "lab timeline import request schema")?;
            let workspace = Workspace::open(workspace_path)?;
            let proposal = workspace.record().propose_lab_timeline_import(request)?;
            let receipt = workspace.record().apply(proposal)?;
            write_commit_receipt(receipt, json)
        }
        RecordCommand::ImportVitalTimeline => {
            let request: VitalTimelineImportRequest = read_stdin_json()?;
            require_schema(
                request.schema_version,
                "vital timeline import request schema",
            )?;
            let workspace = Workspace::open(workspace_path)?;
            let proposal = workspace.record().propose_vital_timeline_import(request)?;
            let receipt = workspace.record().apply(proposal)?;
            write_commit_receipt(receipt, json)
        }
        RecordCommand::ImportSubjectPreferences => {
            let request: SubjectPreferenceMigrationRequest = read_stdin_json()?;
            require_schema(
                request.schema_version,
                "Subject Preference migration request schema",
            )?;
            let workspace = Workspace::open(workspace_path)?;
            let proposal = workspace
                .record()
                .propose_subject_preference_migration(request)?;
            let receipt = workspace.record().apply(proposal)?;
            write_commit_receipt(receipt, json)
        }
    }
}

fn run_analysis(command: AnalysisCommand, workspace_path: &Path, json: bool) -> Result<()> {
    match command {
        AnalysisCommand::Run => {
            let request: AnalysisRunRequest = read_stdin_json()?;
            require_schema(request.schema_version, "analysis run request schema")?;
            let analysis = Workspace::open(workspace_path)?.analyze(request.options)?;
            write_analysis(analysis, json)
        }
        AnalysisCommand::Status => {
            let request: AnalysisStatusRequest = read_stdin_json()?;
            require_schema(request.schema_version, "analysis status request schema")?;
            let analysis_id = AnalysisId::parse(&request.analysis_id)?;
            let freshness = Workspace::open(workspace_path)?
                .analyses()
                .status(&analysis_id)?;
            write_analysis_status(analysis_id, freshness, json)
        }
        AnalysisCommand::Export => {
            let request: AnalysisExportRequest = read_stdin_json()?;
            require_schema(request.schema_version, "analysis export request schema")?;
            let workspace = Workspace::open(workspace_path)?;
            let analysis_id = AnalysisId::parse(&request.analysis_id)?;
            let analysis = workspace.analyses().show(&analysis_id)?;
            let snapshot = workspace.record().snapshot(analysis.record_revision)?;
            let bundle = workspace.renderer().report_bundle(
                &snapshot,
                &analysis,
                RenderContext {
                    generated_at: request.generated_at,
                },
            )?;
            write_report_bundle(bundle, json)
        }
    }
}

fn run_schema(command: SchemaCommand, json: bool) -> Result<()> {
    match command {
        SchemaCommand::Show { resource } => write_schema(resource, json),
    }
}

fn read_stdin_json<T: DeserializeOwned>() -> Result<T> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(u64::try_from(MAX_JSON_INPUT_BYTES + 1).expect("JSON input limit fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_JSON_INPUT_BYTES {
        return Err(Error::InputTooLarge);
    }
    serde_json::from_slice(&bytes).map_err(Error::from)
}

fn require_schema(schema_version: u32, name: &str) -> Result<()> {
    if schema_version == 1 {
        Ok(())
    } else {
        Err(Error::Unsupported(name.to_owned()))
    }
}

fn write_workspace_status(status: crate::WorkspaceStatus, json: bool) -> Result<()> {
    if json {
        let response = WorkspaceStatusResponse {
            schema_version: status.schema_version,
            kind: "workspace_status",
            workspace_id: status.workspace_id,
            subject_id: status.subject_id,
            record_revision: status.record_revision.value(),
        };
        let stdout = io::stdout();
        let mut stream = stdout.lock();
        serde_json::to_writer(&mut stream, &response)?;
        writeln!(stream)?;
    } else {
        println!(
            "Workspace {} for Subject {} is at revision {}.",
            status.workspace_id.as_str(),
            status.subject_id.as_str(),
            status.record_revision.value()
        );
    }
    Ok(())
}

fn write_source_status(status: SourceStatus, json: bool) -> Result<()> {
    if json {
        let response = SourceStatusResponse {
            schema_version: 1,
            kind: "source_status",
            status,
        };
        let stdout = io::stdout();
        let mut stream = stdout.lock();
        serde_json::to_writer(&mut stream, &response)?;
        writeln!(stream)?;
    } else {
        println!(
            "Source {} is {:?} ({} bytes).",
            status.source_id.as_str(),
            status.content_state,
            status.size_bytes
        );
    }
    Ok(())
}

fn write_record_proposal(proposal: &crate::RecordProposal, json: bool) -> Result<()> {
    if json {
        let response = RecordProposalResponse {
            schema_version: 1,
            kind: "record_proposal",
            proposal_id: proposal.id().as_str().to_owned(),
            expected_revision: proposal.expected_revision(),
            impact: proposal.impact(),
        };
        write_json(&response)?;
    } else {
        println!(
            "Record Proposal {} would add {} Health Fact(s).",
            proposal.id().as_str(),
            proposal.impact().health_facts
        );
    }
    Ok(())
}

fn write_commit_receipt(receipt: CommitReceipt, json: bool) -> Result<()> {
    if json {
        write_json(&CommitReceiptResponse {
            kind: "commit_receipt",
            receipt,
        })?;
    } else {
        println!(
            "Committed record revision {}.",
            receipt.record_revision.value()
        );
    }
    Ok(())
}

fn write_fact_query(
    record_revision: RecordRevision,
    facts: Vec<HealthFact>,
    json: bool,
) -> Result<()> {
    if json {
        write_json(&FactQueryResponse {
            schema_version: 1,
            kind: "fact_query",
            record_revision,
            facts,
        })?;
    } else {
        println!("{} Health Fact(s).", facts.len());
    }
    Ok(())
}

fn write_analysis(analysis: Analysis, json: bool) -> Result<()> {
    if json {
        write_json(&AnalysisResponse {
            kind: "analysis",
            analysis,
        })?;
    } else {
        println!(
            "Analysis {} contains {} claim(s).",
            analysis.id.as_str(),
            analysis.claims.len()
        );
    }
    Ok(())
}

fn write_analysis_status(
    analysis_id: AnalysisId,
    freshness: AnalysisFreshness,
    json: bool,
) -> Result<()> {
    if json {
        write_json(&AnalysisStatusResponse {
            schema_version: 1,
            kind: "analysis_status",
            analysis_id,
            freshness,
        })?;
    } else {
        println!("Analysis {} is {:?}.", analysis_id.as_str(), freshness);
    }
    Ok(())
}

fn write_report_bundle(bundle: ReportBundle, json: bool) -> Result<()> {
    if json {
        write_json(&ReportBundleResponse {
            kind: "report_bundle",
            bundle,
        })?;
    } else {
        println!("Report Bundle contains {} file(s).", bundle.paths().len());
    }
    Ok(())
}

fn write_schema(resource: SchemaResource, json: bool) -> Result<()> {
    let (name, schema) = match resource {
        SchemaResource::RecordChangeSet => (
            "record-change-set",
            serde_json::to_value(schemars::schema_for!(RecordChangeSet))?,
        ),
        SchemaResource::FactQuery => (
            "fact-query",
            serde_json::to_value(schemars::schema_for!(FactQuery))?,
        ),
        SchemaResource::AnalysisOptions => (
            "analysis-options",
            serde_json::to_value(schemars::schema_for!(AnalysisOptions))?,
        ),
        SchemaResource::Analysis => (
            "analysis",
            serde_json::to_value(schemars::schema_for!(Analysis))?,
        ),
        SchemaResource::CommitReceipt => (
            "commit-receipt",
            serde_json::to_value(schemars::schema_for!(CommitReceipt))?,
        ),
        SchemaResource::ReportBundle => (
            "report-bundle",
            serde_json::to_value(schemars::schema_for!(ReportBundle))?,
        ),
    };
    if json {
        write_json(&JsonSchemaResponse {
            schema_version: 1,
            kind: "json_schema",
            name,
            schema,
        })?;
    } else {
        println!("JSON Schema: {name}");
        println!("{}", serde_json::to_string_pretty(&schema)?);
    }
    Ok(())
}

fn write_json(value: &impl Serialize) -> Result<()> {
    let stdout = io::stdout();
    let mut stream = stdout.lock();
    serde_json::to_writer(&mut stream, value)?;
    writeln!(stream)?;
    Ok(())
}
