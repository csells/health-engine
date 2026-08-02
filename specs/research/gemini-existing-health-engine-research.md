# Architectural and Ecosystem Analysis for Health Engine

## 1. Executive Conclusion

The design and implementation of Health Engine necessitate a fundamental architectural departure from traditional health informatics systems. The overarching objective is to produce a portable, MIT-licensed Rust binary that couples deterministic clinical analysis with high-fidelity provenance tracing, specifically catering to autonomous agent ingestion and verifiable human audit. The critical invariants—that raw source files are immutable, all extracted facts maintain direct provenance to their source quotations, corrections are strictly append-only, and the entire workspace is a local-first SQLite database—create a unique intersection of requirements.

An exhaustive evaluation of the open-source health technology landscape reveals a bifurcated ecosystem. On one end of the spectrum reside massive, server-centric, enterprise-grade interoperability platforms, such as Medplum, HAPI FHIR, and EHRbase, which are engineered for institutional data exchange and population-scale analytics. On the opposite end are consumer-facing health dashboards, such as Fasten Health, which provide local-first storage but lack the robust deterministic analysis, genomic interpretation pipelines, and agent-first schemas required by the proposed architecture. Furthermore, domain-specific engines for clinical rules, such as the Clinical Quality Language (CQL) execution frameworks, and pharmacogenomics, notably PharmCAT, are tightly coupled to the Java Virtual Machine (JVM) or Node.js runtimes. This dependency profile fundamentally conflicts with the requirement for a lightweight, dependency-free Rust library and Command Line Interface (CLI).

Consequently, adopting an existing whole-project codebase as-is is not feasible. The optimal path forward is a modular "Adopt, Adapt, and Build" strategy. Health Engine must engineer its core SQLite workspace, the event-sourced provenance model, and the deterministic rules engine from scratch in Rust to mathematically guarantee the required invariants. However, it should aggressively adopt specific Rust-native data structure libraries for edge interoperability, standard public-domain datasets for genomic and clinical terminologies, and emerging integration standards like the Model Context Protocol (MCP) to facilitate agent interactions.

## 2. Best Whole-Project Candidates

The search for a whole-project candidate focused on systems offering local-first storage, portable databases, and Personal Health Record (PHR) workflows.

Fasten Health emerges as the closest architectural sibling in the current ecosystem. It is an open-source, self-hosted PHR designed to aggregate medical records into a local SQLite database, explicitly eschewing cloud dependencies for its core on-premise offering. The system utilizes the Fast Healthcare Interoperability Resources (FHIR) protocol and SMART-on-FHIR authentication to connect with institutional providers. However, Fasten Health is written in Go and operates primarily as a web application and OAuth2 proxy for scraping institutional patient portals. It does not possess a deterministic clinical analysis engine, lacks genomic integration, and does not expose a native agent-facing verification schema. Adopting it would necessitate a complete rewrite in Rust and the discarding of its web-centric frontend, making it unsuitable for direct adoption but highly valuable as a design reference.

The AI Healthcare System represents a modern, agent-integrated clinical decision support system utilizing LangGraph, local Ollama models, and a Python/Rust hybrid stack. It demonstrates advanced Retrieval-Augmented Generation (RAG) architecture and FHIR R4 context handling. Despite these advanced capabilities, its architecture is engineered for enterprise Kubernetes deployments utilizing FastAPI, PostgreSQL, and Redis, or heavy local Docker compositions. It is far too heavyweight and server-oriented to serve as the embedded core of a portable Health Engine CLI.

## 3. Best Component-Level Candidates

While no single project satisfies the holistic requirements, several components provide critical, adoptable capabilities:

First, for FHIR parsing and serialization at the system's boundary, the Rust `fhir` crate (and the alternative `helios-fhir` crate) provides idiomatic Rust structs generated directly from the HL7 specification. These crates are mature, support FHIR R4 and R5, and natively implement `serde` for JSON serialization, making them ideal for handling data import and export without polluting the internal SQLite schema.

Second, the Pharmacogenomics Clinical Annotation Tool (PharmCAT) serves as the gold standard for applying Clinical Pharmacogenetics Implementation Consortium (CPIC) guidelines. While the PharmCAT software relies on Java and Python, the underlying CPIC database is CC0/CC-BY licensed and available via API and TSV downloads. Health Engine can adopt the CPIC data model and build a native Rust interpreter to achieve the same clinical validity without the JVM overhead.

Third, Open-CRAVAT offers a highly modular, SQLite-backed architecture for genomic variant annotation. Crucially, it recently released a Model Context Protocol (MCP) server, making it an ideal external service or agent tool to wrap via standard interfaces rather than attempting to rewrite its extensive Python-based annotation logic in Rust.

## 4. Landscape Map by Category

The open-source health technology landscape is categorized into several distinct architectural domains, each offering different lessons and potential integrations:

