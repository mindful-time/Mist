pub mod adapters;
pub mod application;
pub mod domain;
pub mod model_store;
pub mod platform;
pub mod ports;
pub mod worker;

pub use application::{InstallModel, SpeakSelection};
pub use domain::{Audio, SelectedText, SelectionError, VoiceSettings};
pub use ports::{AudioPlayer, ModelProvisioner, SpeechSynthesizer};
