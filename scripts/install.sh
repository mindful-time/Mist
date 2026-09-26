#!/bin/zsh
set -euo pipefail

project_root="${0:A:h:h}"
app_bundle="$($project_root/scripts/build-app.sh)"
install_directory="$HOME/Applications"

mkdir -p "$install_directory"
/usr/bin/ditto "$app_bundle" "$install_directory/Select to Speak.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -f "$install_directory/Select to Speak.app"

echo "Installed Select to Speak in $install_directory"
echo "Open it once, allow Accessibility access, then select text and press Ctrl+Alt+S."
