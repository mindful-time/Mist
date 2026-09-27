#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"

git config core.hooksPath .githooks
printf '%s\n' 'Mist Git hooks installed from .githooks.'
