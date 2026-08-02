# Repository Guidelines

## Project Structure & Module Organization

Health Engine is a Rust 2024 library with a planned `health-engine` CLI. Put domain and API code in `src/`; keep `src/lib.rs` as the small public surface and place CLI wiring in `src/main.rs`. SQLite migrations live in `migrations/` and use ordered names such as `001_initial.sql`. Product intent is documented in `specs/vision/`, implementation work in `specs/plans/`, research in `specs/research/`, and architectural decisions in `specs/adr/`. Update `CONTEXT.md` when introducing or changing domain terminology.

## Build, Test, and Development Commands

- `cargo check` — type-check all targets quickly during development.
- `cargo build` — compile the library and CLI in debug mode.
- `cargo run -- <args>` — run the CLI locally; for example, `cargo run -- workspace status`.
- `cargo test` — run unit and integration tests.
- `cargo fmt --check` — verify standard Rust formatting.
- `cargo clippy --all-targets --all-features` — enforce the crate's strict Clippy configuration.

Run formatting, Clippy, and tests before submitting a change. The crate requires Rust 1.94 or newer.

## Coding Style & Naming Conventions

Use rustfmt defaults (four-space indentation). Name modules, functions, and files with `snake_case`; types and traits with `PascalCase`; constants with `SCREAMING_SNAKE_CASE`. Keep public interfaces narrow and move implementation details into focused modules. Unsafe Rust is forbidden, and all Clippy and pedantic warnings are denied. Preserve the domain language in `CONTEXT.md`, especially distinctions among Sources, Health Facts, Analyses, and Workspaces.

## Testing Guidelines

Place focused unit tests beside implementation code in `#[cfg(test)]` modules; put cross-module and CLI behavior in `tests/`. Use descriptive behavior names such as `correction_preserves_original_fact`. Add regression tests for fixes and migration tests for schema changes. Fixtures must be synthetic: never use real personal health data. There is no stated numeric coverage threshold, but new behavior should exercise success, validation, and failure paths.

## Commit & Pull Request Guidelines

History is currently minimal, so use short imperative commit subjects, for example `Add workspace migration`, and keep commits focused. Pull requests should explain the behavior and rationale, link relevant issues or ADRs, and list validation commands run. Include sample CLI output for user-visible changes and call out schema, privacy, or compatibility effects explicitly.

## Security & Data Boundaries

Never commit personal health data, Workspace databases, secrets, or downloaded third-party reference datasets. Keep accepted facts append-only except for the explicitly Operator-authorized Expungement contract, use forward-only migrations, and preserve provenance and origin-appropriate Evidence Assurance in every data-model change. Keep private migration copies, databases, manifests, logs, and comparison artifacts only under ignored `tmp/`; never modify the original health repository during migration validation.
