# Mist

**Select text anywhere. Press Ctrl+Space. Hear it locally.**

Mist is a small cross-platform desktop companion powered by
[Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M). It reads selected text without sending it to a server.

1. Select text in an application.
2. Press **Ctrl+Space**.
3. Mist adds the text to the queue and starts speaking.

The normal desktop surface is only a soft, animated mist. It stays small and translucent while idle, grows with speech, and reacts to the generated audio.

## What Mist does

- Reads selected text across macOS, Windows, and Linux.
- Speaks locally with 54 Kokoro voices across nine language families.
- Starts long passages sentence by sentence in **Real-time** mode.
- Offers **Complete audio** mode for whole-selection buffering.
- Supports playback speeds from 0.5× to 3×.
- Replaces a playing voice preview as soon as another voice is selected.
- Keeps a separate draggable queue with play, pause, resume, select, and delete.
- Removes completed items and immediately stops audio when an active item is deleted.

## Selection and privacy

Mist first asks the operating system for the selected text. If an application does not expose it, Mist can send the platform Copy shortcut and read the result.

Automatic Copy is enabled by default and can be disabled in **Settings → Playback**. **Speak copied text** is also available from the menu.

When Mist used automatic Copy, it clears that temporary value after playback only when the clipboard still matches. This cleanup is best effort because operating systems do not provide one portable atomic operation.

Mist does not request screen-recording permission. Speech synthesis and playback stay on the device.

## Voices and languages

The first launch shows voices grouped by language:

- American English
- British English
- Spanish
- French
- Hindi
- Italian
- Japanese
- Brazilian Portuguese
- Mandarin Chinese

Selecting a voice changes the language and starts its preview immediately. The previous preview is cancelled instead of playing to completion.

**Download voices** installs the pinned multilingual Kokoro model and voice pack in the user application-data directory. Both downloads are SHA-256 verified.

## Platform support

| Platform | Selected-text adapter | Automatic fallback | Preferred inference |
| --- | --- | --- | --- |
| macOS | Accessibility API and native Service | Command-C | Core ML on Apple Silicon, then CPU |
| Windows | Microsoft UI Automation | Ctrl-C | CUDA, then DirectML, then CPU |
| Linux | X11 PRIMARY or Wayland portals | Ctrl-C | CUDA, then CPU |

The CPU path is always available. Choosing **Standard** in model settings forces CPU. Unsupported options such as WebGPU and MLX remain disabled rather than appearing usable.

Core ML was validated locally with the production multilingual model and finite, non-silent 24 kHz output in English, Spanish, Japanese, and Mandarin.

Windows and Linux builds, tests, and CPU fallbacks run in CI. CUDA and DirectML still require acceptance testing on matching physical GPU hardware.

See [inference backend research](docs/INFERENCE_BACKEND_RESEARCH.md) for provider details and validation gates.

## Install

For the simplest path, use the
[Mist download page](https://mindful-time.github.io/Mist/). It chooses your
operating system and shows the matching verified download when a public release
is available. macOS and Windows installers are also platform-signed.

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

The installers verify release checksums before installing. Direct DMG, EXE,
AppImage, and DEB downloads are also available on the release page. See the
[distribution guide](docs/DISTRIBUTION.md) for exact artifact names, version
pinning, download-only mode, and release requirements.

## Build and run from source

Requirements: Rust 1.95+ and internet access for the first build and model download.

```sh
make test
make run
```

Choose a voice, select **Download voices**, then select text in another application and press **Ctrl+Space**.

### macOS

Mist supports macOS 13+ and requires the Xcode Command Line Tools.

```sh
MIST_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)" make install
open "/Applications/Mist.app"
```

Install to `/Applications` with a stable Apple certificate so the Accessibility identity survives rebuilds. Avoid keeping production and development copies installed together.

Open Mist once, then enable it in **System Settings → Privacy & Security → Accessibility**. Mist consumes only Control-Space and does not inspect typed text.

The native Service is available under **Services → Speak Selection with Mist**. If hidden, enable it in **Keyboard → Keyboard Shortcuts → Services → Text**.

For a temporary development install:

```sh
MIST_SIGNING_IDENTITY=- MIST_INSTALL_DIR="$HOME/Applications" make install
```

macOS treats each ad-hoc rebuild as a new Accessibility identity, so permission must be granted again.

### Windows

Use Rust's `stable-msvc` toolchain.

```powershell
rustup default stable-msvc
cargo build --release
.\target\release\mist.exe
```

### Linux

Mist supports X11 and Wayland. Install desktop build libraries, DejaVu Sans, Noto Core, Noto CJK, and one audio command: `pw-play`, `paplay`, `aplay`, or `ffplay`.

```sh
make install-linux
"$HOME/.local/bin/mist"
```

Wayland requests Ctrl+Space through the XDG GlobalShortcuts portal. The desktop may show a confirmation or assign another gesture.

Direct Wayland selection needs ext-data-control or wlr-data-control primary-selection support. If unavailable, use automatic Copy, **Speak copied text**, or manual text entry.

For a terminal smoke test:

```sh
mist --install-model
mist --speak "Hello from Kokoro."
```

Set `MIST_MODEL_DIR` to use another model directory. `SELECT_TO_SPEAK_MODEL_DIR` remains accepted for existing installations.

## Architecture

Mist uses hexagonal architecture so the speech model, operating-system adapters, audio output, persistence, and provisioning can change independently.

```text
macOS Accessibility / Windows UIA / Linux selection
                         |
              desktop input adapter
                         |
               application use cases
                 /              \
        speech ports          audio port
             |                    |
      Kokoro adapter       platform playback
```

- `core/domain`: model-neutral values and queue state.
- `core/application`: speech, playback, and model-installation use cases.
- `core/ports`: interfaces owned by the core.
- `adapters/inbound`: desktop UI, shortcuts, selection, and OS integration.
- `adapters/outbound`: speech engines, audio, persistence, and provisioning.
- `runtime`: long-lived desktop orchestration.
- `main.rs`: composition root.

Replacing Kokoro requires another voice catalog, speech-engine factory, and model provisioner. Selection, queue, UI, and playback use cases stay unchanged.

See the complete [architecture guide](docs/ARCHITECTURE.md) and [product specification](docs/SPEC.md).

## Development

Install repository-owned Git hooks once per clone:

```sh
make hooks
```

The hooks run formatting, checks, strict Clippy, tests, coverage-backed CRAP analysis, Smells v0.5.0, OSV, Gitleaks, and release-version checks.

CRAP scores above 5 warn. New or regressed scores above 10 block. Run the gates directly when needed:

```sh
make quality
make crap
make smells
make security
```

Mist is prepared for its first public SemVer release, `0.1.0`. `VERSION`,
`Cargo.toml`, `Cargo.lock`, and bundle metadata must move together.

Only the repository owner may push directly to `main`. Every other change must arrive through a pull request, and pull requests are squash-merged.

Launch at login remains outside the current scope. Tagged releases build draft,
signed installers for macOS and Windows plus AppImage and DEB packages for
Linux. Native selection, GPU acceleration, installation, and uninstall behavior
still require release acceptance testing on every supported operating system
before a draft is published.
