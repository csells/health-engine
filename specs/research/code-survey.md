# Health Engine Code Survey

## Scope

This survey covers the reusable genomics and health-workflow code in:

- `genetic-health` (the current working repository)
- `april-health-stuff`
- `health-tracker-plugin`
- `health-vault` (design documents only)
- the project skills in all three executable repositories

There is no adjacent `health-vault-plugin` directory. `health-tracker-plugin` is the adjacent reusable plugin implementation, while `health-vault` contains related architecture and product decisions; both were inspected.

No personal genome, clinical document, living report, symptom log, or other per-user material belongs in the new repository.

## Executive findings

The repositories contain one evolving Python genomics engine copied into three shapes, plus agent workflows that operate on private health workspaces and maintain structured health state by editing CSV and Markdown files. The best new boundary is not a literal extraction of any copy:

- Build a Rust library and thin CLI that manage the Workspace's canonical SQLite Health Record and deterministic analysis.
- Let agents file raw documents, interpret them, and submit sourced structured facts through Health Engine rather than maintaining parallel CSV/Markdown databases.
- Keep interviewing, document interpretation, and narrative synthesis in skills and applications.
- Download third-party reference data directly from its public source into a local cache; do not redistribute it in the MIT repository or binary.
- Keep the small hand-curated variant knowledge only after auditing and sourcing every entry.
- Treat the current Python reports as research evidence, not compatibility contracts.

The most important discovery is that the copies have diverged in both directions. `genetic-health` has the broadest current source coverage and confidence checks. `health-tracker-plugin` has a cleaner reusable-workspace seam and multiple correctness fixes missing from the main copy. No one copy is safe to port wholesale.

## Repository findings

### `genetic-health`

This is the most capable genomics implementation and the least reusable repository boundary. Its principal entry point, `scripts/run_full_analysis.py`, combines parsing, joins, policy, clinical wording, recommendation generation, serialization, and filesystem output in one large script. The repository is also a private medical archive, so its source documents and living reports cannot seed an OSS repository.

Reusable capabilities:

- 23andMe GRCh37 parsing indexed by rsID and genomic position.
- Haploid-aware genotype classification for X, Y, and mitochondrial calls.
- ClinVar GRCh37 processing and review-confidence gating.
- ClinGen gene-validity and inheritance enrichment.
- ACMG secondary-findings support.
- CPIC allele-definition and phenotype processing.
- ClinPGx/PharmGKB clinical annotations and supporting evidence.
- GWAS Catalog enrichment joined by rsID.
- Transparent streaming of gzip TSV/CSV data.
- Reference-data refresh/build scripts.

Important limitations:

- The main pipeline is approximately 1,800 lines and owns too many responsibilities.
- The complete clinical result exists primarily in Markdown. `comprehensive_results.json` omits disease findings and CPIC calls, so it is not a reliable API.
- The curated variant database lacks uniform citations, effect-allele metadata, build metadata, and review dates.
- Allele-order normalization is described as “strand flipping,” but reversing `AG` to `GA` is not DNA-strand complementation.
- The CLI/parser suggests broader consumer-file compatibility but only genuinely implements a 23andMe-like fourth-column genotype.
- ClinGen data is currently folded into the generated ClinVar table, which can make an older ClinGen interpretation persist until ClinVar is rebuilt.
- CPIC calling is useful but intentionally incomplete for genes dominated by structural variation or absent array coverage.
- Recommendation generation has weaker gating than finding generation in places, and actionable handling does not consistently preserve haploid semantics.
- The documented test count is stale; the current suite contains 26 passing tests, concentrated on zygosity, confidence constants, CPIC behavior, and a few regressions rather than end-to-end clinical claims.

### `health-tracker-plugin`

This repository demonstrates the strongest product boundary for agent use. Its five generalized skills cover setup, record filing, symptom logging, health reporting, and genome analysis. Only genome analysis needs the deterministic engine; the other skills are adapters over a private workspace.

Its bundled Python engine is older and has fewer reference sources than `genetic-health`, but it contains important fixes not merged into the main curated database. Thirteen shared variants differ, including orientation, gene, allele, direction, or claim-strength corrections. It also removes two plainly mislabeled entries:

- `rs57875989`, represented as a PER2 C/G SNP although the identifier refers to a PER3 VNTR.
- `rs7181866`, represented as a COL5A1 C/T marker although the identifier and alleles do not match that claim.

The corrected shared records include `rs1042522`, `rs12649507`, `rs1799752`, `rs2228479`, `rs2282679`, `rs2542052`, `rs25531`, `rs28532698`, `rs4253778`, `rs4880`, `rs4988235`, `rs4994`, and `rs5082`. These are audit inputs, not automatically accepted truth.

The plugin also proves that a generic agent adapter can discover a user Workspace, file Sources, and invoke a shared engine. Its current hand-maintained CSV and Markdown state identifies the missing layer: Health Engine should provide the validated write/query store so every skill does not reinvent it.

### `april-health-stuff`

The genomics directory is an older fork of the same Python pipeline and curated database. It adds no engine architecture that should control the Rust design. Its value is in workflow behavior: a separate subject/workspace, symptom-event logging, record synchronization, and longitudinal narrative reports demonstrate that the engine must not assume that the operator, repository owner, and health subject are the same person.

Those workflows and all personal material remain outside the OSS code repository but are first-class data in a user-owned Health Engine Workspace. Symptom episodes, Clinical Encounters, medications, Care Tasks, and other extracted facts belong in SQLite; model-authored narratives and raw-file organization remain skill responsibilities.

