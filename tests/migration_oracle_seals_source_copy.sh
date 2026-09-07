#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-oracle-seal-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
destination_root="$fixture_root/oracle"
mkdir -p "$source_root/test-results"
printf 'synthetic immutable source\n' > "$source_root/test-results/source.pdf"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$destination_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null

sealed_file="$destination_root/sealed/tree/test-results/source.pdf"
workspace_file="$destination_root/workspace/test-results/source.pdf"
legacy_file="$destination_root/legacy-oracle/test-results/source.pdf"

test -f "$sealed_file"
test -f "$workspace_file"
test -f "$legacy_file"
test "$(stat -f '%Lp' "$destination_root")" = '700'
test "$(stat -f '%Lp' "$destination_root/sealed/tree")" = '500'
test "$(stat -f '%Lp' "$sealed_file")" = '400'
test "$(stat -f '%Lp' "$destination_root/private")" = '700'
test -d "$destination_root/private/manifests"
test -d "$destination_root/private/logs"
test -d "$destination_root/private/temp"
test -d "$destination_root/reference-cache/legacy"
test -d "$destination_root/reference-cache/current"

printf 'workspace mutation\n' >> "$workspace_file"
cmp "$source_root/test-results/source.pdf" "$sealed_file"
cmp "$source_root/test-results/source.pdf" "$legacy_file"
test "$(wc -l < "$workspace_file" | tr -d ' ')" = '2'
