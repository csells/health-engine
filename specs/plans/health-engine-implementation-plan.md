# Health Engine Implementation Plan

## Status

**Plan state:** Approved. Implementation is in progress under the gate evidence below.

The checked-in Rust scaffold is intentionally disposable: it does not currently build because declared library modules and the CLI entry point are absent, and its initial model and migration do not yet enforce the reviewed contracts. Preserve useful vocabulary and types only where they survive the red-green slices below.

| Gate | Status | Required outcome |
|---|---|---|
| 0. Protected migration oracle | Complete | Private corpus and legacy claims classified without changing the original repository |
| 1. Synthetic evidence-ledger journey | Complete | One complete lab journey passes through library and CLI public seams |
| 2. Ledger integrity and privacy | Complete | Mutation, reconciliation, quarantine, concurrency, and Expungement guarantees hold |
| 3. Migration-required record slices | Complete | Every non-genomic migration input has a typed disposition and exercised record path |
| 4. Genomics and reference data | In progress | Legacy and current genomic claims are complete, sourced, safe, and explainable |
| 5. Analysis, reports, and skills | Pending | Familiar reports and thin skills derive only from Health Record and Analysis |
| 6. Full replay and handoff | Pending | Repeatable full migration has zero unexplained differences and no unaccounted inputs |

Update this table only from verification evidence. Use `In progress`, `Complete`, or `Blocked`; a gate is never complete because code merely exists.

## Goal

Build `/Users/csells/Code/csells/health-engine` as a public MIT-licensed Rust crate and CLI that provides one portable, structured, auditable health record and deterministic analysis engine for agents, applications, and direct human use.

Health Engine is broader than genomics. It manages the SQLite record inside a one-Subject Workspace and analyzes genomics, labs, vitals, medications, safety facts, conditions, symptoms, encounters, diagnostic studies, care follow-ups, and other structured health facts together.

The supporting documents are:

- [Vision](../vision/health-engine-vision.md)
- [Domain language](../../CONTEXT.md)
- [Code survey](../research/code-survey.md)
- [Existing-engine ecosystem research](../research/gemini-existing-health-engine-research.md)
- [Architectural decisions](../adr)

## Product boundary

Three layers stay distinct:

1. **Agents and applications** identify, name, and file raw health documents; interpret PDFs, images, and conversations; extract facts; and write narrative reports.
2. **Health Engine** validates, stores, corrects, verifies, and queries structured facts in SQLite and produces sourced, confidence-labeled Analyses.
3. **The Workspace host** chooses the directory, backs it up, and encrypts the whole directory when required.

Health Engine does not interpret arbitrary documents, invent personalized medical instructions, route raw files, or hide personal data in a global location. Personal data exists only in an explicitly selected Workspace. Public reference data lives in a separate replaceable cache.

## Build versus adopt

No surveyed project supplies the combination of a portable one-Subject record, append-only corrections, source-level provenance, agent verification, cross-domain Analysis, and native consumer-genomics support. Build that core in Rust rather than adopting an EHR, FHIR server, personal-health dashboard, or genomics pipeline as the product foundation.

Adopt focused libraries, public datasets, and standard exchange formats behind explicit seams when they reduce implementation risk. Existing systems such as Fasten Health, openEHR, PharmCAT, and Open-CRAVAT are references and potential conformance or optional integration targets, not core runtime dependencies. FHIR import/export and MCP access are plausible adapters after the crate and JSON CLI contracts are stable; neither is the canonical record model or required for the first complete release. Do not add a generic event-sourcing framework, embedded WASM rules runtime, UMLS credential workflow, or external genomics process without a demonstrated requirement and a separately accepted decision.

## Data locations

