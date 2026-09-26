use std::{env, path::Path, sync::mpsc::sync_channel, thread};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use kokoro_en::{KokoroTts, Voice, split_sentences};
use tokio::runtime::{Builder, Runtime};

use crate::{
    domain::{Audio, SelectedText, VoiceSettings},
    ports::SpeechSynthesizer,
};

/// Kokoro v1.0 ONNX adapter. The runtime and model stay warm between selections.
pub struct KokoroSynthesizer {
    runtime: Runtime,
    tts: KokoroTts,
    inference_policy: InferencePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferencePolicy {
    CoreMlAuto,
    CudaAuto,
    DirectMlAuto,
    Cpu,
    CoreMlRequested,
    CudaRequested,
    DirectMlRequested,
}

impl InferencePolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::CoreMlAuto => "CoreML → CPU",
            Self::CudaAuto => "CUDA → CPU",
            Self::DirectMlAuto => "DirectML → CPU",
            Self::Cpu => "CPU",
            Self::CoreMlRequested => "CoreML requested",
            Self::CudaRequested => "CUDA requested",
            Self::DirectMlRequested => "DirectML requested",
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
        let tts = runtime
            .block_on(KokoroTts::new(model_path, voice_path))
            .with_context(|| format!("could not load Kokoro model at {}", model_path.display()))?;

        Ok(Self {
            runtime,
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
        let sentences = split_sentences(text.as_str());
        if sentences.is_empty() {
            bail!("Kokoro could not find any speech chunks");
        }
        let tts = &self.tts;
        let runtime = &self.runtime;

        thread::scope(|scope| {
            let (sender, receiver) = sync_channel::<Result<Audio>>(2);
            let producer = scope.spawn(move || {
                for (index, sentence) in sentences.into_iter().enumerate() {
                    let result = runtime
                        .block_on(tts.synth(sentence, voice.clone()))
                        .with_context(|| format!("Kokoro failed on speech chunk {}", index + 1))
                        .map(|(samples, _elapsed)| Audio::kokoro(samples));
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

fn detect_inference_policy() -> InferencePolicy {
    policy_for(
        env::consts::OS,
        env::var("KOKORO_ORT_PROVIDER").ok().as_deref(),
        nvidia_gpu_present(),
    )
}

fn policy_for(
    operating_system: &str,
    requested_provider: Option<&str>,
    nvidia_gpu_present: bool,
) -> InferencePolicy {
    let requested_provider = requested_provider.unwrap_or("auto").to_ascii_lowercase();
    match (operating_system, requested_provider.as_str()) {
        (_, "cpu") => return InferencePolicy::Cpu,
        ("macos", "coreml") => return InferencePolicy::CoreMlRequested,
        ("windows" | "linux", "cuda") => return InferencePolicy::CudaRequested,
        ("windows", "directml") => return InferencePolicy::DirectMlRequested,
        _ => {}
    }

    match operating_system {
        "macos" => InferencePolicy::CoreMlAuto,
        "windows" if nvidia_gpu_present => InferencePolicy::CudaAuto,
        "windows" => InferencePolicy::DirectMlAuto,
        "linux" if nvidia_gpu_present => InferencePolicy::CudaAuto,
        _ => InferencePolicy::Cpu,
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
            policy_for("macos", Some("auto"), false),
            InferencePolicy::CoreMlAuto
        );
        assert_eq!(policy_for("linux", None, true), InferencePolicy::CudaAuto);
        assert_eq!(
            policy_for("windows", None, false),
            InferencePolicy::DirectMlAuto
        );
        assert_eq!(policy_for("linux", None, false), InferencePolicy::Cpu);
    }

    #[test]
    fn explicit_cpu_override_wins_on_every_platform() {
        for operating_system in ["macos", "windows", "linux"] {
            assert_eq!(
                policy_for(operating_system, Some("CPU"), true),
                InferencePolicy::Cpu
            );
        }
    }

    #[test]
    fn explicit_accelerator_override_is_labeled_as_requested() {
        assert_eq!(
            policy_for("macos", Some("coreml"), false),
            InferencePolicy::CoreMlRequested
        );
        assert_eq!(
            policy_for("linux", Some("cuda"), false),
            InferencePolicy::CudaRequested
        );
        assert_eq!(
            policy_for("windows", Some("directml"), true),
            InferencePolicy::DirectMlRequested
        );
    }
}
