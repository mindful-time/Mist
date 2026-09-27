pub mod clipboard_fallback;
pub mod kokoro;
pub mod playback_preferences;
pub mod system_audio;
pub mod voice_preferences;

#[cfg(target_os = "linux")]
pub mod linux_selection;
#[cfg(target_os = "macos")]
pub mod macos_selection;
#[cfg(target_os = "macos")]
pub mod macos_service;
#[cfg(target_os = "macos")]
pub mod macos_shortcut;
#[cfg(target_os = "linux")]
pub mod wayland_shortcut;
#[cfg(target_os = "windows")]
pub mod windows_selection;
