#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-oracle-symlink-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
destination_root="$fixture_root/oracle"
mkdir -p "$source_root/.agents" "$source_root/.claude/skills/intake"
printf '# Synthetic skill\n' > "$source_root/.claude/skills/intake/SKILL.md"
ln -s ../.claude/skills "$source_root/.agents/skills"

result=$(
    printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$destination_root" |
        "$repo_root/scripts/prepare-migration-oracle"
)

test "$result" = '{"status":"ready"}'
test -L "$destination_root/workspace/.agents/skills"
test "$(readlink "$destination_root/workspace/.agents/skills")" = '../.claude/skills'

python3 - "$destination_root/private/manifests/inventory.jsonl" <<'PY'
import json
import sys

rows = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
alias = next(row for row in rows if row["path"] == ".agents/skills")
assert alias == {
    "disposition": "capability_input",
    "kind": "symlink",
    "owner": "migration",
    "path": ".agents/skills",
    "role": "skill_alias",
    "target": "../.claude/skills",
}
PY
