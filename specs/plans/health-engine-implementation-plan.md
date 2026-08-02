# Health Engine Implementation Plan

## Goal

Build `/Users/csells/Code/csells/health-engine` as a public MIT-licensed Rust crate and CLI that provides one portable, structured, auditable health record and deterministic analysis engine for agents, applications, and direct human use.

Health Engine is broader than genomics. It manages the SQLite record inside a one-Subject Workspace and analyzes genomics, labs, vitals, medications, safety facts, conditions, symptoms, encounters, diagnostic studies, care follow-ups, and other structured health facts together.

The supporting documents are:

- [Vision](../vision/health-engine-vision.md)
- [Domain language](../../CONTEXT.md)
- [Code survey](../research/code-survey.md)
- [Architectural decisions](../adr)

## Product boundary

Three layers stay distinct:

1. **Agents and applications** identify, name, and file raw health documents; interpret PDFs, images, and conversations; extract facts; and write narrative reports.
2. **Health Engine** validates, stores, corrects, verifies, and queries structured facts in SQLite and produces sourced, confidence-labeled Analyses.
3. **The Workspace host** chooses the directory, backs it up, and encrypts the whole directory when required.

Health Engine does not interpret arbitrary documents, invent personalized medical instructions, route raw files, or hide personal data in a global location. Personal data exists only in an explicitly selected Workspace. Public reference data lives in a separate replaceable cache.

## Data locations

| Data | Owner and location |
|---|---|
| PDFs, images, visit summaries, genome exports | Agent-filed paths inside the private Workspace |
| Structured facts, provenance, corrections, verification, analyses | `.health-engine/health.sqlite3` inside the Workspace |
| Human-authored or agent-authored reports | Agent-managed files inside the Workspace |
| ClinVar, ClinGen, ACMG, CPIC, ClinPGx, GWAS, and similar public data | Health Engine public-data cache outside every Workspace |
| Curated project-authored variant knowledge | MIT-licensed engine resource after per-entry audit |
| Code, schemas, documentation, synthetic fixtures | Public `health-engine` repository |

Source paths stored in SQLite are Workspace-relative so the directory remains portable. Health Engine does not require a particular source-folder taxonomy.

## Workspace record

Each Workspace contains exactly one Subject. Its Operator may be the Subject, a caregiver, an agent, or an application.

SQLite uses an append-only fact model:

- A common fact envelope records stable ID, type, effective time, recording time, source reference, source location, author/agent, extraction confidence, and verification status.
- Typed tables hold domain fields for labs, vitals, medications, medication safety, conditions, symptoms, encounters, diagnostic studies, Care Tasks, Care Protocols, genomics, and extensible future facts.
- Corrections reference earlier facts rather than updating them in place.
- Verification records source checks; discrepancies and Record Hazards remain unresolved until explicitly addressed.
- Current-state views resolve correction chains without hiding history.
- Analyses record every fact, rule, engine version, and public-data version they used.
- A changed dependency marks a saved Analysis stale rather than rewriting it.

New agent extractions begin `Unverified`. They remain usable, but that status lowers the confidence of dependent claims and is visible in every output.

## Rust library seams

Keep a small public surface over deep modules:

```rust
let workspace = Workspace::open(path)?;

workspace.record().append(fact)?;
workspace.record().correct(correction)?;
workspace.record().verify(verification)?;
let facts = workspace.record().query(query)?;

let analysis = workspace.analyze(options)?;
```

The primary modules are:

- `Workspace`: explicit path, database lifecycle, migrations, transactions, and one-Subject invariant.
- `HealthRecord`: typed write, correction, verification, and query interface.
- `HealthEngine`: deterministic cross-domain analysis, confidence, provenance, guidance, and freshness.
- `ReferenceDataManager`: public-data status, update checks, downloads, validation, and replacement.
- `Renderer`: terminal, JSON, and Markdown views derived only from structured Analysis.

Agents use the same contracts through JSON CLI commands. Filesystem discovery and document interpretation never leak into the Rust domain model.

## CLI grammar

Use consistent resource command groups:

```text
health-engine workspace init|status
health-engine record add|query|correct|verify|import-genome
health-engine analysis run|status|show|export
health-engine data status|check|update
```

Rules:

- Every command accepts an explicit Workspace path or discovers one only through a documented marker while walking upward.
- Agent-facing commands accept and emit versioned JSON; human-readable output is the default where appropriate.
- Diagnostics go to stderr and include stable error codes and exact remediation commands.
- `data status` is offline, `data check` is read-only network access, and only `data update` changes the public cache.
- Missing reference data never triggers an implicit download.
- Unsupported genome formats and builds fail loudly without exposing genotype rows in diagnostics.

