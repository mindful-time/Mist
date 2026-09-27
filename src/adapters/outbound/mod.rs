//! Secondary adapters grouped by the capability they provide to the core.

pub mod audio;
pub mod persistence;
pub mod provisioning;
pub mod speech;

// Compatibility aliases while callers migrate to the capability packages.
pub use audio::system as system_audio;
pub use persistence::{
    inference as model_preferences, playback as playback_preferences, voice as voice_preferences,
};
pub use provisioning::kokoro as model_store;
pub use speech::kokoro;
pub use speech::kokoro::catalog as kokoro_catalog;
