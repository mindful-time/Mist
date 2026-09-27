#!/bin/sh
set -eu

project_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
binary_directory="${HOME}/.local/bin"
desktop_directory="${XDG_DATA_HOME:-${HOME}/.local/share}/applications"

cd "$project_root"
cargo build --release

mkdir -p "$binary_directory" "$desktop_directory"
cp "$project_root/target/release/mist" "$binary_directory/mist"
if [ -f "$binary_directory/select-to-speak" ]; then
  rm -f "$binary_directory/select-to-speak"
  echo "Removed the legacy select-to-speak executable."
fi
cp "$project_root/linux/dev.akshobhya.SelectToSpeak.desktop" \
  "$desktop_directory/dev.akshobhya.SelectToSpeak.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$desktop_directory"
fi

echo "Installed Mist in $binary_directory"
echo "Select text, then press the shortcut granted by your desktop."
