# Distribution

Mist publishes immutable, checksum-verified desktop artifacts through GitHub
Releases. Release assets use stable names so the installation commands never
need a version embedded in them.

## User installation

macOS and Linux:

```sh
(
  set -eu
  installer=$(mktemp "${TMPDIR:-/tmp}/mist-bootstrap.XXXXXX")
  trap 'rm -f "$installer"' 0
  curl --proto '=https' --tlsv1.2 -LsSf \
    https://github.com/mindful-time/Mist/releases/latest/download/mist-installer.sh \
    --output "$installer"
  sh "$installer"
)
```

Windows PowerShell:

```powershell
& ([scriptblock]::Create((irm -ErrorAction Stop https://github.com/mindful-time/Mist/releases/latest/download/mist-installer.ps1)))
```

The scripts download `SHA256SUMS` first and reject an artifact whose SHA-256
digest does not match. Users who prefer not to pipe a network response into a
shell can download the same files directly from the release page:

- `Mist-macos-aarch64.dmg`
- `Mist-macos-x86_64.dmg`
- `Mist-windows-x86_64-setup.exe`
- `Mist-linux-x86_64.AppImage`
- `Mist-linux-x86_64.deb`

The Linux artifact is an AppImage, not an Android APK. Mist's desktop selection,
global shortcut, tray, and accessibility integrations do not target Android.
The one-command installer extracts the AppImage once and launches its extracted
`AppRun`, so it does not require FUSE 2 and does not re-extract on every launch.
For a directly downloaded AppImage on a system without FUSE 2, run it with
`APPIMAGE_EXTRACT_AND_RUN=1`.

### Installer options

- `MIST_RELEASE=v0.1.0` installs a specific release instead of the latest.
- `MIST_RELEASE_BASE_URL=https://example.invalid/release` uses another asset
  host with the same file names.
- `MIST_DOWNLOAD_ONLY=1` verifies and downloads without installing.
- `MIST_DOWNLOAD_DIR=/path` selects the download destination.
- On macOS, `MIST_INSTALL_DIR` defaults to `$HOME/Applications`.
- On Linux, `MIST_INSTALL_DIR` defaults to `$HOME/.local/lib/mist` and
  `MIST_BIN_DIR` defaults to `$HOME/.local/bin`.

## Release asset contract

Every release must contain the five platform artifacts listed above, plus:

- `Mist.png`
- `mist-installer.sh`
- `mist-installer.ps1`
- `SHA256SUMS`, covering every downloadable artifact except itself

Changing one of these names is a breaking change for the stable installation
commands. Add a new asset for another architecture instead of replacing an
existing architecture with a differently built binary.

Public installation requires a public download host. Releases in a private
GitHub repository require authentication and therefore cannot support the
anonymous commands above.

## Download website

The nontechnical download page lives in `site/` and is deployed by the Pages
workflow. Build it locally with:

```sh
./scripts/build-site.sh
```

The generated site is written to `dist/site`. It detects the visitor's desktop
platform, but never guesses a Mac processor or Linux package: those choices stay
visible. It enables a link only when the latest public GitHub Release contains
the exact asset name from the release contract. Before the first public release,
it shows an explicit unavailable state instead of a broken download.

To publish the site, set **Settings → Pages → Build and deployment → Source** to
**GitHub Actions**. A push to `main` that changes the site then deploys
`https://mindful-time.github.io/Mist/`. The repository or the Pages site and its
release asset host must be public for anonymous users.

## Packaging

`Cargo.toml` holds shared `cargo-packager` metadata. Package each binary on its
native operating system:

```sh
cargo build --release --locked
cargo packager --release --formats dmg       # macOS
cargo packager --release --formats nsis      # Windows
cargo packager --release --formats deb,appimage # Linux
```

Pin `cargo-packager` to the version used by the release workflow. Do not publish
an unsigned macOS or Windows build as a stable release. macOS artifacts require
Developer ID signing, hardened runtime, timestamping, notarization, and
stapling. Windows requires Authenticode signing of both the executable and the
installer.

The release workflow requires these `release-signing` environment secrets:

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application `.p12`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID` and `APPLE_PASSWORD` (an app-specific password)
- `WINDOWS_CERTIFICATE`: base64-encoded Authenticode `.pfx`
- `WINDOWS_CERTIFICATE_PASSWORD`

It also requires these non-secret repository variables:

- `APPLE_TEAM_ID`
- `WINDOWS_SIGNER_SHA256`: the signing certificate's 64-character SHA-256
  fingerprint

`WINDOWS_TIMESTAMP_URL` is an optional repository variable. The workflow uses
the DigiCert timestamp service when it is not set.

Require reviewers on the `release-signing` and `release-publishing`
environments. Restrict `v*` tag creation to release maintainers with a
repository ruleset, and enable GitHub's immutable-releases setting before the
first public release. The workflow also refuses tags outside `main` and refuses
to modify assets after a draft has been published.

## Testing a release candidate

After the signing environments are configured, run **Actions → Release → Run
workflow** on `main`. A manual run builds, signs, notarizes, and validates every
platform artifact, then uploads `mist-release-bundle-<commit>` as a normal
Actions artifact. It does not create a tag or GitHub Release and cannot publish
anything. Use this path to fix packaging failures before choosing a version tag.

## Publishing a release

1. Synchronize `VERSION`, `Cargo.toml`, `Cargo.lock`, and the macOS bundle
   version, then run `./scripts/release-check.sh`.
2. Complete a successful manual release-candidate run for the commit on `main`.
3. Create and push the matching `v<version>` tag. The release workflow packages
   each operating system on its native runner and creates or updates a draft
   GitHub Release.
4. Confirm every file in the release asset contract is present and matches
   `SHA256SUMS`.
5. Exercise the draft on clean machines before publishing:
   - Apple Silicon and Intel macOS: Gatekeeper, first install, update rollback,
     Accessibility permission, selection, hotkey, model download, and speech.
   - Windows x64: Authenticode, install, update, uninstall, UI Automation,
     hotkey, model download, speech, and available GPU/CPU fallbacks.
   - Linux x64: AppImage and DEB install, desktop entry, X11 and Wayland
     selection, portals, hotkey, model download, speech, and CPU fallback.
6. Publish the draft only after the acceptance checks pass. Keep a failed draft
   private and fix it with a new version instead of replacing an already
   published release artifact.
