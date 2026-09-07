#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-oracle-output-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
destination_root="$fixture_root/oracle"
mkdir -p "$source_root/test-results"
printf 'synthetic private sentinel one\n' > "$source_root/test-results/one.pdf"
printf 'synthetic private sentinel two\n' > "$source_root/test-results/two.pdf"
printf 'synthetic private sentinel three\n' > "$source_root/test-results/three.pdf"

result=$(
    printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$destination_root" |
        "$repo_root/scripts/prepare-migration-oracle"
)

test "$result" = '{"status":"ready"}'
test "${result#*synthetic}" = "$result"
test "${result#*$source_root}" = "$result"
