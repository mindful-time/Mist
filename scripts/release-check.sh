#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"

release_version=$(tr -d '[:space:]' < VERSION)
cargo_version=$(awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = "/ {
        value = $0
        sub(/^version = "/, "", value)
        sub(/".*/, "", value)
        print value
        exit
    }
' Cargo.toml)
bundle_version=$(awk '
    /<key>CFBundleShortVersionString<\/key>/ { getline; value = $0; sub(/.*<string>/, "", value); sub(/<\/string>.*/, "", value); print value; exit }
' macos/Info.plist)

if ! printf '%s\n' "$release_version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$'; then
    printf '%s\n' "VERSION must contain a semantic version; found '$release_version'." >&2
    exit 1
fi

if [ "$cargo_version" != "$release_version" ]; then
    printf '%s\n' "Cargo.toml version '$cargo_version' does not match VERSION '$release_version'." >&2
    exit 1
fi

if [ "$bundle_version" != "$release_version" ]; then
    printf '%s\n' "macOS bundle version '$bundle_version' does not match VERSION '$release_version'." >&2
    exit 1
fi

cargo metadata --locked --no-deps --format-version 1 >/dev/null
printf '%s\n' "Mist release version $release_version is synchronized."
