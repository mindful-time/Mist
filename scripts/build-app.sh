#!/bin/zsh
set -euo pipefail

project_root="${0:A:h:h}"
app_bundle="$project_root/dist/Select to Speak.app"
contents="$app_bundle/Contents"

cd "$project_root"
cargo build --release

mkdir -p "$contents/MacOS" "$contents/Resources"
cp "$project_root/target/release/select-to-speak" "$contents/MacOS/select-to-speak"
cp "$project_root/macos/Info.plist" "$contents/Info.plist"

/usr/bin/codesign --force --deep --sign - "$app_bundle"
echo "$app_bundle"

