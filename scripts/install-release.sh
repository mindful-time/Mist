#!/bin/sh
set -eu

repository=${MIST_REPOSITORY:-mindful-time/Mist}
release=${MIST_RELEASE:-latest}
release_base_url=${MIST_RELEASE_BASE_URL:-}
download_only=${MIST_DOWNLOAD_ONLY:-0}

if ! command -v curl >/dev/null 2>&1; then
    printf '%s\n' "Mist installer requires curl." >&2
    exit 1
fi

case "$release" in
    latest)
        release_path=latest/download
        ;;
    v*)
        release_path="download/$release"
        ;;
    *)
        release_path="download/v$release"
        ;;
esac

if [ -z "$release_base_url" ]; then
    release_base_url="https://github.com/$repository/releases/$release_path"
fi
release_base_url=${release_base_url%/}

platform=$(uname -s)
architecture=$(uname -m)
if [ "$platform" = Darwin ] && [ "$architecture" = x86_64 ]; then
    if [ "$(sysctl -in sysctl.proc_translated 2>/dev/null || true)" = 1 ] ||
        [ "$(sysctl -in hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
        architecture=arm64
    fi
fi
case "$platform:$architecture" in
    Darwin:arm64)
        artifact=Mist-macos-aarch64.dmg
        ;;
    Darwin:x86_64)
        artifact=Mist-macos-x86_64.dmg
        ;;
    Linux:x86_64|Linux:amd64)
        artifact=Mist-linux-x86_64.AppImage
        ;;
    *)
        printf '%s\n' "Mist does not yet publish an installer for $platform/$architecture." >&2
        exit 1
        ;;
esac

temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/mist-installer.XXXXXX")
mounted_image=
staged_application=
backup_application=
application_destination=
staged_appimage=
staged_icon=
staged_desktop=
staged_launcher=
staged_release_directory=
staged_current=
cleanup() {
    if [ -n "$mounted_image" ]; then
        hdiutil detach "$mounted_image" -quiet >/dev/null 2>&1 || true
    fi
    if [ -n "$backup_application" ] && [ -e "$backup_application" ]; then
        if [ -n "$application_destination" ] && [ ! -e "$application_destination" ]; then
            mv "$backup_application" "$application_destination" >/dev/null 2>&1 || true
        fi
    fi
    if [ -n "$staged_application" ]; then
        rm -rf "$staged_application"
    fi
    for staged_path in "$staged_appimage" "$staged_icon" "$staged_desktop" "$staged_launcher"; do
        if [ -n "$staged_path" ]; then
            rm -f "$staged_path"
        fi
    done
    if [ -n "$staged_current" ]; then
        rm -f "$staged_current"
    fi
    if [ -n "$staged_release_directory" ]; then
        rm -rf "$staged_release_directory"
    fi
    rm -rf "$temporary_directory"
}
trap cleanup 0
trap 'exit 129' 1
trap 'exit 130' 2
trap 'exit 143' 15

download() {
    remote_name=$1
    local_path=$2
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
        "$release_base_url/$remote_name" --output "$local_path"
}

checksum() {
    checksum_path=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$checksum_path" | awk '{ print $1 }'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$checksum_path" | awk '{ print $1 }'
    else
        printf '%s\n' "Mist installer requires sha256sum or shasum." >&2
        exit 1
    fi
}

escape_double_quoted() {
    printf '%s' "$1" | sed \
        -e 's/\\/\\\\/g' \
        -e 's/"/\\"/g' \
        -e 's/`/\\`/g' \
        -e 's/\$/\\$/g'
}

checksums="$temporary_directory/SHA256SUMS"
artifact_path="$temporary_directory/$artifact"
download SHA256SUMS "$checksums"
download "$artifact" "$artifact_path"