| Data | Owner and location |
|---|---|
| PDFs, images, visit summaries, genome exports | Agent-filed paths inside the private Workspace |
| Structured facts, provenance, corrections, verification, analyses | `.health-engine/health.sqlite3` inside the Workspace |
| Derived reports | Renderer outputs at explicit Workspace-relative paths |
| Human-authored narrative | Human- or agent-managed files inside the Workspace |
| ClinVar, ClinGen, ACMG, CPIC, ClinPGx, GWAS, and similar public data | Explicitly configured Health Engine cache root outside every Workspace; the migration cache is under ignored `tmp/` |
| Curated project-authored variant knowledge | MIT-licensed engine resource after per-entry audit |
| Code, schemas, documentation, synthetic fixtures | Public `health-engine` repository |

Source paths stored in SQLite are Workspace-relative so the directory remains portable. Health Engine does not require a particular source-folder taxonomy.

## Legacy migration contract

Migration from an existing health repository is source-first. Original clinical documents and genome exports are immutable Sources. Structured observations may become Health Facts only after reconciliation to a cited Source or classification as an explicit self-report. Living documents and generated reports are coverage and expected-output oracles, not authoritative clinical truth; claims found only there remain Migration Candidates until confirmed.

Migration completeness means every input file and candidate claim is accounted for as accepted, deduplicated, rejected with a reason, or quarantined with a technical disposition and owner. It does not mean converting every sentence in a derived report into a Health Fact or asking the Subject to resolve technical ambiguity.

Report compatibility is semantic rather than textual. The structured Analysis is the authoritative result, and every substantive legacy generated-report claim must be reproduced, corrected, deliberately excluded with a reason, or retained as a Migration Candidate. Exact wording, ordering, Markdown, and HTML are not compatibility contracts, though legacy filenames or broad sections may be preserved temporarily while skills migrate.

The first complete release is migration-driven. It implements the foundational engine and every record type, analysis capability, reference-data path, Renderer, and adapter required to account for all `genetic-health` inputs and replace its substantive skills and reports. Domains without a concrete migration Source or workflow remain planned until they have an observable acceptance scenario; the implementation does not invent their interfaces speculatively.

Evidence Assurance is origin-specific. Machine-readable Sources become parser-validated through deterministic parsers and independent regression fixtures. Document-extracted facts become source-verified only after a separate check against an exact Source location recording the verifier, method, version, time, scope, and result. A direct Subject or Operator statement is accepted as explicitly attributed self-report, not mislabeled as Source Verification or clinical truth. Failed or incomplete checks remain Migration Candidates. Unverified or quarantined evidence cannot appear as established, Care-affecting Guidance follows a tested assurance policy, and self-reports remain visibly attributed wherever they matter.

| Evidence state | Permitted use |
|---|---|
| Parser-validated | Established Finding input; Guidance only when the rule accepts the format and every other prerequisite passes |
| Source-verified | Established Finding input; Guidance only when every rule-specific prerequisite passes |
| Explicit self-report | Labeled Fact and Finding input; Guidance only when the cited rule explicitly accepts self-report for that prerequisite |
| Unverified | Visible unresolved evidence and confidence deduction; never an established input or sole trigger for Guidance |
| Quarantined, conflicting, or hazardous | Warning, exclusion, `why-not`, or reconciliation input; never a silent trigger for Guidance |

Evidence Assurance describes how faithfully an input is known from its origin; Confidence describes the strength of an Analysis claim. Neither substitutes for the other.

Health Engine is the sole structured-record and deterministic-analysis authority. A generic Health Engine skill exposes its Workspace, schema, Source, Health Record, Verification, Correction, Analysis, and migration-audit workflows. Legacy intake becomes a thin filing and extraction adapter; legacy health-assessment reporting becomes a presentation adapter over Health Record queries and complete Analysis JSON. Migration testing uses a gitignored `tmp/` working copy, and the original personal archive remains read-only until a separately authorized cutover.

Migration records a Legacy Reference Baseline containing digests and available version or provenance metadata for the public resources that produced historical reports. That baseline is used only to reproduce and account for legacy claims. Health Engine separately acquires current Installed Data through its explicit update workflow, produces the current Analysis from it, and reports claim differences caused by code corrections separately from differences caused by reference-data changes. Public reference resources remain outside the private Workspace database.

