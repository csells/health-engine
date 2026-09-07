#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-oracle-classification-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
destination_root="$fixture_root/oracle"
mkdir -p \
    "$source_root/.claude/skills/example" \
    "$source_root/data" \
    "$source_root/reports/current" \
    "$source_root/scripts" \
    "$source_root/test-results"
printf 'synthetic source\n' > "$source_root/test-results/source.pdf"
printf 'synthetic genome\n' > "$source_root/data/genome.txt"
printf 'date,value\n2026-01-01,1\n' > "$source_root/data/measurements.csv"
printf 'synthetic reference\n' > "$source_root/data/reference.tsv"
printf 'synthetic oracle\n' > "$source_root/reports/current/summary.md"
printf 'synthetic skill\n' > "$source_root/.claude/skills/example/SKILL.md"
printf 'synthetic code\n' > "$source_root/scripts/example.py"
printf 'synthetic unknown\n' > "$source_root/unknown.bin"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$destination_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null

python3 - "$destination_root/private/manifests/inventory.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
by_role = {row["role"]: row for row in rows}
expected = {
    "source": "pending_ingestion",
    "machine_readable_source": "pending_reconciliation",
    "reference_resource": "legacy_baseline",
    "derived_oracle": "oracle_only",
    "skill": "capability_input",
    "code": "capability_input",
    "unsupported_artifact": "quarantined",
}
assert set(by_role) == set(expected)
for role, disposition in expected.items():
    assert by_role[role]["disposition"] == disposition
    assert by_role[role]["owner"] == "migration"
PY
