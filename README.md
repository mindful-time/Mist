# Mist

A tiny cross-platform living-mist desktop pet that reads selected text with
Kokoro:

- **macOS / Windows / Linux:** select text in a supported app and press
  **Ctrl+Space**.
- **macOS native-app alternative:** choose **Services → Speak Selection with
  Mist** from the application or text context menu.
- **Fallback:** when an app does not expose its selection, Mist can send the
  platform Copy shortcut and use that text. This is enabled by default and can
  be disabled in settings. **Speak copied text** remains available explicitly.

The normal desktop surface is only a translucent, animated mist. It changes
color with the chosen voice and reacts throughout playback to time-windowed
loudness and spectral brightness from the real generated audio.
Voice selection, status, settings, and quit live in the macOS menu bar or the
Windows/Linux system tray; setup and recoverable errors open a focused panel
only when needed.

Every captured selection appears in a compact Spotlight-style pill beneath the
mist in its own transparent always-on-top queue window. Drag the pill to move
the queue independently while the mist remains in place; its separate round
control pauses or resumes active speech and plays waiting items, while the ×
control deletes that exact item. Deleting active speech stops its platform
audio session before the next item starts. Choosing any waiting item moves it
to the top before playback. Pause, resume, and delete wake the platform audio
adapter immediately instead of waiting for the visual sampling interval. New
items play automatically by default; turn off **Play new queue items
automatically** to start each item yourself. Completed items disappear. When
Mist used automatic Copy, it clears that temporary clipboard value after
playback only if it still matches. macOS and Windows also require the clipboard
change token to match; Linux uses a content fingerprint. Operating systems do
not provide Mist with one portable atomic compare-and-clear operation, so this
cleanup is intentionally documented as best effort. Disable automatic Copy if
another clipboard manager or a sensitive clipboard workflow makes any mutation
unacceptable.