Legacy recommendations enter current Guidance only when their exact Reference Source recommendation, strength, prerequisites, qualifications, and applicable evidence can be preserved. Explicit personal routines may be retained as Subject Preferences. Unsupported recommendations remain Migration Candidates or historical narrative with an exclusion reason; semantic compatibility does not promote them, and migration does not perform open-ended medical web research to rescue them outside the accepted reference-data or curated-knowledge process.

Source identity is content-addressed. Identical bytes form one Source with multiple Source Aliases, while different documents remain distinct Sources even when they describe the same event. The engine may read Source bytes only to validate identity, size, media type, and aliases; it neither interprets nor manages the file, and raw private digests never appear in normal output. Moving an alias does not stale an Analysis while identical Source content remains available; changed or unavailable evidence invalidates dependent source Verification or raises an audit warning.

Idempotency never deduplicates facts by value alone. Retrying the same command or Extraction Run returns its original Commit Receipt. A later run's exact same Source, location, and typed assertion is `already present`; a changed assertion becomes a Discrepancy or Migration Candidate. Equal assertions from different Sources remain separate immutable Health Facts and may be linked by Reconciliation for one normal-view presentation.

Accepted history is append-only except for explicit Operator-authorized Expungement after a wrong-Subject import, accidental sensitive ingestion, or deliberate privacy request. Expungement physically removes affected PHI-bearing Facts, provenance, staged proposals, receipts, derived Analyses, Report Bundles, and SQLite journals or temporary artifacts under engine control while retaining only a tombstone with a fixed non-PHI reason code. Agents may propose but never execute it autonomously; the Workspace host receives affected Source aliases and explicit remediation for files, snapshots, and backups outside engine control.

Ordinary writes use a guarded transaction seam. A versioned, idempotent Record Change Set identifies the exact Workspace, Subject, evidence origin and references, extractor, and expected record revision and contains all related additions, Corrections, Verifications, Reconciliations, and provenance. Health Engine validates it without mutation into a content-addressed Record Proposal with an impact preview, then atomically applies that exact proposal only if its hash and expected revision still match. Retries return the original Commit Receipt; each Extraction Run commits atomically, while a Source may be revisited by later versioned runs with different declared coverage. Expungement remains a separate explicitly authorized interface.

Every document extraction is an Extraction Run that records the extractor and version, Source, declared regions and domains examined, omissions, failures, and resulting candidates. Coverage—not an assumption that the whole Source was understood—distinguishes `not found` from `not looked for`.

The common Fact envelope uses source-faithful Clinical Time rather than a universal date. It supports exact instants with known offsets, dates, intervals, and partial dates with explicit precision, preserving source text when normalization would lose information. Engine-assigned UTC recording time is separate; domain facts may add collection, issuance, onset, recovery, or other meaningful times, and missing components or timezones are never invented.

Humans are not technical transcription or clinical-validation oracles. Deterministic parsers, independent source-fidelity checks, rules, and reference-data conformance own those responsibilities. Skills interrupt an Operator only for identity, explicit self-reports, current preferences, clinician-question selection, or destructive authorization, recording answers as their own sourced assertions rather than proof that another Source was wrong. Unresolved technical or clinical ambiguity remains quarantined, visible, and unable to silently drive Guidance.

The migrated Workspace preserves the familiar human entry points: `reports/HEALTH_REPORT.html` for the Subject, `reports/PHYSICIAN_SUMMARY.md` for providers, and `reports/current/health-summary.md` and `open-loops.md` as convenient readable views. One deterministic Renderer projects all four as an atomic Report Bundle from an immutable Record Snapshot, its matching complete Analysis, and explicit render context. Each exposes its generation time, record revision, Analysis identity, and claim identities. Replacement skills invoke and publish this projection while retaining the existing natural-language interactions; any agent-authored narrative remains separate and non-authoritative.

