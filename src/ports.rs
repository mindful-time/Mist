use crate::domain::{Audio, SelectedText, VoiceSettings};

/// Outbound port for a text-to-speech engine.
pub trait SpeechSynthesizer {
    fn synthesize(&mut self, text: &SelectedText, voice: &VoiceSettings) -> anyhow::Result<Audio>;
}

/// Outbound port for the system's audio output.
pub trait AudioPlayer {
    fn play(&mut self, audio: &Audio) -> anyhow::Result<()>;
}

/// Outbound port for the downloadable speech-model artifacts.
pub trait ModelProvisioner {
    fn is_ready(&self) -> bool;
    fn install(&self) -> anyhow::Result<()>;
}
