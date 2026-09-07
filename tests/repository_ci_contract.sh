#!/usr/bin/env bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

require_file() {
  local path="$1"
  [[ -f "${repo_root}/${path}" ]] || fail "missing ${path}"
}

require_literal() {
  local path="$1"
  local text="$2"
  grep -Fq -- "${text}" "${repo_root}/${path}" || fail "${path} missing: ${text}"
}

require_pattern() {
  local path="$1"
  local pattern="$2"
  local description="$3"
  grep -Eqi -- "${pattern}" "${repo_root}/${path}" || fail "${path} missing: ${description}"
}

require_file "LICENSE"
require_literal "LICENSE" "MIT License"
require_literal "LICENSE" "Permission is hereby granted, free of charge"

require_file "README.md"
require_pattern "README.md" "not ready for medical use" "medical-use warning"
require_pattern "README.md" "does not diagnose|not a diagnosis" "diagnostic limitation"
require_pattern "README.md" "qualified professional|qualified clinician" "clinical confirmation guidance"
require_pattern "README.md" "emergency services" "emergency guidance"

require_file ".github/workflows/ci.yml"
require_pattern ".github/workflows/ci.yml" "toolchain:[[:space:]]*['\"]?1\\.94\\.0['\"]?" "Rust 1.94.0 toolchain"
require_literal ".github/workflows/ci.yml" "cargo fmt --all --check"
require_literal ".github/workflows/ci.yml" "cargo check --all-targets --all-features --locked"
require_literal ".github/workflows/ci.yml" "cargo clippy --all-targets --all-features --locked -- -D warnings"
require_literal ".github/workflows/ci.yml" "cargo test --all-targets --all-features --locked"
require_literal ".github/workflows/ci.yml" "cargo doc --all-features --no-deps --locked"
require_literal ".github/workflows/ci.yml" "RUSTDOCFLAGS: \"-D warnings\""
require_pattern ".github/workflows/ci.yml" "cargo-deny-action@" "cargo-deny action"
require_literal ".github/workflows/ci.yml" "command: check"
require_literal ".github/workflows/ci.yml" "arguments: advisories bans licenses sources"

require_file "deny.toml"
for section in advisories licenses bans sources; do
  require_literal "deny.toml" "[${section}]"
done
require_pattern "deny.toml" "unknown-registry[[:space:]]*=[[:space:]]*['\"]deny['\"]" "unknown registry denial"
require_pattern "deny.toml" "unknown-git[[:space:]]*=[[:space:]]*['\"]deny['\"]" "unknown Git source denial"
python3 -c 'import pathlib, tomllib; tomllib.loads(pathlib.Path("deny.toml").read_text())' \
  || fail "deny.toml is not valid TOML"

printf 'repository CI contract: ok\n'
