# Inference backends across macOS, Windows, and Linux

Research date: 2026-09-27. This note distinguishes hardware/API availability
from a backend that Mist has actually integrated and validated.

## Decision summary

| Target | Backend candidate | Current Mist support | Same ONNX model and voice pack? | Honest status |
| --- | --- | --- | --- | --- |
| All supported targets | ONNX Runtime CPU | Yes | Yes | Production baseline |
| Apple silicon macOS | Core ML | Yes | Yes, subject to graph partitioning | Integrated; exact multilingual model/session smoke passed locally |
| macOS / Windows / Linux | Native ONNX Runtime WebGPU | No | Yes, subject to graph partitioning | Best single cross-platform experiment |
| Apple silicon macOS; Linux CPU/NVIDIA | MLX | No | No: the graph and weights need an MLX port | Separate future engine; no documented Windows distribution |
| Windows with NVIDIA | CUDA | GPU-first automatic path compiled; no hardware validation recorded | Yes | CUDA runtime/GPU acceptance still required |
| Windows with a DirectX 12 GPU | DirectML | Automatic fallback after CUDA is integrated | Yes, subject to provider coverage | Native compile covered by CI; hardware acceptance still required |
| Linux with NVIDIA | CUDA | GPU-first automatic path compiled; no hardware validation recorded | Yes | CUDA runtime/GPU acceptance still required |
| Linux with Vulkan GPU/driver | Native ONNX Runtime WebGPU | No | Yes, subject to provider coverage | Best broad Linux experiment |
| Linux with AMD ROCm | MIGraphX | No | Yes, subject to provider coverage | Vendor-specific later option; not the smallest path |

Hardware or API presence is not backend support. Apple silicon, Metal, DirectX,
Vulkan, CUDA drivers, Safari WebGPU, or a graphics dependency in the lockfile
does not prove that Mist's speech graph is executing there. The UI may enable an
accelerator only after the packaged provider loads the exact model and reports
itself as the active backend.

## What ships today

