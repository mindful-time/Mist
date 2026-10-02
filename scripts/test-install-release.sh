#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
installer="$project_root/scripts/install-release.sh"

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

assert_equal() {
    expected=$1
    actual=$2
    description=$3
    if [ "$actual" != "$expected" ]; then
        printf 'FAIL: %s\n  expected: %s\n  actual:   %s\n' \
            "$description" "$expected" "$actual" >&2
        exit 1
    fi
}

assert_contains() {
    file=$1
    expected=$2
    description=$3
    if ! grep -F "$expected" "$file" >/dev/null 2>&1; then
        printf 'FAIL: %s\n  missing: %s\n' "$description" "$expected" >&2
        exit 1
    fi
}

checksum() {
    file=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" | awk '{ print $1 }'
    else
        shasum -a 256 "$file" | awk '{ print $1 }'
    fi
}

line_count() {
    awk 'END { print NR + 0 }' "$1"
}

test_root=$(mktemp -d "${TMPDIR:-/tmp}/mist-release-test.XXXXXX")
case "$test_root" in
    "${TMPDIR:-/tmp}"/mist-release-test.*) ;;
    *) fail "mktemp returned an unexpected path: $test_root" ;;
esac
cleanup() {
    rm -rf "$test_root"
}
trap cleanup 0
trap 'exit 129' 1
trap 'exit 130' 2
trap 'exit 143' 15

fixture_directory="$test_root/release"
fake_bin="$test_root/bin"
install_directory="$test_root/install"
binary_directory="$test_root/user-bin"
data_directory="$test_root/data"
home_directory="$test_root/home"
temporary_directory="$test_root/tmp"
extract_log="$test_root/extract.log"
run_log="$test_root/run.log"
curl_log="$test_root/curl.log"
mkdir -p "$fixture_directory" "$fake_bin" "$home_directory" "$temporary_directory"
: > "$extract_log"
: > "$run_log"
: > "$curl_log"

# The fake tools keep this test Linux-specific even when it runs on a macOS
# development machine. The mv shim implements the GNU -T operation used by the
# Linux installer, while delegating every other invocation to the host mv.
{
    printf '%s\n' '#!/bin/sh'
    printf '%s\n' 'case "${1:-}" in'
    printf '%s\n' '    -s) printf '\''%s\n'\'' Linux ;;'
    printf '%s\n' '    -m) printf '\''%s\n'\'' x86_64 ;;'
    printf '%s\n' '    *) printf '\''%s\n'\'' Linux ;;'
    printf '%s\n' 'esac'
} > "$fake_bin/uname"

{
    printf '%s\n' '#!/bin/sh'
    printf '%s\n' 'set -eu'
    printf '%s\n' 'output='
    printf '%s\n' 'url='
    printf '%s\n' 'while [ "$#" -gt 0 ]; do'
    printf '%s\n' '    case "$1" in'
    printf '%s\n' '        --output) shift; output=$1 ;;'
    printf '%s\n' '        https://*) url=$1 ;;'
    printf '%s\n' '    esac'
    printf '%s\n' '    shift'
    printf '%s\n' 'done'
    printf '%s\n' '[ -n "$output" ] && [ -n "$url" ]'
    printf '%s\n' 'name=${url##*/}'
    printf '%s\n' 'printf '\''%s\n'\'' "$name" >> "$MIST_TEST_CURL_LOG"'
    printf '%s\n' '[ -f "$MIST_TEST_RELEASE_DIR/$name" ]'
    printf '%s\n' '/bin/cp "$MIST_TEST_RELEASE_DIR/$name" "$output"'
} > "$fake_bin/curl"

{
    printf '%s\n' '#!/bin/sh'
    printf '%s\n' 'set -eu'
    printf '%s\n' 'if [ "${1:-}" = -fT ] && [ "$#" -eq 3 ]; then'
    printf '%s\n' '    source=$2'
    printf '%s\n' '    destination=$3'
    printf '%s\n' '    /bin/rm -f "$destination"'
    printf '%s\n' '    exec /bin/mv "$source" "$destination"'
    printf '%s\n' 'fi'
    printf '%s\n' 'exec /bin/mv "$@"'
} > "$fake_bin/mv"
chmod 755 "$fake_bin/uname" "$fake_bin/curl" "$fake_bin/mv"

write_appimage() {
    version=$1
    artifact="$fixture_directory/Mist-linux-x86_64.AppImage"
    {
        printf '%s\n' '#!/bin/sh'
        printf '%s\n' 'set -eu'
        printf 'version=%s\n' "$version"
        printf '%s\n' '[ "${1:-}" = --appimage-extract ]'
        printf '%s\n' 'printf '\''%s\n'\'' "$version" >> "$MIST_TEST_EXTRACT_LOG"'
        printf '%s\n' 'mkdir -p squashfs-root'
        printf '%s\n' 'printf '\''%s\n'\'' "$version" > squashfs-root/version'
        printf '%s\n' '{'
        printf '%s\n' '    printf '\''%s\n'\'' '\''#!/bin/sh'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''set -eu'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''version=$(sed -n "1p" "$app_dir/version")'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''printf "Mist fixture %s" "$version"'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''for argument do printf " <%s>" "$argument"; done'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''printf "\\n"'\'''
        printf '%s\n' '    printf '\''%s\n'\'' '\''printf "%s\\n" "$version" >> "$MIST_TEST_RUN_LOG"'\'''
        printf '%s\n' '} > squashfs-root/AppRun'
        printf '%s\n' 'chmod 755 squashfs-root/AppRun'
    } > "$artifact"
    chmod 755 "$artifact"
}

