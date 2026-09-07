#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-reference-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
run_root="$fixture_root/oracle"
mkdir -p "$source_root/data"
printf 'synthetic reference\n' > "$source_root/data/reference.tsv"
printf 'Synthetic license\n' > "$source_root/data/LICENSE.txt"
printf '2026-01-01\n' > "$source_root/data/CREATED_2026-01-01.txt"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$run_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null
printf '{"run_root":"%s"}\n' "$run_root" |
    "$repo_root/scripts/build-migration-ledgers" >/dev/null

python3 - "$run_root/private/reference-baseline.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
reference = next(row for row in rows if row["source_path"] == "data/reference.tsv")
assert reference == {
    "disposition": "legacy_baseline",
    "license_evidence": ["data/LICENSE.txt"],
    "owner": "migration",
    "provenance_evidence": ["data/CREATED_2026-01-01.txt"],
    "sha256": "7d8d86a00513278c4edb415dd80e4c72c1be20da1f10a6235e077724eb93a22e",
    "source_path": "data/reference.tsv",
    "version": {"state": "unknown"},
}
PY
