#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-oracle-escape-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
destination_root="$fixture_root/oracle"
external_root="$fixture_root/external"
mkdir -p "$source_root/data" "$external_root"
printf 'synthetic private sentinel\n' > "$external_root/outside.txt"
ln -s ../../external/outside.txt "$source_root/data/escaping-link.txt"

set +e
stdout=$(
    printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$destination_root" |
        "$repo_root/scripts/prepare-migration-oracle" 2> "$fixture_root/stderr"
)
status=$?
set -e

test "$status" -ne 0
test -z "$stdout"
test "$(cat "$fixture_root/stderr")" = '{"error":"unsafe_source_tree"}'
test ! -e "$destination_root"
test ! -L "$destination_root"
