# Health Engine

A public, MIT-licensed, user-agnostic domain for turning supplied health evidence into deterministic, provenance-rich findings. It exists so many private workspaces, skills, and applications can share one interpretation engine without sharing personal data. Third-party reference data is acquired directly from its public sources into a configured untracked local cache and is not redistributed in the tracked repository, crate, or binary.

## Language

**Health Engine**:
The deterministic processor and workspace API that stores validated Health Facts in a user-owned Workspace, applies reference data and evidence rules, and produces structured Analyses. It does not perform model-based document interpretation or narrative synthesis, and it never places personal data in tracked or distributed project artifacts or the public-data cache.
_Avoid_: Analyzer, pipeline, health assistant

**Health Fact**:
A typed, structured fact written and queried through the Health Engine, such as a genomic call, lab result, vital measurement, medication event, symptom answer, or care follow-up. It preserves when it happened, who or what reported it, and where it came from. Agents may extract Health Facts from messy documents or conversations, but the engine validates and persists them.
_Avoid_: Document, model interpretation, unsourced assertion

**Evidence Assurance**:
The origin-appropriate validation basis carried by a Health Fact: parser-validated machine-readable evidence, source-verified document extraction, explicitly attributed self-report, or unverified extraction. These are distinct bases rather than a false universal ranking; policy decides how each may support a Finding or Guidance, and quarantine is not an assurance level.
_Avoid_: Confidence, clinical truth, generic verified flag, user approval

**Clinical Time**:
The source-faithful representation of when a Health Fact was effective or observed, including an exact instant, date, interval, or partial date with explicit precision. It never invents a missing timezone or calendar component and remains distinct from when Health Engine recorded the Fact.
_Avoid_: Recording time, guessed timestamp, report date

**Lab Result**:
A Health Fact for one measured or qualitative test result, carrying its collection time, original test name, value, units, the reporting lab's reference range and flag, and a source reference. The engine uses Lab Results for longitudinal trends without replacing one lab's range with another's or treating an unmeasured test as normal.
_Avoid_: Panel summary, report-date value, universal reference range

**Vital Measurement**:
A dated measurement such as weight, blood pressure, pulse, temperature, or oxygen saturation, including units, setting, observer or device when known, and relevant context such as fasting state or medication dose.
_Avoid_: Context-free number, undated current value

**Symptom Episode**:
A structured event capturing onset, phases, duration, recovery, the Subject's exact quoted description, associated symptom observations, possible triggers, treatments and response, witnesses, and source. An observation distinguishes `present`, `absent`, `unknown`, and `not asked`.
_Avoid_: Paraphrased diagnosis, blank-as-negative, undated symptom list

**Medication Event**:
A dated fact that starts, stops, changes, misses, or records as-needed use of a medication or supplement, including dose, route, schedule, indication, and source when known. The current regimen is derived from events rather than overwritten in place.
_Avoid_: Mutable medication list, inferred adherence, timeless prescription

**Medication Safety Fact**:
A sourced allergy, intolerance, adverse reaction, contraindication, or prescribing caution. Its kind, substance, observed reaction, severity, certainty, and verification remain distinct rather than being flattened into a generic allergy list.
_Avoid_: Medication Event, unlabeled bad reaction, inferred allergy

**Condition Assertion**:
A dated, sourced statement that a diagnosis or health condition is suspected, confirmed, ruled out, inactive, or resolved. The current condition picture is derived from assertions and corrections rather than maintained as an overwritten list.
_Avoid_: Timeless diagnosis, problem-list copy, untested negative

**Clinical Encounter**:
A dated visit, emergency visit, admission, consultation, or similar care event that records its provider, facility, reason, and Sources and groups the Health Facts, decisions, orders, and Care Tasks produced there.
_Avoid_: Source document, narrative summary, undated provider note

**Diagnostic Study**:
A dated imaging, pathology, procedure, or other diagnostic record preserving the body site, method, findings, clinician impression, result status, Source, and related Clinical Encounter. Findings and impression remain distinct.
_Avoid_: Agent summary, context-free result, collapsed impression

**Record Hazard**:
A durable warning that a Source or chart statement is known or credibly suspected to be wrong, misleading, or dangerously incomplete. It cites the problematic statement, the correction and evidence, and follows related facts into queries and Analyses.
_Avoid_: Deleted source, hidden note, ordinary discrepancy

**Canonical Test**:
The stable identity used to recognize the same laboratory test across source-specific names and safely convertible units. Every normalized value points back to the untouched original Lab Result; uncertain mappings remain separate.
_Avoid_: Display name, guessed synonym, overwritten source value