An Operator's request to process intake, add a result, or record a reading authorizes ordinary additions when identity, origin-appropriate Evidence Assurance, validation, and concurrency checks pass. The skill proposes and applies them automatically and returns a concise Commit Receipt covering Sources filed, facts accepted or deduplicated, views refreshed, Care Tasks affected, and newly stale Analyses. It interrupts only for uncertain identity, genuinely ambiguous self-report, a serious conflict affecting Guidance, or destructive authorization; Source-demonstrated Corrections are clearly shown in the receipt.

Migration Differences are engineering verification evidence, not Subject decisions. Every substantive legacy-versus-current difference must produce either a fixed implementation or ingestion defect, or an evidence-backed explanation that the legacy result was wrong, unsupported, deduplicated, or changed by newer reference data. Detailed reconciliation artifacts remain in the ignored migration workspace; final handoff reports only the remaining differences and their explanations.

## Execution discipline

Implementation uses strict vertical red-green TDD. For each behavior: name the public seam, add one failing synthetic behavior test, run it and observe the expected failure, add only enough implementation to pass, and continue with the next behavior. After a coherent slice is green, review and refactor through the same public seam, then rerun its gate. Do not write horizontal batches of imagined tests or test record behavior by querying SQLite behind the interface.

Private migration failures must first become structurally equivalent synthetic regressions before code changes. Personal values, filenames, hashes, genotypes, document text, and report claims never enter committed fixtures, snapshots, terminal output, process arguments, logs, or final summaries. PHI-bearing command payloads travel through stdin or a Workspace-private file, not shell arguments. Long-running suites use progress reporting, per-stage timeouts, child-process cleanup, and machine-readable logs under ignored `tmp/`; unavailable live dependencies are reported as inconclusive or failed evidence, never silent passes.

The original `genetic-health` repository remains read-only. Before any personal data is copied, `tmp/` is gitignored and permission-restricted; all working copies, Workspaces, databases, migration manifests, claim ledgers, legacy-reference bytes, and detailed differences remain there. No push, publication, or real cutover occurs without separate explicit authorization.

## Workspace record

Each Workspace contains exactly one Subject. Its Operator may be the Subject, a caregiver, an agent, or an application.

SQLite uses an append-only fact model:

- A common immutable fact envelope records stable ID, type, Clinical Time, recording time, source identity and location, author or agent, and origin-appropriate Evidence Assurance.
- Type-specific relational projections hold queryable domain fields while a canonical versioned payload preserves lossless interchange; deterministic queries never depend on an unvalidated JSON-blob schema.
- Corrections reference earlier facts rather than updating them in place.
- Verification records source checks; discrepancies and Record Hazards remain unresolved until explicitly addressed.
- Current-state views resolve correction chains without hiding history.
- Analyses are typed dependency graphs that record every Fact, Correction, rule, policy decision, engine version, curated-knowledge version, and public-data version they used.
- A changed dependency marks a saved Analysis stale rather than rewriting it.

New agent extractions begin `Unverified`. They remain usable, but that status lowers the confidence of dependent claims and is visible in every output.

## Rust library seams

Keep a small public surface over deep modules:

```rust
let workspace = Workspace::open(path)?;

let source = workspace.sources().register(source_descriptor)?;
let proposal = workspace.record().propose(change_set)?;
let receipt = workspace.record().apply(proposal)?;
let facts = workspace.record().query(query)?;

let analysis = workspace.analyze(options)?;
let explanation = workspace.analyses().explain(analysis.id(), claim_id)?;
let snapshot = workspace.record().snapshot(analysis.record_revision())?;
let reports = workspace.renderer().report_bundle(&snapshot, &analysis, render_context)?;
reports.publish(workspace.path())?;
```

The primary modules are:

