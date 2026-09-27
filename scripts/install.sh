#!/bin/zsh
set -euo pipefail

project_root="${0:A:h:h}"
signing_identity="${MIST_SIGNING_IDENTITY:-${SELECT_TO_SPEAK_SIGNING_IDENTITY:-}}"
if [[ -z "$signing_identity" ]]; then
  echo "MIST_SIGNING_IDENTITY is required for installation." >&2
  echo "Use an Apple Development or Developer ID identity so Accessibility access survives rebuilds." >&2
  exit 1
fi

app_bundle="$(MIST_SIGNING_IDENTITY="$signing_identity" "$project_root/scripts/build-app.sh")"
install_directory="${MIST_INSTALL_DIR:-${SELECT_TO_SPEAK_INSTALL_DIR:-/Applications}}"
legacy_app="$install_directory/Select to Speak.app"

mkdir -p "$install_directory"
/usr/bin/ditto "$app_bundle" "$install_directory/Mist.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -f "$install_directory/Mist.app"
if [[ -d "$legacy_app" ]]; then
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
    -u "$legacy_app" || true
  /bin/rm -rf "$legacy_app"
  echo "Removed the legacy Select to Speak app bundle."
fi

echo "Installed Mist in $install_directory"
echo "Open it once, allow Accessibility access, then select text and press Ctrl+Alt+S."
