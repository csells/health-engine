#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-report-csv-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
run_root="$fixture_root/oracle"
mkdir -p "$source_root/reports/current"
printf 'Date,Test,Value\n2026-01-01,Synthetic,1\n' \
    > "$source_root/reports/current/lab-timeline.csv"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$run_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null

python3 - "$run_root/private/manifests/inventory.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
assert rows == [
    {
        "disposition": "pending_reconciliation",
        "kind": "file",
        "owner": "migration",
        "path": "reports/current/lab-timeline.csv",
        "role": "machine_readable_source",
    }
]
PY