- `Workspace`: explicit path, database lifecycle, migrations, transactions, and one-Subject invariant.
- `SourceRegistry`: opaque Source identity, private digest verification, path aliases, changed or missing detection, freshness effects, and Extraction Run coverage; it validates bytes but never files, moves, or interprets them.
- `HealthRecord`: typed write, correction, verification, and query interface.
- `HealthEngine`: deterministic cross-domain analysis, confidence, provenance, guidance, and freshness.
- `ReferenceDataManager`: an explicit cache root, public-data status, update checks, bounded downloads, validation, and replacement.
- `Renderer`: terminal, JSON, Markdown, HTML, and atomic Report Bundle projections derived only from a Record Snapshot and matching structured Analysis.

Agents use the same contracts through JSON CLI commands. Filesystem discovery and document interpretation never leak into the Rust domain model.

## CLI grammar

Use consistent resource command groups:

```text
health-engine workspace init|status
health-engine source register|status
health-engine record propose|apply|query|history|import-genome|expunge
health-engine analysis run|status|show|export|explain|impact|diff|why-not
health-engine data status|check|update
```

Rules:

- Every mutation requires explicit Workspace and Subject identity. Read-only commands may discover a Workspace through a documented marker while walking upward.
- Agent-facing commands accept and emit versioned JSON; human-readable output is the default where appropriate.
- PHI-bearing JSON is read from stdin or an explicit Workspace-private file, never from process arguments.
- Diagnostics go to stderr and include stable error codes and exact remediation commands.
- `data status` is offline, `data check` is read-only network access, and only `data update` changes the public cache.
- Every data command receives or resolves a documented cache root and reports it; library callers always supply one explicitly.
- Missing reference data never triggers an implicit download.
- Unsupported genome formats and builds fail loudly without exposing genotype rows in diagnostics.
- `analysis explain` accepts a claim ID, `impact` a Fact ID, `diff` two Analysis IDs, and `why-not` a rule or candidate key.
- `record import-genome` is a specialized non-mutating proposal constructor: it streams into a private hash-bound stage, seals a Record Proposal against the expected revision, and returns its identity for `record apply`. Skills may invoke both steps under ordinary intake authorization, but no importer writes canonical facts directly.

## Gate 0: Establish a protected migration oracle

1. Add `tmp/` to `.gitignore`, verify Git ignores it, restrict its permissions, and keep all private working copies, SQLite files, manifests, logs, and comparison artifacts beneath it.
2. Copy the legacy repository into `tmp/` without modifying the original, then record read-only before-and-after attestations for the original tree.
3. Inventory every legacy file by role: authoritative Source, machine-readable Source, derived view, skill, code, reference resource, or unsupported artifact.
4. Build a machine-readable private semantic-claim ledger from generated reports and living documents. Give each claim a stable private identity, source location, normalized meaning, and disposition under a versioned difference schema; classify claims as expected outputs or Migration Candidates, never as facts merely because they appear in prose.
5. Record the Legacy Reference Baseline using digests, versions, provenance, and license metadata without committing resource bytes.
6. Produce a private capability inventory that maps each exercised legacy behavior to a planned record, Analysis, report, or skill slice.

Acceptance:

- Every input file and substantive legacy claim has an initial classification and a reproducible inventory entry.
- The original repository remains byte-for-byte unchanged, and no private filename, value, hash, genotype, document text, or report claim appears in tracked or distributed files or terminal output. The authorized ignored `tmp/` tree is a private migration Workspace, not OSS content.
- The migration oracle can be rebuilt from a fresh protected copy.

## Gate 1: Prove the synthetic evidence-ledger journey

Use one synthetic lab journey to prove every public seam before multiplying domains.

1. Make the Rust library and CLI build; add the MIT license, public documentation skeleton, CI, strict linting, dependency/license checks, medical-use disclaimer, structured errors, deterministic IDs and clocks, and versioned JSON Schemas.
2. Initialize and reopen a portable one-Subject Workspace through library and CLI contracts.
3. Register a Source behind an opaque identity, preserve Workspace-relative aliases, detect drift or absence without exposing its digest, apply the defined freshness behavior, and reject unsafe paths.
4. Record an Extraction Run with exact examined scope, omissions, failures, coverage, and resulting candidates.
5. Propose and atomically apply a lab Change Set, return an idempotent Commit Receipt, and query it through current and history views.
6. Verify and correct the fact, run a typed Analysis, mark it stale when a dependency changes, reanalyze it, and render only claims present in the Analysis.

