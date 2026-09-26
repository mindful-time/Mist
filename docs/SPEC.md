# Select to Speak — MVP specification

## Goal

Build a small Rust desktop application that stays visible like a pet and reads
user-selected text aloud with the local Hexgrad Kokoro-82M text-to-speech model.

## Platforms and interaction

- macOS, Windows, and Linux: the primary interaction is **select text anywhere,
  then press Ctrl+Alt+S**.
- macOS reads the focused accessibility element. The user is prompted once for
  Accessibility permission. The native AppKit Service remains available as an
  additional action in applications that expose macOS Services.
- Windows reads the focused control through Microsoft UI Automation.
- Linux X11 reads PRIMARY selection and registers the shortcut through X11.
- Linux Wayland asks the compositor for a shortcut through the XDG
  GlobalShortcuts portal, displays the gesture actually granted, and reads the
  compositor's primary selection through ext-data-control or wlr-data-control.
  The installed desktop entry provides the stable host identity required by the
  portal. If either protocol is unavailable, the pet reports the limitation and
  retains copied-text/manual-entry fallbacks.
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
- Primary selection capture must not modify or reconstruct the user's regular
  clipboard.
- Slow model loading, synthesis, download, and playback must not block the pet's
  UI event loop.

## Acceptance checks

- Domain and application use-cases have unit tests with in-memory adapters.
- The native target passes `cargo check`, `cargo test`, and clippy with warnings
  denied.
- A macOS `.app` bundle advertises the text Service through `Info.plist`.
- **Ctrl+Alt+S** is requested on every OS. Wayland displays the gesture actually
  granted by the compositor, and unsupported portal capabilities fail visibly.
- The repository documents build and usage instructions for macOS, Windows, and
  Linux, including the Wayland limitation.

## Deferred

Voice/language selection, launch at login, interrupting current speech,
production signing/notarization, and graphical installers are outside this MVP.
