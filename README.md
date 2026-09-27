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

Every captured selection appears as a small floating queue bubble beneath the
mist. New items play automatically by default; turn off **Play new queue items
automatically** to click each bubble yourself. Completed items disappear. When
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
selection.

The first launch presents eight distinct voice mists and a **Download voices**
action. That fetches the quantized Kokoro v1.0 ONNX model (~92 MB) plus the
curated English voice catalog (~4 MB) from the Apache-2.0 Hugging Face release
into your user application-data directory. Every artifact is pinned to one
model revision and SHA-256 verified before use. The chosen voice persists and
can be changed from the menu bar/tray without restarting Kokoro.

## Build and run

Requirements: Rust 1.95+ and internet access for the first build/model download.

```sh
make test
make run
```

Then choose a voice and click **Download voices** in onboarding.

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
for winit plus one audio command: `pw-play`, `paplay`, `aplay`, or `ffplay`.

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

The Rust Kokoro adapter selects a native ONNX Runtime backend automatically:

- macOS probes CoreML, then falls back to CPU.
- Windows probes CUDA, then DirectML, then CPU.
- Linux probes CUDA, then CPU.

The settings panel displays the automatic backend policy while preparing and
playing speech; labels such as `CoreML → CPU` show the fallback order rather
than claiming which provider ultimately accepted every graph node. Set
`KOKORO_ORT_PROVIDER=cpu`, `coreml`, `cuda`, or `directml` to override that
policy when troubleshooting; explicit accelerators are labeled as requested.
The ONNX Runtime provider probe is the hardware detection step, so the app does
not maintain a second, potentially inconsistent GPU detector.

Kokoro can also run through MLX on Apple Silicon, but this app deliberately uses
CoreML instead. CoreML is available to the native Rust/ONNX pipeline and keeps
the same application architecture on macOS, Windows, and Linux; an MLX backend
would require a separate Apple-only runtime and model package.

## Architecture

The code uses a small hexagonal architecture:

```text
macOS Accessibility + Service / Windows UIA / Linux selection
                              |
          global shortcut / mist + floating queue
                       |
             SpeakSelection use-case
                /                \
    SpeechSynthesizer port    AudioPlayer port
              |                     |
         Kokoro ONNX        macOS / Windows / Linux
```

- `domain.rs`: selected text, ordered speech queue, the voice catalog, mist
  palettes, and audio values.
- `application.rs`: the `SpeakSelection` use-case.
- `ports.rs`: speech synthesis, model provisioning, and playback interfaces.
- `adapters/macos_selection.rs`: macOS Accessibility selection adapter.
- `adapters/macos_service.rs`: optional incoming macOS Services adapter.
- `adapters/windows_selection.rs`: Windows UI Automation selection adapter.
- `adapters/linux_selection.rs`: Linux X11/Wayland selection adapter.
- `adapters/wayland_shortcut.rs`: Wayland GlobalShortcuts portal adapter.
- `platform.rs`: shared native-event and selection-capture coordinator.
- `adapters/kokoro.rs`: outgoing streaming Kokoro/ONNX adapter with native
  accelerator detection.
- `adapters/system_audio.rs`: outgoing macOS/Windows/Linux audio adapter.
- `adapters/voice_preferences.rs`: validated, persistent voice-selection
  adapter.
- `adapters/clipboard_fallback.rs`: optional Copy fallback with fingerprint and
  platform-token-checked best-effort cleanup.
- `adapters/playback_preferences.rs`: automatic-play and fallback settings.
- `ui/mist.rs`: the embedded, audio-reactive mist renderer.
- `ui/tray.rs`: the cross-platform menu-bar/system-tray adapter.
- `ui/voice_gallery.rs`: the accessible onboarding and settings voice gallery.
- `ui/queue_tray.rs`: bounded floating queue bubbles beneath the mist.
- `ui.rs` and `worker.rs`: the floating surface and background command seam.

The core has no AppKit, ONNX, filesystem, or process knowledge and is covered by
unit tests using in-memory port fakes.

## Scope

This version is intentionally English-first. Additional languages,
launch-at-login, interruption, graphical installers, and production
signing/notarization remain outside the current scope.
