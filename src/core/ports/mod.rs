//! Core-owned driven ports, grouped by the capability an adapter supplies.

mod audio;
mod models;
mod speech;

pub use audio::AudioPlayer;
pub use models::ModelProvisioner;
pub use speech::{
    DynSpeechSynthesizer, LoadedSpeechEngine, SpeechEngineFactory, SpeechSynthesizer, VoiceCatalog,
};
