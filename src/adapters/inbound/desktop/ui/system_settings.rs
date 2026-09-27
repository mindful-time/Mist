//! Links from the desktop adapter to native system settings.

#[cfg(target_os = "macos")]
pub(super) fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Accessibility settings: {error}"))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn open_accessibility_settings() -> Result<(), String> {
    Err("Accessibility settings are only available on macOS".to_owned())
}
