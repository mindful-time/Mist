# Select to Speak — MVP specification

## Goal

Build a small Rust desktop application that stays visible like a pet and reads
user-selected text aloud with the local Hexgrad Kokoro-82M text-to-speech model.

## Platforms and interaction

- macOS: selected text is accepted through a native AppKit Service so the user
  can choose **Speak Selection with Kokoro** from the text context menu.
- Windows and Linux X11: selected text is captured when the user presses the
  global shortcut **Ctrl+Alt+S**.
- Linux Wayland: because compositors restrict universal selection and global
  key hooks, copied text can be spoken from the pet's context menu when the
  compositor exposes clipboard data-control. Compositors without data-control
  are outside the selected/copied-text workflow supported by this MVP; the pet
  still offers manual text entry as a degraded mode.
- The draggable, always-on-top pet shows setup, loading, speaking, ready, and
  error states without opening a conventional application window.

## Speech

- Use the Apache-2.0 Kokoro-82M v1.0 ONNX model referenced by the user's
  Hugging Face Space.
- Synthesis happens on-device. Selected text is not sent to a speech service.
- The default voice is `af_heart` at normal speed.
- The first-run model download is explicit and stored in the user's application
  data directory rather than committed to the source repository.

## Architecture

- The implementation is Rust and follows ports-and-adapters (hexagonal)
  architecture.
- Domain and application code must not import GUI, OS, network, filesystem,
  ONNX, or audio-device libraries.
- OS selection, Kokoro inference, model download, and audio playback are
  adapters behind the application boundary.
- Selection capture must not modify or reconstruct the user's clipboard.
- Slow model loading, synthesis, download, and playback must not block the pet's
  UI event loop.

## Acceptance checks

- Domain and application use-cases have unit tests with in-memory adapters.
- The native target passes `cargo check`, `cargo test`, and clippy with warnings
  denied.
- A macOS `.app` bundle advertises the text Service through `Info.plist`.
- The repository documents build and usage instructions for macOS, Windows, and
  Linux, including the Wayland limitation.

## Deferred

Voice/language selection, launch at login, interrupting current speech,
production signing/notarization, and graphical installers are outside this MVP.
