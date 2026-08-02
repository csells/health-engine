# Health Engine Vision

## The problem

Health information arrives as disconnected genetics, labs, vital measurements, medication changes, symptom episodes, imaging conclusions, orders, referrals, and personal observations. Agents can read and organize those inputs, but each skill currently reimplements the rules for deciding what changed, what is stale, what conflicts, and what deserves attention.

## The idea

Health Engine is the shared structured-record and deterministic-analysis layer underneath those agents and applications. Each user has a portable private Workspace containing agent-filed source documents, a Health Engine-managed SQLite record, and derived Analyses. Agents identify, name, route, and preserve messy documents, turn them into typed Health Facts, then use the engine to validate and write those facts with source references. Every extracted fact points back to its source so an agent can systematically re-check PDFs against SQLite and record matches or discrepancies. The engine queries those facts, analyzes them together with current public reference data, and returns one structured, provenance-rich Analysis for humans, agents, and applications.

Corrections never erase history. A fixed value becomes current, but the original extraction, reason for correction, supporting evidence, and verification trail remain available for audit.

Agent-extracted facts are useful immediately but begin as unverified. Their status remains visible and reduces the confidence of dependent conclusions until an agent checks them against the original Source.

Analyses are derived snapshots, not permanent truth. A fact correction, new observation, updated public dataset, or changed rule marks earlier Analyses stale; they remain available for audit but are recomputed before being presented as current.

Each Workspace belongs to one health subject. The person or agent operating it may be the subject or a caregiver, but records from different people never share a Workspace.

Genomics is one domain, not the product boundary. The engine is intended to grow across structured labs, vitals, medications, symptoms, and care follow-ups while keeping the same guarantees: no hidden personal storage, no unsourced claims, explicit confidence, and every conclusion traceable to its inputs and rules.

For labs, that means preserving the reporting lab's value, units, range, flag, collection time, and source while detecting longitudinal trends, reversals, abnormal results, and measurements that have gone stale. Equivalent test names and exactly convertible units form one trend without overwriting their originals. A missing measurement is never presented as a reassuring normal result.

For follow-up care, agents turn orders, repeat intervals, referrals, appointments, and expected results into structured Care Tasks. The engine keeps their source and state and can say what is pending or overdue without inventing a schedule.

For vital measurements, the engine tracks weight, blood pressure, pulse, temperature, oxygen saturation, and future measurement types together with their setting and context, so a trend remains interpretable rather than becoming a list of context-free numbers.

For medications and supplements, starts, stops, dose changes, missed doses, and as-needed use form a timeline. The engine can show what regimen was in effect when a lab, vital, or symptom changed and combine that history with pharmacogenomic guidance without confusing temporal association with proof of cause.

Medication safety remains explicit: allergies, intolerances, adverse reactions, contraindications, and prescribing cautions retain their different meanings, evidence, severity, and verification instead of collapsing into one error-prone list.

Diagnoses and conditions form a sourced history of what was suspected, confirmed, ruled out, inactive, or resolved. The current picture is derived without allowing an old suspicion or copied-forward chart entry to masquerade as present truth.

Known chart errors become durable Record Hazards linked to the facts they threaten. Originals remain immutable, but every relevant query and Analysis carries the warning so a documented mistake cannot keep propagating.

Recurring symptoms become structured episodes without losing the person's exact words. Present, absent, unknown, and not-asked observations stay distinct, allowing the engine to find frequency, patterns, and deviations without turning a description into a diagnosis.

Reusable care protocols can guide what an agent asks and which time-sensitive actions it surfaces, but their content always has an explicit origin: clinician instructions, a cited guideline, the Subject's preference, or an accepted and labeled agent suggestion. The engine never invents a personalized medical requirement.

Clinical encounters preserve the context of visits, emergency care, admissions, and consultations by grouping the provider, reason, observations, diagnoses, medication changes, orders, and follow-up tasks produced together.

Imaging, pathology, procedures, and other diagnostic studies preserve the study method, body site, findings, clinician impression, result status, source, and encounter context without collapsing findings into an agent-authored conclusion.

## Health Record coverage

The structured record is intended to cover the normal breadth of personal health data without requiring a separate product decision for every ordinary record type. In addition to genomics, labs, vitals, medications, medication safety, conditions, symptoms, Care Tasks, Care Protocols, Clinical Encounters, and Diagnostic Studies, the model should accommodate procedures, immunizations, family history, reproductive history, social and environmental exposures, implanted devices, functional status, and other sourced clinical observations as typed Health Facts. New fact types follow the same provenance, correction, verification, and confidence rules.

## The boundary

The engine does not interpret arbitrary documents, interview people, file Sources, or write model-generated health narratives. Skills and applications do those jobs. The Rust crate owns the portable SQLite record, supplied source references, validation and query interfaces, deterministic health logic, reference-data updates, cross-domain analysis, confidence, provenance, and structured guidance. The user chooses where the Workspace lives and owns everything in it.

Whole-Workspace encryption belongs to the application, vault, or encrypted filesystem containing it. The engine does not make a partial encryption promise while agents need direct access to source documents; it instead avoids hidden copies, telemetry, and stray personal-data files.

## Success

The same engine can support a personal health repository, a caregiver-managed workspace, a health-tracker skill, or another application without copying analysis code or moving anyone's private data into the OSS project.