write_checksums() {
    artifact_checksum=$(checksum "$fixture_directory/Mist-linux-x86_64.AppImage")
    icon_checksum=$(checksum "$fixture_directory/Mist.png")
    {
        printf '%s  %s\n' "$artifact_checksum" Mist-linux-x86_64.AppImage
        printf '%s  %s\n' "$icon_checksum" Mist.png
    } > "$fixture_directory/SHA256SUMS"
}

run_installer() {
    HOME="$home_directory" \
    PATH="$fake_bin:/usr/bin:/bin:/usr/sbin:/sbin" \
    TMPDIR="$temporary_directory" \
    XDG_DATA_HOME="$data_directory" \
    MIST_INSTALL_DIR="$install_directory" \
    MIST_BIN_DIR="$binary_directory" \
    MIST_RELEASE_BASE_URL=https://fixtures.invalid/mist \
    MIST_TEST_RELEASE_DIR="$fixture_directory" \
    MIST_TEST_EXTRACT_LOG="$extract_log" \
    MIST_TEST_CURL_LOG="$curl_log" \
    sh "$installer"
}

printf '%s\n' 'fixture icon' > "$fixture_directory/Mist.png"
write_appimage v1
write_checksums
printf '%s\n' '# checksum mismatch' >> "$fixture_directory/Mist-linux-x86_64.AppImage"

if run_installer > "$test_root/bad-checksum.out" 2>&1; then
    fail 'installer accepted an AppImage whose checksum did not match'
fi
assert_contains "$test_root/bad-checksum.out" \
    'Checksum verification failed for Mist-linux-x86_64.AppImage.' \
    'checksum mismatch has a clear diagnostic'
[ ! -e "$install_directory" ] || fail 'checksum failure changed the installation directory'
assert_equal 0 "$(line_count "$extract_log")" 'checksum failure happens before extraction'

write_appimage v1
write_checksums
v1_checksum=$(checksum "$fixture_directory/Mist-linux-x86_64.AppImage")
run_installer > "$test_root/install-v1.out"

v1_release="$install_directory/releases/$v1_checksum"
launcher="$binary_directory/mist"
[ -x "$v1_release/squashfs-root/AppRun" ] || fail 'installer did not extract AppRun'
[ -x "$launcher" ] || fail 'installer did not create an executable launcher'
assert_equal "$v1_release" "$(readlink "$install_directory/current")" \
    'current points at the verified v1 release'
assert_equal 1 "$(line_count "$extract_log")" 'AppImage is extracted once during installation'

MIST_TEST_RUN_LOG="$run_log" "$launcher" 'hello world' --flag > "$test_root/run-v1.out"
assert_equal 'Mist fixture v1 <hello world> <--flag>' \
    "$(sed -n '1p' "$test_root/run-v1.out")" \
    'launcher executes the extracted AppRun with unchanged arguments'
assert_equal v1 "$(sed -n '1p' "$run_log")" 'v1 AppRun recorded its execution'
assert_equal 1 "$(line_count "$extract_log")" 'launching Mist does not re-extract the AppImage'
if grep -F APPIMAGE_EXTRACT_AND_RUN "$launcher" >/dev/null 2>&1; then
    fail 'launcher still enables per-launch AppImage extraction'
fi

# Reinstalling the same release repairs damage anywhere in the extracted
# payload, not only damage to AppRun.
printf '%s\n' damaged > "$v1_release/squashfs-root/version"
printf '%s\n' unexpected > "$v1_release/squashfs-root/unexpected-file"
run_installer > "$test_root/reinstall-v1.out"
assert_equal "$v1_release" "$(readlink "$install_directory/current")" \
    'same-version reinstall keeps the current release'
[ ! -e "$v1_release/squashfs-root/unexpected-file" ] || \
    fail 'same-version reinstall kept an unexpected extracted payload file'
assert_equal 2 "$(line_count "$extract_log")" \
    'same-version repair performs exactly one staging extraction'
MIST_TEST_RUN_LOG="$run_log" "$launcher" repaired > "$test_root/run-repaired-v1.out"
assert_equal 'Mist fixture v1 <repaired>' "$(sed -n '1p' "$test_root/run-repaired-v1.out")" \
    'same-version reinstall repairs a corrupted extracted payload'

write_appimage v2
write_checksums
v2_checksum=$(checksum "$fixture_directory/Mist-linux-x86_64.AppImage")
[ "$v2_checksum" != "$v1_checksum" ] || fail 'update fixture did not change its checksum'
run_installer > "$test_root/install-v2.out"

v2_release="$install_directory/releases/$v2_checksum"
[ -d "$v1_release" ] || fail 'update removed the previous verified release'
[ -x "$v2_release/squashfs-root/AppRun" ] || fail 'update did not install the new AppRun'
assert_equal "$v2_release" "$(readlink "$install_directory/current")" \
    'update atomically selects the checksum-addressed v2 release'
MIST_TEST_RUN_LOG="$run_log" "$launcher" updated > "$test_root/run-v2.out"
assert_equal 'Mist fixture v2 <updated>' "$(sed -n '1p' "$test_root/run-v2.out")" \
    'stable launcher follows current to the updated release'
assert_equal 3 "$(line_count "$extract_log")" 'update performs one extraction'
assert_equal v2 "$(sed -n '3p' "$run_log")" 'updated AppRun recorded its execution'

assert_contains "$curl_log" SHA256SUMS 'installer downloaded the checksum manifest'
assert_contains "$curl_log" Mist-linux-x86_64.AppImage 'installer downloaded the AppImage'
assert_contains "$curl_log" Mist.png 'installer downloaded and verified the icon'

printf '%s\n' 'Linux release installer smoke test passed.'
