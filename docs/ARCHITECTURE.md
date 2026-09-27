# Mist architecture

Mist uses ports and adapters (hexagonal architecture), following the primary
and secondary adapter terminology in the
[AWS Prescriptive Guidance](https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/hexagonal-architecture.html).
The Rust core owns the product vocabulary, use cases, and the ports those use
cases require. Operating-system APIs, Kokoro, audio processes, downloads, and
filesystem preferences are replaceable adapters.

## Dependency direction

```text
desktop UI / Ctrl+Space / OS services
                 |
                 v
        adapters/inbound
                 |
                 v
             core
     domain + use cases + ports
                 ^
                 |
        adapters/outbound
 speech model / audio / storage / model files
```

The dependency rule points inward: adapters may depend on `core`; `core` must
not depend on adapters, a particular operating system, a GUI framework, or
Kokoro. `main.rs` is the composition root that chooses concrete adapters and
wires them to the core ports.

## Source layout

```text
src/
  core/
    application/
      models.rs      model-installation use case
      playback.rs    cancellation, pause, and session coordination
      speech.rs      selection and preview speech use cases
    domain/
      audio.rs       playback-synchronised audio features
      inference.rs   opaque provider identities and capabilities
      playback.rs    playback preferences and speed
      queue.rs       speech queue state machine
      selection.rs   validated selected text and capture errors
      voice.rs       model-neutral voice and language values
    ports/
      audio.rs       audio-output port
      models.rs      model-provisioning port
      speech.rs      synthesis, engine-factory, and catalog ports
  adapters/
    inbound/
      desktop/
        platform.rs  desktop input orchestration
        ui/          mist, queue, settings, and tray presentation
      selection/
        clipboard.rs temporary Copy fallback
      os/
        macos/       Accessibility, Service, and global shortcut
        windows/     Windows UI Automation selection
        linux/       X11/Wayland selection and portal shortcut
    outbound/
      audio/
        system.rs    native audio-process implementation
      persistence/
        inference.rs selected inference provider
        playback.rs  queue, mode, and speed preferences
        voice.rs     selected voice preference
      provisioning/
        kokoro.rs    verified Kokoro artifact provisioning
      speech/
        kokoro/
          mod.rs     Kokoro/ONNX speech-engine implementation
          catalog.rs Kokoro languages, voices, and preview copy
  runtime/
    worker.rs        long-lived desktop orchestration
  main.rs            composition root
```

The inbound side contains the primary adapters that drive the application:
desktop UI, global shortcuts, OS Services, and selected-text capture. The
outbound side contains the secondary adapters driven through core-owned ports:
speech engines, audio output, persistence, and model provisioning. Root-level
module re-exports remain temporarily available for compatibility, but new code
must use the canonical capability packages.

## Replacing Kokoro

A speech-model adapter supplies these core ports:

- `SpeechEngineFactory` creates a warm `SpeechSynthesizer`.
- `VoiceCatalog` supplies model-owned language, voice, preview, and palette data.
- `ModelProvisioner` verifies or installs the adapter's artifacts.

The composition root injects those implementations into the runtime worker.
Core types use opaque `VoiceId` and `InferenceProviderId` values, so adding a
different local model does not add model-specific enums or branches to the
domain. A replacement can live beside Kokoro under `adapters/outbound/speech`
and be selected at composition time.

## Platform acceleration

Provider availability is evidence-based. An adapter reports only providers it
can actually initialize on the current device; unsupported choices stay
disabled in the UI. CPU remains the portable fallback. Adding CoreML,
DirectML, CUDA, WebGPU, or MLX therefore belongs in an outbound speech adapter,
never in the domain or UI.

## References

- [AWS Prescriptive Guidance: Hexagonal architecture pattern](https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/hexagonal-architecture.html)
- [Alistair Cockburn: Hexagonal Architecture](https://alistaircockburn.com/Articles/Hexagonal-Architecture)
