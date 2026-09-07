#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture_root=$(mktemp -d "$repo_root/tmp/migration-capability-test.XXXXXX")
trap 'chmod -R u+rwX "$fixture_root" 2>/dev/null || true; rm -rf "$fixture_root"' EXIT

source_root="$fixture_root/source"
run_root="$fixture_root/oracle"
mkdir -p "$source_root/.claude/skills/intake" "$source_root/scripts"
printf '# Synthetic intake skill\n' > "$source_root/.claude/skills/intake/SKILL.md"
printf 'print("synthetic")\n' > "$source_root/scripts/analyze.py"

printf '{"source_root":"%s","destination_root":"%s"}\n' "$source_root" "$run_root" |
    "$repo_root/scripts/prepare-migration-oracle" >/dev/null
printf '%s\n' \
    '{"capabilities":[' \
    '  {"id":"intake","evidence_paths":[".claude/skills/intake/SKILL.md"],"planned_gate":5,"replacement":"thin_intake_adapter"},' \
    '  {"id":"genomic_analysis","evidence_paths":["scripts/analyze.py"],"planned_gate":4,"replacement":"typed_analysis"}' \
    ']}' \
    > "$run_root/private/capability-profile.json"

printf '{"run_root":"%s"}\n' "$run_root" |
    "$repo_root/scripts/build-migration-ledgers" >/dev/null

python3 - "$run_root/private/capabilities.json" <<'PY'
import json
import sys

document = json.load(open(sys.argv[1], encoding="utf-8"))
assert document == {
    "capabilities": [
        {
            "disposition": "pending_replacement",
            "evidence_paths": [".claude/skills/intake/SKILL.md"],
            "id": "intake",
            "owner": "migration",
            "planned_gate": 5,
            "replacement": "thin_intake_adapter",
        },
        {
            "disposition": "pending_replacement",
            "evidence_paths": ["scripts/analyze.py"],
            "id": "genomic_analysis",
            "owner": "migration",
            "planned_gate": 4,
            "replacement": "typed_analysis",
        },
    ],
    "schema_version": 1,
}
PY
