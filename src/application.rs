use crate::{
    domain::{SelectedText, VoiceSettings},
    ports::{AudioPlayer, ModelProvisioner, SpeechSynthesizer},
};

/// Application use-case. It knows nothing about AppKit, Kokoro, or system audio.
pub struct SpeakSelection<S, P> {
    synthesizer: S,
    player: P,
    voice: VoiceSettings,
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
        }
    }

    pub fn execute(&mut self, text: SelectedText) -> anyhow::Result<()> {
        let audio = self.synthesizer.synthesize(&text, &self.voice)?;
        self.player.play(&audio)
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
    }

    struct FakePlayer {
        sample_count: Arc<Mutex<usize>>,
    }

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
        fn play(&mut self, audio: &Audio) -> anyhow::Result<()> {
            *self.sample_count.lock().unwrap() = audio.samples.len();
            Ok(())
        }
    }

    #[test]
    fn sends_synthesized_audio_to_the_player() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sample_count = Arc::new(Mutex::new(0));
        let mut use_case = SpeakSelection::new(
            FakeSynthesizer { seen: seen.clone() },
            FakePlayer {
                sample_count: sample_count.clone(),
            },
            VoiceSettings::default(),
        );

        use_case
            .execute(SelectedText::new("Read this").unwrap())
            .unwrap();

        assert_eq!(&*seen.lock().unwrap(), &["Read this"]);
        assert_eq!(*sample_count.lock().unwrap(), 3);
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
}
