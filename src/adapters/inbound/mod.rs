//! Primary adapters that translate user and operating-system input into core actions.

pub mod desktop;
pub mod os;
pub mod selection;

// Compatibility aliases while callers migrate to the capability packages.
pub use desktop::platform;
#[cfg(target_os = "linux")]
pub use os::linux::{selection as linux_selection, shortcut as wayland_shortcut};
#[cfg(target_os = "macos")]
pub use os::macos::{
    selection as macos_selection, service as macos_service, shortcut as macos_shortcut,
};
#[cfg(target_os = "windows")]
pub use os::windows::selection as windows_selection;
pub use selection::clipboard as clipboard_fallback;
