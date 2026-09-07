#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-claims-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
run_root="$fixture_root/oracle"
mkdir -p "$source_root/reports"
printf '%s\n' \
    '# Synthetic report' \
    '' \
    '- Finding: synthetic marker increased.' \
    '' \
    'Synthetic explanation with stable meaning.' \
    > "$source_root/reports/generated.md"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$run_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null

result=$(printf '{"run_root":"%s"}\n' "$run_root" | "$repo_root/scripts/build-migration-ledgers")
test "$result" = '{"status":"ready"}'

python3 - "$run_root/private/claims.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
assert [row["normalized_meaning"] for row in rows] == [
    "Finding: synthetic marker increased.",
    "Synthetic explanation with stable meaning.",
]
assert [row["location"] for row in rows] == [
    {"kind": "line", "start": 3},
    {"kind": "line", "start": 5},
]
assert len({row["id"] for row in rows}) == 2
assert all(len(row["id"]) == 64 for row in rows)
assert all(row["source_path"] == "reports/generated.md" for row in rows)
assert all(row["disposition"] == "expected_output" for row in rows)
assert all(row["owner"] == "migration" for row in rows)
PY
