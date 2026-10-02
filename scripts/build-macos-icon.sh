#!/bin/zsh
set -euo pipefail

if (( $# != 1 )); then
    print -u2 "Usage: $0 /path/to/Mist.icns"
    exit 2
fi

project_root="${0:A:h:h}"
output_path="${1:A}"
source_icon="$project_root/assets/mist-orb-v1.png"
icon_temp_dir="$(mktemp -d /private/tmp/mist-icon.XXXXXX)"
trap 'rm -r -- "$icon_temp_dir"' EXIT
iconset="$icon_temp_dir/Mist.iconset"
mkdir "$iconset"

for size in 16 32 128 256 512; do
    /usr/bin/sips -z "$size" "$size" "$source_icon" \
        --out "$iconset/icon_${size}x${size}.png" >/dev/null
    retina_size=$(( size * 2 ))
    /usr/bin/sips -z "$retina_size" "$retina_size" "$source_icon" \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done

/usr/bin/iconutil --convert icns "$iconset" --output "$output_path"
print -r -- "$output_path"
