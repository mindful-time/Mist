#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_directory="$project_root/dist/site"

rm -rf "$output_directory"
mkdir -p "$output_directory/assets"

cp "$project_root/site/index.html" "$output_directory/index.html"
cp "$project_root/site/styles.css" "$output_directory/styles.css"
cp "$project_root/site/site.js" "$output_directory/site.js"
cp "$project_root/assets/mist-v2.png" "$output_directory/assets/mist.png"
: > "$output_directory/.nojekyll"

printf '%s\n' "Built Mist website at $output_directory"