Speech is generated locally with
[Kokoro-82M](https://huggingface.co/spaces/hexgrad/Kokoro-TTS); selected text is
never sent to a server. Long selections are synthesized sentence by sentence,
so playback starts after the first audio chunk instead of waiting for the whole
selection. The persisted Playback page calls this default **Real-time** mode;
choose **Complete audio** when whole-selection buffering is preferred. Playback
speed is also persisted from 0.5×–3× and applies to every language and voice.

The first launch presents Kokoro's 54 voices across nine language families:
American and British English, Spanish, French, Hindi, Italian, Japanese,
Brazilian Portuguese, and Mandarin. **Download voices** fetches the full
multilingual Kokoro v1.0 ONNX model (~311 MiB) and combined voice pack
(~27 MiB) into your user application-data directory. Both artifacts are pinned
to one revision and SHA-256 verified before use. The chosen voice—and therefore
its language—persists. Selecting another voice immediately cancels the current
sample and starts the new preview; it never waits behind stale preview audio.
Mist embeds the OFL-licensed Noto Sans Devanagari UI fallback so Hindi text does
not depend on an optional operating-system font. The remaining scripts use the
native macOS/Windows fonts or the Linux packages listed below.

## Build and run

Requirements: Rust 1.95+ and internet access for the first build/model download.

```sh
make test
make run
```

Then choose a voice and click **Download voices** in onboarding.

### Commit and release gates

Install the repository-owned Git hooks once per clone:

```sh
make hooks
```

Every commit then runs Rust formatting, `cargo check`, strict Clippy, all tests,
coverage-backed CRAP analysis, the pinned
[`mindful-time/smells`](https://github.com/mindful-time/smells) v0.5.0
staged-code policy, an OSV dependency scan, and a staged Gitleaks scan. CRAP
scores above 5 warn; new or regressed scores above 10 block against the reviewed
baseline. Install
[OSV-Scanner](https://google.github.io/osv-scanner/installation/) and
[Gitleaks](https://github.com/gitleaks/gitleaks#installing) before committing;
Smells is isolated and executed with `uvx`; CRAP uses `cargo-llvm-cov` and
`cargo-crap 0.5.0`. Run the same gate directly with `make quality`, CRAP alone
with `make crap`, the complete repository smell analysis with `make smells`,
and a full-history secret scan with `make security`. The committed policies
use the current repository measurements as required upper bounds, so new
growth fails while existing review findings remain visible in its report.

The pre-push hook repeats the commit gate, scans the full repository with
Smells v0.5.0, performs the full-history secret scan, and checks that `VERSION`,
`Cargo.toml`, `Cargo.lock`, and the macOS bundle metadata resolve consistently.
Mist is currently prepared as SemVer release `0.5.0`; future releases must bump
the tracked version files together.

### macOS app bundle

Requires macOS 13+ and the Xcode Command Line Tools:

```sh
MIST_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)" \
make install
open "/Applications/Mist.app"
```

The installer requires an Apple Development or Developer ID certificate and
uses `/Applications` as the canonical location. This keeps the app's macOS
Accessibility identity stable across rebuilds. To make an explicitly temporary
development install instead, use ad-hoc signing and a separate home-directory
destination:

```sh
MIST_SIGNING_IDENTITY=- \
MIST_INSTALL_DIR="$HOME/Applications" \
make install
```

macOS ties an ad-hoc install's Accessibility grant to that exact build, so it
requires granting permission again after every rebuild. Do not keep production
and development copies installed at the same time.

Open the installed app once. macOS asks for Accessibility permission so the
global shortcut can read selections from applications with custom context
menus. Enable **Mist** in **System Settings → Privacy & Security →
Accessibility**, select text in a supported application, and press
**Ctrl+Space**. Mist uses that permission for a narrow keyboard event tap that
recognizes and consumes only Control-Space; this works even when macOS reserves
the same chord for input-source switching, and it does not inspect typed text.
Electron/Chromium applications such as Codex receive a second native capture
attempt after Mist activates their accessibility tree. If a verified,
non-protected target still does not expose selected text, the default-on
Command-C fallback feeds the selection into the same visible queue.

The native Service remains available for applications that support it. If it is
hidden, enable it in **System Settings → Keyboard → Keyboard Shortcuts →
Services → Text**. The app never requests screen-recording permission. Direct
Accessibility capture does not touch the clipboard; the optional automatic
fallback sends Command-C only after direct capture fails.

### Windows

Use Rust's standard `stable-msvc` toolchain; ONNX Runtime does not provide the
GNU Windows artifact used by this build.

```powershell
rustup default stable-msvc
cargo build --release
.\target\release\mist.exe
```

### Linux

The pet supports X11 and Wayland. Install the normal desktop build dependencies
for winit, DejaVu Sans, Noto Core, and Noto CJK fonts, plus one audio command:
`pw-play`, `paplay`, `aplay`, or `ffplay`. On Debian/Ubuntu, the font packages
are `fonts-dejavu-core fonts-noto-core fonts-noto-cjk`. Mist loads these fonts
locally so English, Japanese, Mandarin, and Hindi labels render correctly.

```sh
make install-linux
"$HOME/.local/bin/mist"
```

The shortcut reads Windows selections through Microsoft UI Automation and Linux
X11 selections through PRIMARY. On Wayland,
the app requests **Ctrl+Space** through the XDG GlobalShortcuts portal. The
desktop may show a one-time confirmation dialog or assign a different gesture,
which the pet displays. The Linux installer also installs the stable desktop
identity required by the portal. Reading the selected text requires the
compositor's ext-data-control or wlr-data-control primary-selection support. If
either capability is unavailable, automatic Copy may also be restricted by the
desktop. Copy the text and use **Speak copied text** from the pet menu, or use
manual text entry.

For a terminal smoke test:

```sh
mist --install-model
mist --speak "Hello from Kokoro."
```

Set `MIST_MODEL_DIR` to use a different model directory. The legacy
`SELECT_TO_SPEAK_MODEL_DIR` name remains accepted for existing installations.

### Inference acceleration

The current Rust Kokoro adapter selects a native ONNX Runtime backend
automatically:

- macOS uses CPU for the multilingual runtime.
- Windows probes CUDA, then falls back to CPU.
- Linux probes CUDA, then CPU.

The Model page uses friendly capability labels—Recommended, Standard,
Accelerated, Graphics acceleration, and Apple optimized—while retaining the
actual runtime in secondary detail. CUDA is enabled only on Windows/Linux when
a usable NVIDIA device is detected. WebGPU and MLX remain disabled because the
current Kokoro adapter does not integrate them; Apple Silicon or Metal support
alone is not reported as working acceleration. The research and validation
gates are recorded in `docs/INFERENCE_BACKEND_RESEARCH.md`.

## Architecture

The code uses a small hexagonal architecture:

```text
macOS Accessibility + Service / Windows UIA / Linux selection
                              |
          global shortcut / mist + floating queue
                       |
             SpeakSelection use-case
                /                \
    SpeechSynthesizer port    AudioPlayer port ← playback control
              |                     |
      selected engine        macOS / Windows / Linux
       factory port
              |
   Kokoro adapter today

  VoiceCatalog port ← UI / preferences / preview copy
         |
  Kokoro catalog today
```

- `core/domain/*`: selected text, speech queue, model-neutral voice/language,
  playback, inference, and audio values.
- `core/application/*`: speech, model-installation, and token-scoped playback
  use cases.
- `core/ports/*`: core-owned speech, catalog, audio, and provisioning
  interfaces.
- `adapters/inbound/desktop/*`: desktop input orchestration and the complete
  mist/queue/settings/tray UI primary adapter.
- `adapters/inbound/os/*`: macOS Accessibility/Services, Windows UI Automation,
  and Linux X11/Wayland integrations.
- `adapters/inbound/selection/clipboard.rs`: optional, identity-checked Copy
  fallback.
- `adapters/outbound/speech/kokoro/*`: Kokoro/ONNX engine plus its language and
  voice catalog.
- `adapters/outbound/audio/*`, `persistence/*`, and `provisioning/*`: system
  audio, filesystem preferences, and verified model artifacts.
- `runtime/worker.rs`: long-lived desktop orchestration outside the core.
- `main.rs`: composition root.

The core has no Kokoro catalog, AppKit, ONNX, filesystem, or process knowledge
and is covered by unit tests using in-memory port fakes. `main.rs` is the
composition root: replacing Kokoro means supplying another `VoiceCatalog`,
`SpeechEngineFactory`, and `ModelProvisioner`; queue, UI, playback, and
selection use-cases remain unchanged.
The complete package map and dependency rule live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Scope

Launch-at-login, graphical installers, and production signing/notarization
remain outside the current scope. Native selection behavior still requires
release testing on each supported OS even when cross-target compilation passes.
