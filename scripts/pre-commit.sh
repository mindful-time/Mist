#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf '%s\n' "Mist commit gate requires '$1'. $2" >&2
        exit 1
    fi
}

require_command cargo "Install Rust 1.95 or newer."
require_command uv "Install uv: https://docs.astral.sh/uv/getting-started/installation/"
require_command osv-scanner "Install OSV-Scanner: https://google.github.io/osv-scanner/installation/"
require_command gitleaks "Install Gitleaks: https://github.com/gitleaks/gitleaks#installing"
require_command jq "Install jq: https://jqlang.github.io/jq/download/"

printf '%s\n' 'Mist commit gate: rustfmt'
cargo fmt --all -- --check

printf '%s\n' 'Mist commit gate: cargo check'
cargo check --locked --all-targets

printf '%s\n' 'Mist commit gate: Clippy'
cargo clippy --locked --all-targets -- -D warnings

printf '%s\n' 'Mist commit gate: tests'
cargo test --locked --all-targets

printf '%s\n' 'Mist commit gate: CRAP coverage (warn >5, block >10)'
"$project_root/scripts/crap-gate.sh"

printf '%s\n' 'Mist commit gate: deterministic Smells 0.5.0 scan'
uvx --from smells==0.5.0 smells check \
    --staged \
    --policy quality-policy.json \
    --format table \
    --log smells-findings.log \
    --report smells-report.json

printf '%s\n' 'Mist commit gate: OSV dependency audit'
osv-scanner scan source --lockfile Cargo.lock

printf '%s\n' 'Mist commit gate: staged-secret scan'
gitleaks git --pre-commit --staged --redact .

printf '%s\n' 'Mist commit gate passed.'
