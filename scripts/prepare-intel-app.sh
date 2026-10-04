#!/bin/sh
set -eu
app=${1:?Expected an Intel Mist.app path}
runtime_source=${MIST_INTEL_ORT_SOURCE:?Build the pinned Intel ONNX Runtime first}
binary="$app/Contents/MacOS/mist"
test "$(lipo -archs "$binary")" = x86_64
test "$(vtool -show-build "$binary" | awk '$1 == "minos" { print $2 }')" = 13.3

# A static runtime must not leave dependencies on the runner or its build cache.
library_output=$(otool -L "$binary")
dependencies=$(printf '%s\n' "$library_output" | awk 'NR > 1 && $1 !~ /^\/System\/Library\// && $1 !~ /^\/usr\/lib\// { print $1 }')
if [ -n "$dependencies" ]; then
    printf 'Intel app has non-system dynamic dependencies:\n%s\n' "$dependencies" >&2
    exit 1
fi
/usr/libexec/PlistBuddy -c 'Set :LSMinimumSystemVersion 13.3' "$app/Contents/Info.plist"
licenses="$app/Contents/Resources/licenses"
mkdir -p "$licenses"
cp "$runtime_source/LICENSE" "$licenses/ONNX-Runtime-LICENSE"
cp "$runtime_source/ThirdPartyNotices.txt" "$licenses/ONNX-Runtime-ThirdPartyNotices.txt"
cp "${ORT_LIB_PATH:?Missing the Intel runtime build path}/provenance.txt" "$licenses/ONNX-Runtime-provenance.txt"
