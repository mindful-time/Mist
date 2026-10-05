#!/bin/sh
set -eu

[ "$#" -eq 3 ] || { echo 'Usage: prepare-deb.sh INPUT OUTPUT VERSION' >&2; exit 1; }
input=$1
output=$2
version=$3
if ! printf '%s\n' "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-rc\.[1-9][0-9]*)?$'; then
    echo 'Expected a stable or release-candidate version.' >&2
    exit 1
fi
[ -f "$input" ] && [ ! -L "$input" ] || { echo 'Input must be a regular DEB.' >&2; exit 1; }
[ ! -e "$output" ] && [ ! -L "$output" ] || { echo 'Refusing to replace an existing artifact.' >&2; exit 1; }
[ "$(dpkg-deb --field "$input" Version)" = "$version" ] || { echo 'DEB source version does not match the release.' >&2; exit 1; }
debian_version=$(printf '%s' "$version" | sed 's/-rc\./~rc./')
case "$version" in
    *-rc.*) dpkg --compare-versions "$debian_version" lt "${version%%-rc.*}" ;;
esac

# Work next to the destination so the final hard link is atomic and no-clobber.
deb_work=$(mktemp -d "$(dirname "$output")/.mist-deb.XXXXXX")
trap 'rm -rf -- "$deb_work"' 0
dpkg-deb --raw-extract "$input" "$deb_work/package"
awk -v version="$debian_version" '
    /^Version:/ { print "Version: " version; next }
    { print }
' "$deb_work/package/DEBIAN/control" > "$deb_work/control"
mv "$deb_work/control" "$deb_work/package/DEBIAN/control"
dpkg-deb --build --root-owner-group "$deb_work/package" "$deb_work/prepared.deb" >/dev/null
[ "$(dpkg-deb --field "$deb_work/prepared.deb" Version)" = "$debian_version" ]
ln "$deb_work/prepared.deb" "$output"
printf 'Prepared DEB version %s for Mist %s.\n' "$debian_version" "$version"
