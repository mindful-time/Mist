use std::{env, path::Path};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::process::Command;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use kokoro_en::{KokoroTts, Voice};
use tokio::runtime::{Builder, Runtime};

use crate::{
    domain::{Audio, SelectedText, VoiceSettings},
    ports::SpeechSynthesizer,
};

/// Kokoro v1.0 ONNX adapter. The runtime and model stay warm between selections.
pub struct KokoroSynthesizer {
    runtime: Runtime,
    tts: KokoroTts,
    backend: InferenceBackend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceBackend {
    CoreMlAuto,
    CudaAuto,
    DirectMlAuto,
    Cpu,
}

impl InferenceBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::CoreMlAuto => "CoreML auto",
            Self::CudaAuto => "CUDA auto",
            Self::DirectMlAuto => "DirectML auto",
            Self::Cpu => "CPU",
        }
    }
}

impl KokoroSynthesizer {
    pub fn load(model_path: &Path, voice_path: &Path) -> Result<Self> {
        let backend = detect_backend();
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start the Kokoro runtime")?;
        let tts = runtime
            .block_on(KokoroTts::new(model_path, voice_path))
            .with_context(|| format!("could not load Kokoro model at {}", model_path.display()))?;

        Ok(Self {
            runtime,
            tts,
            backend,
        })
    }

    pub fn backend_label(&self) -> &'static str {
        self.backend.label()
    }
}

impl SpeechSynthesizer for KokoroSynthesizer {
    fn synthesize(&mut self, text: &SelectedText, settings: &VoiceSettings) -> Result<Audio> {
        let voice = Voice::new(&settings.name).with_speed(settings.speed);
        let (samples, _) = self
            .runtime
            .block_on(self.tts.synth(text.as_str(), voice))
            .context("Kokoro could not synthesize the selected text")?;
        Ok(Audio::kokoro(samples))
    }

    fn synthesize_streaming(
        &mut self,
        text: &SelectedText,
        settings: &VoiceSettings,
        on_chunk: &mut dyn FnMut(Audio) -> Result<()>,
    ) -> Result<()> {
        let voice = Voice::new(&settings.name).with_speed(settings.speed);
        let source = text.as_str().to_owned();
        let tts = &self.tts;

        self.runtime.block_on(async {
            let (mut sink, mut chunks) = tts.stream::<String, _>(voice);
            sink.synth(source)
                .await
                .context("Kokoro could not queue the selected text")?;
            drop(sink);

            let mut emitted = false;
            while let Some((samples, _elapsed)) = chunks.next().await {
                if samples.is_empty() {
                    continue;
                }
                emitted = true;
                on_chunk(Audio::kokoro(samples))?;
            }

            if !emitted {
                bail!("Kokoro produced no streaming audio");
            }
            Ok(())
        })
    }
}

fn detect_backend() -> InferenceBackend {
    backend_for(
        env::consts::OS,
        env::var("KOKORO_ORT_PROVIDER").ok().as_deref(),
        nvidia_gpu_present(),
    )
}

fn backend_for(
    operating_system: &str,
    requested_provider: Option<&str>,
    nvidia_gpu_present: bool,
) -> InferenceBackend {
    let requested_provider = requested_provider.unwrap_or("auto").to_ascii_lowercase();
    match (operating_system, requested_provider.as_str()) {
        (_, "cpu") => return InferenceBackend::Cpu,
        ("macos", "coreml") => return InferenceBackend::CoreMlAuto,
        ("windows" | "linux", "cuda") => return InferenceBackend::CudaAuto,
        ("windows", "directml") => return InferenceBackend::DirectMlAuto,
        _ => {}
    }

    match operating_system {
        "macos" => InferenceBackend::CoreMlAuto,
        "windows" if nvidia_gpu_present => InferenceBackend::CudaAuto,
        "windows" => InferenceBackend::DirectMlAuto,
        "linux" if nvidia_gpu_present => InferenceBackend::CudaAuto,
        _ => InferenceBackend::Cpu,
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn nvidia_gpu_present() -> bool {
    Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn nvidia_gpu_present() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_selects_native_acceleration_with_cpu_fallback() {
        assert_eq!(
            backend_for("macos", Some("auto"), false),
            InferenceBackend::CoreMlAuto
        );
        assert_eq!(backend_for("linux", None, true), InferenceBackend::CudaAuto);
        assert_eq!(
            backend_for("windows", None, false),
            InferenceBackend::DirectMlAuto
        );
        assert_eq!(backend_for("linux", None, false), InferenceBackend::Cpu);
    }

    #[test]
    fn explicit_cpu_override_wins_on_every_platform() {
        for operating_system in ["macos", "windows", "linux"] {
            assert_eq!(
                backend_for(operating_system, Some("CPU"), true),
                InferenceBackend::Cpu
            );
        }
    }
}
