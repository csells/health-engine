#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-html-claims-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
run_root="$fixture_root/oracle"
mkdir -p "$source_root/reports"
printf '%s\n' \
    '<!doctype html><html><body>' \
    '<h1>Synthetic report</h1>' \
    '<p>Synthetic finding with <strong>inline evidence</strong>.</p>' \
    '<ul><li>Synthetic follow-up remains open.</li></ul>' \
    '</body></html>' \
    > "$source_root/reports/report.html"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$run_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null
printf '{"run_root":"%s"}\n' "$run_root" |
    "$repo_root/scripts/build-migration-ledgers" >/dev/null

python3 - "$run_root/private/claims.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
assert [row["normalized_meaning"] for row in rows] == [
    "Synthetic finding with inline evidence.",
    "Synthetic follow-up remains open.",
]
assert [row["location"] for row in rows] == [
    {"index": 1, "kind": "html_block"},
    {"index": 2, "kind": "html_block"},
]
assert all(row["source_path"] == "reports/report.html" for row in rows)
PY
