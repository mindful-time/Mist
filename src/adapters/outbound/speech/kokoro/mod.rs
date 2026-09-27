//! Kokoro implementation of the core speech-engine port.

pub mod catalog;

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::sync_channel,
    thread,
};

use anyhow::{Context, Result, anyhow, bail};
use kokoro_micro::{Device, TtsEngine};
use tokio::runtime::Builder;

use crate::{
    domain::{
        Audio, InferenceProviderId, ProviderCapability, ProviderPerformance, SelectedText,
        VoiceSettings,
    },
    ports::{DynSpeechSynthesizer, LoadedSpeechEngine, SpeechEngineFactory, SpeechSynthesizer},
};

/// Kokoro v1.0 ONNX adapter. The runtime and model stay warm between selections.
pub struct KokoroSynthesizer {
    tts: TtsEngine,
}

/// Composition adapter that hides Kokoro construction from the worker and
/// application core. Swap this factory together with its catalog to use a
/// different speech model.
#[derive(Clone, Debug)]
pub struct KokoroEngineFactory {
    model_path: PathBuf,
    voices_path: PathBuf,
}

impl KokoroEngineFactory {
    pub fn new(model_path: impl Into<PathBuf>, voices_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            voices_path: voices_path.into(),
        }
    }
}

impl SpeechEngineFactory for KokoroEngineFactory {
    fn load(&self) -> Result<LoadedSpeechEngine> {
        let synthesizer = KokoroSynthesizer::load(&self.model_path, &self.voices_path)?;
        let runtime_backend = synthesizer.runtime_backend_label();
        Ok(LoadedSpeechEngine {
            synthesizer: DynSpeechSynthesizer::new(synthesizer),
            runtime_backend,
        })
    }
}

pub fn provider_capabilities() -> Vec<ProviderCapability> {
    capabilities_for(env::consts::OS, env::consts::ARCH, cuda_device_present())
}

pub fn configure_inference_provider(provider: &InferenceProviderId) {
    let requested = provider_environment_value(provider.as_str());
    // SAFETY: main calls this once during single-threaded process startup,
    // before eframe, the speech worker, or ONNX Runtime creates any threads.
    unsafe {
        if let Some(requested) = requested {
            env::set_var("KOKORO_ORT_PROVIDER", requested);
        } else {
            env::remove_var("KOKORO_ORT_PROVIDER");
        }
    }
}

fn provider_environment_value(provider: &str) -> Option<&'static str> {
    match provider {
        "cpu" => Some("cpu"),
        "cuda" => Some("cuda"),
        "coreml" => Some("coreml"),
        _ => None,
    }
}

fn cuda_device_present() -> bool {
    if !matches!(env::consts::OS, "windows" | "linux") {
        return false;
    }
    Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
}

fn capabilities_for(
    operating_system: &str,
    architecture: &str,
    cuda_device_present: bool,
) -> Vec<ProviderCapability> {
    let auto_detail = match operating_system {
        "macos" => "Chooses Core ML GPU when available, then ONNX CPU",
        "windows" => "Chooses CUDA, then DirectML, then ONNX CPU",
        "linux" => "Chooses CUDA when validated, then ONNX CPU",
        _ => "Chooses a validated backend · currently ONNX CPU",
    };
    let mlx_detail = if operating_system == "macos" && architecture == "aarch64" {
        "Apple Silicon detected; MLX is not integrated in this Rust build"
    } else {
        "MLX · requires Apple Silicon and a separate speech engine"
    };
    vec![
        provider(
            "auto",
            "Recommended",
            ProviderPerformance::Recommended,
            true,
            auto_detail,
        ),
        provider(
            "cpu",
            "Standard",
            ProviderPerformance::Standard,
            true,
            "ONNX CPU · most compatible · always available",
        ),
        provider(
            "cuda",
            "Accelerated",
            ProviderPerformance::Accelerated,
            matches!(operating_system, "windows" | "linux") && cuda_device_present,
            if matches!(operating_system, "windows" | "linux") {
                if cuda_device_present {
                    "CUDA · supported NVIDIA device detected"
                } else {
                    "CUDA · no usable NVIDIA device detected"
                }
            } else {
                "CUDA · unavailable on macOS"
            },
        ),
        provider(
            "coreml",
            "Accelerated",
            ProviderPerformance::Accelerated,
            operating_system == "macos" && architecture == "aarch64",
            if operating_system == "macos" && architecture == "aarch64" {
                "Core ML · Apple GPU with ONNX CPU fallback"
            } else {
                "Core ML · requires Apple Silicon macOS"
            },
        ),
        provider(
            "webgpu",
            "Graphics acceleration",
            ProviderPerformance::Accelerated,
            false,
            if operating_system == "macos" {
                "WebGPU via Metal · planned exact-model validation"
            } else {
                "WebGPU · not available in this build"
            },
        ),
        provider(
            "mlx",
            "Apple optimized",
            ProviderPerformance::Planned,
            false,
            mlx_detail,
        ),
    ]
}