## Phase 1: Bootstrap the OSS project

1. Create the Rust library/binary package, MIT license, README, contribution guide, security policy, and medical-use disclaimer.
2. Keep plans in `specs/plans/`, vision in `specs/vision/`, research in `specs/research/`, decisions in `specs/adr/`, and reserve `docs/` for user-facing documentation.
3. Add formatting, strict linting, unit tests, dependency/license checks, and CI.
4. Define stable IDs, timestamps, source references, verification states, Confidence categories, and structured errors.
5. Check in versioned JSON Schemas for agent commands and Analysis.

Acceptance:

- `cargo test`, `cargo fmt --check`, and strict `cargo clippy` pass.
- Schemas and examples validate in CI.
- No fixture or repository file contains real personal health data.

## Phase 2: Build the Workspace and Health Record

1. Implement `workspace init` and `Workspace::open` around `.health-engine/health.sqlite3`.
2. Add forward-only SQLite migrations and transactional writes.
3. Enforce one Subject per Workspace and keep Operator provenance separate.
4. Implement the common fact envelope, source references, typed payload validation, and current-state views.
5. Implement append-only Correction, Verification, Discrepancy, and Record Hazard workflows.
6. Add query filters for type, date, source, verification status, related entity, and current/history views.

Acceptance:

- A Workspace can be copied to another path and opened without rewriting source references.
- No supported command edits or deletes accepted history in place.
- Failed writes roll back completely.
- Source-audit queries return every fact attributed to a given PDF or other Source.

## Phase 3: Deliver a lab-centered vertical slice

Use labs to prove the complete agent-to-analysis path before multiplying fact types.

1. Accept a structured Lab Result from JSON and store its collection time, original name, value, units, reporting-lab range and flag, source path, and source location.
2. Add Canonical Test identities, conservative name mapping, and exact unit conversion while retaining untouched originals.
3. Query longitudinal results across sources.
4. Detect trends, reversals, lab-reported abnormalities, and missing/stale measurements only when a sourced interval exists.
5. Generate and save Analysis JSON with provenance, verification effects, confidence, warnings, and freshness dependencies.
6. Render the same Analysis as terminal text and Markdown.

Acceptance:

- An agent can file a synthetic PDF, submit extracted facts, query them by source, verify them, and generate a trend Analysis.
- `normal`, `not measured`, `never measured`, `not found`, and `not looked for` remain distinct.
- Every rendered claim exists in the structured Analysis.

## Phase 4: Complete the general Health Record

Add typed facts and queries for:

- Vital Measurements with setting, observer/device, and context.
- Medication Events and supplements as starts, stops, dose changes, missed doses, and as-needed use.
- Allergies, intolerances, adverse reactions, contraindications, and prescribing cautions without collapsing their meanings.
- Condition Assertions with suspected, confirmed, ruled-out, inactive, and resolved states.
- Symptom Episodes with exact quotes and `present`, `absent`, `unknown`, and `not asked` observations.
- Clinical Encounters grouping provider, reason, facts, orders, and follow-up.
- Diagnostic Studies preserving method, body site, findings, clinician impression, and status.
- Care Tasks for orders, referrals, repeat intervals, expected results, due dates, and completion.
- Sourced Care Protocols for reusable questions and time-sensitive actions.
- Procedures, immunizations, family history, reproductive history, social/environmental exposure, devices, and other extensible observations.

Acceptance:

- Current medication and condition views are derived from history rather than overwritten lists.
- A known chart error follows every related query and Analysis.
- Episode queries can compare frequency and deviations without paraphrasing the Subject.
- An ordered test cannot silently become completed or normal.

## Phase 5: Build cross-domain Analysis

1. Define Findings, Guidance, Exclusions, Warnings, Care Task status, and source/evidence references as the complete Analysis contract.
2. Put `High`, `Moderate`, `Low`, `Conflicting`, or `Unknown` on every claim, together with the rationale and native source rating.
3. Propagate unverified, disputed, corrected, or hazardous inputs into dependent confidence and warnings.
4. Compare medication timelines with labs, vitals, and symptoms as temporal relationships, not unsupported causal claims.
5. Detect stale tests from sourced Care Tasks or cited guidelines, not invented schedules.
6. Preserve every historical Analysis and mark it stale when a fact, correction, rule, engine version, or reference-data version changes.
7. Keep source-backed actionable guidance in the structured contract; agents remain responsible for personalized narrative synthesis.

