# Intel macOS support feasibility

Research date: 2026-10-04. Scope: repository inspection, published Rust crate
source, and first-party ONNX Runtime documentation/source/releases. No native
runtime build, dependency change, workflow change, or Intel acceptance test was
performed.

## Conclusion

Intel macOS support is technically feasible. The current release fails because
Mist's resolved Rust package cannot download an Intel macOS runtime, not because
ONNX Runtime has removed the native Intel target. Microsoft's build instructions
explicitly describe Intel macOS builds; the exact `v1.28.0` source accepts
`x86_64` and retains an Intel runner branch in its reusable macOS CI workflow.
([Official macOS build instructions](https://onnxruntime.ai/docs/build/inferencing.html#macos),
[`v1.28.0` build arguments](https://github.com/microsoft/onnxruntime/blob/v1.28.0/tools/ci_build/build_args.py),
[`v1.28.0` macOS CI workflow](https://github.com/microsoft/onnxruntime/blob/v1.28.0/.github/workflows/macos-ci-build-and-test-workflow.yml))

The recommended first implementation experiment is a native Intel CPU-only
build of ONNX Runtime **1.28.0**, statically linked through the existing
`ort`/`ort-sys` **2.0.0-rc.13**. Keep the current model, multilingual voices,
Windows APIs, and Apple silicon Core ML path. This is a proposed route, not a
tested supported Mist distribution.

## What failed

The local [`Cargo.lock`](../Cargo.lock) resolves both Rust packages to
`2.0.0-rc.13`. The vendored
[`kokoro-micro` manifest](../third_party/kokoro-micro/Cargo.toml) requests
`ort = "2.0.0-rc.11"` without an exact pin and leaves its default features
enabled. Mist's [root manifest](../Cargo.toml) additionally enables Core ML for
every macOS architecture.

The resolved package's immutable
[download matrix](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/download/dist.tsv)
contains ARM macOS binaries built from ONNX Runtime 1.28.0, but no
`x86_64-apple-darwin` entry. The
[resolver](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/download/resolve.rs)
first filters by target, so disabling Core ML or enabling lax feature matching
cannot create an Intel CPU download.

The accompanying XCFramework message is a warning from an
[iOS framework probe](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/static_link/apple.rs):
it handles iOS target triples and warns on other Apple targets. The
[main build script](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/main.rs)
then continues to user-provided libraries or downloads. That warning does not
establish an Intel architecture limitation.

## Upstream support and available artifacts

The following are actual macOS C/C++ release assets, checked through the
version-specific release APIs; a hypothetical download URL is insufficient.

| ONNX Runtime release | macOS assets found | Consequence |
| --- | --- | --- |
| [1.28.0](https://api.github.com/repos/microsoft/onnxruntime/releases/tags/v1.28.0) | `onnxruntime-osx-arm64-1.28.0.tgz` only | No official Intel archive for the runtime currently supplied by `ort` |
| [1.24.2](https://api.github.com/repos/microsoft/onnxruntime/releases/tags/v1.24.2) | `onnxruntime-osx-arm64-1.24.2.tgz` only | Moving back one Rust release would still need another runtime source |
| [1.23.2](https://api.github.com/repos/microsoft/onnxruntime/releases/tags/v1.23.2) | ARM, `onnxruntime-osx-x86_64-1.23.2.tgz`, and universal2 archives | A real older Intel archive exists, with compatibility and maintenance costs |

Core ML also remains a possible Intel backend. Its official requirements cover
Mac computers with macOS 10.15+, and `CPUAndGPU` targets compatible GPUs without
requiring a Neural Engine. Microsoft's
[`v1.28.0` Core ML build definition](https://github.com/microsoft/onnxruntime/blob/v1.28.0/cmake/onnxruntime_providers_coreml.cmake)
uses Apple platform checks rather than an ARM-only restriction. A custom build
must enable the provider explicitly. These facts establish an available build
path, not accelerated execution of Mist's audio graph on a particular Intel
GPU. ([Core ML requirements, build instructions, and compute options](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html))

CPU is the appropriate initial Intel baseline. Mist's
[session loader](../third_party/kokoro-micro/src/device.rs) already creates a CPU
session and reports CPU when automatic provider initialization/model loading
falls back. An implementation should make Intel's initial CPU policy explicit
and preserve Apple silicon's Core ML selection. Removing the Intel Core ML
feature alone would not supply the missing runtime.

## Runtime and Rust API compatibility

`rc.13` defaults to `api-27`; its API feature chain determines the C API version
requested at runtime. Loading a library older than the requested API is rejected
by dynamic loading, and direct linking still requests that API through
`GetApi`. Therefore substituting ONNX Runtime 1.23.2 beneath the current feature
set would fail, even if Rust compilation/linking succeeds.
([Published `rc.13` manifest](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/Cargo.toml),
[API version selection](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/src/version.rs),
[runtime initialization](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/src/lib.rs),
[`v1.28.0` C API contract](https://github.com/microsoft/onnxruntime/blob/v1.28.0/include/onnxruntime/core/session/onnxruntime_c_api.h))

The older Rust versions do not provide a simple fix:

- `rc.12` defaults to API 24 and its download table contains only ARM macOS
  entries. ([Manifest](https://github.com/pykeio/ort/blob/v2.0.0-rc.12/Cargo.toml),
  [download table](https://github.com/pykeio/ort/blob/v2.0.0-rc.12/ort-sys/build/download/dist.txt))
- `rc.11` uses API 23 and `ndarray 0.17`, matching the vendored crate's array
  major/minor version. Source inspection found its session input types,
  `Tensor::from_array`, tensor extraction, model loading, and Core ML compute
  unit methods used by Mist. Its published download table still lacks Intel.
  This makes explicit linking to 1.23.2 a candidate for macOS, not proof of a
  working build. ([Published `ort` archive](https://crates.io/api/v1/crates/ort/2.0.0-rc.11/download),
  [published `ort-sys` archive](https://crates.io/api/v1/crates/ort-sys/2.0.0-rc.11/download),
  [API 23](https://github.com/pykeio/ort/blob/v2.0.0-rc.11/ort-sys/src/version.rs),
  [tensor creation](https://github.com/pykeio/ort/blob/v2.0.0-rc.11/src/value/impl_tensor/create.rs),
  [session inputs](https://github.com/pykeio/ort/blob/v2.0.0-rc.11/src/session/input.rs),
  [Core ML](https://github.com/pykeio/ort/blob/v2.0.0-rc.11/src/ep/coreml.rs))
- A **global `rc.11` downgrade is incompatible with Mist's current Windows
  source**: its published DirectML implementation exposes device selection but
  lacks `with_performance_preference` and `PerformancePreference`, which Mist's
  [provider builder](../third_party/kokoro-micro/src/device.rs) uses.
  ([Published `rc.11` archive](https://crates.io/api/v1/crates/ort/2.0.0-rc.11/download),
  [`rc.11` DirectML source](https://github.com/pykeio/ort/blob/v2.0.0-rc.11/src/ep/directml.rs))
- `rc.10` uses `ndarray 0.16`, while Mist passes owned `ndarray 0.17` arrays
  into `ort`. It is not an interchangeable dependency pin.
  ([`rc.10` manifest](https://github.com/pykeio/ort/blob/v2.0.0-rc.10/Cargo.toml),
  [Mist tensor inputs](../third_party/kokoro-micro/src/lib.rs))

Keeping `rc.13` but selecting `api-23` with defaults disabled is another
source-level candidate for a legacy 1.23.2 library: the macOS tensor/session
methods and Core ML registration used here do not require API 27. However,
Cargo feature unification must leave API 27 disabled for the Intel build;
adding API 23 alongside current defaults does not lower the requested version.
This configuration has not been compiled or exercised and is not recommended
ahead of building the current runtime.
([Feature chain](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/Cargo.toml),
[session options](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/src/session/builder/impl_options.rs),
[Core ML registration](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/src/ep/coreml.rs))

## Implementation routes

| Route | What it requires | Assessment |
| --- | --- | --- |
| Native 1.28.0 source build, static linking | Pinned upstream source and build dependencies; native Intel compiler; complete static archives/build tree supplied to `ort-sys`; explicit CPU policy | Recommended experiment. Keeps the current Rust API and runtime version; adds build time and a runtime cache/provenance obligation |
| Explicit pinned 1.28.0 shared library | An Intel library built from the same pinned source; explicit linker/library path; bundling and macOS loader configuration | Viable alternative if static archive integration proves difficult. Avoids the Pyke downloader but adds runtime packaging and signing work |
| Older 1.23.2 Intel archive | Explicit archive/checksum; compatible API feature selection or scoped Rust dependency changes; security/model/OS review | Conditional fallback. A dependency pin alone does not work, and a blanket Rust downgrade breaks Windows APIs |

The existing `ort-sys` build script checks `ORT_LIB_PATH`/`ORT_LIB_LOCATION`
before downloads, and supports dynamic preference through
`ORT_PREFER_DYNAMIC_LINK`. Its static linker recognizes both a single
`libonnxruntime.a` and a full ONNX Runtime build tree with dependency archives;
it contains an Intel macOS mapping and optional Core ML archive handling.
It does **not** compile ONNX Runtime automatically. Exact archive completeness,
dependency layout, and link success remain to be tested.
([Build precedence](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/main.rs),
[environment variables](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/vars.rs),
[static linking](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/static_link/mod.rs))

Dynamic delivery must put real libraries in the installed app, use paths valid
after installation, and sign nested code before the app is notarized. The Rust
helper's macOS development behavior creates symlinks beside build outputs; that
does not demonstrate a self-contained app bundle. Choosing `load-dynamic` also
requires explicit initialization before inference.
([Dylib helper](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/dynamic_link.rs),
[runtime initialization](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/src/lib.rs),
[Apple bundle guidance](https://developer.apple.com/documentation/xcode/embedding-nonstandard-code-structures-in-a-bundle),
[Apple notarization guidance](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution))

Redistribution must include the selected runtime's MIT license and the notices
for the dependencies actually included by its build, whether linkage is static
or dynamic. Mist's existing packaging resources do not explicitly list these
runtime files. ([`v1.28.0` license](https://github.com/microsoft/onnxruntime/blob/v1.28.0/LICENSE),
[`v1.28.0` third-party notices](https://github.com/microsoft/onnxruntime/blob/v1.28.0/ThirdPartyNotices.txt),
[current resources](../Cargo.toml))

## Acceptance gates before claiming support

1. **Native build and regression coverage.** Add ordinary Intel CI alongside
   ARM and run the project's required checks using the chosen runtime path.
   [Current CI](../.github/workflows/ci.yml) uses `macos-latest`, while the
   [platform release workflow](../.github/workflows/release-platform.yml) includes
   `macos-15-intel`. Verify the Rust graph, native archive architecture,
   requested C API, and runtime version. Record source/dependency checksums,
   compiler/SDK versions, cache inputs, and build duration. Cross-compilation
   alone cannot establish native behavior.
2. **Same-model inference and audio.** Preserve the model/voice hashes in the
   [existing backend research](INFERENCE_BACKEND_RESEARCH.md): ONNX IR 9,
   opset 20, dynamic tokens, LSTM/STFT and other audio operations, and the
   multilingual voice pack. Generate finite, non-silent 24 kHz English,
   Spanish, Japanese, and Mandarin audio on native Intel; check short and
   long/chunked input, voice/speed selection, cancellation, cold/warm latency,
   memory, and audible playback. Assert that the reported baseline is CPU.
   Operator/opset coverage does not establish audio parity or acceptable speed.
3. **Deployment target.** Mist currently advertises macOS 13.0. The general
   runtime build documentation states a 13.3 default, while the tagged 1.28.0
   official ARM packaging job sets 14.0; the older 1.23.2 packaging job sets
   13.4. Inspect the actual Mach-O minimum OS and every shipped library, then
   test the intended oldest Intel OS. Keeping 13.0 requires an explicitly
   configured compatible build and evidence; otherwise align metadata and
   documentation with the verified floor. None of those upstream numbers alone
   proves Mist's final minimum OS.
   ([General build documentation](https://onnxruntime.ai/docs/build/inferencing.html#macos),
   [`1.28.0` packaging target](https://github.com/microsoft/onnxruntime/blob/v1.28.0/tools/ci_build/github/azure-pipelines/templates/mac-cpu-packing-jobs.yml),
   [`1.23.2` packaging target](https://github.com/microsoft/onnxruntime/blob/v1.23.2/tools/ci_build/github/azure-pipelines/templates/mac-cpu-packing-jobs.yml))
4. **Installed signed package.** Build the Intel app/DMG, verify native
   architecture and dependencies, install on a clean Intel Mac, and exercise
   model download/cache, selection capture, Services/hotkey permissions,
   playback, and relaunch. Confirm notarization, stapling, Gatekeeper acceptance,
   and runtime notices. For a dylib route, repeat without build/cache/library
   directories available. The existing signing workflow needs validation for
   the actual new artifact layout.
5. **Legacy fallback review, if pursued.** Check applicable upstream advisories,
   dependency fixes, and model results before accepting 1.23.2. The 1.28.0
   release documents memory-safety/input-validation and dependency security
   fixes, plus an STFT correctness fix. This research has not established their
   applicability to Mist's pinned model or audited a legacy runtime; the older
   archive cannot be treated as an equivalent drop-in merely because it exists.
   ([1.28.0 release notes](https://github.com/microsoft/onnxruntime/releases/tag/v1.28.0))

Intel Core ML acceleration can be evaluated after the CPU baseline passes,
using a provider-enabled runtime and the exact same graph. Session/provider
registration, successful synthesis, and measured GPU execution/performance are
separate checks. The existing Apple silicon smoke results do not validate Intel.
