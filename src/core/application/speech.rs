use super::super::{
    domain::{AudioFeatures, PlaybackMode, SelectedText, VoiceSettings},
    ports::{AudioPlayer, SpeechSynthesizer},
};

/// Application use case. It knows nothing about GUI, model runtimes, or system audio.
pub struct SpeakSelection<S, P> {
    synthesizer: S,
    player: P,
    voice: VoiceSettings,
    playback_mode: PlaybackMode,
}

impl<S, P> SpeakSelection<S, P>
where
    S: SpeechSynthesizer,
    P: AudioPlayer,
{
    pub fn new(synthesizer: S, player: P, voice: VoiceSettings) -> Self {
        Self {
            synthesizer,
            player,
            voice,
            playback_mode: PlaybackMode::RealTime,
        }
    }

    pub fn execute(&mut self, text: SelectedText) -> anyhow::Result<()> {
        self.execute_with_playback_started(text, || {})
    }

    pub fn set_voice(&mut self, voice: VoiceSettings) {
        self.voice = voice;
    }

    pub fn set_playback_mode(&mut self, mode: PlaybackMode) {
        self.playback_mode = mode;
    }

    /// Plays the short catalog sample with the voice the user just activated.
    /// This is a separate application action from speaking an OS text selection.
    pub fn preview_voice_with_playback_cues(
        &mut self,
        voice: VoiceSettings,
        preview_text: SelectedText,
        on_playback: impl FnMut(AudioFeatures),
    ) -> anyhow::Result<()> {
        self.set_voice(voice);
        self.execute_with_playback_cues(preview_text, on_playback)
    }

    pub fn execute_with_playback_started(
        &mut self,
        text: SelectedText,
        on_playback_started: impl FnOnce(),
    ) -> anyhow::Result<()> {
        let mut on_playback_started = Some(on_playback_started);
        self.execute_with_playback_cues(text, |_| {
            if let Some(notify) = on_playback_started.take() {
                notify();
            }
        })
    }

    pub fn execute_with_playback_cues(
        &mut self,
        text: SelectedText,
        mut on_playback: impl FnMut(AudioFeatures),
    ) -> anyhow::Result<()> {
        let player = &mut self.player;
        let mut emitted_audio = false;

        if self.playback_mode.streams_audio() {
            self.synthesizer
                .synthesize_streaming(&text, &self.voice, &mut |audio| {
                    if audio.samples.is_empty() {
                        return Ok(());
                    }
                    emitted_audio = true;
                    player.play(&audio, &mut on_playback)
                })?;
        } else {
            let audio = self.synthesizer.synthesize(&text, &self.voice)?;
            if !audio.samples.is_empty() {
                emitted_audio = true;
                player.play(&audio, &mut on_playback)?;
            }
        }

        if !emitted_audio {
            anyhow::bail!("the speech engine produced no audio");
        }
        Ok(())
    }
}
