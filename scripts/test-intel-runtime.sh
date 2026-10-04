#!/bin/sh
set -eu

if [ "$(uname -s)" != Darwin ]; then
    printf '%s\n' 'SKIP: Intel runtime archive regression requires macOS developer tools.'
    exit 0
fi

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
producer="$project_root/scripts/link-intel-onnxruntime.sh"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/mist-intel-runtime-test.XXXXXX")
case "$test_root" in
    "${TMPDIR:-/tmp}"/mist-intel-runtime-test.*) ;;
    *) printf '%s\n' 'FAIL: unexpected temporary directory' >&2; exit 1 ;;
esac
test_root=$(CDPATH= cd -- "$test_root" && pwd)
trap 'rm -rf "$test_root"' 0
trap 'exit 129' 1
trap 'exit 130' 2
trap 'exit 143' 15

configuration="$test_root/Release fixture"
dependency="$configuration/_deps/fixture-dependency-build/component"
mkdir -p "$configuration/model_package" "$dependency"

# Use actual Intel Mach-O objects and archives, even on an Apple Silicon host.
for library in common flatbuffers framework graph lora mlas optimizer providers util; do
    printf 'int fixture_%s(void) { return 0; }\n' "$library" |
        xcrun clang -arch x86_64 -mmacosx-version-min=13.3 -x c -c - \
            -o "$test_root/$library.o"
    xcrun libtool -static -o "$configuration/libonnxruntime_$library.a" \
        "$test_root/$library.o"
done

printf '%s\n' 'int fixture_dependency(void) { return 42; }' |
    xcrun clang -arch x86_64 -mmacosx-version-min=13.3 -x c -c - \
        -o "$test_root/dependency.o"
xcrun libtool -static -o "$dependency/libfixture_dependency.a" "$test_root/dependency.o"

printf '%s\n' \
    'extern int fixture_dependency(void);' \
    'int fixture_model_package(void) { return fixture_dependency(); }' |
    xcrun clang -arch x86_64 -mmacosx-version-min=13.3 -x c -c - \
        -o "$test_root/model_package.o"
xcrun libtool -static -o "$configuration/model_package/libmodel_package.a" \
    "$test_root/model_package.o"

printf '%s\n' \
    'extern int fixture_model_package(void);' \
    'int fixture_session(void) { return fixture_model_package(); }' |
    xcrun clang -arch x86_64 -mmacosx-version-min=13.3 -x c -c - \
        -o "$test_root/session.o"
xcrun libtool -static -o "$configuration/libonnxruntime_session.a" "$test_root/session.o"

sh "$producer" "$configuration"
test "$(xcrun lipo -archs "$configuration/libonnxruntime.a")" = x86_64
grep -F "$configuration/model_package/libmodel_package.a" \
    "$configuration/runtime-archives.txt" >/dev/null
grep -F "$dependency/libfixture_dependency.a" "$configuration/runtime-archives.txt" >/dev/null

# Only the merged library is passed to the linker: unresolved transitive symbols
# must be supplied by it, not accidentally found in the producer's build tree.
printf '%s\n' \
    'extern int fixture_session(void);' \
    'int main(void) { return fixture_session() == 42 ? 0 : 1; }' |
    xcrun clang -arch x86_64 -mmacosx-version-min=13.3 -x c - -x none \
        -L "$configuration" -lonnxruntime -o "$test_root/fixture"
test "$(xcrun lipo -archs "$test_root/fixture")" = x86_64

rm "$configuration/model_package/libmodel_package.a"
if sh "$producer" "$configuration" > "$test_root/missing-model.log" 2>&1; then
    printf '%s\n' 'FAIL: runtime producer accepted a missing model-package archive.' >&2
    exit 1
fi

printf '%s\n' 'PASS: Intel static archive includes model-package and nested dependency symbols.'