Acceptance:

- The complete journey passes through both the public Rust API and versioned JSON CLI using synthetic data.
- Invalid proposals, stale revisions, and failures cause no partial mutation; retries return the same receipt.
- The Workspace remains portable, source originals remain lossless, and every rendered claim maps to a structured claim ID.

## Gate 2: Enforce ledger integrity and privacy

1. Prove transaction rollback, optimistic concurrency, content and command idempotency, bounded extraction, and deterministic replay.
2. Implement Source Aliases, Reconciliation, conflicts, Discrepancies, Record Hazards, and current/history views without erasing assertions or provenance.
3. Exercise Corrections, multi-source assertions, explicit self-report, quarantine, and resolvable versus unresolved ambiguity.
4. Make the Evidence Assurance policy executable: parser-validated inputs retain parser evidence; document extractions require source Verification before appearing as established; direct self-reports remain explicitly attributed; checked failures are quarantined; unresolved conflicts and hazards remain visible; and Care Guidance obeys rule-specific prerequisites and cannot rely on disallowed, hazardous, or quarantined evidence.
5. Implement separately authorized Expungement for dependent PHI-bearing facts, provenance, proposals, receipts, Analyses, Report Bundles, temporary state, and SQLite journals under engine control, leaving only a fixed-code non-PHI tombstone and returning external Source, snapshot, and backup remediation. Agents may propose but cannot authorize it.
6. Verify restrictive permissions, crash recovery, path safety, controlled-artifact cleanup, and diagnostic, process-argument, and external-remediation redaction.

Acceptance:

- Database constraints and public-interface tests enforce append-only history, the narrow Expungement exception, concurrency, and idempotency.
- Quarantined or hazardous inputs cannot silently become established findings or drive Guidance.
- Failure paths leave no private material in logs, diagnostics, temporary files, or tracked artifacts.

## Gate 3: Implement migration-required record slices

Order domains from the private inventory rather than this document. For each exercised legacy domain—expected to include labs, vitals, medications and supplements, conditions, encounters, diagnostic studies, Care Tasks, and Subject Preferences—complete one vertical slice before starting the next:

1. Create an independent synthetic example and add one failing public-interface behavior test.
2. Add its versioned schema, source-faithful Clinical Time, typed relational projection, canonical payload, validation, query, and current/history behavior.
3. Carry Source, Extraction Run, origin-appropriate Evidence Assurance, Verification where applicable, Correction, Reconciliation, and quarantine behavior through the slice.
4. Add its Analysis dependency and report projection without inventing unsupported causation, schedules, normality, or absence.
5. Replay the matching private migration subset, classify every input, and turn each implementation failure into a structurally equivalent synthetic regression before fixing it.

Domain-specific invariants include preserving original lab values, ranges, flags, and names; distinguishing absence states through Extraction Run coverage; reconstructing medication and condition views from history; keeping medication timing separate from causation; and preventing an ordered study or task from becoming completed, normal, or scheduled without evidence.

Acceptance:

- Every matching private input has an accepted, deduplicated, rejected, or quarantined disposition before work advances to the next domain.
- Current views are derived from immutable history, and known errors and conflicts follow every related query and Analysis.
- No private data appears in committed tests, snapshots, diagnostics, process arguments, or logs.

Verification evidence (2026-08-02): synthetic public-interface contracts and fresh protected replays pass for labs, vitals, medications and supplements, conditions, Diagnostic Studies, Care Tasks, and Subject Preferences. Every exercised private subset is accepted or deliberately quarantined; the derived tracking choice remains a confirmation-required Migration Candidate rather than being promoted to a Fact. The verified capability inventory contains no distinct encounter input, so the migration-driven first release does not invent an encounter interface. `cargo test --locked` and strict all-target Clippy are green after the complete Gate 3 replay.

