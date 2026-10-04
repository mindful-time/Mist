#!/bin/sh
set -eu
configuration=${1:?Expected the Intel runtime build configuration directory}
configuration=$(CDPATH= cd -- "$configuration" && pwd)
archive_list="$configuration/runtime-archives.txt"
dependency_list="$configuration/dependency-archives.txt"

# ort-sys supports one complete archive, avoiding its version-sensitive
# dependency-directory guesses and hard-coded list of component libraries.
: > "$archive_list"
for library in common flatbuffers framework graph lora mlas optimizer providers session util; do
    printf '%s\n' "$configuration/libonnxruntime_$library.a" >> "$archive_list"
done
printf '%s\n' "$configuration/model_package/libmodel_package.a" >> "$archive_list"
find "$configuration/_deps" -type f -path '*-build/*' -name '*.a' > "$dependency_list"
test -s "$dependency_list"
LC_ALL=C sort "$dependency_list" >> "$archive_list"
while IFS= read -r archive; do
    test -s "$archive"
    test "$(lipo -archs "$archive")" = x86_64
done < "$archive_list"
xcrun libtool -static -D -filelist "$archive_list" -o "$configuration/libonnxruntime.a"
test -s "$configuration/libonnxruntime.a"
test "$(lipo -archs "$configuration/libonnxruntime.a")" = x86_64
