# Select to Speak

A tiny cross-platform desktop pet that reads selected text with Kokoro:

- **macOS:** select text, right-click, then choose **Services → Speak
  Selection with Kokoro**.
- **Windows / Linux X11:** select text and press **Ctrl+Alt+S**.
- **Linux Wayland fallback:** copy selected text, right-click the pet, and choose
  **Speak copied text**.

Speech is generated locally with
[Kokoro-82M](https://huggingface.co/spaces/hexgrad/Kokoro-TTS); selected text is
never sent to a server.

The first launch shows **Download voice**. That fetches the quantized Kokoro v1.0
ONNX model (~92 MB) and the `af_heart` voice (~0.5 MB) from the Apache-2.0
Hugging Face release into your user application-data directory. Downloads are
pinned to a model revision and SHA-256 verified before use.

## Build and run

Requirements: Rust and internet access for the first build/model download.

```sh
make test
make run
```

Then click **Download voice** on the pet.

### macOS app bundle

Requires macOS 13+ and the Xcode Command Line Tools:

```sh
make install
open "$HOME/Applications/Select to Speak.app"
```

Highlight text in Safari, Notes, Mail, or another native macOS app and choose
the Service.

If the Service is hidden, enable it in **System Settings → Keyboard →
Keyboard Shortcuts → Services → Text**. This is a macOS setting; the app does
not require Accessibility or screen-recording permission.

### Windows

```powershell
cargo build --release
.\target\release\select-to-speak.exe
```

### Linux

The pet supports X11 and Wayland. Install the normal desktop build dependencies
for winit plus one audio command: `pw-play`, `paplay`, `aplay`, or `ffplay`.

```sh
cargo build --release
./target/release/select-to-speak
```

The global shortcut library currently works on X11. Wayland intentionally
restricts global input hooks, so use the copy + pet context-menu fallback there.
Clipboard access needs the compositor's standard data-control protocol; the pet
shows an error if that protocol is unavailable.

For a terminal smoke test:

```sh
select-to-speak --install-model
select-to-speak --speak "Hello from Kokoro."
```

Set `SELECT_TO_SPEAK_MODEL_DIR` to use a different model directory.

## Architecture

The code uses a small hexagonal architecture:

```text
macOS Service / Windows-Linux hotkey / pet UI
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
- `adapters/macos_service.rs`: incoming macOS Services adapter.
- `platform.rs`: macOS Service or Windows/Linux global-shortcut bridge.
- `adapters/kokoro.rs`: outgoing Kokoro/ONNX adapter.
- `adapters/system_audio.rs`: outgoing macOS/Windows/Linux audio adapter.
- `ui.rs` and `worker.rs`: the floating pet and background command boundary.

The core has no AppKit, ONNX, filesystem, or process knowledge and is covered by
unit tests using in-memory port fakes.

## Scope

This first version is intentionally English-first. Kokoro exposes more languages
and voices, but voice selection, launch-at-login, interruption, installers, and
production signing/notarization are kept out of the MVP.
