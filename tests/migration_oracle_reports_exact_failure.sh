#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-verifier-error-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
run_root="$fixture_root/oracle"
mkdir -p \
    "$source_root/.claude/skills/report" \
    "$source_root/data" \
    "$source_root/reports"
printf 'synthetic reference\n' > "$source_root/data/reference.tsv"
printf '%s\n' '# Report' 'Synthetic claim.' > "$source_root/reports/report.md"
printf '# Synthetic skill\n' > "$source_root/.claude/skills/report/SKILL.md"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$run_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null
printf '%s\n' \
    '{"capabilities":[' \
    '  {"id":"report","evidence_paths":[".claude/skills/report/SKILL.md"],"planned_gate":5,"replacement":"report_bundle"}' \
    ']}' \
    > "$run_root/private/capability-profile.json"
printf '{"run_root":"%s"}\n' "$run_root" |
    "$repo_root/scripts/build-migration-ledgers" >/dev/null
chmod 644 "$run_root/private/capabilities.json"

set +e
stdout=$(printf '{"run_root":"%s"}\n' "$run_root" |
    "$repo_root/scripts/verify-migration-oracle" 2> "$fixture_root/stderr")
status=$?
set -e

test "$status" -ne 0
test -z "$stdout"
test "$(cat "$fixture_root/stderr")" = '{"error":"private_permissions"}'
