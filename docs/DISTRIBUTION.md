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

The current Linux binaries target x86_64 desktops with an AVX2-capable CPU and
an Ubuntu 24.04 runtime baseline: glibc 2.39+ and libstdc++ providing
`GLIBCXX_3.4.32` (GCC 13.2+). The DEB declares these library minimums; the
release build rejects binaries exceeding this ABI baseline. The bundled,
locked ONNX Runtime archive failed the same linking probe on Ubuntu 22.04 and
passed on 24.04. AppImage does not bundle glibc or make Ubuntu 22.04,
Alpine/musl, or arbitrary older distributions compatible. Supporting them
requires a separately built compatible inference runtime and native testing.
([glibc symbol map](https://raw.githubusercontent.com/bminor/glibc/glibc-2.38/stdlib/Versions),
[GCC ABI mapping](https://gcc.gnu.org/onlinedocs/libstdc++/manual/abi.html),
[ORT prebuilt requirements](https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.13))

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

Publishing the website is deliberately opt-in. The Pages workflow deploys only
from public `main` when the repository variable `DOWNLOAD_SITE_ENABLED` is
`true`. Keep it unset or `false` while preparing the first release. After the
complete release passes acceptance testing and is published, enable the
variable and run **Actions → Pages → Run workflow**. This gate prevents future
deployments; it does not unpublish a site that is already deployed.

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

### Intel macOS runtime

`ort-sys` 2.0.0-rc.13 has no prebuilt Intel Mac archive. Intel builds use
ONNX Runtime 1.28.0 at source commit
`da9b5e364c465de65c49d91e696cd6485270757f`, compiled natively with the CPU
provider and static linkage. Apple Silicon keeps its existing Core ML runtime.
The shared `.github/actions/intel-onnxruntime` action prepares this runtime in
both CI and Release. Its exact cache key includes the compiler/SDK identity
and builder/action content; it does not use approximate restore keys.

For a local Intel source build, install Xcode Command Line Tools and uv, then:

```sh
git clone --filter=blob:none --no-checkout --depth=1 --branch v1.28.0 https://github.com/microsoft/onnxruntime.git target/intel-onnxruntime/source
git -C target/intel-onnxruntime/source fetch --depth=1 origin da9b5e364c465de65c49d91e696cd6485270757f
git -C target/intel-onnxruntime/source checkout --detach da9b5e364c465de65c49d91e696cd6485270757f
sh scripts/build-intel-onnxruntime.sh target/intel-onnxruntime/source target/intel-onnxruntime/build
export ORT_LIB_PATH="$PWD/target/intel-onnxruntime/build"
export ORT_LIB_PROFILE=Release
export ORT_PREFER_DYNAMIC_LINK=0
export MACOSX_DEPLOYMENT_TARGET=13.3
export MIST_INTEL_ORT_SOURCE="$PWD/target/intel-onnxruntime/source"
cargo build --release --locked
cargo packager --release --formats app
sh scripts/prepare-intel-app.sh dist/packages/Mist.app
```

The builder pins Python 3.12.12 and CMake 3.31.6, retains all CPU operators,
and disables FetchContent's installed-package fallback so dependency sources
come from the pinned runtime manifest rather than Homebrew packages.
It records the source revision, dependency-manifest hash, compiler/SDK, cache
configuration, and static archive hashes. `ORT_LIB_PATH` points to the complete
build tree because `ort-sys` must link its dependency archives as well.
Do not publish only the main ONNX archive or rely on an installed system runtime.

The Intel app targets macOS 13.3. Packaging verifies the binary's architecture,
minimum OS, and system-only dynamic dependencies, sets matching bundle metadata,
and includes the runtime MIT license, third-party notices, and provenance.
Intel CI runs application/audio tests, builds this unsigned app, and generates
non-silent multilingual audio from the pinned production model. These checks
are not signing, audible-playback, Gatekeeper, or clean macOS 13.3 acceptance.
See [the research and outstanding acceptance gates](INTEL_MAC_SUPPORT_RESEARCH.md).

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

### Main protection and GitHub plan requirements

Mist remains public to enforce protections on the personal GitHub Free plan;
the website is kept offline separately until the release is ready. If made
private during preparation, anonymous downloads cannot work, and Free cannot
enforce branch/tag rulesets, environment secrets, or environment protection
rules. Existing public-repository environment rules are ignored while private;
their mere presence is not evidence that signing is protected.
GitHub Pro enables private branch/tag rulesets and environment secrets, but
required environment reviewers on Free, Pro, and Team still require a public
repository. Do not move signing values into unprotected repository secrets to
work around these limits.
([ruleset availability](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets),
[environment availability](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments))

Keep the maintainer as the only writer while private protections are
unavailable, and use feature branches and PRs even before server enforcement
is enabled. `.github/CODEOWNERS` requests maintainer review; it is not an
access-control rule.

Once the plan supports rulesets (or the repository is made public), apply the
two rulesets under `.github/rulesets/` in **Settings → Rules → Rulesets**. Their
combined policy requires PRs, resolved review threads, repository validation,
the three OS checks, full quality/security gates, and the Linux release-package check;
blocks deletion and force pushes; and lets only `mindful-time` merge. The
maintainer exception is PR-only and does not bypass the separate CI ruleset.
Zero mandatory approving reviews allows the solo maintainer to merge their
own PR without an impossible self-approval requirement. Other contributors
still cannot merge, even when CI passes. These JSON files are desired
configuration, not proof that GitHub has applied it.

For a new configuration, apply each JSON with `gh api --method POST
repos/mindful-time/Mist/rulesets --input <file>`. If a matching ruleset already
exists, update that exact ruleset ID with `PUT` instead of creating a duplicate.
Read back both rulesets and the effective rules for `main` before claiming
protection is enabled.

The public repository now has `Main PR and CI requirements` and `Main
maintainer merges only` active. Apply future edits to the JSON files to these
existing rulesets; editing a file does not update server configuration.

## Testing a release candidate

PRs targeting `main` run the read-only CI workflow, including required quality,
native builds/tests, and `Linux package validation`. CI builds and validates
the Linux production packages without signing secrets. The Release workflow
does not start on PRs; signing, bundle assembly, and publication belong only to
Release. PR artifacts are test candidates, not approved public downloads.
Pushes to `main` rerun CI as an integration check; pushes to a PR branch do not
start a duplicate CI run.

Run **Actions → Release → Run workflow** on `main`. Choose `linux`, `macos`, or
`windows` to validate one platform and download its individual Actions artifacts.
macOS requires the Apple signing credentials; Windows requires the Windows
signing credentials. Linux can be tested while either signing setup is pending.
Every candidate or tagged release validates its source/version, then verifies
successful integration CI for the exact release commit before platform packaging
or access to signing environments. CI owns audio and other tests, CRAP, Smells,
OSV, and Gitleaks; Release does not repeat them. The verifier requires the latest
push-to-`main` run of `.github/workflows/ci.yml` and all required jobs in its latest
attempt to succeed. PR/merge-ref runs, wrong commits, failed/skipped/missing jobs,
API errors, incomplete pagination, and concurrent reruns fail closed. If main CI
is still running, wait and retry the candidate. For a partial or failed rerun,
rerun all CI jobs on that main commit rather than bypassing the release gate.
Linux packaging is shared between CI and Release; PR CI cannot call the
signing/publishing jobs.

After both signing setups are configured, choose `all` to build, sign, notarize,
and validate every platform artifact, then upload
`mist-release-bundle-<commit>` as a normal Actions artifact. Manual runs do not
create a tag or GitHub Release and cannot publish anything. Tagged releases
always require all five packages. Use candidate runs to fix packaging failures
before choosing a version tag.

## Publishing a release

1. Synchronize `VERSION`, `Cargo.toml`, `Cargo.lock`, and the macOS bundle
   version, then run `./scripts/release-check.sh`.
2. Wait for all required integration-CI checks on the exact commit on `main`,
   then complete a successful manual release-candidate run for that commit.
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
