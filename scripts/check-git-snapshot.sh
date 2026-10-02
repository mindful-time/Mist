#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
mode=${1:?Expected index or commit}
case "$mode" in
    index) snapshot_id=$(git write-tree) ;;
    commit) snapshot_id=$(git rev-parse --verify "${2:?Expected commit}^{commit}") ;;
    *) printf '%s\n' 'Expected index or commit.' >&2; exit 2 ;;
esac

temporary_root=$(mktemp -d "${TMPDIR:-/tmp}/mist-quality.XXXXXX")
trap 'rm -r "$temporary_root"' 0
trap 'exit 130' INT
trap 'exit 143' HUP TERM
snapshot="$temporary_root/snapshot"
report_directory="$project_root/target/quality/$mode-$snapshot_id"
mkdir -p "$report_directory"

# Keep inherited GIT_INDEX_FILE/GIT_DIR for capturing the original index, then
# remove local Git environment before operating on a disposable shared clone.
for variable in $(git rev-parse --local-env-vars); do unset "$variable"; done
git -c core.hooksPath=/dev/null clone --quiet --local --shared --no-checkout "$project_root" "$snapshot"
git -C "$snapshot" config core.hooksPath /dev/null
if [ "$mode" = index ]; then
    git -C "$snapshot" read-tree "$snapshot_id"
    git -C "$snapshot" checkout-index --all
else
    git -C "$snapshot" checkout --quiet --detach "$snapshot_id"
fi

MIST_QUALITY_REPORT_DIR=$report_directory
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$project_root/target}
export MIST_QUALITY_REPORT_DIR CARGO_TARGET_DIR
printf 'Mist quality gate: %s snapshot %s (reports: %s)\n' "$mode" "$snapshot_id" "$report_directory"
cd "$snapshot"
sh scripts/check-quality.sh "$mode" "${3:-}"