**Open-Source EHR and Interoperability Servers:** Projects like Medplum provide a full-stack TypeScript FHIR platform with a clinical data repository, React components, and a sophisticated bot framework for automated workflows. HAPI FHIR is the reference implementation for FHIR in Java, highly enterprise-focused and far too heavy for a CLI. EHRbase represents the leading openEHR server in Java, which is useful for studying openEHR's robust versioning and provenance models but is impossible to embed locally. Traditional monolithic EHRs like OpenEMR and GNU Health are built on legacy stacks (PHP/Python) and assume a multi-tenant provider workflow, rendering them irrelevant for a local-first, patient-controlled engine.

**Local-First and PHR Applications:** Fasten Health leads the open-source, local-first PHR space with its Go-based, SQLite-backed aggregator. Proprietary systems like Apple HealthKit and Google Health Connect, while closed-source, are highly instructive for local, encrypted, SQLite-backed health data schema design that prioritizes rapid time-series retrieval over hierarchical document storage.

**Clinical Rules and Guidelines:** The CQL Execution Framework is the standard implementation of the Clinical Quality Language in TypeScript and Java. It executes JSON ELM (Expression Logical Model) representations of clinical logic. Embedding this requires a JavaScript engine (like V8) or JVM, violating the lightweight Rust mandate. Other frameworks like CDS Hooks provide API specifications for clinical decision support, which are excellent conceptual models for agent communication paradigms.

**Genomics and Pharmacogenomics:** Genomic interpretation requires processing highly specialized data formats. PharmCAT processes Variant Call Format (VCF) files to infer haplotypes and apply CPIC guidelines. Open-CRAVAT provides extensive variant annotation. The underlying data sources—ClinVar for pathogenic variants, ClinGen for gene-disease validity, and the GWAS Catalog—are freely available but often require complex ETL (Extract, Transform, Load) pipelines to utilize locally.

## 5. Comparison Matrix

The following matrix evaluates 15 serious candidates against the specific constraints of the Health Engine architecture.

| **Project Name**      | **Category**      | **Language** | **Storage Model** | **Local Fit** | **Provenance Fit** | **Clinical/Genomics Fit** | **License**        | **Recommendation**  |
| --------------------- | ----------------- | ------------ | ----------------- | ------------- | ------------------ | ------------------------- | ------------------ | ------------------- |
| **Fasten Health**     | PHR App           | Go           | SQLite            | High          | Low                | Low                       | Open (Unspecified) | Learn From          |
| **Medplum**           | FHIR Platform     | TypeScript   | Postgres/Redis    | Low           | Moderate           | Low                       | Apache 2.0         | Learn From          |
| **EHRbase**           | openEHR Server    | Java         | Postgres          | Low           | High               | Low                       | Apache 2.0         | Learn From          |
| **HAPI FHIR**         | FHIR Server       | Java         | RDBMS             | Low           | Moderate           | Low                       | Apache 2.0         | Reject              |
| **OpenEMR**           | Legacy EHR        | PHP          | MySQL             | Low           | Low                | Low                       | GPL                | Reject              |
| **AI Healthcare Sys** | AI CDS            | Rust/Python  | Postgres/SQLite   | Moderate      | Moderate           | Moderate                  | MIT                | Learn From          |
| **PharmCAT**          | PGx Engine        | Java/Python  | VCF/Filesystem    | Moderate      | Low                | High                      | Open               | Adopt Data, Rewrite |
| **Open-CRAVAT**       | Variant Annotator | Python       | SQLite            | High          | Moderate           | High                      | MIT                | Wrap via MCP        |
| **cql-execution**     | Rules Engine      | TS/JS        | In-memory JSON    | Moderate      | Low                | High                      | Apache 2.0         | Reject              |
| **fhir (Rust)**       | Library           | Rust         | N/A               | High          | N/A                | N/A                       | MIT/Apache         | Adopt               |
| **helios-fhir**       | Library           | Rust         | N/A               | High          | N/A                | N/A                       | MIT                | Learn From          |
| **OMOP CDM**          | Data Model        | SQL DDL      | RDBMS             | Low           | Low                | High                      | Open               | Reject for PHR      |
| **Open mHealth**      | Standard          | JSON Schema  | N/A               | High          | Low                | Low                       | Apache 2.0         | Learn From          |
| **ClinVar (NCBI)**    | Public Dataset    | XML/VCF/TSV  | N/A               | High          | High (Source)      | High                      | Public Domain      | Adopt               |
| **CPIC Database**     | Public Dataset    | API/TSV      | N/A               | High          | High (Source)      | High                      | CC0 / CC-BY        | Adopt               |

## 6. Deep Dives on Strongest Candidates

### 1. Fasten Health

