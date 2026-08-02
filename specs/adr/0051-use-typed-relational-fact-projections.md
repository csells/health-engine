# Use typed relational Fact projections behind the Health Record seam

SQLite will store an immutable common Fact envelope plus type-specific relational projections and a canonical versioned payload for lossless interchange. Health Record owns all constraints, transactions, Corrections, Reconciliations, current-state views, and schema migration behind its interface; append-only history does not imply a generic event-store framework, and deterministic queries will not depend on an unvalidated JSON-blob schema.
