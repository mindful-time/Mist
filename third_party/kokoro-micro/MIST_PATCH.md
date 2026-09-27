# Mist's Kokoro Micro patch

This directory vendors `kokoro-micro` 1.3.0 from crates.io under its original
Apache-2.0 license. Mist keeps the upstream public API and multilingual data,
and adds one execution-provider feature that upstream 1.3.0 does not expose:

- `coreml`: compiles ONNX Runtime's Core ML provider and configures
  `CPUAndGPU` compute units. `Device::Auto` tries Core ML first and falls back
  to ONNX CPU if registration or exact-model loading fails.

The unmodified source release corresponds to upstream commit
`5381b4f7f0e3bb20e9412b16fc0d13a6920ca656`. The functional patch is confined
to `Cargo.toml` and `src/device.rs`; `LICENSE` and `NOTICE` are retained.

Mist's ignored `tests/kokoro_smoke.rs` check loads the pinned production model,
asserts the reported Core ML backend on Apple Silicon, and synthesizes English,
Spanish, Japanese, and Mandarin samples.