Mist vendors and documents a narrow patch over `kokoro-micro = 1.3.0`. The
published upstream crate exposes only `cuda = ["ort/cuda"]`; its provider list
contains only `CUDAExecutionProvider`, and an empty provider list resolves
`Auto` to CPU. See the immutable upstream
[`1.3.0` published-source manifest](https://github.com/DavidValin/kokoro-micro/blob/5381b4f7f0e3bb20e9412b16fc0d13a6920ca656/Cargo.toml#L24-L47)
and
[`device.rs`](https://github.com/DavidValin/kokoro-micro/blob/5381b4f7f0e3bb20e9412b16fc0d13a6920ca656/src/device.rs#L130-L173).
The upstream README makes the original contract explicit: without the `cuda`
feature, inference is CPU-only
([Kokoro Micro device documentation](https://github.com/DavidValin/kokoro-micro#choosing-a-device)).

Mist's patch adds Core ML and DirectML feature gates and makes `Device::Auto`
try the compiled providers in platform order before creating a CPU session:
Core ML -> CPU on Apple silicon macOS, CUDA -> DirectML -> CPU on Windows, and
CUDA -> CPU on Linux. Explicit CPU remains available. A provider is skipped if
it cannot register or load the exact model. The loaded synthesizer exports
`TtsEngine::backend()` through the speech port, so the UI receives the backend
from the created model session instead of echoing the requested policy.

On the current Apple silicon Mac, the pinned production model and voice pack
loaded as `CoreMLExecutionProvider (device 0)` and generated finite, non-silent
24 kHz English, Spanish, Japanese, and Mandarin samples. That validates the
Core ML session path and multilingual parity. It does not separately prove how
many graph nodes ran on the physical GPU. Windows and Linux builds still need
real GPU acceptance runs; compile-only CI cannot establish acceleration.

WebGPU, MLX, and MIGraphX remain disabled. Mist does not expose a backend merely
because the host has a compatible graphics API or hardware brand.

## Exact multilingual artifact contract

The pinned model (SHA-256
`7d5df8ecf7d4b1878015a32686053fd0eebe2bc377234608764cc0ef3636a6c5`) is
ONNX IR 9, opset 20. Direct inspection found a dynamic
`tokens[1, sequence_length]` input, fixed `style[1,256]`, scalar `speed`, and
2,464 graph nodes. Its operators include convolution, transposed convolution,
LSTM, STFT, cumulative sum, nonzero, layer normalization, resize, and dynamic
shape operations. Dynamic shapes and less common audio operators make
per-provider testing mandatory even when a provider advertises opset 20.

All ONNX execution-provider paths can keep the same model, 54-voice NPZ pack,
and multilingual behavior. Kokoro Micro performs language-specific
phonemization, tokenization, chunking, and voice-style lookup on the CPU before
calling ONNX Runtime with only `tokens`, `style`, and `speed`; provider choice
changes the session, not the language pipeline
([engine and voice state](https://github.com/DavidValin/kokoro-micro/blob/5381b4f7f0e3bb20e9412b16fc0d13a6920ca656/src/lib.rs#L115-L129),
[preprocessing](https://github.com/DavidValin/kokoro-micro/blob/5381b4f7f0e3bb20e9412b16fc0d13a6920ca656/src/lib.rs#L348-L466),
[ONNX inputs](https://github.com/DavidValin/kokoro-micro/blob/5381b4f7f0e3bb20e9412b16fc0d13a6920ca656/src/lib.rs#L546-L613)).
An MLX implementation is the exception: it can reuse the Rust language/voice
semantics, and MLX can read NPZ arrays, but the ONNX graph itself must be
reimplemented and its weights converted and verified.

## macOS: Core ML first, WebGPU as the shared experiment

ONNX Runtime's Core ML provider supports macOS 10.15+, can use Apple CPU, GPU,
and Neural Engine, and accepts an explicit `CPUAndGPU` compute-unit policy. It
also warns that dynamic shapes may reduce performance and provides
`ProfileComputePlan` to report where operators are dispatched
([Core ML provider requirements and options](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)).
The Rust `ort` version resolved in this repository exposes Core ML, and its
download manifest contains ARM64 macOS packages for Core ML alone and Core ML
plus WebGPU
([Rust feature list](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/Cargo.toml#L95-L112),
[binary matrix](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/download/dist.tsv#L1-L4)).

A preliminary diagnostic probe on the current ARM64 Mac loaded Mist's exact
model with ONNX Runtime 1.19.2 using `CoreMLExecutionProvider` plus CPU
fallback. ONNX Runtime reported 129 Core ML partitions and 1,048 supported
nodes out of 2,476
optimized nodes; a four-token inference returned finite audio. This is useful
evidence that the artifact can be partitioned, but it is **not** acceptance:
the probe used an older Python runtime than Mist's pinned Rust runtime, did not
use a real voice/text sample, delegated more than half the optimized graph
outside Core ML, and did not prove which Core ML compute unit ran each node.

Mist now implements the first production step: it registers Core ML with the
`CPUAndGPU` policy, keeps CPU as the automatic fallback, and exposes the loaded
session backend. The real Rust adapter's exact-model multilingual smoke closes
the earlier Python probe's model/runtime gap. A later performance acceptance
test should still use `ProfileComputePlan` or equivalent instrumentation to
measure physical GPU assignment and benchmark representative text. MLX is not
the first implementation step because it cannot reuse the ONNX graph.

Intel macOS needs a separate decision. Core ML itself supports compatible Intel
Macs, but the pinned Rust binding's published download matrix contains no
`x86_64-apple-darwin` package, and MLX requires Apple silicon. Supporting Intel
would therefore require a separately built ONNX Runtime distribution and native
testing; CPU is the only currently evidenced fallback.

## Windows: DirectML for breadth, CUDA for NVIDIA

CUDA works only
with NVIDIA GPUs and a matching ONNX Runtime/CUDA/cuDNN runtime. ONNX Runtime
1.28 GPU packages use CUDA 13 and cuDNN 9 by default, and CUDA/cuDNN major
compatibility is strict
([CUDA provider requirements](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)).
The pinned Rust download matrix has an x86-64 Windows CUDA 13 package, but not a
Windows ARM64 CUDA package. CUDA should therefore be an optional NVIDIA-capable
artifact or provider, not a requirement for the general Windows installer.

For the broad Windows path, DirectML accepts DirectX 12 devices from NVIDIA,
AMD, Intel, and Qualcomm and is available from Windows 10 version 1903. It
supports ONNX opset 20, matching Mist's model, but requires sequential session
execution and disabled memory-pattern optimization
([DirectML provider](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)).
The pinned Rust binary matrix includes DirectML packages for Windows x86-64 and
ARM64. DirectML is in sustained engineering and Microsoft recommends WinML for
new Windows deployments, but using the existing Rust ONNX boundary is the
smaller change than introducing a separate WinRT model host
([Windows guidance](https://onnxruntime.ai/docs/get-started/with-windows.html)).

Mist now compiles the practical provider order: CUDA (only when its complete
runtime and model load succeed), then DirectML, then CPU. WebGPU over
D3D12/Vulkan is a useful cross-platform experiment, but should not displace the
established DirectML path until it wins the same exact-model tests. The pinned
Rust matrix supplies WebGPU for Windows x86-64, not ARM64.

## Linux: CUDA for NVIDIA, WebGPU for a cross-vendor path

On x86-64 NVIDIA systems the existing CUDA path is the smallest acceleration
change, with the same CUDA/cuDNN compatibility and packaging obligations as
Windows. Linux additionally needs the required runtime libraries (including
zlib for cuDNN 9) available to the packaged ONNX Runtime
([ONNX Runtime install requirements](https://onnxruntime.ai/docs/install/)).
The pinned Rust matrix supplies CUDA 13 for Linux x86-64, but not ARM64.

For AMD and Intel GPUs, native ONNX Runtime WebGPU is the smallest shared
experiment because Dawn uses Vulkan on Linux. It still requires a working
Vulkan driver and a packaged WebGPU provider, and the exact Kokoro graph may be
partly assigned to CPU. The pinned Rust matrix includes a WebGPU runtime for
Linux x86-64, not ARM64.

The old ONNX Runtime ROCm provider is not a future path: it was removed in ONNX
Runtime 1.23, and the project directs AMD users to MIGraphX
([ROCm retirement](https://onnxruntime.ai/docs/execution-providers/ROCm-ExecutionProvider.html)).
MIGraphX can accelerate ONNX on AMD GPUs, but it brings a versioned ROCm stack
and the pinned Rust download matrix does not provide a ready MIGraphX bundle
([MIGraphX provider](https://onnxruntime.ai/docs/execution-providers/MIGraphX-ExecutionProvider.html)).
It is a later vendor-specific adapter only if WebGPU cannot meet correctness or
latency targets.

## MLX: possible, but it is a separate Kokoro port

MLX is a machine-learning array framework optimized for Apple silicon; its
supported macOS installation requires Apple silicon and macOS 14 or newer.
It provides CPU and GPU devices using unified memory. Current MLX 0.32.2 also
publishes Linux CPU and NVIDIA CUDA 12/13 packages, subject to its documented
GPU architecture, driver, toolkit, and glibc requirements; the official install
matrix does not document a Windows distribution
([MLX overview](https://ml-explore.github.io/mlx/build/html/),
[installation requirements](https://ml-explore.github.io/mlx/build/html/install.html)).
Those official device claims do not include the Apple Neural Engine, so Mist
must not market an MLX adapter as ANE inference without separate evidence.
The development Mac inspected for this report is ARM64 on macOS 26.6.2 and
therefore meets MLX's platform prerequisites, but the adapter is still absent.

MLX is not an ONNX execution provider. Its documented loader accepts `.npy`,
`.npz`, `.safetensors`, and `.gguf`, not `.onnx`
([`mlx.core.load`](https://ml-explore.github.io/mlx/build/html/python/_autosummary/mlx.core.load.html)).
Consequently Mist cannot point MLX at its current `kokoro-v1.0.onnx` artifact.
MLX can instead export an MLX function as `.mlxfn` and import that function
from C++, but the graph first has to exist in MLX
([MLX export documentation](https://ml-explore.github.io/mlx/build/html/usage/export.html)).

A real MLX backend would require all of the following:

1. Implement Kokoro's neural graph with MLX operations and convert/verify the
   model weights in a documented MLX-loadable format.
2. Preserve or deliberately replace Kokoro Micro's multilingual phonemization,
   voice-style lookup, token limits, chunking, speed control, and 24 kHz output
   contract.
3. Package a native boundary. The most suitable desktop options are a small
   C/C++ bridge to MLX or a Swift bridge; MLX's C++ library currently has to be
   built from source and the app must package `mlx.metallib`
   ([MLX C++ build instructions](https://ml-explore.github.io/mlx/build/html/install.html#c-api)).
   MLX C exposes opaque arrays, CPU/GPU streams, and operations suitable for an
   FFI boundary
   ([MLX C overview](https://ml-explore.github.io/mlx-c/build/html/overview.html)).
4. Implement it as a separate `SpeechEngineFactory`/`SpeechSynthesizer` adapter,
   leaving the application and domain ports unchanged.
5. Validate multilingual audio against the current engine and measure cold
   start, first-audio latency, real-time factor, memory, and power use on each
   supported Apple chip before calling it faster.

This is substantially more work than selecting another ONNX provider. It is a
reasonable future adapter—especially for Apple silicon—but not a cross-platform
Windows solution or a switch that can be added to the current Kokoro Micro
session.

## WebGPU: one API family across all three OSes, but not connected to Mist

There are two different facts that should not be conflated:

- `wgpu` is a native Rust graphics/compute API based on WebGPU. Its native
  backends include Metal on macOS, Direct3D 12 on Windows, and Vulkan on
  Windows/Linux
  ([wgpu supported platforms](https://github.com/gfx-rs/wgpu#supported-platforms)).
  It does not parse or execute ONNX models by itself; using it directly would
  require implementing the model's compute kernels and graph execution.
- ONNX Runtime now has a **native WebGPU execution provider**. It uses Dawn and
  maps macOS to Metal, Windows to Direct3D 12 or Vulkan, and Linux to Vulkan.
  Under the current upstream native packaging contract, applications
  bundle/register its provider shared library or build ONNX Runtime with
  `--use_webgpu`
  ([ONNX Runtime WebGPU EP](https://onnxruntime.ai/docs/execution-providers/WebGPU-ExecutionProvider.html)).

The second path is the relevant one for Mist because it can retain the ONNX
model. The Rust `ort` version resolved by Mist's dependency has a WebGPU wrapper
and advertises WebGPU runtimes for ARM64 macOS and x86-64 Windows/Linux
([wrapper](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/src/ep/webgpu.rs#L77-L174),
[distribution manifest](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/download/dist.tsv#L1-L17)).
That makes a Rust spike plausible, but does not make the binding-specific bundle
interchangeable with the upstream plugin or enable it in Kokoro Micro. Kokoro
Micro 1.3.0 never registers the WebGPU builder. A real implementation therefore
needs a new ONNX/WebGPU engine adapter (or an upstream Kokoro Micro feature), a
version-compatible runtime/provider packaged and signed with Mist, provider
registration at session creation, truthful active-provider reporting, and CPU
fallback.

The exact Kokoro model is **not yet validated** on the WebGPU provider. ONNX
Runtime assigns only supported nodes/subgraphs to an execution provider
([execution-provider architecture](https://onnxruntime.ai/docs/execution-providers/));
unsupported nodes may remain on CPU. The model's `tokens` input is `int64`, and
the WebGPU provider requires the corresponding device feature when enabling
64-bit integer support. Its graph-capture mode also requires all nodes to be on
WebGPU and static shapes, conditions Mist's variable text length and initial CPU
fallback do not meet. Graph capture should therefore remain off for the first
spike
([WebGPU provider configuration](https://onnxruntime.ai/docs/execution-providers/WebGPU-ExecutionProvider.html)).
Before enabling this option, a spike must:

1. load Mist's exact pinned ONNX model with the native WebGPU provider;
2. synthesize the multilingual smoke-test corpus;
3. inspect provider assignment/profiling for CPU fallback;
4. compare waveforms and perceptual output with CPU; and
5. benchmark first-audio latency and sustained real-time factor.

A diagnostic with ONNX Runtime 1.30.0's official ARM64 macOS WebGPU plugin
registered `WebGpuExecutionProvider` and enumerated a GPU-typed provider
device, but Dawn then failed to acquire a supported adapter during session
creation. ONNX Runtime visibly fell back to CPU, where the exact model loaded
and returned finite audio. This does **not** show that macOS lacks WebGPU; it
shows only that this provider package and execution context could not create an
adapter. Mist must therefore gate WebGPU availability on successful adapter
creation, exact-model session creation, and profile evidence—not on provider
registration or device enumeration.

So the precise answer is: all three desktop OSes can host native WebGPU
inference through their native graphics stacks, and ONNX Runtime provides a
plausible common route for this ONNX model, but Mist's current runtime does not
register it and the exact Kokoro artifact has not passed that route yet.

## Build and distribution profiles

One binary should not claim every provider merely because Cargo can compile its
bindings. Mist needs explicit release profiles whose bundled ONNX Runtime and
native dependencies match the providers it offers:

| Release profile | Packaged runtime | Host requirements | Initial provider order |
| --- | --- | --- | --- |
| macOS ARM64 | ONNX Runtime with Core ML; optionally a separately tested WebGPU build/plugin | System Core ML/Metal; bundled dynamic libraries must be inside and signed with the app | Core ML -> CPU |
| Windows x86-64 current build | ONNX Runtime with CUDA and DirectML providers | DirectML-capable driver; compatible NVIDIA runtime for CUDA | CUDA -> DirectML -> CPU |
| Windows ARM64 future package | Matching ONNX Runtime DirectML artifact and native release test | Windows 10 1903+ and DirectX 12 driver | DirectML -> CPU |
| Linux x86-64 standard | CPU build initially; WebGPU build/plugin after validation | For WebGPU, compatible Vulkan loader/driver | WebGPU -> CPU |
| Linux x86-64 NVIDIA | Separate CUDA-capable ONNX Runtime build | Compatible NVIDIA driver, CUDA 13, cuDNN 9, and required system libraries | CUDA -> CPU |
| Intel macOS, Windows ARM64 WebGPU, Linux ARM64 GPU | No matching accelerated artifact in the pinned Rust download matrix | Custom ONNX Runtime build and native test infrastructure | CPU until supplied |

The Rust binding's prebuilt matrix is useful for prototypes, but it is not a
release contract controlled by Mist. Production artifacts should pin ONNX
Runtime/provider versions and checksums, preserve their notices, verify dynamic
library discovery from the installed app, and test the packaged artifact rather
than only `cargo run`. Native WebGPU's upstream plugin is a `.dylib`, `.dll`, or
`.so`; the pinned Rust binding's static-link path still expects a dynamic Dawn
`webgpu_dawn` library
([Rust WebGPU link behavior](https://github.com/pykeio/ort/blob/002f41a8e175eac7f6695ff361d2e51a50874c48/ort-sys/build/static_link/mod.rs#L58-L70)).
CUDA and MIGraphX add vendor runtimes, while Core ML uses Apple's system
framework and DirectML can be built with its redistributable
([WebGPU packaging](https://onnxruntime.ai/docs/execution-providers/WebGPU-ExecutionProvider.html),
[DirectML build/distribution notes](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)).

## UI capability wording

Use outcome-oriented names in the compact UI, but retain the actual provider in
the accessible description/details. Icons must supplement text, never replace
it.

| Compact label | Secondary detail | Availability rule |
| --- | --- | --- |
| **Recommended** (spark/automatic icon) | Chooses the fastest backend validated for this device; show the active provider after load | Enabled; may resolve to Standard |
| **Standard** (leaf/steady icon) | Most compatible; runs locally | Enabled; ONNX Runtime CPU |
| **Accelerated** (bolt icon) | Provider and native stack, for example “Core ML · Apple GPU”, “DirectML · DX12”, “CUDA · NVIDIA”, or “WebGPU · Vulkan” | Enabled only after packaged-runtime, device, and exact-model probes succeed |
| **Apple optimized** (chip icon) | MLX · Apple silicon · experimental | Hidden or disabled as “MLX engine not installed”; never shown as available merely from chip detection |

Avoid unconditional **Fast**/**Slow** labels: backend performance depends on
model partitioning, text length, cold start, and device generation. If speed
language is desired, derive it from shipped benchmark tiers and show the active
backend after the session successfully loads. A failed accelerator must visibly
fall back to **Standard**, not continue displaying an accelerated state.

Do not present a requested policy as the active backend. `nvidia-smi`, a DirectX
12 adapter, an Apple chip, or a Vulkan device can make an option eligible for a
probe, but only the created session can make it active. Persist the user's
preference separately from the current session result.

## Recommended implementation order

1. Keep ONNX CPU as the production fallback on every OS.
2. Patch/fork Kokoro Micro's narrow `Device` layer, or create a Mist-owned ONNX
   Kokoro adapter, so composition supplies an ordered execution-provider list.
   Do not leak Core ML, DirectML, CUDA, or WebGPU identifiers into the domain or
   application layers. Return both the requested policy and the provider that
   actually loaded the model.
3. Keep the implemented mature-provider order: Core ML -> CPU on macOS,
   CUDA -> DirectML -> CPU on Windows, and CUDA -> CPU on Linux. Run the exact
   multilingual corpus on native GPU release machines before claiming hardware
   acceleration for Windows or Linux.
4. In parallel, spike native ONNX Runtime WebGPU once against the exact model on
   Metal, Direct3D 12, and Vulkan. If correctness, provider assignment, and
   latency pass, it becomes the shared cross-vendor path—especially for Linux
   AMD/Intel—without changing the model or voices.
5. Require every accelerated provider to pass: exact artifact checksum; real
   multilingual synthesis; finite/nonempty audio; voice and speed parity;
   provider-assignment/profile evidence; forced provider failure with immediate
   CPU fallback; cold/warm first-audio latency; sustained real-time factor;
   memory; pause/cancel responsiveness; and packaged-app smoke tests.
6. Treat MIGraphX as a later AMD-specific Linux optimization and MLX as a later
   Apple-only model-port project. Neither is needed for the first truthful GPU
   release.

The single smallest **code** experiment is WebGPU because one provider API maps
to Metal, Direct3D 12/Vulkan, and Vulkan. The smallest **production-risk** path
is target-specific Core ML, DirectML, and existing CUDA, each retaining CPU
fallback. Results from the exact-model spike should decide which path ships;
the settings UI must not decide in advance.