fn provider(
    id: &str,
    display_name: &'static str,
    performance: ProviderPerformance,
    available: bool,
    detail: &'static str,
) -> ProviderCapability {
    ProviderCapability {
        id: InferenceProviderId::new(id).expect("provider identifiers are not empty"),
        display_name,
        performance,
        available,
        detail,
    }
}

impl KokoroSynthesizer {
    pub fn load(model_path: &Path, voice_path: &Path) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start the Kokoro runtime")?;
        let model_path_string = model_path.to_string_lossy();
        let voice_path_string = voice_path.to_string_lossy();
        let tts = runtime
            .block_on(TtsEngine::on_device(
                model_path_string.as_ref(),
                voice_path_string.as_ref(),
                requested_device(),
            ))
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("could not load Kokoro model at {}", model_path.display()))?;

        Ok(Self { tts })
    }

    pub fn runtime_backend_label(&self) -> String {
        self.tts.backend().to_string()
    }
}

impl SpeechSynthesizer for KokoroSynthesizer {
    fn synthesize(&mut self, text: &SelectedText, settings: &VoiceSettings) -> Result<Audio> {
        let samples = self
            .tts
            .synthesize_with_options(
                text.as_str(),
                Some(settings.voice_id.as_str()),
                settings.speed,
                1.0,
                None,
            )
            .map_err(anyhow::Error::msg)
            .context("Kokoro could not synthesize the selected text")?;
        Ok(Audio::new(samples, 24_000))
    }

    fn synthesize_streaming(
        &mut self,
        text: &SelectedText,
        settings: &VoiceSettings,
        on_chunk: &mut dyn FnMut(Audio) -> Result<()>,
    ) -> Result<()> {
        let sentences = speech_chunks(text.as_str());
        if sentences.is_empty() {
            bail!("Kokoro could not find any speech chunks");
        }
        let tts = &self.tts;
        let voice = settings.voice_id.as_str();
        let speed = settings.speed;

        thread::scope(|scope| {
            let (sender, receiver) = sync_channel::<Result<Audio>>(2);
            let producer = scope.spawn(move || {
                for (index, sentence) in sentences.into_iter().enumerate() {
                    let result = tts
                        .synthesize_with_options(&sentence, Some(voice), speed, 1.0, None)
                        .map_err(anyhow::Error::msg)
                        .with_context(|| format!("Kokoro failed on speech chunk {}", index + 1))
                        .map(|samples| Audio::new(samples, 24_000));
                    let failed = result.is_err();
                    if sender.send(result).is_err() || failed {
                        return;
                    }
                }
            });

            let mut emitted = false;
            for audio in receiver {
                let audio = audio?;
                if audio.samples.is_empty() {
                    continue;
                }
                emitted = true;
                on_chunk(audio)?;
            }
            producer
                .join()
                .map_err(|_| anyhow!("Kokoro streaming worker panicked"))?;

            if !emitted {
                bail!("Kokoro produced no streaming audio");
            }
            Ok(())
        })
    }
}

fn requested_device() -> Device {
    requested_device_for(
        env::consts::OS,
        env::var("KOKORO_ORT_PROVIDER").ok().as_deref(),
    )
}

