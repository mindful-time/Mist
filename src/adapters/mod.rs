pub mod kokoro;
pub mod system_audio;

#[cfg(target_os = "linux")]
pub mod linux_selection;
#[cfg(target_os = "macos")]
pub mod macos_selection;
#[cfg(target_os = "macos")]
pub mod macos_service;
#[cfg(target_os = "linux")]
pub mod wayland_shortcut;
#[cfg(target_os = "windows")]
pub mod windows_selection;
