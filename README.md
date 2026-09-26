# Select to Speak

A tiny cross-platform desktop pet that reads selected text with Kokoro:

- **macOS / Windows / Linux:** select text anywhere and press **Ctrl+Alt+S**.
- **macOS native-app alternative:** choose **Services → Speak Selection with
  Kokoro** from the application or text context menu.
- **Windows/Linux fallback:** copy text, right-click the pet, and choose **Speak
  copied text**. Manual text entry is available in that same menu.

Speech is generated locally with
[Kokoro-82M](https://huggingface.co/spaces/hexgrad/Kokoro-TTS); selected text is
never sent to a server.

The first launch shows **Download voice**. That fetches the quantized Kokoro v1.0
ONNX model (~92 MB) and the `af_heart` voice (~0.5 MB) from the Apache-2.0
Hugging Face release into your user application-data directory. Downloads are
pinned to a model revision and SHA-256 verified before use.

## Build and run

Requirements: Rust 1.95+ and internet access for the first build/model download.

```sh
make test
make run
```

Then click **Download voice** on the pet.

### macOS app bundle

Requires macOS 13+ and the Xcode Command Line Tools:

```sh
SELECT_TO_SPEAK_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)" \
make install
open "/Applications/Select to Speak.app"
```

The installer requires an Apple Development or Developer ID certificate and
uses `/Applications` as the canonical location. This keeps the app's macOS
Accessibility identity stable across rebuilds. To make an explicitly temporary
development install instead, use ad-hoc signing and a separate home-directory
destination:

```sh
SELECT_TO_SPEAK_SIGNING_IDENTITY=- \
SELECT_TO_SPEAK_INSTALL_DIR="$HOME/Applications" \
make install
```

macOS ties an ad-hoc install's Accessibility grant to that exact build, so it
requires granting permission again after every rebuild. Do not keep production
and development copies installed at the same time.

Open the installed app once. macOS asks for Accessibility permission so the
global shortcut can read selections from applications with custom context
menus. Enable **Select to Speak** in **System Settings → Privacy & Security →
Accessibility**, select text in any application, and press **Ctrl+Alt+S**
(Control+Option+S on a Mac keyboard).

The native Service remains available for applications that support it. If it is
hidden, enable it in **System Settings → Keyboard → Keyboard Shortcuts →
Services → Text**. The app never requests screen-recording permission and does
not change your clipboard to capture a selection.

### Windows

Use Rust's standard `stable-msvc` toolchain; ONNX Runtime does not provide the
GNU Windows artifact used by this build.

```powershell
rustup default stable-msvc
cargo build --release
.\target\release\select-to-speak.exe
```

### Linux

The pet supports X11 and Wayland. Install the normal desktop build dependencies
for winit plus one audio command: `pw-play`, `paplay`, `aplay`, or `ffplay`.

```sh
make install-linux
"$HOME/.local/bin/select-to-speak"
```

The shortcut reads Windows selections through Microsoft UI Automation and Linux
X11 selections through PRIMARY; it never rewrites your clipboard. On Wayland,
the app requests **Ctrl+Alt+S** through the XDG GlobalShortcuts portal. The
desktop may show a one-time confirmation dialog or assign a different gesture,
which the pet displays. The Linux installer also installs the stable desktop
identity required by the portal. Reading the selected text requires the
compositor's ext-data-control or wlr-data-control primary-selection support. If
either capability is unavailable, copy the text and use **Speak copied text**
from the pet menu, or use manual text entry.

For a terminal smoke test:

```sh
select-to-speak --install-model
select-to-speak --speak "Hello from Kokoro."
```

Set `SELECT_TO_SPEAK_MODEL_DIR` to use a different model directory.

## Architecture

The code uses a small hexagonal architecture:

```text
macOS Accessibility + Service / Windows UIA / Linux selection
                              |
                 global shortcut / pet UI
                       |
             SpeakSelection use-case
                /                \
    SpeechSynthesizer port    AudioPlayer port
              |                     |
         Kokoro ONNX        macOS / Windows / Linux
```

- `domain.rs`: selected text, voice settings, and audio values.
- `application.rs`: the `SpeakSelection` use-case.
- `ports.rs`: speech synthesis, model provisioning, and playback interfaces.
- `adapters/macos_selection.rs`: macOS Accessibility selection adapter.
- `adapters/macos_service.rs`: optional incoming macOS Services adapter.
- `adapters/windows_selection.rs`: Windows UI Automation selection adapter.
- `adapters/linux_selection.rs`: Linux X11/Wayland selection adapter.
- `adapters/wayland_shortcut.rs`: Wayland GlobalShortcuts portal adapter.
- `platform.rs`: shared native-event and selection-capture coordinator.
- `adapters/kokoro.rs`: outgoing Kokoro/ONNX adapter.
- `adapters/system_audio.rs`: outgoing macOS/Windows/Linux audio adapter.
- `ui.rs` and `worker.rs`: the floating pet and background command boundary.

The core has no AppKit, ONNX, filesystem, or process knowledge and is covered by
unit tests using in-memory port fakes.

## Scope

This first version is intentionally English-first. Kokoro exposes more languages
and voices, but voice selection, launch-at-login, interruption, installers, and
production signing/notarization are kept out of the MVP.
