PRAGMA foreign_keys = ON;

CREATE TABLE workspace (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    workspace_id TEXT NOT NULL UNIQUE,
    subject_id TEXT NOT NULL UNIQUE,
    record_revision INTEGER NOT NULL CHECK (record_revision >= 0)
);

CREATE TABLE ledger_mutation_guard (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    expungement_enabled INTEGER NOT NULL CHECK (expungement_enabled IN (0, 1))
);

INSERT INTO ledger_mutation_guard (singleton, expungement_enabled) VALUES (1, 0);

CREATE TABLE expungement_cleanup_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    pending INTEGER NOT NULL CHECK (pending IN (0, 1))
);

INSERT INTO expungement_cleanup_state (singleton, pending) VALUES (1, 0);

CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    sha256 BLOB NOT NULL UNIQUE CHECK (length(sha256) = 32),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    media_type TEXT NOT NULL
);

CREATE TABLE source_aliases (
    alias TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id)
);

CREATE INDEX source_aliases_source ON source_aliases(source_id, alias);

CREATE TABLE extraction_runs (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id),
    extractor_name TEXT NOT NULL,
    extractor_version TEXT NOT NULL,
    record_revision INTEGER NOT NULL CHECK (record_revision > 0)
);

CREATE INDEX extraction_runs_source ON extraction_runs(source_id, record_revision, id);

CREATE TABLE extraction_run_regions (
    extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    locator TEXT NOT NULL,
    PRIMARY KEY (extraction_run_id, position)
);

CREATE TABLE extraction_run_domains (
    extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    domain TEXT NOT NULL,
    PRIMARY KEY (extraction_run_id, position)
);

CREATE TABLE extraction_run_issues (
    extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id),
    issue_kind TEXT NOT NULL CHECK (issue_kind IN ('omission', 'failure')),
    position INTEGER NOT NULL CHECK (position >= 0),
    code TEXT NOT NULL,
    region_locator TEXT,
    PRIMARY KEY (extraction_run_id, issue_kind, position)
);

CREATE TABLE extraction_run_candidates (
    extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    candidate_key TEXT NOT NULL,
    PRIMARY KEY (extraction_run_id, position)
);

CREATE TABLE record_commits (
    idempotency_key TEXT PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    expected_revision INTEGER NOT NULL CHECK (expected_revision >= 0),
    record_revision INTEGER NOT NULL UNIQUE CHECK (record_revision > 0),
    receipt_json TEXT NOT NULL
);

CREATE TABLE facts (
    id TEXT PRIMARY KEY,
    fact_type TEXT NOT NULL,
    clinical_time_kind TEXT NOT NULL,
    clinical_start_kind TEXT NOT NULL,
    clinical_start_value TEXT NOT NULL,
    clinical_end_kind TEXT,
    clinical_end_value TEXT,
    clinical_source_text TEXT,
    recorded_at TEXT NOT NULL,
    source_id TEXT REFERENCES sources(id),
    source_region_locator TEXT,
    cited_source_id TEXT REFERENCES sources(id),
    cited_source_region_locator TEXT,
    extraction_run_id TEXT REFERENCES extraction_runs(id),
    evidence_assurance TEXT NOT NULL,
    author_kind TEXT NOT NULL,
    author_identifier TEXT NOT NULL,
    canonical_payload TEXT NOT NULL,
    record_revision INTEGER NOT NULL CHECK (record_revision > 0),
    CHECK ((source_id IS NULL) = (source_region_locator IS NULL)),
    CHECK ((cited_source_id IS NULL) = (cited_source_region_locator IS NULL)),
    CHECK ((clinical_end_kind IS NULL) = (clinical_end_value IS NULL))
);

CREATE INDEX facts_type_revision ON facts(fact_type, record_revision, id);
CREATE INDEX facts_source ON facts(source_id, record_revision, id);

