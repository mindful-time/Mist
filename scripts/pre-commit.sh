#!/bin/sh
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec sh "$project_root/scripts/check-git-snapshot.sh" index
