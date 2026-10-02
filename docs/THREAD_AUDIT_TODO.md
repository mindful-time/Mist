# Mist thread-audit ledger

Audited on 2026-09-27 from Codex thread
`01a0de4d-0156-7573-8a78-fe8d8dce5b86` (host `local`, title
“Build a Rust select-to-speak app”) by replaying every available thread page
and reconciling the requests with the current working tree, commit history,
`README.md`, `docs/SPEC.md`, and `docs/SELECTION_CAPTURE_RESEARCH.md`.

This is the implementation checklist for the current turn. Checked items are
already captured by committed code and documentation; unchecked items are
either defects, stale records, or verification work still required.

## Accepted product decisions

- [x] **decision · captured** — The product is named **Mist** and uses a
  compact, translucent, audio-reactive mist as its normal desktop surface.
- [x] **decision · captured** — The shared implementation is Rust with
  hexagonal boundaries; macOS, Windows, and Linux selection/audio integration
  remains behind platform adapters.
- [x] **decision · captured** — The main gesture is select text in any
  supported application and press **Ctrl+Space**.
- [x] **decision · captured** — Native selection capture is attempted first.
  A default-on, configurable Copy fallback is allowed only after eligible
  direct-capture failures, with best-effort, identity-checked cleanup.
- [x] **decision · captured** — Captured selections enter a separately
  draggable queue beneath the mist. Queue items can be played, paused, resumed,
  selected out of order, and deleted without moving the mist.
- [x] **decision · captured** — Queue autoplay and Real-time playback default
  to on and are persisted; Complete audio and global playback speed are
  available from Playback.
- [x] **decision · captured** — Settings use a left rail for Voices, Playback,
  Privacy, and Model. Language sits above Voice in the combined Voices page.
  Unsupported compute providers are visibly disabled rather than presented as
  working controls.
- [x] **decision · captured** — Selecting another voice immediately interrupts
  the current preview and starts the latest request.
- [x] **decision · captured** — Kokoro exposes 54 voices across nine language
  families, backed by multilingual synthesis smoke tests.
- [x] **decision · captured** — Accessibility setup belongs in Privacy and no
  longer forces a closed settings panel open.
- [x] **decision · captured** — Hexagonal package ownership is explicit:
  model-neutral domain/application/ports live in `core`, desktop UI and OS
  selection live in primary `adapters/inbound` packages, and speech, audio,
  persistence, and provisioning live in secondary `adapters/outbound`
  packages. `main.rs` remains the composition root.

## Defects to diagnose and fix

- [x] **UI-1 · duplicate ownership** — Remove the settings-level
  Accessibility callout while preserving recovery inside Privacy and keeping
  first-run model installation discoverable in Voices.
- [x] **UI-2 · layout collision** — Move the Model restart-required message
  out of the provider list so it cannot overlap the CPU row in Settings.
- [x] **AUDIO-1 · preview cancellation** — Keep voice cards interactive during
  a preview; cancel the active preview through the token-scoped playback
  controller and replace it with the newly selected voice without stale audio
  queueing behind it.
- [x] **LANG-1 · false English-only capability** — Add a real multilingual
  Kokoro runtime/catalog and persisted language selection. Do not merely enable
  controls for backends or voices that cannot synthesize.
- [x] **DOC-1 · stale product records** — Update `README.md` and
  `docs/SPEC.md` for interruptible previews, multilingual support, current
  Settings ownership, and actual model artifacts.

## Verification and delivery

- [x] Add deterministic regression checks for each defect and observe them fail
  before the corresponding fix.
- [x] Run formatting, native checks, strict Clippy, all tests, OSV, Gitleaks,
  and the 0.1.0 release-version gate.
- [ ] Compile-check the supported Windows and Linux targets when their Rust
  target toolchains are installed; record unavailable native runtime coverage
  honestly.
- [ ] Exercise the rebuilt macOS app: Settings navigation, provider layout,
  preview replacement, language/voice selection, Ctrl+Space capture, queue
  play/pause/delete, and clipboard fallback.
- [ ] Review the final diff against repository standards and this ledger.
- [ ] Commit the verified work, rebuild the signed bundle, and install the
  committed app without changing its established Accessibility identity.

## Open risks, not silently treated as complete

- **issue/risk · open-by-design** — Windows and Linux have compile-time and
  adapter tests in this macOS workspace, but final native OS interaction still
  requires testing on real Windows and Linux desktops.
- **verification · partial** — Apple silicon macOS now prefers Core ML and the
  exact multilingual production model passed a real Rust synthesis smoke test,
  with CPU retained as fallback. Physical GPU node assignment still needs
  profiling. Windows now compiles CUDA -> DirectML -> CPU and Linux compiles
  CUDA -> CPU; both still need exact-model tests on real GPU hardware. WebGPU
  and MLX remain disabled; see `docs/INFERENCE_BACKEND_RESEARCH.md`.
- **verification · open-by-design** — macOS Accessibility authorization is
  tied to the installed app identity. Ad-hoc development bundles cannot prove
  the canonical signed install retains permission across rebuilds.

## Superseded or rejected directions

- **superseded** — Right-click/context-menu activation as the only primary
  interaction. Ctrl+Space is the cross-platform primary gesture; a native
  macOS Service can remain an optional alternative.
- **superseded** — A large mechanical control panel attached to the mist. The
  normal desktop surface remains mist-only; queue controls and Settings are
  separate surfaces.
- **superseded** — Waiting for a voice sample to finish before choosing another
  voice. Preview replacement is immediate.
- **rejected** — Claiming an accelerator or language is supported based only on
  device branding or disabled UI. Capability labels must be backed by the
  active runtime.

No `CONTEXT.md`, ADR/DDR directory, open-question file, issue ledger, or prior
TODO tracker was present. This ledger records the uncaptured decisions without
inventing additional architectural decisions.
