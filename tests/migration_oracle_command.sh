#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-oracle-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
destination_root="$fixture_root/oracle"

mkdir -p \
    "$source_root/.claude/skills/intake" \
    "$source_root/.git" \
    "$source_root/data" \
    "$source_root/reports/current" \
    "$source_root/scripts" \
    "$source_root/test-results"

printf 'synthetic document\n' > "$source_root/test-results/2026-01-01_example_lab.pdf"
printf '# synthetic genome\nrs1\t1\t100\tAG\n' > "$source_root/data/genome.txt"
printf 'date,value\n2026-01-01,1\n' > "$source_root/data/measurements.csv"
printf 'synthetic reference\n' > "$source_root/data/reference.tsv"
printf '# Synthetic current view\n' > "$source_root/reports/current/health-summary.md"
printf '# Synthetic generated report\n' > "$source_root/reports/generated.md"
printf '# Synthetic skill\n' > "$source_root/.claude/skills/intake/SKILL.md"
printf 'print("synthetic")\n' > "$source_root/scripts/analyze.py"
printf 'synthetic git metadata\n' > "$source_root/.git/config"

source_digest_before=$(find "$source_root" -type f -not -path '*/.git/*' -exec shasum -a 256 {} + | LC_ALL=C sort | shasum -a 256 | cut -d ' ' -f 1)

result=$(
    printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$destination_root" |
        "$repo_root/scripts/prepare-migration-oracle"
)

test "$result" = '{"status":"ready"}'
test "$(stat -f '%Lp' "$destination_root")" = '700'
test -f "$destination_root/private/manifests/original-before.jsonl"
test -f "$destination_root/private/manifests/original-after.jsonl"
test -f "$destination_root/private/manifests/sealed-copy.jsonl"
test -f "$destination_root/private/manifests/workspace.jsonl"
test -f "$destination_root/private/manifests/inventory.jsonl"
test -f "$destination_root/workspace/data/genome.txt"
test ! -e "$destination_root/workspace/.git"
cmp "$source_root/data/genome.txt" "$destination_root/workspace/data/genome.txt"
cmp "$destination_root/private/manifests/original-before.jsonl" "$destination_root/private/manifests/original-after.jsonl"

source_digest_after=$(find "$source_root" -type f -not -path '*/.git/*' -exec shasum -a 256 {} + | LC_ALL=C sort | shasum -a 256 | cut -d ' ' -f 1)
test "$source_digest_before" = "$source_digest_after"

python3 - "$destination_root/private/manifests/inventory.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
assert len(rows) == 8
assert {row["role"] for row in rows} == {
    "code",
    "derived_oracle",
    "machine_readable_source",
    "reference_resource",
    "skill",
    "source",
}
assert all(row["disposition"] != "unclassified" for row in rows)
assert all(row["owner"] == "migration" for row in rows)
PY