CREATE TABLE lab_results (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    test_name TEXT NOT NULL,
    canonical_test_identifier TEXT,
    canonical_test_display_name TEXT,
    numeric_value TEXT,
    numeric_comparator TEXT,
    numeric_original TEXT,
    text_value TEXT,
    units TEXT,
    reference_range_original TEXT,
    reference_range_lower TEXT,
    reference_range_upper TEXT,
    reported_flag TEXT,
    performing_lab TEXT,
    CHECK ((canonical_test_identifier IS NULL) = (canonical_test_display_name IS NULL)),
    CHECK ((numeric_value IS NOT NULL) != (text_value IS NOT NULL)),
    CHECK ((numeric_comparator IS NULL) = (numeric_original IS NULL)),
    CHECK (numeric_comparator IS NULL OR numeric_value IS NOT NULL)
);

CREATE TABLE vital_measurements (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    vital_kind TEXT NOT NULL CHECK (vital_kind IN ('blood_pressure', 'pulse', 'weight')),
    primary_value TEXT NOT NULL,
    secondary_value TEXT,
    original_primary TEXT NOT NULL,
    original_secondary TEXT,
    units TEXT,
    measured_by TEXT,
    note TEXT,
    CHECK ((secondary_value IS NULL) = (original_secondary IS NULL)),
    CHECK ((vital_kind = 'blood_pressure') = (secondary_value IS NOT NULL))
);

CREATE TABLE medication_events (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    event_kind TEXT NOT NULL CHECK (event_kind IN (
        'regimen_reported', 'started', 'stopped', 'dose_changed', 'schedule_changed',
        'missed_dose', 'as_needed_use'
    )),
    product_kind TEXT NOT NULL CHECK (product_kind IN ('medication', 'supplement')),
    name TEXT NOT NULL,
    normalized_identity TEXT,
    strength TEXT,
    dose_value TEXT,
    dose_unit TEXT,
    dose_original TEXT,
    route TEXT,
    schedule TEXT,
    indication TEXT,
    adherence_context TEXT,
    CHECK (
        (dose_value IS NULL AND dose_unit IS NULL AND dose_original IS NULL)
        OR (dose_value IS NOT NULL AND dose_unit IS NOT NULL AND dose_original IS NOT NULL)
    )
);

CREATE TABLE condition_assertions (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    assertion_state TEXT NOT NULL CHECK (assertion_state IN (
        'suspected', 'confirmed', 'ruled_out', 'inactive', 'resolved'
    )),
    name TEXT NOT NULL,
    normalized_identity TEXT,
    body_site TEXT,
    assertion_text TEXT NOT NULL
);

CREATE TABLE diagnostic_studies (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    study_kind TEXT NOT NULL CHECK (study_kind IN ('imaging', 'pathology', 'procedure', 'other')),
    result_status TEXT NOT NULL CHECK (result_status IN (
        'ordered', 'scheduled', 'performed', 'preliminary', 'final', 'amended', 'cancelled'
    )),
    name TEXT NOT NULL,
    body_site TEXT,
    method TEXT,
    impression TEXT,
    resulted_time_kind TEXT,
    resulted_time_value TEXT,
    ordering_provider TEXT,
    interpreting_provider TEXT,
    performing_organization TEXT,
    CHECK ((resulted_time_kind IS NULL) = (resulted_time_value IS NULL))
);

CREATE TABLE diagnostic_study_findings (
    study_fact_id TEXT NOT NULL REFERENCES diagnostic_studies(fact_id),
    position INTEGER NOT NULL CHECK (position >= 0),
    section TEXT,
    body_site TEXT,
    finding_text TEXT NOT NULL,
    PRIMARY KEY (study_fact_id, position)
);

CREATE TABLE care_tasks (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    task_key TEXT NOT NULL,
    task_kind TEXT NOT NULL CHECK (task_kind IN (
        'repeat_test', 'referral', 'appointment', 'awaited_result', 'other'
    )),
    task_status TEXT NOT NULL CHECK (task_status IN (
        'pending', 'due', 'overdue', 'awaiting_result', 'completed', 'cancelled'
    )),
    action TEXT NOT NULL,
    due_kind TEXT CHECK (due_kind IN ('on', 'interval')),
    due_time_kind TEXT,
    due_time_value TEXT,
    due_interval_value INTEGER CHECK (due_interval_value > 0),
    due_interval_unit TEXT CHECK (due_interval_unit IN ('days', 'weeks', 'months', 'years')),
    due_source_text TEXT,
    requested_by TEXT,
    source_text TEXT NOT NULL,
    CHECK (
        (due_kind IS NULL AND due_time_kind IS NULL AND due_time_value IS NULL
         AND due_interval_value IS NULL AND due_interval_unit IS NULL AND due_source_text IS NULL)
        OR
        (due_kind = 'on' AND due_time_kind IS NOT NULL AND due_time_value IS NOT NULL
         AND due_interval_value IS NULL AND due_interval_unit IS NULL AND due_source_text IS NULL)
        OR
        (due_kind = 'interval' AND due_time_kind IS NULL AND due_time_value IS NULL
         AND due_interval_value IS NOT NULL AND due_interval_unit IS NOT NULL
         AND due_source_text IS NOT NULL)
    )
);

