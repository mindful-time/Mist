#!/bin/zsh
set -euo pipefail

project_root="${0:A:h:h}"
if [[ -z "${SELECT_TO_SPEAK_SIGNING_IDENTITY:-}" ]]; then
  echo "SELECT_TO_SPEAK_SIGNING_IDENTITY is required for installation." >&2
  echo "Use an Apple Development or Developer ID identity so Accessibility access survives rebuilds." >&2
  exit 1
fi

app_bundle="$($project_root/scripts/build-app.sh)"
install_directory="${SELECT_TO_SPEAK_INSTALL_DIR:-/Applications}"

mkdir -p "$install_directory"
/usr/bin/ditto "$app_bundle" "$install_directory/Select to Speak.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -f "$install_directory/Select to Speak.app"

echo "Installed Select to Speak in $install_directory"
echo "Open it once, allow Accessibility access, then select text and press Ctrl+Alt+S."
