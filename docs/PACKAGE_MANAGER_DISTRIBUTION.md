# Package-manager and direct-download distribution

Checked on 2026-10-03. Package generation is implemented below, but no release,
tag, tap, Chocolatey submission, signing setting, or website deployment has
been created by this work. Preparation is not evidence of public downloads.

## Recommendation

Use public GitHub Release assets as the shared download host. Add a maintained
Homebrew cask in our own tap for macOS and a Chocolatey package for Windows.
Offer the same artifacts as ordinary browser and curl downloads. A download
website is optional for this first distribution step.
([GitHub direct release-asset links](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases),
[Homebrew taps](https://docs.brew.sh/Taps),
[Chocolatey installer wrapper](https://docs.chocolatey.org/en-us/create/functions/install-chocolateypackage/))

## Homebrew

An independent tap is a Git repository containing package definitions; it is
not the official `homebrew/cask` catalog. A proposed `mindful-time/homebrew-tap`
repository would map to the tap name `mindful-time/tap`. Use a fully qualified
cask name to identify our package rather than assuming the short name `mist`
is available. Neither the repository nor a working install command exists yet.
([Homebrew tap naming and installation](https://docs.brew.sh/Taps))

For binary distribution, the cask can reference architecture-specific DMGs,
declare checksums and install `Mist.app`. Publish tested DMGs before claiming
the cask is usable. Official catalog inclusion is a separate submission and
acceptance decision: assessed macOS artifacts must pass Homebrew's Gatekeeper
checks without disabling or bypassing macOS protections.
([Cask fields and app artifacts](https://docs.brew.sh/Cask-Cookbook),
[official cask acceptance](https://docs.brew.sh/Acceptable-Casks))

A custom tap is not a replacement for Developer ID signing and notarization.
Apple documents that downloaded apps are assessed for those properties.
Unnotarized or unidentified-developer apps can encounter security alerts.
([Apple's downloaded-app protections](https://support.apple.com/en-us/102445))

## Chocolatey and MSI

Chocolatey can download and run native EXE or MSI installers. A package needs
correct installer arguments and checksum metadata; the community feed adds
validation, verification and moderation. Test silent installation, upgrades
and uninstall behavior before submission. Do not promise instant approval.
([Chocolatey EXE/MSI installation](https://docs.chocolatey.org/en-us/create/functions/install-chocolateypackage/),
[community moderation requirements](https://docs.chocolatey.org/en-us/community-repository/moderation/))

Mist currently builds `Mist-windows-x86_64-setup.exe` using NSIS, not an MSI.
Chocolatey therefore does not require changing installer formats. If MSI is
desired, the pinned cargo-packager 0.11.8 supports it through the `wix` format.
Add and test that output, its signing and release-asset handling explicitly;
renaming an EXE is not conversion.
([Mist packaging configuration](../Cargo.toml),
[current platform release workflow](../.github/workflows/release-platform.yml),
[cargo-packager formats](https://docs.rs/cargo-packager/0.11.8/cargo_packager/enum.PackageFormat.html))

The reviewed Chocolatey moderation requirements do not state a blanket
Authenticode certificate requirement. That is not a guarantee of package
approval or Windows compatibility: unsigned downloads may warn or be blocked
by Windows policy or Smart App Control. Newly signed downloads can also show
SmartScreen warnings while reputation develops. A package manager does not
itself establish our publisher identity.
([Chocolatey requirements](https://docs.chocolatey.org/en-us/community-repository/moderation/),
[Microsoft SmartScreen and Smart App Control guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation))

## Current readiness and next work

Read-only GitHub checks returned a public repository, no GitHub Releases and
no `release-signing` environment secret names. The latest successful Release
workflow was a PR run, not publication. These are point-in-time checks, not a
claim about credentials the maintainer may possess elsewhere.

The workflow inspected on 2026-10-03 required signing credentials and assembled
all platforms. That combined workflow has since been removed: current native
releases are independent. The macOS installation script checks codesign,
Gatekeeper and the expected Apple team before installing; its Windows script
checks Authenticode and the expected certificate before either installation or
download-only completion. The macOS download-only mode checks the checksum,
but does not perform those installation-time signature checks.
([Platform release workflow](../.github/workflows/release-platform.yml),
[macOS/Linux installer](../scripts/install-release.sh),
[Windows installer](../scripts/install-release.ps1),
[existing release policy](DISTRIBUTION.md))

## Implemented preparation

`scripts/build-package-managers.mjs` generates a Homebrew cask and Chocolatey
recipe from the actual release files and `SHA256SUMS`. It rejects missing,
duplicate or wrong checksums and requires a Windows signer fingerprint.
Homebrew selects the ARM/Intel DMG and pins each checksum. Chocolatey pins
the EXE checksum and verifies Authenticode plus the exact publisher before
silent NSIS installation. No quarantine/Gatekeeper bypass is added.

Recipe generation remains a tested standalone preparation tool, not part of the
native release pipelines. Its current templates require a common asset-host tag
and all Mac/Windows installers. Before use with independent platform releases,
adapt and test the generator's version-pinned URLs for those published channels;
do not advertise the currently generated common-tag recipes as installable.
Both stable `x.y.z` and `x.y.z-rc.N` versions have fixture tests; Windows CI also
runs `choco pack`. Fixtures do not prove native installation or real signatures.

For a signed candidate downloaded from Actions:

```sh
node scripts/build-package-managers.mjs 0.1.0-rc.1 /absolute/candidate/assets \
  /absolute/new/package-output WINDOWS_CERTIFICATE_SHA256
```

The last argument is the non-secret 64-hex certificate fingerprint, not a
private key. Use a fresh output directory; the generator will not overwrite
existing files. Templates live in `packaging/` and use the existing EXE, not MSI.
Chocolatey CLI 2+ supports the dotted RC version; users must explicitly opt in
to a published candidate with `--pre --version=0.1.0-rc.1`. Do not advertise
that command until the package is available in the chosen feed. Homebrew and
Chocolatey repository publication are not implied by generated recipe files.

## Remaining publication steps

1. Complete the relevant native signing setup and create that platform's draft
   through its protected tag workflow; other platforms do not need to be ready.
2. Test install, upgrade, uninstall, first launch, permissions, hotkey, and
   speech on clean macOS ARM/Intel, Windows x64, and supported Linux desktops.
   Specifically test Chocolatey's per-user install using the same Windows
   account, upgrades, and automatic NSIS uninstall; do not submit until those
   pass. Chocolatey's default uninstaller discovers registry changes, so also
   confirm it tracks only Mist.
   ([automatic uninstaller](https://docs.chocolatey.org/en-us/choco/features/auto-uninstaller/))
3. Adapt and validate the package generator against the independent published
   signed installers, then choose the tap/feed setup. After acceptance, create the proposed
   `mindful-time/homebrew-tap`, copy the generated cask to `Casks/mist.rb`, and
   validate it with Homebrew before advertising
   `brew install --cask mindful-time/tap/mist`.
4. Check availability of the proposed `mist-tts` package ID, then submit the
   tested generated `.nupkg` to Chocolatey for moderation. Do not advertise
   `choco install mist-tts` until that version is approved/available.
5. Enable the download website only after those published URLs work.

No tap repository is created automatically, and no Chocolatey API key or
automatic community-feed push is added. Preserve the Linux DEB/AppImage route
already described in the distribution guide. The existing verified bootstrap
supports curl; direct download examples below avoid executing a remote script.

After `v0.1.0` is actually published (these links do not exist yet):

```sh
# Apple Silicon; use x86_64 instead of aarch64 for an Intel Mac.
curl --fail --location --proto '=https' --tlsv1.2 --remote-name \
  https://github.com/mindful-time/Mist/releases/download/v0.1.0/Mist-macos-aarch64.dmg
```

```powershell
curl.exe --fail --location --proto '=https' --tlsv1.2 --remote-name https://github.com/mindful-time/Mist/releases/download/v0.1.0/Mist-windows-x86_64-setup.exe
```

Direct curl downloads alone do not verify hashes or signatures. Check the
release's `SHA256SUMS` and native publisher, or use the verified bootstraps in
[DISTRIBUTION.md](DISTRIBUTION.md). DMG is for macOS, EXE for Windows;
MSI packaging is not part of this change.

An unsigned public tester preview would require an explicitly approved,
separate preview flow; it must not silently weaken the stable-release checks.
The current independent native RC flow is documented in
[DISTRIBUTION.md](DISTRIBUTION.md); it does not publish a tap or community package.
