# Mist — living desktop voice specification

## Goal

Build a small Rust desktop application that stays visible as a living mist and
reads user-selected text aloud with the local Hexgrad Kokoro-82M
text-to-speech model.

## Platforms and interaction

- macOS, Windows, and Linux: the primary interaction is **select text in a
  supported app, then press Ctrl+Space**.
- macOS reads the focused accessibility element. The user is prompted once for
  Accessibility permission. The native AppKit Service remains available as an
  additional action in applications that expose macOS Services. When an
  Electron/Chromium app does not initially expose its focused element, Mist
  activates that app's accessibility tree and retries before classifying the
  result for automatic Copy fallback.
- Windows reads the focused control through Microsoft UI Automation.
- Linux X11 reads PRIMARY selection and registers the shortcut through X11.
- Linux Wayland asks the compositor for a shortcut through the XDG
  GlobalShortcuts portal, displays the gesture actually granted, and reads the
  compositor's primary selection through ext-data-control or wlr-data-control.
  The installed desktop entry provides the stable host identity required by the
  portal. If either protocol is unavailable, the pet reports the limitation and
  retains copied-text/manual-entry fallbacks.
- During normal operation the draggable, always-on-top surface contains only a
  translucent animated mist: no card, chrome, title, buttons, or permanent
  copy is visible on the desktop. The native window shadow, backing disk,
  outline, and decorative pulse rings are disabled. Its idle footprint stays
  compact and low-opacity so work remains legible beneath it; speech can make
  the same surface grow, become denser, and brighten before it returns to the
  compact footprint after playback, without expanding into a blocking card.
- The native mist is a soft circular vapor volume, not the website's horizontal
  field. A translucent circular texture provides volume while independent fine
  wisps flow through a feathered round envelope, with no hard border or backing
  disk. Audible playback advects the texture locally inside that circle and
  accelerates individual wisps; quiet speech still activates flow, louder audio
  strengthens it, and pause/completion gently settle it. Synthesis and idle
  galleries do not activate or reset the playback flow clock. A missing
  selection permission must not suppress animation of audio already playing.
  The installed application icon is a still of this same text-free design;
  the floating surface and voice swatches animate it.
- The mist changes movement and intensity with speech lifecycle and audio
  energy. Audio features are eased between playback windows so movement never
  jumps at the 40 ms sampling boundary; when idle, the compact mist keeps a
  clearly visible fluid breath and gentle internal texture drift without
  becoming visually intrusive. Speaking
  is visibly more expressive than idle—using stronger
  expansion, density, and directional flow—without abrupt phase changes. Each
  voice has a stable, recognisable color palette, texture crop, and flow pattern.
- Setup, permission failures, and actionable errors may temporarily expand into
  an accessible panel because the user must be able to recover without a
  terminal. Settings owns a left rail for Voices, Playback, Privacy, and Model.
  Language accordions contain their voice galleries on the Voices page so the
  two related decisions stay together. At most one language is expanded, and
  the open section can be collapsed without changing the selected voice.
  Accessibility recovery appears only in Privacy; it never traps the settings
  panel open. First-run model setup remains discoverable from Voices.
- A native macOS menu-bar / Windows and Linux system-tray menu provides voice
  selection, settings, status, and quit actions while keeping controls off the
  floating desktop surface. Voice choices behave as one exclusive group, so
  selecting one voice can never leave a second voice marked as selected.
- Captured text appears in a compact floating bubble queue directly beneath the
  mist in a separate transparent, always-on-top viewport. The queue is bounded,
  shows only short previews, and expands downward without resizing or moving
  the mist viewport. Each row is a small frosted pill with separate circular
  playback and delete controls. Dragging the pill moves only the queue window
  without stealing control clicks.
- Queue playback is automatic by default. A persisted setting switches to
  click-to-play. Choosing any waiting or failed item moves it to the top before
  playback while preserving the relative order of the remaining items.
  Successful playback removes the item; failed items remain available for an
  explicit retry. Active playback can be paused and resumed; pause, resume, and
  cancellation wake the system-player adapter immediately rather than waiting
  for the visual sampling interval. Deleting a waiting item removes only that
  item; deleting the active item stops its system player, releases any
  temporary clipboard lease, and waits for the worker cancellation
  acknowledgement before starting the next item.
- Direct OS selection capture runs first. A persisted, default-on setting may
  send the platform Copy shortcut only when direct capture fails. Mist clears
  that temporary clipboard value after playback only when both its content and
  platform change token, when available, still match. On desktops without a
  portable change token, cleanup is content-checked. Because the supported OS
  clipboard APIs do not share an atomic compare-and-clear operation, all
  automatic cleanup is documented as best effort and the fallback can be
  disabled for clipboard-sensitive workflows.

## Speech

- The shipped adapter uses the Apache-2.0 Kokoro-82M v1.0 ONNX model referenced
  by the user's Hugging Face Space. The application boundary remains
  model-neutral so a later speech engine can replace Kokoro without changing
  selection, queue, playback, or UI use-cases.
- Synthesis happens on-device. Selected text is not sent to a speech service.
- The default voice is `af_heart` at normal speed. Onboarding presents the
  adapter-provided voice catalog as visual mist choices, and the selected voice
  is persisted in the platform application-data directory. The Kokoro adapter
  exposes all 54 voices across American/British English, Spanish, French,
  Hindi, Italian, Japanese, Brazilian Portuguese, and Mandarin.
- Activating a voice card selects that voice and immediately streams a short
  local preview through the same speech-engine and system-audio path used for
  selected text. Voice cards remain available during a preview: activating a
  different card cancels the current token-scoped playback, drops stale queued
  preview requests, and starts the newest sample. Queue speech still protects
  itself from an unrelated preview.
