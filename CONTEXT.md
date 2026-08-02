# Health Engine

A public, MIT-licensed, user-agnostic domain for turning supplied health evidence into deterministic, provenance-rich findings. It exists so many private workspaces, skills, and applications can share one interpretation engine without sharing personal data. Third-party reference data is acquired directly from its public sources into an untracked local cache and is not redistributed in the repository, crate, or binary.

## Language

**Health Engine**:
The deterministic processor and workspace API that stores validated Health Facts in a user-owned Workspace, applies reference data and evidence rules, and produces structured Analyses. It does not perform model-based document interpretation or narrative synthesis, and it never places personal data in the OSS repository or public-data cache.
_Avoid_: Analyzer, pipeline, health assistant

**Health Fact**:
A typed, structured fact written and queried through the Health Engine, such as a genomic call, lab result, vital measurement, medication event, symptom answer, or care follow-up. It preserves when it happened, who or what reported it, and where it came from. Agents may extract Health Facts from messy documents or conversations, but the engine validates and persists them.
_Avoid_: Document, model interpretation, unsourced assertion

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

**Confidence**:
One of `High`, `Moderate`, `Low`, `Conflicting`, or `Unknown`, with a plain-language rationale, attached to every Finding and Guidance claim alongside the source's original evidence rating. Confidence is part of the structured Analysis and appears in every CLI rendering.
_Avoid_: Hidden threshold, report-section proxy, impact score

**Genome Input**:
A genomic evidence stream whose file format and genome build have been positively identified and validated before analysis. Version one supports 23andMe raw data; an unknown format or build is an actionable error, never a guessed interpretation.
_Avoid_: Generic four-column file, best-effort parsing

**Renderer**:
A presentation adapter that turns an Analysis into terminal text, Markdown, or another human-readable form without adding, removing, or reinterpreting clinical meaning.
_Avoid_: Analyzer, report generator

**Reference Source**:
One independently updated public evidence source, such as ClinVar, ClinGen, CPIC, ClinPGx, or the GWAS Catalog. A source may publish several files that must be interpreted together.
_Avoid_: Bundled database

**Installed Data**:
The current local copy of each Reference Source, including its version and provenance. Analysis always uses the installed data. An explicit update replaces an older copy with a newer one.
_Avoid_: Snapshot, release, active pack

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
An original user-supplied health artifact—such as a lab PDF, visit summary, image, or genome export—filed and preserved in the Workspace by an agent or application. Health Engine stores the source reference and location supplied with each extracted Health Fact but does not manage the file.
_Avoid_: Health Fact, extraction, generated report

**Health Record**:
The canonical structured store inside a Workspace. Health Facts, corrections, source provenance, and care state are created, validated, written, and queried only through Health Engine interfaces.
_Avoid_: Agent-authored CSV, living Markdown, public database

**Verification**:
A status and auditable comparison between a Health Fact and its immutable Source. A new extraction begins `Unverified`; a check records who or what reviewed it, when, the source location checked, and whether it became `Verified` or a Discrepancy was found.
_Avoid_: Assumption, silent review, source-free approval

**Discrepancy**:
A recorded disagreement between structured Health Facts, an immutable Source, or two Sources. It remains visible until explicitly resolved and never silently rewrites either source or history.
_Avoid_: Overwrite, hidden correction, stale note

**Correction**:
An append-only Health Fact that replaces an earlier fact as the current interpretation while preserving the original, the reason for change, supporting evidence, author, and time.
_Avoid_: UPDATE, edit in place, deletion

**Care Task**:
A structured follow-up recorded in the Health Record, such as a repeat test, referral, appointment, or awaited result. It carries what is due, when or at what interval, who requested it, its source, and its current state.
_Avoid_: Inferred reminder, prose open loop, undocumented schedule

**Care Protocol**:
A sourced set of questions, observations, and time-sensitive actions to use for a Subject's condition or recurring event. Its content comes from a clinician, cited guideline, explicit Subject preference, or accepted and labeled agent suggestion—not from Health Engine invention.
_Avoid_: Hardcoded medical checklist, anonymous advice, inferred order

**Adapter**:
A skill, CLI surface, or application integration that files Sources, writes extracted Health Facts through Health Engine, queries the Health Record, and consumes Analysis.
_Avoid_: Engine, pipeline
