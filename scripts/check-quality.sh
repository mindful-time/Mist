#!/bin/sh
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
mode=${1:-commit}
case "$mode" in index|commit) ;; *) exit 2 ;; esac

for tool in cargo uv uvx osv-scanner gitleaks jq node; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Mist quality gate requires %s. See CONTRIBUTING.md.\n' "$tool" >&2
        exit 1
    fi
done
report_directory=${MIST_QUALITY_REPORT_DIR:-$project_root/target/quality}
mkdir -p "$report_directory"
MIST_QUALITY_REPORT_DIR=$report_directory
export MIST_QUALITY_REPORT_DIR

printf '%s\n' 'Mist quality gate: formatting, compile checks, Clippy, build, tests'
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --all-targets
cargo test --locked --all-targets
sh scripts/test-distribution.sh
sh scripts/release-check.sh
sh scripts/build-site.sh
node --check site/mist.js
node --check site/site.js

printf '%s\n' 'Mist quality gate: CRAP with real coverage (warn >5, block new/regressed >10)'
sh scripts/crap-gate.sh
printf '%s\n' 'Mist quality gate: Smells 0.5.0 (exit 2 also blocks)'
if [ "$mode" = index ]; then
    uvx --from smells==0.5.0 smells check --staged --policy quality-policy.json \
        --format table --log "$report_directory/smells-findings.log" --report "$report_directory/smells-report.json"
else
    uvx --from smells==0.5.0 smells check --path . --policy quality-policy.json \
        --format table --log "$report_directory/smells-findings.log" --report "$report_directory/smells-report.json"
fi
printf '%s\n' 'Mist quality gate: OSV dependency audit'
osv-scanner scan source --lockfile Cargo.lock --lockfile uv.lock:scripts/test-workflows.py.lock
printf '%s\n' 'Mist quality gate: redacted secret scan'
if [ "$mode" = index ]; then
    gitleaks git --pre-commit --staged --redact .
else
    base=${2:-}
    case "$base" in
        ''|0000000000000000000000000000000000000000|0000000000000000000000000000000000000000000000000000000000000000)
            gitleaks git --redact . ;;
        *)
            git rev-parse --verify "$base^{commit}" >/dev/null
            gitleaks git --redact --log-opts="$base..HEAD" . ;;
    esac
fi
printf '%s\n' 'Mist quality gate passed.'
