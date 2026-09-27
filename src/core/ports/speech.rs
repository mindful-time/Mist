use crate::core::domain::{
    Audio, LanguageId, LanguageProfile, SelectedText, VoiceProfile, VoiceSettings,
};

/// Driven port for a text-to-speech engine.
pub trait SpeechSynthesizer {
    fn synthesize(&mut self, text: &SelectedText, voice: &VoiceSettings) -> anyhow::Result<Audio>;

    fn synthesize_streaming(
        &mut self,
        text: &SelectedText,
        voice: &VoiceSettings,
        on_chunk: &mut dyn FnMut(Audio) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        on_chunk(self.synthesize(text, voice)?)
    }
}

pub struct DynSpeechSynthesizer(Box<dyn SpeechSynthesizer>);

impl DynSpeechSynthesizer {
    pub fn new(synthesizer: impl SpeechSynthesizer + 'static) -> Self {
        Self(Box::new(synthesizer))
    }
}

impl SpeechSynthesizer for DynSpeechSynthesizer {
    fn synthesize(&mut self, text: &SelectedText, voice: &VoiceSettings) -> anyhow::Result<Audio> {
        self.0.as_mut().synthesize(text, voice)
    }

    fn synthesize_streaming(
        &mut self,
        text: &SelectedText,
        voice: &VoiceSettings,
        on_chunk: &mut dyn FnMut(Audio) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.0.as_mut().synthesize_streaming(text, voice, on_chunk)
    }
}

/// A model-neutral speech engine selected by the composition root.
pub struct LoadedSpeechEngine {
    pub synthesizer: DynSpeechSynthesizer,
    /// Backend reported by the loaded model session, not merely the user's
    /// requested policy. Adapters own the provider-specific wording.
    pub runtime_backend: String,
}

/// Driven port for constructing the selected speech-model adapter.
pub trait SpeechEngineFactory: Send + Sync {
    fn load(&self) -> anyhow::Result<LoadedSpeechEngine>;
}

/// Driven port for adapter-owned voices, language defaults, and previews.
///
/// The UI and application core never assume Kokoro identifiers or catalog
/// contents. A different model can replace this port and its engine factory.
pub trait VoiceCatalog: Send + Sync {
    fn languages(&self) -> &[LanguageProfile];
    fn voices(&self) -> &[VoiceProfile];
    fn default_voice_id(&self) -> &str;
    fn default_voice_for(&self, language: LanguageId) -> Option<&str>;
    fn preview_text(&self, language: LanguageId) -> Option<&str>;

    fn profile(&self, voice_id: &str) -> Option<&VoiceProfile> {
        self.voices().iter().find(|voice| voice.id == voice_id)
    }

    fn settings(&self, voice_id: &str) -> Option<VoiceSettings> {
        self.profile(voice_id)
            .and_then(|profile| VoiceSettings::new(profile.id))
    }

    fn default_settings(&self) -> VoiceSettings {
        self.settings(self.default_voice_id())
            .expect("a voice catalog must contain its declared default voice")
    }

    fn language_for(&self, voice_id: &str) -> Option<LanguageId> {
        self.profile(voice_id).map(|profile| profile.language)
    }

    fn voice_count(&self, language: LanguageId) -> usize {
        self.voices()
            .iter()
            .filter(|voice| voice.language == language)
            .count()
    }
}
