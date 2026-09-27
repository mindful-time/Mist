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
    inference_policy: InferencePolicy,
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
        let inference_policy = synthesizer.inference_policy_label().to_owned();
        Ok(LoadedSpeechEngine {
            synthesizer: DynSpeechSynthesizer::new(synthesizer),
            inference_policy,
        })
    }
}

pub fn provider_capabilities() -> Vec<ProviderCapability> {
    capabilities_for(env::consts::OS, env::consts::ARCH, cuda_device_present())
}

pub fn configure_inference_provider(provider: &InferenceProviderId) {
    let requested = match provider.as_str() {
        "cpu" => Some("cpu"),
        "cuda" => Some("cuda"),
        _ => None,
    };
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
        "macos" => "Chooses a validated backend · currently ONNX CPU",
        "windows" => "Chooses CUDA when validated, then ONNX CPU",
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferencePolicy {
    AutoCpu,
    CudaAuto,
    Cpu,
    CudaRequested,
}

impl InferencePolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::AutoCpu => "CPU",
            Self::CudaAuto => "CUDA → CPU",
            Self::Cpu => "CPU",
            Self::CudaRequested => "CUDA requested",
        }
    }
}

impl KokoroSynthesizer {
    pub fn load(model_path: &Path, voice_path: &Path) -> Result<Self> {
        let inference_policy = detect_inference_policy();
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

        Ok(Self {
            tts,
            inference_policy,
        })
    }

    pub fn inference_policy_label(&self) -> &'static str {
        self.inference_policy.label()
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
    match env::var("KOKORO_ORT_PROVIDER")
        .unwrap_or_else(|_| "auto".to_owned())
        .to_ascii_lowercase()
        .as_str()
    {
        "cpu" => Device::Cpu,
        "cuda" => Device::Gpu,
        _ if matches!(env::consts::OS, "windows" | "linux") => Device::Auto,
        _ => Device::Cpu,
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

fn detect_inference_policy() -> InferencePolicy {
    policy_for(
        env::consts::OS,
        env::var("KOKORO_ORT_PROVIDER").ok().as_deref(),
    )
}

/// Mirrors the providers compiled into the multilingual Kokoro runtime.
fn policy_for(operating_system: &str, requested_provider: Option<&str>) -> InferencePolicy {
    let requested_provider = requested_provider.unwrap_or("auto").to_ascii_lowercase();
    match (operating_system, requested_provider.as_str()) {
        (_, "cpu") => return InferencePolicy::Cpu,
        ("windows" | "linux", "cuda") => return InferencePolicy::CudaRequested,
        _ => {}
    }

    match operating_system {
        "windows" | "linux" => InferencePolicy::CudaAuto,
        _ => InferencePolicy::AutoCpu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_policy_matches_the_loader_provider_order() {
        assert_eq!(policy_for("macos", Some("auto")), InferencePolicy::AutoCpu);
        assert_eq!(policy_for("linux", None), InferencePolicy::CudaAuto);
        assert_eq!(policy_for("windows", None), InferencePolicy::CudaAuto);
    }

    #[test]
    fn explicit_cpu_override_wins_on_every_platform() {
        for operating_system in ["macos", "windows", "linux"] {
            assert_eq!(
                policy_for(operating_system, Some("CPU")),
                InferencePolicy::Cpu
            );
        }
    }

    #[test]
    fn explicit_accelerator_override_is_labeled_as_requested() {
        assert_eq!(
            policy_for("linux", Some("cuda")),
            InferencePolicy::CudaRequested
        );
    }

    #[test]
    fn capability_matrix_disables_backends_the_device_cannot_run() {
        let mac = capabilities_for("macos", "aarch64", false);
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
