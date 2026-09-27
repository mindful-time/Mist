//! Hexagonal adapters grouped by the direction in which they cross the core boundary.

pub mod inbound;
pub mod outbound;

// Stable compatibility paths for existing callers. New composition code should
// prefer `adapters::inbound::*` and `adapters::outbound::*`.
#[cfg(target_os = "windows")]
pub use inbound::windows_selection;
pub use inbound::{clipboard_fallback, platform};
#[cfg(target_os = "linux")]
pub use inbound::{linux_selection, wayland_shortcut};
#[cfg(target_os = "macos")]
pub use inbound::{macos_selection, macos_service, macos_shortcut};

pub use outbound::{
    kokoro, kokoro_catalog, model_preferences, model_store, playback_preferences, system_audio,
    voice_preferences,
};