CREATE TABLE subject_preferences (
    fact_id TEXT PRIMARY KEY REFERENCES facts(id),
    preference_key TEXT NOT NULL,
    preference_category TEXT NOT NULL CHECK (preference_category IN (
        'tracking', 'presentation', 'personal_routine'
    )),
    preference_state TEXT NOT NULL CHECK (preference_state IN ('active', 'withdrawn')),
    statement TEXT NOT NULL,
    source_text TEXT NOT NULL
);

CREATE TABLE verifications (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL REFERENCES facts(id),
    source_id TEXT NOT NULL REFERENCES sources(id),
    source_region_locator TEXT NOT NULL,
    verifier_kind TEXT NOT NULL,
    verifier_identifier TEXT NOT NULL,
    method_name TEXT NOT NULL,
    method_version TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    scope TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('verified', 'discrepancy')),
    record_revision INTEGER NOT NULL CHECK (record_revision > 0)
);

CREATE INDEX verifications_fact_revision ON verifications(fact_id, record_revision, id);

CREATE TABLE discrepancy_resolutions (
    id TEXT PRIMARY KEY,
    discrepancy_verification_id TEXT NOT NULL UNIQUE REFERENCES verifications(id),
    supporting_verification_id TEXT NOT NULL REFERENCES verifications(id),
    rationale TEXT NOT NULL,
    author_kind TEXT NOT NULL,
    author_identifier TEXT NOT NULL,
    resolved_at TEXT NOT NULL,
    record_revision INTEGER NOT NULL CHECK (record_revision > 0),
    CHECK (discrepancy_verification_id != supporting_verification_id)
);

CREATE TABLE corrections (
    id TEXT PRIMARY KEY,
    prior_fact_id TEXT NOT NULL UNIQUE REFERENCES facts(id),
    replacement_fact_id TEXT NOT NULL UNIQUE REFERENCES facts(id),
    reason TEXT NOT NULL,
    author_kind TEXT NOT NULL,
    author_identifier TEXT NOT NULL,
    corrected_at TEXT NOT NULL,
    supporting_verification_id TEXT NOT NULL REFERENCES verifications(id),
    record_revision INTEGER NOT NULL CHECK (record_revision > 0)
);

CREATE INDEX corrections_replacement ON corrections(replacement_fact_id);

CREATE TABLE reconciliations (
    id TEXT PRIMARY KEY,
    agreement TEXT NOT NULL CHECK (agreement IN ('equivalent', 'conflicting')),
    author_kind TEXT NOT NULL,
    author_identifier TEXT NOT NULL,
    reconciled_at TEXT NOT NULL,
    rationale TEXT NOT NULL,
    record_revision INTEGER NOT NULL CHECK (record_revision > 0)
);

