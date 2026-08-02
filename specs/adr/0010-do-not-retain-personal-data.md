# Superseded: do not retain personal data

**Status:** Superseded by ADR 0017.

The original decision made Health Engine stateless with respect to people and left private storage entirely to callers. That would force every skill and application to reinvent the structured record, validation, provenance, correction, and query layer. Health Engine will instead manage personal data inside an explicit user-owned Workspace while keeping it out of tracked or distributed project artifacts and the public reference-data cache.
