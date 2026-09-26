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
echo "Open it once, then right-click selected text and choose Services > Speak Selection with Kokoro."

