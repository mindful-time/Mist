#!/bin/zsh
set -euo pipefail

project_root="${0:A:h:h}"
app_bundle="$project_root/dist/Mist.app"
contents="$app_bundle/Contents"
signing_identity="${MIST_SIGNING_IDENTITY:-${SELECT_TO_SPEAK_SIGNING_IDENTITY:--}}"

cd "$project_root"
cargo build --release

mkdir -p "$contents/MacOS" "$contents/Resources"
cp "$project_root/target/release/mist" "$contents/MacOS/mist"
cp "$project_root/macos/Info.plist" "$contents/Info.plist"

/usr/bin/codesign --force --deep --sign "$signing_identity" "$app_bundle"
echo "$app_bundle"
