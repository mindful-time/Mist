#!/bin/sh
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
node --test scripts/test-git-hooks.mjs scripts/test-crap-policy.mjs scripts/test-package-managers.mjs scripts/test-preview-release.mjs scripts/test-release-ci.mjs scripts/test-release-channel.mjs scripts/test-site.mjs
uv run --no-project --locked --script scripts/test-workflows.py
sh scripts/test-install-release.sh
sh scripts/test-intel-runtime.sh
for script in scripts/*.sh .githooks/*; do sh -n "$script"; done
# PowerShell trust-decision tests also run natively in Windows CI.
if command -v pwsh >/dev/null 2>&1; then
    pwsh -NoProfile -File scripts/test-chocolatey.ps1
fi
