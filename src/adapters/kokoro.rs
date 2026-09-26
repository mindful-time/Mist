use std::path::Path;

use anyhow::{Context, Result};
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
}

impl KokoroSynthesizer {
    pub fn load(model_path: &Path, voice_path: &Path) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start the Kokoro runtime")?;
        let tts = runtime
            .block_on(KokoroTts::new(model_path, voice_path))
            .with_context(|| format!("could not load Kokoro model at {}", model_path.display()))?;

        Ok(Self { runtime, tts })
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
}
