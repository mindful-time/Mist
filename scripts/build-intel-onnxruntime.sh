#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    printf '%s\n' 'Usage: build-intel-onnxruntime.sh SOURCE_DIRECTORY BUILD_DIRECTORY' >&2
    exit 2
fi
if [ "$(uname -s):$(uname -m)" != Darwin:x86_64 ]; then
    printf '%s\n' 'The Intel runtime must be built and tested on a native Intel Mac.' >&2
    exit 1
fi

runtime_source=$(CDPATH= cd -- "$1" && pwd)
mkdir -p "$2"
runtime_build=$(CDPATH= cd -- "$2" && pwd)
runtime_revision=da9b5e364c465de65c49d91e696cd6485270757f
test "$(git -C "$runtime_source" rev-parse HEAD)" = "$runtime_revision"
test -z "$(git -C "$runtime_source" status --porcelain --untracked-files=no)"
test "$(tr -d '[:space:]' < "$runtime_source/VERSION_NUMBER")" = 1.28.0

# Do not restrict operators: Kokoro uses dynamic shapes, LSTM, and STFT.
# Keep the complete CPU graph and dependency archives needed by ort-sys rc.13.
uv run --no-project --python 3.12.12 --with cmake==3.31.6 \
    python "$runtime_source/tools/ci_build/build.py" \
    --build_dir "$runtime_build" --config Release --parallel 3 \
    --update --build --skip_submodule_sync --skip_tests \
    --compile_no_warning_as_error \
    --cmake_extra_defines \
    CMAKE_OSX_ARCHITECTURES=x86_64 CMAKE_OSX_DEPLOYMENT_TARGET=13.3 \
    FETCHCONTENT_TRY_FIND_PACKAGE_MODE=NEVER \
    onnxruntime_BUILD_UNIT_TESTS=OFF onnxruntime_BUILD_SHARED_LIB=OFF \
    onnxruntime_USE_COREML=OFF

for library in common flatbuffers framework graph lora mlas optimizer providers session util; do
    archive="$runtime_build/Release/libonnxruntime_$library.a"
    test -s "$archive"
    test "$(lipo -archs "$archive")" = x86_64
done

# Retain enough provenance to identify the exact code, dependencies, and tools.
{
    printf 'ONNX Runtime 1.28.0\nSource: %s\nArchitecture: x86_64\nBackend: CPU\nDeployment target: 13.3\n' "$runtime_revision"
    xcodebuild -version
    xcrun --show-sdk-version
    xcrun clang --version
    shasum -a 256 "$runtime_source/cmake/deps.txt" "$runtime_build/Release/CMakeCache.txt"
    for archive in "$runtime_build/Release"/libonnxruntime_*.a; do
        shasum -a 256 "$archive"
    done
} > "$runtime_build/provenance.txt"
printf 'Intel CPU runtime is ready in %s\n' "$runtime_build"
