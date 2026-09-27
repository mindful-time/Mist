use crate::core::domain::{Audio, AudioFeatures};

/// Driven port for the system's audio output.
pub trait AudioPlayer {
    /// Starts and waits for an audio chunk. Samples are emitted only after the
    /// platform player launches and track its playback position.
    fn play(
        &mut self,
        audio: &Audio,
        on_sample: &mut dyn FnMut(AudioFeatures),
    ) -> anyhow::Result<()>;
}
