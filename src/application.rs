use crate::{
    domain::{SelectedText, VoiceSettings},
    ports::{AudioPlayer, SpeechSynthesizer},
};

/// Application use-case. It knows nothing about AppKit, Kokoro, or `afplay`.
pub struct SpeakSelection<S, P> {
    synthesizer: S,
    player: P,
    voice: VoiceSettings,
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
    use std::sync::{Arc, Mutex};

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
}