**Analysis**:
A versioned, structured set of findings, guidance, exclusions, warnings, and provenance produced by the Health Engine from a specific Health Record state, installed reference-data versions, and engine rules. Saved Analyses remain historical artifacts and are marked stale when any dependency changes; human-readable views are derived from the structured value.
_Avoid_: Report, output, results

**Guidance**:
A clinically actionable step supported directly by a Reference Source, such as CPIC medication guidance or ACMG follow-up. It carries its source, strength, prerequisites, and uncertainty and never extends beyond what that source supports.
_Avoid_: Agent advice, inferred treatment, generic wellness tip

**Subject Preference**:
An explicit choice by the Subject about tracking, presentation, or a personal routine. It may shape a Care Protocol or Renderer but is not clinician instruction or source-backed Guidance.
_Avoid_: Guidance, clinician order, medical requirement

**Confidence**:
One of `High`, `Moderate`, `Low`, `Conflicting`, or `Unknown`, with a plain-language rationale, attached to every Finding and Guidance claim alongside the source's original evidence rating. Confidence is part of the structured Analysis and appears in every CLI rendering.
_Avoid_: Hidden threshold, report-section proxy, impact score

**Genome Input**:
A genomic evidence stream whose file format and genome build have been positively identified and validated before analysis. Version one supports 23andMe raw data; an unknown format or build is an actionable error, never a guessed interpretation.
_Avoid_: Generic four-column file, best-effort parsing

**Renderer**:
A deterministic presentation adapter that turns a Record Snapshot and its Analysis into terminal text, JSON, Markdown, HTML, or a Report Bundle without adding, removing, or reinterpreting clinical meaning.
_Avoid_: Analyzer, report generator

**Record Snapshot**:
An immutable logical view of the Health Record at one revision, used with a matching Analysis to make rendering and replay deterministic.
_Avoid_: Database backup, mutable current view, report

**Report Bundle**:
The atomically published set of required derived report files produced by one Renderer invocation from a Record Snapshot, matching Analysis, and explicit render context. Every substantive statement maps to a structured claim identity; agent-authored narrative remains separate and non-authoritative.
_Avoid_: Living database, agent rewrite, independent report state

**Reference Source**:
One independently updated public evidence source, such as ClinVar, ClinGen, CPIC, ClinPGx, or the GWAS Catalog. A source may publish several files that must be interpreted together.
_Avoid_: Bundled database

**Installed Data**:
The current local copy of each Reference Source at an explicit cache root, including its version and provenance. Analysis always uses the installed data selected for the operation. An explicit update replaces an older copy with a newer one.
_Avoid_: Snapshot, release, active pack

**Legacy Reference Baseline**:
The recorded digests and available version or provenance metadata for public reference resources that produced a legacy Analysis. It exists to explain and compare historical claims but is not the Installed Data used for a current Analysis.
_Avoid_: Installed Data, current data, bundled data

**Curated Variant Knowledge**:
The small, project-maintained set of genetic variants and interpretations shipped with the engine. Every entry is verified, corrected, and supported by cited sources before it can produce a finding or guidance.
_Avoid_: Python dictionary, unsourced SNP claim

**Source Manifest**:
The checked-in, non-data recipe that identifies a Reference Source's upstream resources, integrity checks, applicable terms, and deterministic transformations needed to build its Installed Data.
_Avoid_: Bundled data, vendored database

**Reference Data Unavailable**:
A structured, actionable failure returned when an operation requires reference data that is absent, incomplete, or corrupt. It identifies what is unavailable and the command that installs or repairs it; it never causes an implicit download or a silently partial Analysis.
_Avoid_: File not found, empty results, automatic download

**Update Check**:
A read-only comparison between Installed Data and currently available upstream versions. It reports whether newer data is available and how to update it, but never downloads or changes anything.
_Avoid_: Auto-update, background refresh

**Workspace**:
The user-owned location for exactly one Subject, managed through Health Engine APIs and CLI commands. It contains immutable source material, the structured Health Record, and derived Analyses, while remaining separate from the OSS code repository and public reference-data cache. Its Operator may be the Subject or a caregiver acting for them.
_Avoid_: Code repository, global engine state, public cache

**Subject**:
The one person whose health data a Workspace contains.
_Avoid_: User, account holder, operator

**Operator**:
The person, agent, or application acting on a Workspace. An Operator may be the Subject or may act for them as a caregiver.
_Avoid_: Subject, patient identity

**Source**:
An original user-supplied health artifact—such as a lab PDF, visit summary, image, or genome export—filed and preserved in the Workspace by an agent or application. Health Engine may read its bytes only to validate content identity, size, media type, and aliases; it stores a private digest behind an opaque Source ID but never copies, moves, deletes, or interprets the file.
_Avoid_: Health Fact, extraction, generated report

