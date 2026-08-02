PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS workspace_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subject (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS facts (
    id TEXT PRIMARY KEY,
    fact_type TEXT NOT NULL,
    effective_date TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    source_json TEXT NOT NULL,
    source_path TEXT,
    author_json TEXT NOT NULL,
    extraction_confidence REAL NOT NULL CHECK (extraction_confidence >= 0.0 AND extraction_confidence <= 1.0),
    payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS facts_type_date ON facts(fact_type, effective_date);
CREATE INDEX IF NOT EXISTS facts_source_path ON facts(source_path);

CREATE TABLE IF NOT EXISTS corrections (
    id TEXT PRIMARY KEY,
    prior_fact_id TEXT NOT NULL UNIQUE REFERENCES facts(id),
    replacement_fact_id TEXT NOT NULL UNIQUE REFERENCES facts(id),
    reason TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS verifications (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL REFERENCES facts(id),
    outcome TEXT NOT NULL CHECK (outcome IN ('verified', 'discrepancy')),
    reviewer_json TEXT NOT NULL,
    source_location_json TEXT,
    notes TEXT,
    checked_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS verifications_fact_time ON verifications(fact_id, checked_at DESC);

CREATE TABLE IF NOT EXISTS analyses (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    dependency_fingerprint TEXT NOT NULL,
    analysis_json TEXT NOT NULL
);

CREATE TRIGGER IF NOT EXISTS facts_no_update
BEFORE UPDATE ON facts
BEGIN
    SELECT RAISE(ABORT, 'accepted facts are append-only');
END;

CREATE TRIGGER IF NOT EXISTS facts_no_delete
BEFORE DELETE ON facts
BEGIN
    SELECT RAISE(ABORT, 'accepted facts cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS corrections_no_update
BEFORE UPDATE ON corrections
BEGIN
    SELECT RAISE(ABORT, 'corrections are append-only');
END;

CREATE TRIGGER IF NOT EXISTS corrections_no_delete
BEFORE DELETE ON corrections
BEGIN
    SELECT RAISE(ABORT, 'corrections cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS verifications_no_update
BEFORE UPDATE ON verifications
BEGIN
    SELECT RAISE(ABORT, 'verifications are append-only');
END;

CREATE TRIGGER IF NOT EXISTS verifications_no_delete
BEFORE DELETE ON verifications
BEGIN
    SELECT RAISE(ABORT, 'verifications cannot be deleted');
END;

PRAGMA user_version = 1;
