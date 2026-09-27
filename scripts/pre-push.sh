#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

"$project_root/scripts/pre-commit.sh"

printf '%s\n' 'Mist push gate: full Smells 0.5.0 scan'
uvx --from smells==0.5.0 smells check \
    --path "$project_root" \
    --policy "$project_root/quality-policy.json" \
    --format table \
    --log "$project_root/smells-findings.log" \
    --report "$project_root/smells-report.json"

"$project_root/scripts/release-check.sh"
gitleaks git --redact "$project_root"