- Changing voice affects subsequent speech without restarting the app.
- Selecting a voice from the native menu updates the one exclusive choice and,
  when the app is ready, immediately plays its preview without opening voice
  settings.
- Long selections are synthesized and played as ordered sentence chunks. The
  UI distinguishes generation of the first chunk from audible playback.
  The persisted Playback page calls these modes **Real-time** and **Complete
  audio**. Real-time is the default and begins after the first sentence chunk;
  Complete audio buffers the full generated selection before audio starts. A
  persisted 0.5×–3× speed setting applies across languages and voices.
- The current multilingual runtime prefers Core ML with CPU fallback on Apple
  silicon macOS, CUDA then DirectML then CPU on Windows, and CUDA then CPU on
  Linux. Explicit CPU remains available. The UI reports the backend from the
  loaded model session; Windows/Linux acceleration still requires native GPU
  acceptance evidence. WebGPU and MLX remain disabled when the shipped adapter
  cannot run them; the UI uses friendly Recommended, Standard, Accelerated,
  and Apple-optimized labels with the technical runtime in secondary detail.
  It never equates hardware branding with runtime support or claims unmeasured
  speed.
- The Model restart notice occupies header space and cannot intersect a
  provider row in the expanded settings viewport.
- The first-run model download is explicit and stored in the user's application
  data directory rather than committed to the source repository. The pinned,
  checksum-verified multilingual bundle is approximately 311 MiB for the model
  and 27 MiB for the combined voice pack.

## Architecture

- The implementation is Rust and follows ports-and-adapters (hexagonal)
  architecture.
- Domain and application code must not import GUI, OS, network, filesystem,
  ONNX, or audio-device libraries.
- OS selection, system tray, preferences, speech inference, model download, and
  audio playback are adapters behind the application boundary.
- Primary adapters are packaged under `adapters/inbound` by desktop, selection,
  and operating-system capability. The complete desktop UI belongs to the
  inbound desktop adapter rather than at the source root. Secondary adapters
  are packaged under `adapters/outbound` by speech, audio, persistence, and
  provisioning capability.
- Core code is packaged under `core/domain`, `core/application`, and
  `core/ports`; each layer is subdivided by capability instead of accumulating
  unrelated implementations in a single file.
- Voice IDs, model-specific defaults, localized preview copy, and engine
  construction belong to `VoiceCatalog`, `SpeechEngineFactory`, and
  `ModelProvisioner` outbound ports. Kokoro-specific catalog data and runtime
  construction must not appear in domain, application, worker, or UI logic.
- Primary selection capture must not modify or reconstruct the user's regular
  clipboard. Clipboard mutation belongs only to the explicit, configurable
  fallback adapter. Cleanup must check the captured value and any available
  platform change token immediately before clearing, and must not claim atomic
  preservation guarantees the OS does not provide.
- Slow model loading, synthesis, download, and playback must not block the pet's
  UI event loop.
- On macOS the frontmost application PID is captured on the UI thread before
  selection work moves to its worker. The worker must not call AppKit or
  HIToolbox; automatic Command-C uses CoreGraphics events directly.
- Playback control uses an out-of-band, token-scoped application handle because
  synchronous platform playback blocks the speech worker. OS process pause and
  termination remain inside the audio adapter.

## Acceptance checks

- Domain and application use-cases have unit tests with in-memory adapters.
- The native target passes `cargo check`, `cargo test`, and clippy with warnings
  denied.
- Repository hooks run formatting, check, strict Clippy, tests, coverage-backed
  CRAP analysis, the pinned Smells v0.5.0 policy, OSV dependency scanning, and
  staged-secret scanning before commits. CRAP scores of 5 or higher emit warnings;
  new, regressed, or newly boundary-crossing scores of 10 or higher block against the reviewed committed
  baseline. The pre-push hook adds a full-repository Smells scan and
  full-history secret scan, then keeps package and native bundle SemVer
  metadata synchronized. Handwritten platform FFI remains in scanned Rust
  source rather than being hidden by policy exclusions.
- A macOS `.app` bundle advertises the text Service through `Info.plist`.
- The bundled mist texture is embedded in the executable so packaged builds
  cannot silently omit it.
- After onboarding, the floating surface renders only mist. The tray/menu-bar
  menu can reopen voice settings, and setup or error panels remain keyboard and
  screen-reader legible.
- Every voice card exposes a keyboard-focusable selection action and routes the
  exact card voice to the active speech adapter. A second activation can replace
  a playing preview immediately.
- Playback-synchronised, time-windowed loudness and brightness reach
  presentation state only after the platform audio player has actually
  started.
- **Ctrl+Space** is requested on every OS. Wayland displays the gesture actually
  granted by the compositor, and unsupported portal capabilities fail visibly.
- Every successful hotkey capture enters the visible queue before playback.
  Automatic and click-to-play modes persist across launches. Queue pills remain
  draggable and expose keyboard-accessible play/pause and delete controls.
- Failures classified by the platform adapter as permission or protected
  content never trigger automatic Copy fallback.
- The repository documents build and usage instructions for macOS, Windows, and
  Linux, including the Wayland limitation.

## Deferred

Launch at login is outside this version. The release workflow builds graphical
installers and enforces macOS notarization and Windows signing, but public
publishing remains gated on signing credentials, protected release
environments, and clean-machine acceptance on each supported operating system.
Native end-to-end validation on hardware not available to this macOS build
machine remains an external release prerequisite.