## Gate 4: Implement genomics and reference data

1. Implement a strict, streaming, bounded-memory 23andMe GRCh37 parser that rejects unknown formats and builds and correctly handles CRLF, no-calls, haploid calls, allele ordering, declared coverage, and strand rules. It writes only to a private hash-bound stage, seals a guarded Record Proposal, and never mutates canonical state before `record apply` validates its hash and expected revision.
2. Reproduce historical claims only against the private Legacy Reference Baseline; acquire current ClinGen, ClinVar GRCh37, ACMG Secondary Findings, CPIC, ClinPGx, and GWAS data through the explicit Installed Data lifecycle.
3. Give every reference adapter offline status, read-only check, explicit update, validation, bounded network timeouts and retries, atomic replacement, version provenance, license notice, and failure tests. Required missing data fails before Analysis rather than downloading implicitly.
4. For each genomic claim family, create tiny synthetic resources and public worked expectations before replaying the corresponding private claims.
5. Enforce build-safe joins, SNV and array limitations, haploid and no-call behavior, CPIC coverage/default-allele/phase rules, and explicit refusal when an array cannot support a safe call.
6. Audit each inherited Curated Variant Knowledge entry for license, exact source, applicability, and divergence before including it.
7. Classify every current-versus-legacy difference as an implementation/ingestion defect to fix, a corrected legacy result, or a change attributable to a newer reference version.

Acceptance:

- Every legacy genomic claim is accounted for, and every current claim carries sources, versions, confidence, coverage limits, and source-backed Guidance where applicable.
- Known false-positive and unsafe star-allele cases are impossible through tested public seams.
- Repeated import is idempotent, observable, bounded in memory and staged disk, crash-cleanable, and deterministic for the same inputs and reference versions.

## Gate 5: Complete Analysis, reports, and skills

1. Complete the typed Analysis dependency graph for Findings, Guidance, Exclusions, Warnings, Care Task status, confidence deductions, missing prerequisites, and exact dependencies on Facts, Sources, rules, policies, engine versions, curated knowledge, and Installed Data.
2. Implement `explain`, `impact`, `diff`, and `why-not` from graph edges; treat fingerprints as integrity digests rather than explanations.
3. Add only the cross-domain rules exercised by the migration, preserving temporal relationships without asserting unsupported causation and deriving stale care only from sourced tasks or guidance.
4. Generate `reports/HEALTH_REPORT.html`, `reports/PHYSICIAN_SUMMARY.md`, `reports/current/health-summary.md`, and `reports/current/open-loops.md` as one atomic Report Bundle from a Record Snapshot, its matching complete Analysis, and explicit render context. Include generation time, record revision, Analysis ID, and structured claim identifiers.
5. Add the generic Health Engine skill and thin intake and assessment adapters in this repository. They file Sources, call only public CLI contracts, auto-apply routine high-assurance intake, and interrupt humans only under the accepted answerable-decision policy.
6. Use migration differences internally to fix defects or produce evidence-backed corrections; never ask the Subject to adjudicate engine output.

Acceptance:

- Every substantive rendered claim maps to a typed Analysis claim, its evidence graph, and the record revision that produced it.
- Skills retain the familiar natural-language intake and reporting experience without parallel structured state or direct SQLite writes.
- The original legacy repository remains unchanged, and no private default enters the public skill.

## Gate 6: Replay, harden, and hand off