/// Maps adapter-owned provider preferences to the model runtime's portable
/// device contract. `Auto` always tries compiled accelerators before CPU.
fn requested_device_for(_operating_system: &str, requested_provider: Option<&str>) -> Device {
    match requested_provider
        .unwrap_or("auto")
        .to_ascii_lowercase()
        .as_str()
    {
        "cpu" => Device::Cpu,
        // Accelerated preferences are priorities, not availability promises.
        // The runtime still has to reach a usable CPU session if the preferred
        // provider cannot register or cannot load the exact model.
        "cuda" | "coreml" => Device::Auto,
        _ => Device::Auto,
    }
}

fn speech_chunks(text: &str) -> Vec<String> {
    const MAX_CHARS: usize = 280;
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let mut characters = 0;
    for character in text.chars() {
        chunk.push(character);
        characters += 1;
        let boundary = matches!(character, '.' | '!' | '?' | '。' | '！' | '？' | '\n');
        if boundary || characters >= MAX_CHARS {
            let trimmed = chunk.trim();
            if !trimmed.is_empty() {
                chunks.push(trimmed.to_owned());
            }
            chunk.clear();
            characters = 0;
        }
    }
    let trimmed = chunk.trim();
    if !trimmed.is_empty() {
        chunks.push(trimmed.to_owned());
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_device_tries_acceleration_before_cpu_on_every_platform() {
        for operating_system in ["macos", "windows", "linux"] {
            assert_eq!(
                requested_device_for(operating_system, Some("auto")),
                Device::Auto
            );
        }
    }

    #[test]
    fn explicit_cpu_override_wins_on_every_platform() {
        for operating_system in ["macos", "windows", "linux"] {
            assert_eq!(
                requested_device_for(operating_system, Some("CPU")),
                Device::Cpu
            );
        }
    }

    #[test]
    fn accelerated_preferences_keep_cpu_as_the_last_resort() {
        assert_eq!(requested_device_for("linux", Some("cuda")), Device::Auto);
        assert_eq!(requested_device_for("macos", Some("coreml")), Device::Auto);
    }

    #[test]
    fn provider_preferences_translate_only_supported_explicit_overrides() {
        assert_eq!(provider_environment_value("cpu"), Some("cpu"));
        assert_eq!(provider_environment_value("cuda"), Some("cuda"));
        assert_eq!(provider_environment_value("coreml"), Some("coreml"));
        assert_eq!(provider_environment_value("auto"), None);
        assert_eq!(provider_environment_value("webgpu"), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_build_compiles_cuda_then_directml_before_cpu_fallback() {
        assert_eq!(
            kokoro_micro::compiled_gpu_providers(),
            ["CUDAExecutionProvider", "DmlExecutionProvider"]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_build_compiles_cuda_before_cpu_fallback() {
        assert_eq!(
            kokoro_micro::compiled_gpu_providers(),
            ["CUDAExecutionProvider"]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn apple_silicon_build_compiles_coreml_before_cpu_fallback() {
        assert_eq!(
            kokoro_micro::compiled_gpu_providers(),
            ["CoreMLExecutionProvider"]
        );
    }

    #[test]
    fn capability_matrix_disables_backends_the_device_cannot_run() {
        let mac = capabilities_for("macos", "aarch64", false);
        assert!(capability(&mac, "coreml").available);
        assert!(!capability(&mac, "cuda").available);
        assert!(!capability(&mac, "webgpu").available);
        assert!(!capability(&mac, "mlx").available);
        assert!(capability(&mac, "cpu").available);

        let windows = capabilities_for("windows", "x86_64", true);
        assert!(capability(&windows, "cuda").available);
        assert!(!capability(&windows, "webgpu").available);
        assert!(!capability(&windows, "mlx").available);
    }

    #[test]
    fn streaming_chunks_recognize_latin_and_cjk_sentence_boundaries() {
        assert_eq!(
            speech_chunks("Hola. Bonjour!"),
            ["Hola.".to_owned(), "Bonjour!".to_owned()]
        );
        assert_eq!(
            speech_chunks("你好。今日は晴れです。"),
            ["你好。".to_owned(), "今日は晴れです。".to_owned()]
        );
    }

    fn capability<'a>(capabilities: &'a [ProviderCapability], id: &str) -> &'a ProviderCapability {
        capabilities
            .iter()
            .find(|capability| capability.id.as_str() == id)
            .unwrap()
    }
}
