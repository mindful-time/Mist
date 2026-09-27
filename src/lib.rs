pub mod adapters;
pub mod core;
pub mod runtime;

// Stable compatibility paths for callers while the canonical ownership lives
// under `core`.
pub use adapters::model_store;
pub use adapters::platform;
pub use core::{application, domain, playback, ports};
pub use runtime::worker;

pub use application::{InstallModel, SpeakSelection};
pub use domain::{
    Audio, AudioFeatures, InferenceProviderId, LanguageId, LanguageProfile, MistPalette,
    PlaybackMode, PlaybackPreferences, PlaybackSpeed, ProviderCapability, ProviderPerformance,
    QueueError, QueueItemId, QueueItemState, QueuedSpeech, SelectedText, SelectionCaptureError,
    SelectionError, SpeechQueue, VoiceId, VoiceProfile, VoiceSettings,
};
pub use playback::{PlaybackController, PlaybackPhase, PlaybackStopped, PlaybackToken};
pub use ports::{
    AudioPlayer, DynSpeechSynthesizer, LoadedSpeechEngine, ModelProvisioner, SpeechEngineFactory,
    SpeechSynthesizer, VoiceCatalog,
};