**Source Alias**:
A Workspace-relative path that resolves to the same content-addressed Source as another path. It preserves where identical bytes were encountered without creating a second Source identity.
_Avoid_: Source, duplicate fact, copied evidence

**Migration Candidate**:
A possible Health Fact that migration cannot accept because it appears only in derived legacy material or has incomplete or failed source-fidelity evidence. It remains outside the canonical Health Record with an explicit technical disposition until it gains valid Source evidence or is confirmed as a self-report.
_Avoid_: Health Fact, Source, accepted fact

**Migration Difference**:
A substantive difference between a legacy result and the Health Engine migration result. It is verification evidence that must lead to a fixed implementation or ingestion defect, or to an evidence-backed explanation that the legacy result was wrong or superseded; it is not a decision delegated to the Subject.
_Avoid_: User review item, unexplained parity failure, cosmetic diff

**Health Record**:
The canonical structured store inside a Workspace. Health Facts, corrections, source provenance, and care state are created, validated, written, and queried only through Health Engine interfaces.
_Avoid_: Agent-authored CSV, living Markdown, public database

**Record Change Set**:
An atomic collection of proposed additions, Corrections, Verifications, Reconciliations, and provenance for one explicit Workspace and Subject. It carries its schema version, evidence origin and references, extractor identity, idempotency key, and expected record revision.
_Avoid_: Ad hoc write, partial import, SQL transaction

**Extraction Run**:
A versioned attempt by an identified extractor to examine a declared scope of one Source and propose Health Facts. It records the regions and domains examined, omissions, failures, and coverage needed to distinguish `not found` from `not looked for`; a Source may have multiple Extraction Runs.
_Avoid_: Source, Health Fact, Analysis, whole-document assumption

**Record Proposal**:
The immutable result of validating a Record Change Set without changing canonical state. It identifies the exact proposed effects, safety findings, impact, and content hash that may later be committed.
_Avoid_: Health Fact, accepted change, preview prose

**Commit Receipt**:
The durable result of applying a Record Proposal, including the new record revision, accepted identities, deduplication outcomes, and newly stale Analyses. Repeating the same idempotent request returns the same receipt.
_Avoid_: Report, log message, Analysis

**Verification**:
A status and auditable source-fidelity comparison between a Health Fact and its immutable Source. It establishes that the fact was transcribed faithfully, not that the Source's medical claim is clinically true; a check records who or what reviewed it, when, the exact location checked, and whether it became `Verified` or a Discrepancy was found.
_Avoid_: Clinical validation, assumption, silent review, source-free approval

**Discrepancy**:
A recorded disagreement between structured Health Facts, an immutable Source, or two Sources. It remains visible until explicitly resolved and never silently rewrites either source or history.
_Avoid_: Overwrite, hidden correction, stale note

**Reconciliation**:
An auditable relationship stating that Health Facts from different Sources describe the same observation and whether they agree. It preserves every Source-specific Fact while allowing normal views to present equivalent observations once.
_Avoid_: Correction, merge, deletion, fuzzy deduplication

**Correction**:
An append-only Health Fact that replaces an earlier fact as the current interpretation while preserving the original, the reason for change, supporting evidence, author, and time.
_Avoid_: UPDATE, edit in place, deletion

**Expungement**:
An explicitly Operator-authorized destruction of PHI-bearing record entries and derived artifacts under Health Engine control after a wrong-Subject import, accidental sensitive ingestion, or deliberate privacy request. It retains only a fixed-code non-PHI audit tombstone, reports external Source and backup remediation to the host, and is never an ordinary Correction or autonomous agent action.
_Avoid_: Correction, routine deletion, redaction, agent cleanup

**Care Task**:
A structured follow-up recorded in the Health Record, such as a repeat test, referral, appointment, or awaited result. It carries what is due, when or at what interval, who requested it, its source, and its current state.
_Avoid_: Inferred reminder, prose open loop, undocumented schedule

**Care Protocol**:
A sourced set of questions, observations, and time-sensitive actions to use for a Subject's condition or recurring event. Its content comes from a clinician, cited guideline, explicit Subject preference, or accepted and labeled agent suggestion—not from Health Engine invention.
_Avoid_: Hardcoded medical checklist, anonymous advice, inferred order

**Adapter**:
A skill, CLI surface, or application integration that files Sources, writes extracted Health Facts through Health Engine, queries the Health Record, and consumes Analysis.
_Avoid_: Engine, pipeline
