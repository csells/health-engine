# Health Engine

Health Engine is an early-stage Rust library and CLI for a portable, structured, auditable personal health record and deterministic health analysis.

The project is being built in public. Its current implementation is not ready for medical use.

## Safety

Health Engine does not diagnose disease or replace a clinician. Outputs may be incomplete or wrong. Confirm health decisions with a qualified professional, and use local emergency services for emergencies.

## Architecture

- Agents and applications file original health documents and extract structured facts.
- Health Engine validates and stores those facts in a SQLite database inside the user's Workspace.
- Public reference data is downloaded separately and is never bundled into this MIT-licensed repository.
- Every conclusion carries provenance, verification state, and confidence.

See the [vision](specs/vision/health-engine-vision.md) and [implementation plan](specs/plans/health-engine-implementation-plan.md).
