#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"

update_baseline=false
if [ "${1:-}" = "--update-baseline" ]; then
    update_baseline=true
elif [ "$#" -ne 0 ]; then
    printf 'Usage: %s [--update-baseline]\n' "$0" >&2
    exit 2
fi

crap_version=$(cargo crap --version)
if [ "$crap_version" != "cargo-crap 0.5.0" ]; then
    printf 'CRAP gate requires cargo-crap 0.5.0; found: %s\n' "$crap_version" >&2
    exit 1
fi

mkdir -p target/quality
warning_threshold=5
blocking_threshold=10
coverage_report=target/quality/lcov.info
crap_report=target/quality/crap-report.json
delta_report=target/quality/crap-delta.json
baseline_report=quality/crap-baseline.json
coverage_cache=$(mktemp -d "${TMPDIR:-/tmp}/mist-coverage-cache.XXXXXX")
trap 'rm -r "$coverage_cache"' EXIT HUP INT TERM
SMELLS_CACHE_DIR=$coverage_cache
export SMELLS_CACHE_DIR

# rustup installations are discovered by cargo-llvm-cov. Homebrew Rust does
# not ship llvm-tools-preview, so use Homebrew's matching LLVM when present.
if [ -z "${LLVM_COV:-}" ] && [ -x /opt/homebrew/opt/llvm/bin/llvm-cov ]; then
    LLVM_COV=/opt/homebrew/opt/llvm/bin/llvm-cov
    export LLVM_COV
fi
if [ -z "${LLVM_PROFDATA:-}" ] && [ -x /opt/homebrew/opt/llvm/bin/llvm-profdata ]; then
    LLVM_PROFDATA=/opt/homebrew/opt/llvm/bin/llvm-profdata
    export LLVM_PROFDATA
fi

cargo llvm-cov --locked --lcov --output-path "$coverage_report"
cargo crap \
    --lcov "$coverage_report" \
    --exclude 'third_party/**' \
    --threshold "$warning_threshold" \
    --format json \
    --sort file \
    --output "$crap_report"

if [ "$update_baseline" = true ]; then
    mkdir -p "$(dirname -- "$baseline_report")"
    cargo crap \
        --lcov "$coverage_report" \
        --exclude 'third_party/**' \
        --threshold "$warning_threshold" \
        --format json \
        --sort file \
        --output "$baseline_report"
    printf 'CRAP baseline updated after review: %s\n' "$baseline_report"
    exit 0
fi

if [ ! -f "$baseline_report" ]; then
    printf 'CRAP gate cannot run without the committed baseline: %s\n' "$baseline_report" >&2
    printf '%s\n' 'Generate it only after reviewing the full warning report.' >&2
    exit 1
fi

cargo crap \
    --lcov "$coverage_report" \
    --exclude 'third_party/**' \
    --threshold "$blocking_threshold" \
    --baseline "$baseline_report" \
    --format json \
    --sort file \
    --output "$delta_report"

blocking_count=$(jq '[.entries[] | select((.status == "new" or .status == "regressed") and .crap > 10)] | length' "$delta_report")

if [ "$blocking_count" -ne 0 ]; then
    printf '%s\n' \
        '{' \
        '  "gate": "crap",' \
        '  "result": "blocked",' \
        '  "warning_threshold": 5,' \
        '  "blocking_threshold": 10,' \
        '  "policy": "new or regressed functions above 10 block; existing debt is ratcheted",' \
        '  "why": "CRAP combines decision complexity with missing test coverage.",' \
        '  "remediation": "Reduce decision complexity and add focused tests for uncovered branches.",' \
        '  "evidence_report": "target/quality/crap-delta.json"' \
        '}' >&2
    jq -r '.entries[] | select((.status == "new" or .status == "regressed") and .crap > 10) | "BLOCK \(.file):\(.line) \(.function) CRAP=\(.crap) status=\(.status)"' "$delta_report" >&2
fi

# Emit one agent-readable annotation for every function above the warning
# threshold, even when no function crosses the blocking threshold.
cargo crap \
    --lcov "$coverage_report" \
    --exclude 'third_party/**' \
    --threshold "$warning_threshold" \
    --format github >&2

if [ "$blocking_count" -ne 0 ]; then
    exit 1
fi

printf 'CRAP gate passed: warn above %s, block above %s, report=%s\n' \
    "$warning_threshold" "$blocking_threshold" "$crap_report"