### `health-vault`

This repository contains architecture and product documents rather than executable engine code. Its useful constraints are privacy separation, provenance, conservative handling of clinical evidence, and the desirability of public data sources. Its earlier decision to replace PharmGKB with FDA data was driven by distribution/commercial constraints; direct user downloads now let the Health Engine retain ClinPGx functionality without redistributing its files.

## Skill boundary

The surveyed skills divide cleanly:

Engine responsibilities:

- Initializing and migrating the Workspace's SQLite Health Record.
- Validating, writing, correcting, verifying, and querying sourced Health Facts.
- Tracking provenance, discrepancies, Record Hazards, and stale Analyses.
- Modeling labs, vitals, medications, medication safety, conditions, symptoms, encounters, studies, Care Tasks, and Care Protocols.
- Strict genome-format and build validation.
- Genotype normalization and haploid/diploid/no-call handling.
- Reference-data acquisition, validation, and update checks.
- Evidence joins and source-specific interpretation.
- Cross-domain deterministic analysis over the Subject's record.
- Confidence categorization and source-backed guidance.
- Complete structured Analysis output and deterministic rendering.

Skill or application responsibilities:

- Reading arbitrary PDFs, screenshots, images, and portal exports.
- Filing original health records into a private workspace.
- Asking follow-up questions and resolving ambiguous documents.
- Extracting Health Facts and source locations, then writing them through Health Engine.
- Re-checking Sources against SQLite and recording Verification or Discrepancy results through Health Engine.
- Maintaining narrative reports and combining structured Analysis with the user's goals.
- Hosting or encrypting the whole Workspace when needed.

A reusable skill should become a thin adapter over the same record: file Sources, submit extracted facts, query missing/unverified data, invoke Analysis, and use its JSON for narrative work. The genome skill additionally runs reference-data status/check/update and deterministic genome import.

## Non-genomic findings from the skills

The non-genomic skills repeatedly implement the same valuable invariants by hand:

- Source documents are append-only and every structured fact must point back to one.
- Collection/service date matters more than download or report date.
- A lab's own value, units, range, and flag must be preserved even when names and units are normalized for trending.
- “Normal,” “not measured recently,” “never measured,” “not found,” and “not looked for” are different states.
- Medication starts, stops, and dose changes must be aligned with later labs, vitals, and symptoms without assuming causation.
- Patient-reported descriptions must remain verbatim; `absent`, `unknown`, and `not asked` are different observations.
- Orders, referrals, expected results, and repeat intervals must become durable Care Tasks rather than prose reminders.
- Known chart errors must follow the facts they threaten so they cannot be copied into another report.
- New facts must trigger re-evaluation of trends and conclusions; stale prose is the dominant failure mode.

Today those invariants are spread across `lab-timeline.csv`, living Markdown documents, symptom logs, skills, and generated reports. A Workspace SQLite record gives agents one transactional, queryable, auditable interface while leaving raw-file judgment and narrative synthesis where agents are strongest.

## Reference-data inventory

The current working pipeline uses approximately 68 MiB of compressed public reference data. The largest file is the roughly 54 MiB ClinVar table, so GitHub size was technically feasible, but licensing and clean project boundaries favor direct download.

The initial Rust updater must cover:

| Source | Current role | Required join/handling |
|---|---|---|
| ClinVar GRCh37 VCF | Variant significance, condition, review status, allele frequency | GRCh37 position and allele; SNVs only for array input |
| ClinGen | Gene-disease validity and inheritance | Gene/disease semantics; remain independent of ClinVar storage |
| ACMG Secondary Findings | Actionable incidental findings | Gene and inheritance/guideline context |
| CPIC | Star-allele definitions and diplotype phenotype | rsID only; no strand reversal; explicit coverage limits |
| ClinPGx clinical annotations | Drug-gene claims and evidence | Annotation/allele/evidence joins; preserve source terms locally |
| GWAS Catalog | Enrichment of curated findings | rsID only; never join GRCh38 positions to GRCh37 input |
| Curated Variant Knowledge | Project-authored interpretations | Bundled only after per-entry audit and citation |

The existing updater already builds ClinVar, ClinGen, ACMG, CPIC, and GWAS resources. ClinPGx acquisition is not yet integrated and must become a first-class source adapter.

## Correctness requirements carried forward

- Reject unknown file formats and genome builds loudly.
- Preserve CRLF handling, no-calls, haploid calls, and allele-order normalization.
- Never describe allele-order normalization as strand complementation.
- Join each source only on keys valid for its genome build.
- Do not claim indels or structural variants from array SNP input.
- Keep CPIC plus-strand behavior and IUPAC wildcard semantics.
- Treat CPIC default `*1` as absence of observed defining variants, not a directly observed allele.
- Report incomplete array coverage and uncallable genes explicitly.
- Put a confidence category, rationale, and native source rating on every emitted claim.
- Preserve uncertain, conflicting, and excluded evidence with its reason.
- Generate terminal and Markdown output only from the complete structured Analysis.
- Keep all personal data out of the OSS repository, public-data cache, telemetry, and tests; personal data belongs only in an explicitly selected Workspace.

## Conclusion

The Rust project should be a greenfield synthesis: the main repository supplies the broad genomic source pipeline and longitudinal lab lessons, the plugin supplies the best reusable adapter seam, the April repository proves one-Subject Workspaces with distinct caregiver Operators and rich symptom modeling, and the vault design reinforces privacy and evidence provenance. The new engine should reuse facts and tests after verification, not reuse the Python architecture or promise output compatibility.