CREATE TABLE reconciliation_fact_edges (
    reconciliation_id TEXT NOT NULL REFERENCES reconciliations(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    fact_id TEXT NOT NULL REFERENCES facts(id),
    PRIMARY KEY (reconciliation_id, position),
    UNIQUE (reconciliation_id, fact_id)
);

CREATE INDEX reconciliation_edges_fact
    ON reconciliation_fact_edges(fact_id, reconciliation_id, position);

CREATE TABLE record_hazards (
    id TEXT PRIMARY KEY,
    problematic_statement TEXT NOT NULL,
    danger TEXT NOT NULL,
    corrected_understanding TEXT NOT NULL,
    author_kind TEXT NOT NULL,
    author_identifier TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    verification_state TEXT NOT NULL CHECK (verification_state IN ('suspected', 'verified')),
    record_revision INTEGER NOT NULL CHECK (record_revision > 0)
);

CREATE TABLE record_hazard_fact_edges (
    hazard_id TEXT NOT NULL REFERENCES record_hazards(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    fact_id TEXT NOT NULL REFERENCES facts(id),
    PRIMARY KEY (hazard_id, position),
    UNIQUE (hazard_id, fact_id)
);

CREATE INDEX record_hazard_edges_fact
    ON record_hazard_fact_edges(fact_id, hazard_id, position);

CREATE TABLE record_hazard_source_edges (
    hazard_id TEXT NOT NULL REFERENCES record_hazards(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    source_id TEXT NOT NULL REFERENCES sources(id),
    PRIMARY KEY (hazard_id, position),
    UNIQUE (hazard_id, source_id)
);

CREATE TABLE record_hazard_resolutions (
    id TEXT PRIMARY KEY,
    hazard_id TEXT NOT NULL UNIQUE REFERENCES record_hazards(id),
    supporting_verification_id TEXT NOT NULL REFERENCES verifications(id),
    rationale TEXT NOT NULL,
    author_kind TEXT NOT NULL,
    author_identifier TEXT NOT NULL,
    resolved_at TEXT NOT NULL,
    record_revision INTEGER NOT NULL CHECK (record_revision > 0)
);

CREATE TABLE expungement_tombstones (
    id TEXT PRIMARY KEY,
    reason_code TEXT NOT NULL CHECK (
        reason_code IN ('wrong_subject', 'accidental_sensitive_ingestion', 'privacy_request')
    )
);

CREATE TABLE analyses (
    id TEXT PRIMARY KEY,
    record_revision INTEGER NOT NULL CHECK (record_revision > 0),
    created_at TEXT NOT NULL,
    engine_version TEXT NOT NULL,
    freshness TEXT NOT NULL CHECK (freshness IN ('fresh', 'stale'))
);

CREATE TABLE analysis_claims (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL REFERENCES analyses(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    claim_kind TEXT NOT NULL,
    claim_key TEXT NOT NULL,
    statement TEXT NOT NULL,
    confidence TEXT NOT NULL,
    UNIQUE (analysis_id, position)
);

CREATE TABLE analysis_claim_fact_edges (
    claim_id TEXT NOT NULL REFERENCES analysis_claims(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    fact_id TEXT NOT NULL REFERENCES facts(id),
    PRIMARY KEY (claim_id, position)
);

CREATE TABLE analysis_claim_verification_edges (
    claim_id TEXT NOT NULL REFERENCES analysis_claims(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    verification_id TEXT NOT NULL REFERENCES verifications(id),
    PRIMARY KEY (claim_id, position)
);

CREATE TABLE analysis_warnings (
    analysis_id TEXT NOT NULL REFERENCES analyses(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    fact_position INTEGER NOT NULL CHECK (fact_position >= 0),
    code TEXT NOT NULL,
    related_fact_id TEXT NOT NULL REFERENCES facts(id),
    PRIMARY KEY (analysis_id, position, fact_position)
);

CREATE TRIGGER prevent_sources_update
BEFORE UPDATE ON sources
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_sources_delete
BEFORE DELETE ON sources
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_source_aliases_update
BEFORE UPDATE ON source_aliases
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_source_aliases_delete
BEFORE DELETE ON source_aliases
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_runs_update
BEFORE UPDATE ON extraction_runs
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_runs_delete
BEFORE DELETE ON extraction_runs
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_regions_update
BEFORE UPDATE ON extraction_run_regions
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_regions_delete
BEFORE DELETE ON extraction_run_regions
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_domains_update
BEFORE UPDATE ON extraction_run_domains
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_domains_delete
BEFORE DELETE ON extraction_run_domains
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_issues_update
BEFORE UPDATE ON extraction_run_issues
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_issues_delete
BEFORE DELETE ON extraction_run_issues
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_candidates_update
BEFORE UPDATE ON extraction_run_candidates
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_extraction_run_candidates_delete
BEFORE DELETE ON extraction_run_candidates
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_commits_update
BEFORE UPDATE ON record_commits
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_commits_delete
BEFORE DELETE ON record_commits
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_facts_update
BEFORE UPDATE ON facts
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_facts_delete
BEFORE DELETE ON facts
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_lab_results_update
BEFORE UPDATE ON lab_results
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_lab_results_delete
BEFORE DELETE ON lab_results
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_vital_measurements_update
BEFORE UPDATE ON vital_measurements
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_vital_measurements_delete
BEFORE DELETE ON vital_measurements
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_medication_events_update
BEFORE UPDATE ON medication_events
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_medication_events_delete
BEFORE DELETE ON medication_events
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_condition_assertions_update
BEFORE UPDATE ON condition_assertions
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_condition_assertions_delete
BEFORE DELETE ON condition_assertions
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_diagnostic_studies_update
BEFORE UPDATE ON diagnostic_studies
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_diagnostic_studies_delete
BEFORE DELETE ON diagnostic_studies
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_diagnostic_study_findings_update
BEFORE UPDATE ON diagnostic_study_findings
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_diagnostic_study_findings_delete
BEFORE DELETE ON diagnostic_study_findings
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_care_tasks_update
BEFORE UPDATE ON care_tasks
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_care_tasks_delete
BEFORE DELETE ON care_tasks
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_subject_preferences_update
BEFORE UPDATE ON subject_preferences
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_subject_preferences_delete
BEFORE DELETE ON subject_preferences
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_verifications_update
BEFORE UPDATE ON verifications
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_verifications_delete
BEFORE DELETE ON verifications
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_discrepancy_resolutions_update
BEFORE UPDATE ON discrepancy_resolutions
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_discrepancy_resolutions_delete
BEFORE DELETE ON discrepancy_resolutions
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_corrections_update
BEFORE UPDATE ON corrections
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_corrections_delete
BEFORE DELETE ON corrections
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_reconciliations_update
BEFORE UPDATE ON reconciliations
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_reconciliations_delete
BEFORE DELETE ON reconciliations
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_reconciliation_fact_edges_update
BEFORE UPDATE ON reconciliation_fact_edges
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_reconciliation_fact_edges_delete
BEFORE DELETE ON reconciliation_fact_edges
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazards_update
BEFORE UPDATE ON record_hazards
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazards_delete
BEFORE DELETE ON record_hazards
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazard_fact_edges_update
BEFORE UPDATE ON record_hazard_fact_edges
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazard_fact_edges_delete
BEFORE DELETE ON record_hazard_fact_edges
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazard_source_edges_update
BEFORE UPDATE ON record_hazard_source_edges
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazard_source_edges_delete
BEFORE DELETE ON record_hazard_source_edges
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazard_resolutions_update
BEFORE UPDATE ON record_hazard_resolutions
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_record_hazard_resolutions_delete
BEFORE DELETE ON record_hazard_resolutions
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_claims_update
BEFORE UPDATE ON analysis_claims
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_claims_delete
BEFORE DELETE ON analysis_claims
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_claim_fact_edges_update
BEFORE UPDATE ON analysis_claim_fact_edges
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_claim_fact_edges_delete
BEFORE DELETE ON analysis_claim_fact_edges
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_claim_verification_edges_update
BEFORE UPDATE ON analysis_claim_verification_edges
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_claim_verification_edges_delete
BEFORE DELETE ON analysis_claim_verification_edges
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_warnings_update
BEFORE UPDATE ON analysis_warnings
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analysis_warnings_delete
BEFORE DELETE ON analysis_warnings
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analyses_content_update
BEFORE UPDATE OF id, record_revision, created_at, engine_version ON analyses
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_analyses_delete
BEFORE DELETE ON analyses
WHEN (SELECT expungement_enabled FROM ledger_mutation_guard WHERE singleton = 1) = 0
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_expungement_tombstones_update
BEFORE UPDATE ON expungement_tombstones
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;

CREATE TRIGGER prevent_expungement_tombstones_delete
BEFORE DELETE ON expungement_tombstones
BEGIN
    SELECT RAISE(ABORT, 'append-only ledger');
END;


PRAGMA user_version = 1;