1. From a fresh protected copy and blank Workspace, run the complete migration twice and prove idempotency.
2. Validate every file and candidate disposition, Source digest and alias, Extraction Run scope, Correction, Reconciliation, quarantine, Analysis dependency, Guidance prerequisite, freshness decision, and report revision.
3. Reconcile the semantic-claim ledger. Fix every implementation or ingestion defect; retain only differences backed by evidence that the legacy result was wrong, unsupported, deduplicated, or changed by newer reference data.
4. Repeat on a second clean protected copy using the exact same pinned Installed Data digests. Compare a canonical logical export of Facts, relationships, dependency edges, claim keys, and report claim manifests. A checked-in normalization schema may remove only enumerated volatile fields such as recording and generation times and machine-local paths; it may not erase clinical values, evidence, dispositions, or explanations.
5. Run formatting, strict linting, all tests, schemas, documentation, dependency and license checks, privacy scanning, crash recovery, bounded-memory checks, and clean-machine synthetic end-to-end verification.
6. Produce a sanitized handoff summarizing implementation, verification commands and results, plan deviations, and remaining explained differences. Do not push, publish, cut over, or alter the original repository without separate authorization.

Acceptance:

- There are zero unexplained semantic differences, unaccounted inputs, or unclassified candidates.
- Document-derived report facts are source-faithful, deterministic inputs are parser-validated, self-reports are explicitly attributed, and unresolved or quarantined inputs cannot drive Guidance.
- Two clean runs with pinned code and Installed Data produce the same canonical logical result, the original repository is unchanged, and private data appears only under ignored `tmp/`.

## Verification matrix

| Area | Required verification |
|---|---|
| Workspace and SQLite | Forward migration, transaction rollback, append-only history, guarded and staged proposals, Expungement under engine control, portability, concurrency, crash recovery, and deterministic replay tests |
| Sources and extraction | Content identity, aliases, drift, path safety, exact-location provenance, declared coverage, omission/failure, `not found` versus `not looked for`, and facts-by-source tests |
| Agent contract | Versioned JSON Schema, invalid payload, explicit identity, idempotency, revision conflict, receipt, stable error-code, stdout/stderr, and redaction tests |
| Labs and vitals | Name/unit normalization, original preservation, Clinical Time, range/flag, trend/reversal, context, and absence-state tests |
| Other migration domains | Timeline reconstruction, origin-appropriate Evidence Assurance, explicitly attributed self-report, conflicting assertions, safety-type distinction, task/study state, Subject Preference, and no-causation/no-invented-schedule tests |
| Analysis and trust | Typed dependency completeness, confidence propagation, Verification, quarantine, hazards, Guidance limits, `explain`/`impact`/`diff`/`why-not`, and stale-state tests |
| Genomics | Streaming bounds, format/build rejection, CRLF, haploid/no-call, join-build safety, array limits, CPIC coverage, and known false-positive regression tests |
| Reference data | Legacy-baseline reproduction, offline status, read-only check, failed update, atomic replacement, current-version provenance, license notice, and missing-required-data tests |
| Renderers and skills | Every human claim maps to an Analysis claim ID; four stable report paths, record/Analysis metadata, familiar interactions, auto-apply receipts, and no parallel-state tests |
| Migration | Complete inventory and claim-ledger disposition, two clean idempotent replays with pinned reference digests, canonical logical equality under an enumerated normalization schema, zero unexplained semantic differences, and evidence for every retained difference |
| Privacy | Synthetic-only committed fixtures, original-tree attestation, no telemetry or hidden copies, restrictive permissions, ignored private artifacts, and PHI-free diagnostics, arguments, journals, and temporary files |
| Long runs | Visible stage progress, bounded memory, per-stage timeout, child cleanup, machine-readable ignored logs, and honest inconclusive/failure reporting |

## Completion criteria

The first complete release exists when every `genetic-health` input and candidate claim is accounted for under the migration contract; its required structured health and genomics facts can be submitted with origin-appropriate assurance, corrected, and queried through Health Engine; its substantive report capabilities can be reproduced from a current sourced Analysis; its skills use the engine instead of parallel structured stores; public reference data can be updated explicitly; stale conclusions are detected; two clean migrations agree; and every remaining semantic difference has an evidence-backed explanation—without personal data or third-party datasets entering tracked or distributed project contents.
