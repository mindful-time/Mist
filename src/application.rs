use crate::{
    domain::{AudioFeatures, SelectedText, VoiceSettings},
    ports::{AudioPlayer, ModelProvisioner, SpeechSynthesizer},
};

pub const VOICE_PREVIEW_TEXT: &str = "Hello. This is how I'll bring your selected words to life.";

/// Application use-case. It knows nothing about AppKit, Kokoro, or system audio.
pub struct SpeakSelection<S, P> {
    synthesizer: S,
    player: P,
    voice: VoiceSettings,
    streaming_playback: bool,
}

/// Application use-case for the explicit first-run model installation.
pub struct InstallModel<M> {
    models: M,
}

impl<M> InstallModel<M>
where
    M: ModelProvisioner,
{
    pub fn new(models: M) -> Self {
        Self { models }
    }

    pub fn execute(&self) -> anyhow::Result<()> {
        if !self.models.is_ready() {
            self.models.install()?;
        }
        Ok(())
    }
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
            streaming_playback: true,
        }
    }

    pub fn execute(&mut self, text: SelectedText) -> anyhow::Result<()> {
        self.execute_with_playback_started(text, || {})
    }

    pub fn set_voice(&mut self, voice: VoiceSettings) {
        self.voice = voice;
    }

    pub fn set_streaming_playback(&mut self, enabled: bool) {
        self.streaming_playback = enabled;
    }

    /// Plays the short catalog sample with the voice the user just activated.
    /// This is a separate application action from speaking an OS text selection.
    pub fn preview_voice_with_playback_cues(
        &mut self,
        voice: VoiceSettings,
        on_playback: impl FnMut(AudioFeatures),
    ) -> anyhow::Result<()> {
        self.set_voice(voice);
        let sample = SelectedText::new(VOICE_PREVIEW_TEXT)
            .expect("the built-in voice preview copy must remain valid");
        self.execute_with_playback_cues(sample, on_playback)
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

        if self.streaming_playback {
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
            anyhow::bail!("Kokoro produced no audio");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::domain::Audio;

    struct FakeSynthesizer {
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl SpeechSynthesizer for FakeSynthesizer {
        fn synthesize(
            &mut self,
            text: &SelectedText,
            _voice: &VoiceSettings,
        ) -> anyhow::Result<Audio> {
            self.seen.lock().unwrap().push(text.as_str().to_owned());
            Ok(Audio::kokoro(vec![0.0, 0.25, -0.25]))
        }

        fn synthesize_streaming(
            &mut self,
            text: &SelectedText,
            _voice: &VoiceSettings,
            on_chunk: &mut dyn FnMut(Audio) -> anyhow::Result<()>,
        ) -> anyhow::Result<()> {
            self.seen.lock().unwrap().push(text.as_str().to_owned());
            on_chunk(Audio::kokoro(vec![0.0, 0.25]))?;
            on_chunk(Audio::kokoro(vec![-0.25]))
        }
    }

    struct FakePlayer {
        sample_counts: Arc<Mutex<Vec<usize>>>,
    }

    struct FailingPlayer;

    struct FakeModels {
        ready: bool,
        installs: Arc<AtomicUsize>,
        install_error: bool,
    }

    impl ModelProvisioner for FakeModels {
        fn is_ready(&self) -> bool {
            self.ready
        }

        fn install(&self) -> anyhow::Result<()> {
            self.installs.fetch_add(1, Ordering::Relaxed);
            if self.install_error {
                anyhow::bail!("installation failed");
            }
            Ok(())
        }
    }

    impl AudioPlayer for FakePlayer {
        fn play(
            &mut self,
            audio: &Audio,
            on_sample: &mut dyn FnMut(crate::domain::AudioFeatures),
        ) -> anyhow::Result<()> {
            on_sample(crate::domain::AudioFeatures {
                energy: audio.energy(),
                brightness: 64,
            });
            self.sample_counts.lock().unwrap().push(audio.samples.len());
            Ok(())
        }
    }

    impl AudioPlayer for FailingPlayer {
        fn play(
            &mut self,
            _audio: &Audio,
            _on_sample: &mut dyn FnMut(crate::domain::AudioFeatures),
        ) -> anyhow::Result<()> {
            anyhow::bail!("audio device unavailable")
        }
    }

    #[test]
    fn streams_synthesized_audio_chunks_to_the_player_in_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sample_counts = Arc::new(Mutex::new(Vec::new()));
        let playback_starts = Arc::new(AtomicUsize::new(0));
        let mut use_case = SpeakSelection::new(
            FakeSynthesizer { seen: seen.clone() },
            FakePlayer {
                sample_counts: sample_counts.clone(),
            },
            VoiceSettings::default(),
        );

        use_case
            .execute_with_playback_started(SelectedText::new("Read this").unwrap(), {
                let playback_starts = playback_starts.clone();
                move || {
                    playback_starts.fetch_add(1, Ordering::Relaxed);
                }
            })
            .unwrap();

        assert_eq!(&*seen.lock().unwrap(), &["Read this"]);
        assert_eq!(&*sample_counts.lock().unwrap(), &[2, 1]);
        assert_eq!(playback_starts.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn complete_mode_plays_one_buffer_after_synthesis() {
        let sample_counts = Arc::new(Mutex::new(Vec::new()));
        let mut use_case = SpeakSelection::new(
            FakeSynthesizer {
                seen: Arc::new(Mutex::new(Vec::new())),
            },
            FakePlayer {
                sample_counts: sample_counts.clone(),
            },
            VoiceSettings::default(),
        );
        use_case.set_streaming_playback(false);

        use_case
            .execute(SelectedText::new("Read after synthesis").unwrap())
            .unwrap();

        assert_eq!(&*sample_counts.lock().unwrap(), &[3]);
    }

    #[test]
    fn does_not_report_playback_before_the_player_starts() {
        let playback_starts = Arc::new(AtomicUsize::new(0));
        let mut use_case = SpeakSelection::new(
            FakeSynthesizer {
                seen: Arc::new(Mutex::new(Vec::new())),
            },
            FailingPlayer,
            VoiceSettings::default(),
        );

        let error = use_case
            .execute_with_playback_started(SelectedText::new("Read this").unwrap(), {
                let playback_starts = playback_starts.clone();
                move || {
                    playback_starts.fetch_add(1, Ordering::Relaxed);
                }
            })
            .unwrap_err();

        assert_eq!(error.to_string(), "audio device unavailable");
        assert_eq!(playback_starts.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn skips_model_installation_when_artifacts_are_ready() {
        let installs = Arc::new(AtomicUsize::new(0));
        InstallModel::new(FakeModels {
            ready: true,
            installs: installs.clone(),
            install_error: false,
        })
        .execute()
        .unwrap();

        assert_eq!(installs.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn installs_models_when_artifacts_are_missing() {
        let installs = Arc::new(AtomicUsize::new(0));
        InstallModel::new(FakeModels {
            ready: false,
            installs: installs.clone(),
            install_error: false,
        })
        .execute()
        .unwrap();

        assert_eq!(installs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn returns_model_installation_errors() {
        let installs = Arc::new(AtomicUsize::new(0));
        let error = InstallModel::new(FakeModels {
            ready: false,
            installs: installs.clone(),
            install_error: true,
        })
        .execute()
        .unwrap_err();

        assert_eq!(error.to_string(), "installation failed");
        assert_eq!(installs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn selected_voice_is_used_without_reloading_the_speech_engine() {
        struct VoiceRecorder(Arc<Mutex<Vec<String>>>);

        impl SpeechSynthesizer for VoiceRecorder {
            fn synthesize(
                &mut self,
                _text: &SelectedText,
                voice: &VoiceSettings,
            ) -> anyhow::Result<Audio> {
                self.0
                    .lock()
                    .unwrap()
                    .push(voice.voice_id.as_str().to_owned());
                Ok(Audio::kokoro(vec![0.2]))
            }
        }

        let voices = Arc::new(Mutex::new(Vec::new()));
        let mut use_case = SpeakSelection::new(
            VoiceRecorder(voices.clone()),
            FakePlayer {
                sample_counts: Arc::new(Mutex::new(Vec::new())),
            },
            VoiceSettings::default(),
        );
        use_case.set_voice(VoiceSettings::from_voice_id("bf_emma").unwrap());
        use_case
            .execute(SelectedText::new("A different voice").unwrap())
            .unwrap();

        assert_eq!(&*voices.lock().unwrap(), &["bf_emma"]);
    }

    #[test]
    fn voice_preview_uses_the_clicked_voice_and_preview_copy() {
        struct PreviewRecorder(Arc<Mutex<Vec<(String, String)>>>);

        impl SpeechSynthesizer for PreviewRecorder {
            fn synthesize(
                &mut self,
                text: &SelectedText,
                voice: &VoiceSettings,
            ) -> anyhow::Result<Audio> {
                self.0
                    .lock()
                    .unwrap()
                    .push((text.as_str().to_owned(), voice.voice_id.as_str().to_owned()));
                Ok(Audio::kokoro(vec![0.2]))
            }
        }

        let previews = Arc::new(Mutex::new(Vec::new()));
        let mut use_case = SpeakSelection::new(
            PreviewRecorder(previews.clone()),
            FakePlayer {
                sample_counts: Arc::new(Mutex::new(Vec::new())),
            },
            VoiceSettings::default(),
        );

        use_case
            .preview_voice_with_playback_cues(
                VoiceSettings::from_voice_id("bm_daniel").unwrap(),
                |_| {},
            )
            .unwrap();

        assert_eq!(
            &*previews.lock().unwrap(),
            &[(VOICE_PREVIEW_TEXT.to_owned(), "bm_daniel".to_owned())]
        );
    }

    #[test]
    fn emits_audio_energy_only_after_each_chunk_starts_playing() {
        let cues = Arc::new(Mutex::new(Vec::new()));
        let mut use_case = SpeakSelection::new(
            FakeSynthesizer {
                seen: Arc::new(Mutex::new(Vec::new())),
            },
            FakePlayer {
                sample_counts: Arc::new(Mutex::new(Vec::new())),
            },
            VoiceSettings::default(),
        );

        use_case
            .execute_with_playback_cues(SelectedText::new("Read this").unwrap(), {
                let cues = cues.clone();
                move |cue| cues.lock().unwrap().push(cue.energy)
            })
            .unwrap();

        assert_eq!(cues.lock().unwrap().len(), 2);
        assert!(cues.lock().unwrap().iter().all(|energy| *energy > 0));
    }
}
