#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

# Git supplies the exact revisions being pushed on stdin. Never substitute
# HEAD or the local index: either can differ from an explicitly pushed ref.
while read -r local_ref local_oid remote_ref remote_oid; do
    for oid in "$local_oid" "$remote_oid"; do
        case "$oid" in ''|*[!0-9a-f]*) printf '%s\n' 'Malformed push revision.' >&2; exit 2 ;; esac
        case "${#oid}" in 40|64) ;; *) exit 2 ;; esac
    done
    case "$local_oid" in *[!0]*) ;; *) continue ;; esac
    sh "$project_root/scripts/check-git-snapshot.sh" commit "$local_oid" "$remote_oid"
done
