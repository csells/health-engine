# Keep one current local copy of each reference source

The Health Engine will acquire required reference resources directly from their public upstream sources and keep the resulting runtime data in an untracked local cache. The repository will contain only source manifests and deterministic transformation code: upstream URLs, license metadata, integrity checks, schemas, filters, joins, and build versions. Derived resources such as the GRCh37 ClinVar table and filtered GWAS associations will be built locally rather than distributed by the project. The cache contains public reference data only, is safe to delete and rebuild, and remains separate from both the installed engine and private Workspaces.

Data-dependent operations will never download or update resources implicitly. When required data is absent, incomplete, or corrupt, library APIs will return a structured `ReferenceDataUnavailable` error. CLI adapters will render that error with what is unavailable, the cache location, the reason, and the exact `health-engine data update` command needed to resolve it. The engine will fail before analysis rather than silently omit evidence or return partial findings.

The reference-data lifecycle uses one consistent CLI command group:

- `health-engine data status` reports the currently installed data without network access or changes.
- `health-engine data check` reports whether newer upstream data is available without changing anything.
- `health-engine data update` downloads newer data and replaces the installed data used by subsequent analyses.

Status and update checks report human-readable output by default and offer a machine-readable form for agents and applications. A check reports installed versions, available versions, applicable terms, and the exact update command. No top-level `status`, `check`, `sync`, or `update` aliases exist.

An update downloads and validates new data before replacing the old copy, so a failed download does not destroy working data. The cache retains only the installed version. Every analysis uses that installed data and records its source versions in Analysis provenance.