Acceptance:

- Every conclusion is traceable to exact facts, sources, rules, and reference versions.
- Saved Analyses cannot be presented as current after a dependency changes.
- No renderer or agent-only prose is the sole carrier of clinical meaning.

## Phase 6: Add genomics and public reference data

1. Implement strict 23andMe GRCh37 import into the Workspace's genomic facts; reject unknown formats and builds.
2. Preserve CRLF handling, no-calls, haploid calls, allele order, and the distinction between ordering and strand complementation.
3. Implement `data status`, `data check`, and `data update` for ClinGen, ClinVar GRCh37, ACMG Secondary Findings, CPIC, ClinPGx, and GWAS Catalog.
4. Download, validate, and build the currently installed copy of each source without redistributing third-party data.
5. Join sources only on build-safe keys and keep source updates independent.
6. Port CPIC coverage/no-call/default-allele rules and explicitly refuse unsafe array calls.
7. Audit and source the inherited Curated Variant Knowledge, including every known divergent plugin record.
8. Produce disease, pharmacogenomic, secondary-finding, GWAS-enriched, protective, and curated claims through the same Analysis model used by labs and medications.

Acceptance:

- Known false-positive and false-star-allele cases remain impossible.
- Every genomic claim carries source versions, confidence, coverage limits, and source-backed guidance.
- A public-data update changes subsequent Analysis provenance and marks earlier Analyses stale.

## Phase 7: Make agents first-class clients

1. Publish a generic Health Engine skill with the CLI schemas and privacy rules.
2. Update record-filing skills to file Sources themselves, then submit extracted facts through `record add`.
3. Add an audit workflow that queries facts by Source, re-reads each PDF, and records Verification or Discrepancy results.
4. Update symptom skills to fetch sourced Care Protocols and write completed episodes.
5. Update health-report skills to consume Analysis JSON and record queries instead of treating living Markdown as the structured database.
6. Update genome-analysis skills to file the raw export, call `record import-genome`, check public data, and run Analysis.
7. Adopt the engine in `genetic-health`, `april-health-stuff`, and `health-tracker-plugin` without changing or deleting their source clinical documents.

Acceptance:

- The three existing Workspaces use one engine without copied Python logic or duplicate structured databases.
- A new user can use the public skill without any person-specific defaults.
- Agents never bypass Health Engine to mutate SQLite directly.

## Phase 8: User documentation and release

1. Write user-facing installation, Workspace, data-update, import, query, analysis, audit, privacy, backup, and troubleshooting documentation under `docs/`.
2. Provide human and agent examples for every command group.
3. Publish Rust API documentation and JSON Schemas.
4. Build signed/checksummed releases for macOS, Linux, and Windows and publish the crate when its public API is ready.
5. Test a clean-machine flow with a synthetic Workspace from initialization through source extraction, verification, analysis, correction, stale detection, and re-analysis.

## Verification matrix

| Area | Required verification |
|---|---|
| SQLite record | Migration, transaction, append-only, correction-chain, portability, and concurrency tests |
| Agent contract | JSON Schema, invalid payload, source-reference, idempotency, and structured-error tests |
| Source audit | Facts-by-source, verified/unverified, discrepancy, and Record Hazard tests |
| Labs/vitals | Name/unit normalization, original preservation, trend/reversal, context, and missing-state tests |
| Medications/conditions | Timeline reconstruction, conflicting assertions, safety-type distinction, and no-causation tests |
| Symptoms/tasks | Exact quotes, four-state answers, pattern comparison, due/overdue/completion tests |
| Analysis | Confidence propagation, provenance, guidance limits, dependency fingerprint, and stale-state tests |
| Genomics | Format/build rejection, haploid/no-call, join-build safety, CPIC coverage, and known regression tests |
| Reference data | Offline status, read-only check, failed update, replacement, version provenance, and license notice tests |
| Privacy | Synthetic-only fixtures, no telemetry, no hidden copies, diagnostic redaction, and explicit Workspace tests |
| Renderers | Every human claim maps to Analysis JSON; stdout/stderr separation and golden output tests |

## Completion criteria

The first complete release exists when an agent can file a Source in a one-Subject Workspace, submit and verify structured health facts through Health Engine, query a coherent longitudinal record, generate a current sourced Analysis spanning general health and genomics, update public reference data explicitly, detect stale conclusions, and render the result for humans or agents—without personal data or third-party datasets entering the OSS repository.