- **Project name and direct links**: Fasten Health (https://github.com/fastenhealth/fasten-onprem).
- **Category**: Application (Personal Health Record).
- **What relevant component it supplies**: Reference architecture for a self-hosted, SQLite-backed PHR.
- **Language and runtime**: Go, Vue.js, Docker.
- **Storage model**: Local SQLite database.
- **Local/offline suitability**: Designed for self-hosted home networks without cloud integration.
- **License and any data-license restrictions**: Open-source, self-hosted model (specific core license unspecified in snippets, functionally treated as open).
- **Current maintenance status**: Active.
- **Latest meaningful release and release date**: Source code updates in late 2024 (e.g., Sep 30, 2024).
- **Recent commit and issue activity**: Active development resolving issues with data parsing, reference ranges, and UI overflows.
- **Maintainer/community health**: Single primary maintainer with growing community contributions.
- **Documentation and test quality**: Good documentation for Docker deployment and local certificate handling.
- **API stability**: Beta/evolving.
- **Personal versus provider/enterprise orientation**: Strictly personal/family orientation.
- **Provenance/versioning/correction support**: Low. It aggregates data rather than maintaining an append-only correction log of extracted unstructured facts.
- **Clinical-domain breadth**: Limited to basic FHIR profiles from portals.
- **Genomics/pharmacogenomics support**: None.
- **Agent/CLI suitability**: Low. Designed for human web interface consumption.
- **Integration difficulty**: High, due to Go/Web-app monolith design.
- **Major risks or deal-breakers**: The codebase is tightly coupled to OAuth2 SMART-on-FHIR proxying rather than the document ingestion and deterministic analysis required by Health Engine.
- **Recommendation**: **Learn From**.
- **Scores**: Architectural fit: 3/5, Data-model fit: 2/5, Local-first fit: 5/5, Provenance/audit fit: 1/5, Clinical-analysis fit: 1/5, Genomics fit: 0/5, Agent integration fit: 1/5, Maintenance maturity: 3/5, Licensing fit: 3/5, Estimated adoption value: 2/5.

### 2. Medplum

- **Project name and direct links**: Medplum (https://github.com/medplum/medplum).
- **Category**: Server and Application Platform.
- **What relevant component it supplies**: Full-stack FHIR implementation, clinical data repository, and automated Bot framework.
- **Language and runtime**: TypeScript, Node.js, React.
- **Storage model**: PostgreSQL for data, Redis for background jobs/caching.
- **Local/offline suitability**: Very low. Designed for scalable cloud deployment.
- **License and any data-license restrictions**: Apache 2.0.
- **Current maintenance status**: Highly active, commercial open-source.
- **Latest meaningful release and release date**: Continuous rolling releases.
- **Recent commit and issue activity**: Thousands of commits, daily updates across 50+ repositories.
- **Maintainer/community health**: Excellent, well-funded corporate backing with a strong open-source community.
- **Documentation and test quality**: Exceptional, extensive integration guides.
- **API stability**: High.
- **Personal versus provider/enterprise orientation**: Strictly provider/enterprise and digital health startup orientation.
- **Provenance/versioning/correction support**: Moderate. Uses standard FHIR versioning.
- **Clinical-domain breadth**: High (full FHIR R4 compliance).
- **Genomics/pharmacogenomics support**: Low (only standard FHIR genomics reporting, no interpretation engine).
- **Agent/CLI suitability**: High. The Medplum Bots framework is highly analogous to agent workflows, allowing headless execution.
- **Integration difficulty**: High (due to Node/Postgres stack).
- **Major risks or deal-breakers**: The heavy reliance on PostgreSQL and Redis violates the portable, single-binary SQLite CLI requirement.
- **Recommendation**: **Learn From**.
- **Scores**: Architectural fit: 1/5, Data-model fit: 3/5, Local-first fit: 1/5, Provenance/audit fit: 3/5, Clinical-analysis fit: 2/5, Genomics fit: 1/5, Agent integration fit: 4/5, Maintenance maturity: 5/5, Licensing fit: 5/5, Estimated adoption value: 1/5 (core).

### 3. Open-CRAVAT

- **Project name and direct links**: Open-CRAVAT (https://github.com/KarchinLab/open-cravat).
- **Category**: Rules Engine / Library (Genomic Variant Annotator).
- **What relevant component it supplies**: Variant impact, annotation, scoring, and MCP server capabilities.
- **Language and runtime**: Python.
- **Storage model**: Local SQLite databases for annotation data.
- **Local/offline suitability**: High. Runs entirely locally on downloaded reference databases.
- **License and any data-license restrictions**: MIT.
- **Current maintenance status**: Active.
- **Latest meaningful release and release date**: v3.1.1 (within the last 4 months).
- **Recent commit and issue activity**: Regular updates to annotators and the core engine.
- **Maintainer/community health**: Maintained by the Karchin Lab at Johns Hopkins University with NCI funding.
- **Documentation and test quality**: High, extensive wiki and automated testing.
- **API stability**: High.
- **Personal versus provider/enterprise orientation**: Research and clinical bioinformatics orientation.
- **Provenance/versioning/correction support**: Moderate (tracks annotation versions).
- **Clinical-domain breadth**: Focused entirely on genomics.
- **Genomics/pharmacogenomics support**: Exceptional breadth of variant annotation.
- **Agent/CLI suitability**: Exceptional. Recently released an official Model Context Protocol (MCP) server for direct LLM agent integration.
- **Integration difficulty**: Moderate. Cannot be compiled into the Rust binary, must be run as an external process.
- **Major risks or deal-breakers**: Python dependency prevents shipping a single standalone Rust binary if bundled directly.
- **Recommendation**: **Adopt + Extend (via wrapping)**. Treat it as a separate, optional agent tool accessed via MCP rather than embedding its logic.
- **Scores**: Architectural fit: 3/5, Data-model fit: 4/5, Local-first fit: 4/5, Provenance/audit fit: 3/5, Clinical-analysis fit: N/A, Genomics fit: 5/5, Agent integration fit: 5/5, Maintenance maturity: 4/5, Licensing fit: 5/5, Estimated adoption value: 4/5.

### 4. PharmCAT

- **Project name and direct links**: PharmCAT (https://github.com/pharmgkb/pharmcat).
- **Category**: Rules Engine / Application.
- **What relevant component it supplies**: Pharmacogenomic variant interpretation applying CPIC guidelines.
- **Language and runtime**: Java (17+) and Python (3.9+).
- **Storage model**: Consumes VCF files, utilizes local filesystem for guideline JSONs.
- **Local/offline suitability**: High.
- **License and any data-license restrictions**: Open software; CPIC data is CC0/CC-BY.
- **Current maintenance status**: Highly active.
- **Latest meaningful release and release date**: v3.4.0 (approximately 2 weeks ago).
- **Recent commit and issue activity**: Over 2,500 commits, continuous updates to match PharmVar and ClinPGx databases.
- **Maintainer/community health**: Maintained by Stanford University & University of Pennsylvania.
- **Documentation and test quality**: High, complete with tutorial repositories.
- **API stability**: Moderate (frequent biological updates).
- **Personal versus provider/enterprise orientation**: Research and clinical laboratory orientation.
- **Provenance/versioning/correction support**: Low (operates as a pipeline tool, not a longitudinal record).
- **Clinical-domain breadth**: Restricted to pharmacogenomics.
- **Genomics/pharmacogenomics support**: Exceptional. The industry standard.
- **Agent/CLI suitability**: Moderate (CLI exists, but lacks modern agent schemas).
- **Integration difficulty**: Very High. Requires a complex bioinformatics stack (`bcftools`, `htslib`, Java, Python).
- **Major risks or deal-breakers**: Integrating a JVM and Python toolchain into a Rust CLI violates the architectural invariants.
- **Recommendation**: **Learn From (Software) / Adopt (Data)**.
- **Scores**: Architectural fit: 1/5, Data-model fit: 4/5, Local-first fit: 3/5, Provenance/audit fit: 1/5, Clinical-analysis fit: N/A, Genomics fit: 5/5, Agent integration fit: 2/5, Maintenance maturity: 5/5, Licensing fit: 5/5, Estimated adoption value: 2/5 (Software), 5/5 (Data).

### 5. fhir (Rust Crate)

- **Project name and direct links**: `fhir` (https://crates.io/crates/fhir).
- **Category**: Library.
- **What relevant component it supplies**: Idiomatic Rust implementation of the HL7 FHIR Release 5 data model.
- **Language and runtime**: Rust.
- **Storage model**: N/A (Data structures only).
- **Local/offline suitability**: Perfect (compiled in).
- **License and any data-license restrictions**: MIT / Apache 2.0.
- **Current maintenance status**: Active (Stable 1.0).
- **Latest meaningful release and release date**: 1.0 (Stable).
- **Recent commit and issue activity**: Regular updates keeping pace with FHIR specs.
- **Maintainer/community health**: Strong open-source utility crate.
- **Documentation and test quality**: High. Includes an mdBook guide and robust `serde` round-trip testing.
- **API stability**: High (follows semantic versioning).
- **Personal versus provider/enterprise orientation**: Neutral framework.
- **Provenance/versioning/correction support**: N/A.
- **Clinical-domain breadth**: Complete FHIR R5 breadth.
- **Genomics/pharmacogenomics support**: Supports FHIR Genomics reporting structures.
- **Agent/CLI suitability**: High (fast serialization to JSON for agent ingestion).
- **Integration difficulty**: Very Low.
- **Major risks or deal-breakers**: High compile times due to the massive size of the FHIR specification generic macros.
- **Recommendation**: **Adopt**.
- **Scores**: Architectural fit: 5/5, Data-model fit: 4/5, Local-first fit: 5/5, Provenance/audit fit: N/A, Clinical-analysis fit: N/A, Genomics fit: N/A, Agent integration fit: 4/5, Maintenance maturity: 4/5, Licensing fit: 5/5, Estimated adoption value: 5/5.

### 6. cql-execution

- **Project name and direct links**: cql-execution (https://github.com/cqframework/cql-execution).
- **Category**: Rules Engine.
- **What relevant component it supplies**: Execution logic for the Clinical Quality Language (CQL).
- **Language and runtime**: TypeScript / JavaScript (Node.js).
- **Storage model**: In-memory execution over provided JSON bundles.
- **Local/offline suitability**: High (if Node.js is present).
- **License and any data-license restrictions**: Apache 2.0.
- **Current maintenance status**: Active.
- **Latest meaningful release and release date**: Continuous updates alongside CQL logic improvements.
- **Recent commit and issue activity**: Very active, integrating with FHIR data sources and value sets.
- **Maintainer/community health**: Maintained by the Clinical Quality Framework (CQF) initiative.
- **Documentation and test quality**: High.
- **API stability**: Moderate (evolving support for CQL 1.5).
- **Personal versus provider/enterprise orientation**: Provider/population analytics orientation.
- **Provenance/versioning/correction support**: Low (stateless engine).
- **Clinical-domain breadth**: Complete coverage of clinical quality measures.
- **Genomics/pharmacogenomics support**: None natively.
- **Agent/CLI suitability**: Low (not designed for iterative agent feedback).
- **Integration difficulty**: Very High.
- **Major risks or deal-breakers**: Demands a JavaScript runtime environment. Executing standard CQL requires translating CQL to JSON ELM via a separate Java-based translator, then executing it in JS/TS. This multi-runtime pipeline destroys the simplicity of a pure Rust CLI.
- **Recommendation**: **Reject**.
- **Scores**: Architectural fit: 1/5, Data-model fit: 3/5, Local-first fit: 3/5, Provenance/audit fit: 1/5, Clinical-analysis fit: 5/5, Genomics fit: 1/5, Agent integration fit: 2/5, Maintenance maturity: 5/5, Licensing fit: 5/5, Estimated adoption value: 1/5.

### 7. EHRbase

- **Project name and direct links**: EHRbase (https://github.com/ehrbase/ehrbase).
- **Category**: Server / Reference Implementation.
- **What relevant component it supplies**: openEHR clinical data repository.
- **Language and runtime**: Java.
- **Storage model**: PostgreSQL.
- **Local/offline suitability**: Low (heavy enterprise server).
- **License and any data-license restrictions**: Apache 2.0.
- **Current maintenance status**: Active.
- **Personal versus provider/enterprise orientation**: Enterprise and national infrastructure orientation.
- **Provenance/versioning/correction support**: Exceptional. Core openEHR semantics enforce strict versioning.
- **Major risks or deal-breakers**: Far too complex and heavyweight for a local CLI.
- **Recommendation**: **Learn From**.
- **Scores**: Architectural fit: 1/5, Data-model fit: 5/5 (Semantics), Local-first fit: 1/5, Provenance/audit fit: 5/5, Clinical-analysis fit: 4/5, Genomics fit: 2/5, Agent integration fit: 2/5, Maintenance maturity: 4/5, Licensing fit: 5/5, Estimated adoption value: 1/5 (Code).

### 8. helios-fhir

- **Project name and direct links**: helios-fhir (https://crates.io/crates/helios-fhir).
- **Category**: Library.
- **What relevant component it supplies**: Strongly-typed Rust representations of FHIR data types.
- **Language and runtime**: Rust.
- **License and any data-license restrictions**: MIT.
- **Current maintenance status**: Active (recently updated v0.2.1).
- **Integration difficulty**: Low.
- **Recommendation**: **Learn From / Alternate**. The `fhir` crate has a larger footprint and longer history, but `helios-fhir` provides an excellent secondary option for boundary serialization.

## 7. Standards Assessment: FHIR vs openEHR vs OMOP vs Open mHealth vs Custom

Selecting the canonical internal data model is the most critical architectural decision for Health Engine. Existing standards were evaluated for their ability to support deterministic analysis, strict provenance mapping, and seamless agent interaction via JSON schemas.

**FHIR (Fast Healthcare Interoperability Resources):** FHIR is the undisputed standard for wire-level exchange between institutional systems. However, its architecture is highly graph-oriented and heavily relies on nested extensions. Using FHIR as the _canonical internal store_ in an SQLite database forces immense complexity upon the application logic and querying patterns. Extracting temporal sequences or joining complex observations requires recursive JSON parsing that degrades SQLite performance. Therefore, Health Engine should use FHIR resources exclusively at the application boundary for import and export, but not for the internal relational structure.

**openEHR:** openEHR brilliantly separates the information model (data types) from the clinical model (archetypes). Its archetype model handles provenance, versioning, and epistemic states masterfully. Specifically, it explicitly codifies "null flavors" to manage missing or unknown data, providing specific vocabularies for "no information" (271), "unknown" (253), "masked" (272), and "not applicable" (273). This directly satisfies the invariant to distinguish "normal," "not measured," and "not asked." However, implementing a full openEHR kernel requires handling XML and BMM (Basic Meta-Model) complexities that are disproportionate for a lightweight CLI. Health Engine should emulate openEHR's semantic handling of state and null flavors but reject its full architectural overhead.

**OMOP CDM (Observational Medical Outcomes Partnership):** Designed for massive population-level observational research, OMOP flattens clinical data into standardized concept IDs. While excellent for cross-institutional statistical analysis, it completely lacks the granularity required for source-document provenance (e.g., linking a specific symptom fact to page 2, paragraph 3 of an ingested PDF). It is too rigid to serve as an individual's canonical PHR.

**Open mHealth:** Open mHealth provides exceptionally clean, simple JSON schemas for mobile health data, such as vitals, weight, and activity. These schemas are highly intuitive and easily generated or parsed by LLM agents. They represent a strong foundation for the vitals and telemetry domains of Health Engine.

**Custom Internal Model (Recommended):** Health Engine must utilize a bespoke, relational SQLite schema as its canonical store. This custom model must:

1. Flatten deeply nested FHIR-like structures into highly normalized relational tables optimized for fast, deterministic SQLite querying.
2. Incorporate Open mHealth principles for observation simplicity.
3. Natively integrate continuous provenance mapping (Fact -> Extraction Event -> Specific PDF Location).
4. Explicitly model symptom states ("present", "absent", "unknown", "not asked") drawing direct inspiration from openEHR null flavors, ensuring patient-reported verbatim wording is preserved in a dedicated text column.

## 8. Storage and Provenance Recommendations

**Storage Model:** SQLite is the optimal canonical store for a portable, local-first Workspace. A single health subject's entire record should reside within one encrypted SQLite file. This enables seamless backups, true portability across devices, and strict offline-first privacy guarantees. To maximize read/write performance during agent ingestion, the database should be configured in Write-Ahead Logging (WAL) mode.

**Provenance & Event Sourcing:** Health Engine's invariants demand that corrections never overwrite history and that known chart errors remain attached to affected facts. To mathematically guarantee this, Health Engine must employ an **append-only event sourcing architecture**. The operational flow is as follows:

1. Agents extract data and emit a "Fact Extraction Event." This event contains the structured JSON and precise source pointers (e.g., file hash, page number, bounding box coordinates, and exact text quotation).
2. Health Engine writes this event to an immutable SQLite event log table.
3. A materialized view (or standard relational state table rebuilt from the log) represents the current "State of the Record."
4. If a human or agent spots an error, a new "Correction Event" is appended. This event explicitly invalidates or modifies the previous event ID. The materialized view updates, but the original erroneous extraction remains in the immutable log, preserving absolute auditability.

**Stale Conclusion Detection:** Because all data enters as discrete events, the deterministic Analysis Engine can build a dependency graph. If a downstream rule (e.g., a cardiovascular risk score) relies on Fact A (a blood pressure reading), and Fact A is subsequently targeted by a Correction Event, the engine deterministically invalidates and recalculates only that specific branch of the Analysis JSON.

## 9. Clinical Rules and Guideline-Engine Recommendations

The requirement for deterministic, source-backed health analysis poses a challenge. Standard rules engines like the CQL Execution framework offer exceptional clinical rigor but require a Node.js or Java runtime, rendering them incompatible with a standalone Rust application.

To maintain the invariants—specifically that temporal association is not automatically presented as causation, and all rules are strictly source-backed—Health Engine must construct its own deterministic rules engine.

- **Execution Model**: The rules engine should utilize a simple, declarative, Directed Acyclic Graph (DAG) execution model.
- **Rule Definition**: Rules should be expressed in a human-readable, version-controlled format (e.g., YAML or a custom Rust DSL). Agents can read these definitions to understand exactly how a conclusion is reached.
- **Evaluation**: The engine deterministically evaluates these rules against the SQLite materialized state views.
- **Extensibility**: To allow the community to contribute complex clinical rules without recompiling the Rust binary, Health Engine should embed a lightweight WebAssembly (WASM) runtime (such as `wasmtime`). This allows clinical rules to be written in any language, compiled to WASM, and executed deterministically within a strict, secure, offline sandbox.

## 10. Genomics and Pharmacogenomics Recommendations

The genomics pipeline must handle raw data ingestion, variant annotation, and pharmacogenomic interpretation natively or via specific standard interfaces.

**Raw Data Parsing:** The primary supported format, 23andMe raw data, is a straightforward tab-separated text file containing four core columns: `rsid`, `chromosome`, `position`, and `genotype` (e.g., AA, AG, CT), aligned to the GRCh37 (hg19) reference genome.

- _Implementation Strategy_: Health Engine must build a native Rust parser that skips the comment headers (lines starting with `#`) and strictly validates the four columns.
- _Critical Safeguard_: Because position coordinates are entirely dependent on the genome build, the parser must explicitly verify the build version. If a file uses GRCh38 but masquerades as GRCh37, or if the format is unknown, the engine must fail deterministically to prevent clinical misinterpretation.

**Variant Interpretation:** ClinVar provides the authoritative database for variant pathogenicity. The data is available in the public domain via FTP in XML, TSV, and VCF formats. However, the VCF files only contain variants with precise locations, while the XML is overly complex.

- _Implementation Strategy_: Health Engine's `data update` tool should natively download the ClinVar TSV summary files and the GRCh37 VCF files, compiling them into a local, highly indexed SQLite cache for rapid variant lookup.

**Pharmacogenomics (PGx):** PharmCAT is the industry standard for PGx, but its reliance on Java and Python bioinformatics tools (like `bcftools`) makes it unsuitable for direct embedding. Fortunately, the underlying knowledge base—the Clinical Pharmacogenetics Implementation Consortium (CPIC) guidelines—is freely available under CC0 1.0 / CC-BY licenses.

- _Implementation Strategy_: Health Engine must engineer a native Rust CPIC interpreter. This interpreter will ingest the CC0 CPIC JSON definitions and evaluate them against the user's genotype store, determining star-allele diplotypes and outputting actionable, confidence-labeled guidance.

**Terminology Licensing Constraints:** A major roadblock in open-source health informatics is the licensing of terminologies like RxNorm and SNOMED CT. While CPIC is CC0, NLM resources require a UMLS license and explicitly prohibit commercial redistribution. Health Engine cannot legally bundle a pre-compiled SQLite database containing RxNorm.

- _Implementation Strategy_: Health Engine must provide utility functions (similar to the Python `umls_downloader` concept) that allow the end-user to input their personal UMLS API key. The CLI will then authenticate, download, and construct the local RxNorm terminology cache on the user's machine on-demand.

## 11. Agent/API/CLI Recommendations

Autonomous Large Language Model (LLM) agents act as the primary interface for ingesting unstructured data (PDFs, images) and extracting structured facts.

**Integration Protocol:** Health Engine must natively adopt the **Model Context Protocol (MCP)**. By exposing itself as an MCP Server, Health Engine provides a standardized interface allowing any compliant LLM agent (e.g., Claude Desktop, local Ollama instances) to securely query the health record, execute verifications, and submit extracted facts. The recent deployment of Open-CRAVAT's MCP server validates this architectural choice for complex bioinformatics interactions.

**JSON Schemas and Verification Workflow:** Agents must communicate using strongly typed JSON schemas. When an agent extracts a fact, it submits it via MCP, supplying the file hash, page number, and the exact text quotation. The engine marks this as "Unverified." The CLI will feature a `health-engine record verify` command. This command randomly samples unverified facts, presenting the extracted JSON alongside the exact quotation from the source PDF to either a secondary independent agent or a human user for verification. This explicitly satisfies the invariant that agents can query facts attributed to a PDF and verify them against the source.

## 12. Proposed Adopt/Adapt/Build Architecture

The architecture maps each component to a specific implementation strategy to minimize reinventing the wheel while protecting the core Rust invariants.

- **Workspace/SQLite record**: Build from scratch (Custom relational event-sourcing schema).
- **Fact schemas**: Build from scratch (Influenced by Open mHealth and FHIR).
- **Terminology and unit normalization**: Build from scratch (Requires on-demand UMLS download utility).
- **Provenance and correction history**: Build from scratch (Append-only event log).
- **Verification workflow**: Build from scratch (Integrated with MCP).
- **Care tasks and protocols**: Build from scratch.
- **Deterministic rules**: Build from scratch (Custom DAG or embedded WASM sandbox).
- **Analysis dependency tracking**: Build from scratch (Event log graph mapping).
- **Genomic import**: Build from scratch (Native 23andMe tab-separated parser).
- **Clinical variant interpretation**: Adopt ClinVar TSV/VCF datasets; Build native SQLite indexer.
- **Pharmacogenomics**: Adopt CPIC CC0 data; Build native Rust diplotype interpreter.
- **Public-data updates**: Build from scratch (The `data update` CLI).
- **JSON and agent contracts**: Build from scratch (Exposed via MCP).
- **Human rendering**: Build from scratch (Markdown output generation).

## 13. Explicit List of Components We Should Adopt

1. **`fhir` (Rust crate)**: Adopt for all boundary FHIR R5 import/export interfaces to ensure standard compliance when sharing data externally.
2. **ClinVar (NCBI) Datasets**: Adopt the GRCh37 VCF and TSV flat files as the primary variant knowledge base, downloading them directly from the public domain FTP.
3. **CPIC Database**: Adopt the CC0 dataset for pharmacogenomic star-allele mapping and drug prescribing guidance.
4. **ClinGen Curation Frameworks**: Adopt the gene-disease validity scoring system and secondary findings lists.

## 14. Explicit List of Components We Should Extend or Wrap

1. **Open-CRAVAT**: Wrap via the Model Context Protocol (MCP). Instead of embedding its extensive Python bioinformatics logic into the Rust core, expose it as an optional, external agent tool via MCP.

## 15. Explicit List of Projects We Should Learn From Without Adopting

1. **Fasten Health**: Learn from its handling of local HTTPS certificates and its multi-user household SQLite access patterns.
2. **Medplum**: Learn from its Bot framework architecture for orchestrating agent workflows and its exhaustive TypeScript schema definitions for data modeling.
3. **openEHR**: Learn from its epistemic modeling of null flavors (e.g., distinct codes for "not asked", "masked", "unknown") to preserve the exact verbatim state of patient-reported symptoms without coercion.
4. **Open mHealth**: Learn from its intuitive JSON schemas for mobile device telemetry and vitals, which are highly readable for LLM agents.
5. **PharmCAT**: Learn from its algorithmic implementation of CPIC rules to guide the development of the native Rust PGx engine.
6. **cql-execution**: Learn from its deterministic execution flow, but reject its JavaScript runtime dependency.

## 16. Explicit List of Components We Should Build Ourselves

1. **Event-Sourced SQLite Schema**: No existing project implements a lightweight, pure-SQLite append-only event store tailored for PHR provenance.
2. **MCP Agent Server**: Build the native Model Context Protocol server exposing the SQLite workspace safely to LLMs.
3. **Deterministic Rules Engine**: Build a DAG-based engine (or embed a WASM runtime) to evaluate clinical rules deterministically in Rust, avoiding JVM/Node dependencies.
4. **Native 23andMe Parser**: Build a robust, strict-validation parser for the raw GRCh37 txt/zip format.
5. **Native CPIC Interpreter**: Build the logic to map genotypes to star alleles and apply CPIC JSON guidelines natively.
6. **UMLS On-Demand Downloader**: Build the CLI utility that accepts user credentials to download RxNorm and SNOMED CT locally, legally circumventing redistribution restrictions.

## 17. Risks, Unresolved Questions, and Required Prototypes

**Major Risks:**

- **Terminology Licensing Friction**: The inability to bundle RxNorm and SNOMED CT out-of-the-box means users must manually register for a UMLS license and supply API keys. This introduces significant onboarding friction.
- **Agent Hallucination**: Relying on autonomous agents to extract structured clinical facts from highly variable, unstructured PDFs carries severe clinical risk. Strict JSON schema enforcement and a mandatory, human-in-the-loop verification pipeline for critical data are non-negotiable.
- **Reference Genome Mismatches**: Consumer DNA tests are gradually migrating from GRCh37 to GRCh38. Automatically and deterministically detecting the build version from raw text files that may lack explicit headers is an unresolved edge case. Incorrect coordinate mapping will yield fatal downstream clinical guidance.

**Required Prototypes:**

- **MCP Agent Interaction**: Prototype a local LLM agent connecting to a mock Health Engine MCP server to prove the agent can accurately query an SQLite database and submit a Fact Extraction Event containing correct provenance hashes.
- **Rust CPIC Engine**: Prototype a minimal Rust module that ingests a mocked 23andMe genotype file and a CPIC JSON definition to successfully output an actionable star-allele diplotype.
- **WASM Clinical Rules**: Prototype compiling a basic clinical rule (e.g., ASCVD risk score) into WASM and executing it deterministically within a Rust sandbox, benchmarking performance against native execution.

## 18. A Prioritized Next-Step Plan

**Phase 1: Core Storage & Provenance (Months 1-2)**

- Design the bespoke SQLite relational schema optimized for the append-only event sourcing log.
- Implement the core Rust library and the `health-engine workspace init` and `status` CLI commands.
- Develop the foundational Fact Schemas based on Open mHealth and openEHR null flavors.

**Phase 2: Agent Interfaces & MCP (Months 2-3)**

- Implement the Model Context Protocol (MCP) server interface for secure agent interaction.
- Implement the `health-engine record add` and `health-engine record query` CLI commands.
- Build the verification workflow architecture (`health-engine record verify`), ensuring provenance linking (Fact -> Source Hash/Quote) functions flawlessly.

**Phase 3: Genomics & Public Data (Months 3-4)**

- Build the `data update` CLI tooling to fetch, parse, and locally index ClinVar TSV/VCF datasets.
- Develop the UMLS authentication and download utility for RxNorm caching.
- Implement the 23andMe raw data parser (`health-engine record import-genome`) with strict GRCh37 build verification.
- Build the native Rust pharmacogenomics interpreter utilizing CPIC CC0 data.

**Phase 4: Analysis & Rendering (Months 5-6)**

- Develop the DAG-based deterministic rules engine (investigating WASM embedding).
- Implement the event-graph dependency tracking to flag stale analyses automatically when Correction Events are appended.
- Finalize human-readable Markdown rendering and CLI audit views (`health-engine analysis run`, `show`, `export`).