expected_checksum=$(awk -v artifact="$artifact" '
    $2 == artifact || $2 == "*" artifact { print $1; exit }
' "$checksums")
if [ -z "$expected_checksum" ]; then
    printf '%s\n' "SHA256SUMS does not contain $artifact." >&2
    exit 1
fi

actual_checksum=$(checksum "$artifact_path")

if [ "$actual_checksum" != "$expected_checksum" ]; then
    printf '%s\n' "Checksum verification failed for $artifact." >&2
    exit 1
fi

if [ "$download_only" = 1 ]; then
    destination=${MIST_DOWNLOAD_DIR:-$(pwd)}
    mkdir -p "$destination"
    cp "$artifact_path" "$destination/$artifact"
    printf '%s\n' "Downloaded and verified $destination/$artifact"
    exit 0
fi

case "$platform" in
    Darwin)
        install_directory=${MIST_INSTALL_DIR:-"$HOME/Applications"}
        case "$install_directory" in
            ""|/|.|..)
                printf '%s\n' "Refusing unsafe MIST_INSTALL_DIR '$install_directory'." >&2
                exit 1
                ;;
        esac
        mounted_image="$temporary_directory/mount"
        mkdir -p "$mounted_image" "$install_directory"
        hdiutil attach "$artifact_path" -nobrowse -readonly -mountpoint "$mounted_image" >/dev/null
        if [ ! -d "$mounted_image/Mist.app" ]; then
            printf '%s\n' "The downloaded image does not contain Mist.app." >&2
            exit 1
        fi
        staged_application="$install_directory/.Mist.new.$$.app"
        backup_application="$install_directory/.Mist.old.$$.app"
        application_destination="$install_directory/Mist.app"
        rm -rf "$staged_application" "$backup_application"
        /usr/bin/ditto "$mounted_image/Mist.app" "$staged_application"
        codesign --verify --deep --strict "$staged_application"
        spctl --assess --type execute "$staged_application"
        expected_apple_team_id=${MIST_EXPECTED_APPLE_TEAM_ID:-__MIST_APPLE_TEAM_ID__}
        if [ "${expected_apple_team_id#__MIST_}" != "$expected_apple_team_id" ]; then
            printf '%s\n' "The Mist installer is missing its Apple signing identity." >&2
            exit 1
        fi
        actual_apple_team_id=$(codesign -dv --verbose=4 "$staged_application" 2>&1 |
            awk -F= '$1 == "TeamIdentifier" { print $2; exit }')
        if [ "$actual_apple_team_id" != "$expected_apple_team_id" ]; then
            printf '%s\n' "The downloaded app is not signed by the expected Apple team." >&2
            exit 1
        fi
        bundle_identifier=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' \
            "$staged_application/Contents/Info.plist")
        if [ "$bundle_identifier" != dev.akshobhya.select-to-speak ]; then
            printf '%s\n' "The downloaded app has an unexpected bundle identifier." >&2
            exit 1
        fi
        trap '' 1 2 15
        if [ -e "$application_destination" ]; then
            mv "$application_destination" "$backup_application"
        fi
        if mv "$staged_application" "$application_destination"; then
            staged_application=
            rm -rf "$backup_application"
            backup_application=
        else
            if [ -e "$backup_application" ]; then
                mv "$backup_application" "$application_destination"
            fi
            exit 1
        fi
        trap 'exit 129' 1
        trap 'exit 130' 2
        trap 'exit 143' 15
        hdiutil detach "$mounted_image" -quiet
        mounted_image=
        printf '%s\n' "Installed Mist at $install_directory/Mist.app"
        printf '%s\n' "Open Mist once and grant Accessibility permission when macOS asks."
        ;;
    Linux)
        application_directory=${MIST_INSTALL_DIR:-"$HOME/.local/lib/mist"}
        binary_directory=${MIST_BIN_DIR:-"$HOME/.local/bin"}
        desktop_directory=${XDG_DATA_HOME:-"$HOME/.local/share"}/applications
        icon_directory=${XDG_DATA_HOME:-"$HOME/.local/share"}/icons/hicolor/512x512/apps
        releases_directory="$application_directory/releases"
        release_directory="$releases_directory/$actual_checksum"
        current_path="$application_directory/current"
        application_path="$current_path/squashfs-root/AppRun"
        icon_name=dev.akshobhya.SelectToSpeak.png

        icon_path="$temporary_directory/$icon_name"
        download Mist.png "$icon_path"
        expected_icon_checksum=$(awk '$2 == "Mist.png" || $2 == "*Mist.png" { print $1; exit }' "$checksums")
        actual_icon_checksum=$(checksum "$icon_path")
        if [ -z "$expected_icon_checksum" ] || [ "$actual_icon_checksum" != "$expected_icon_checksum" ]; then
            printf '%s\n' "Checksum verification failed for Mist.png." >&2
            exit 1
        fi

        desktop_file="$desktop_directory/dev.akshobhya.SelectToSpeak.desktop"
        launcher_path="$binary_directory/mist"
        icon_destination="$icon_directory/$icon_name"
        for destination in "$desktop_file" "$launcher_path" "$icon_destination"; do
            if [ -d "$destination" ]; then
                printf '%s\n' "Refusing to replace directory $destination." >&2
                exit 1
            fi
        done
        if [ -e "$launcher_path" ] || [ -L "$launcher_path" ]; then
            if [ ! -f "$launcher_path" ] ||
                ! grep -Fqx '# Managed by the Mist installer.' "$launcher_path"; then
                printf '%s\n' "Refusing to replace unmanaged file $launcher_path." >&2
                exit 1
            fi
        fi
        if [ -e "$desktop_file" ] || [ -L "$desktop_file" ]; then
            if [ ! -f "$desktop_file" ] ||
                ! grep -Fqx 'X-Mist-Managed=true' "$desktop_file"; then
                printf '%s\n' "Refusing to replace unmanaged file $desktop_file." >&2
                exit 1
            fi
        fi
        if [ -e "$icon_destination" ] || [ -L "$icon_destination" ]; then
            if [ ! -L "$icon_destination" ] ||
                [ "$(readlink "$icon_destination")" != "$current_path/Mist.png" ]; then
                printf '%s\n' "Refusing to replace unmanaged file $icon_destination." >&2
                exit 1
            fi
        fi
        if [ -L "$current_path" ]; then
            case "$(readlink "$current_path")" in
                "$releases_directory"/*) ;;
                *)
                    printf '%s\n' "Refusing to replace unmanaged link $current_path." >&2
                    exit 1
                    ;;
            esac
        elif [ -e "$current_path" ]; then
            printf '%s\n' "Refusing to replace unmanaged path $current_path." >&2
            exit 1
        fi
        if [ -e "$release_directory" ] && [ ! -d "$release_directory" ]; then
            printf '%s\n' "Refusing to replace file $release_directory." >&2
            exit 1
        fi

        mkdir -p "$releases_directory" "$binary_directory" "$desktop_directory" "$icon_directory"
        staged_release_directory="$releases_directory/.new.$$"
        mkdir "$staged_release_directory"
        staged_appimage="$staged_release_directory/Mist.AppImage"
        staged_icon="$staged_release_directory/Mist.png"
        cp "$artifact_path" "$staged_appimage"
        chmod 755 "$staged_appimage"
        cp "$icon_path" "$staged_icon"
        staged_appimage=
        staged_icon=

        (
            unset APPIMAGE_EXTRACT_AND_RUN
            cd "$staged_release_directory"
            ./Mist.AppImage --appimage-extract >/dev/null
        )
        staged_apprun="$staged_release_directory/squashfs-root/AppRun"
        if [ ! -x "$staged_apprun" ]; then
            printf '%s\n' "The downloaded AppImage does not contain an executable AppRun." >&2
            exit 1
        fi
        expected_apprun_checksum=$(checksum "$staged_apprun")

        if [ -d "$release_directory" ]; then
            if [ ! -x "$release_directory/Mist.AppImage" ] ||
                [ "$(checksum "$release_directory/Mist.AppImage")" != "$actual_checksum" ] ||
                [ "$(checksum "$release_directory/Mist.png")" != "$actual_icon_checksum" ] ||
                [ ! -x "$release_directory/squashfs-root/AppRun" ] ||
                [ "$(checksum "$release_directory/squashfs-root/AppRun")" != "$expected_apprun_checksum" ]; then
                printf '%s\n' "Existing Mist release directory failed verification." >&2
                exit 1
            fi
            rm -rf "$staged_release_directory"
        else
            mv "$staged_release_directory" "$release_directory"
        fi
        staged_release_directory=

        staged_launcher="$binary_directory/.mist.new.$$"
        staged_icon="$icon_directory/.${icon_name}.new.$$"
        staged_desktop="$desktop_directory/.dev.akshobhya.SelectToSpeak.desktop.new.$$"
        staged_current="$application_directory/.current.new.$$"
        escaped_application=$(escape_double_quoted "$application_path")
        {
            printf '%s\n' '#!/bin/sh'
            printf '%s\n' '# Managed by the Mist installer.'
            printf 'exec "%s" "$@"\n' "$escaped_application"
        } > "$staged_launcher"
        chmod 755 "$staged_launcher"
        ln -s "$current_path/Mist.png" "$staged_icon"
        escaped_launcher=$(escape_double_quoted "$launcher_path" | sed 's/%/%%/g')
        {
            printf '%s\n' '[Desktop Entry]'
            printf '%s\n' 'Type=Application'
            printf '%s\n' 'Name=Mist'
            printf '%s\n' 'Comment=Speak selected text locally with Kokoro'
            printf 'Exec="%s"\n' "$escaped_launcher"
            printf '%s\n' 'Icon=dev.akshobhya.SelectToSpeak'
            printf '%s\n' 'Terminal=false'
            printf '%s\n' 'Categories=Accessibility;Utility;'
            printf '%s\n' 'X-Mist-Managed=true'
        } > "$staged_desktop"
        ln -s "$release_directory" "$staged_current"

        mv -fT "$staged_launcher" "$launcher_path"
        staged_launcher=
        mv -fT "$staged_icon" "$icon_destination"
        staged_icon=
        mv -fT "$staged_desktop" "$desktop_file"
        staged_desktop=
        mv -fT "$staged_current" "$current_path"
        staged_current=

        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database "$desktop_directory" >/dev/null 2>&1 || true
        fi
        printf '%s\n' "Installed Mist at $application_path"
        case ":$PATH:" in
            *":$binary_directory:"*) ;;
            *) printf '%s\n' "Add $binary_directory to PATH to run 'mist' from a terminal." ;;
        esac
        ;;
esac
