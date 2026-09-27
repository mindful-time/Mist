use std::{thread, time::Duration};

use anyhow::{Context, bail};
#[cfg(not(target_os = "macos"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use sha2::{Digest, Sha256};

use crate::domain::SelectedText;

const COPY_TIMEOUT: Duration = Duration::from_millis(700);
const COPY_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardLease {
    fingerprint: [u8; 32],
    version: Option<u64>,
}

impl ClipboardLease {
    fn new(value: &str, version: Option<u64>) -> Self {
        Self {
            fingerprint: fingerprint(value),
            version,
        }
    }

    fn still_owns(&self, value: &str, current_version: Option<u64>) -> bool {
        self.fingerprint == fingerprint(value) && self.version_matches(current_version)
    }

    fn version_matches(&self, current_version: Option<u64>) -> bool {
        self.version
            .is_none_or(|version| current_version == Some(version))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardCapture {
    pub text: SelectedText,
    pub lease: ClipboardLease,
}

/// Sends the native Copy shortcut to the still-focused source application and
/// reads the value it placed on the regular clipboard. This is deliberately a
/// fallback: callers should first try the platform accessibility/selection API.
pub fn copy_selected_text() -> anyhow::Result<ClipboardCapture> {
    let before_version = clipboard_version();
    let before_text = read_clipboard_text().ok();
    send_copy_shortcut()?;

    let started = std::time::Instant::now();
    loop {
        thread::sleep(COPY_POLL_INTERVAL);
        let version = clipboard_version();
        if let Ok(value) = read_clipboard_text() {
            let changed = match (before_version, version) {
                (Some(before), Some(after)) => before != after,
                _ => before_text.as_deref() != Some(value.as_str()),
            };
            if changed {
                let lease = ClipboardLease::new(&value, version);
                let text = match SelectedText::new(&value) {
                    Ok(text) => text,
                    Err(error) => {
                        let _ = clear_if_owned(&lease);
                        return Err(error.into());
                    }
                };
                return Ok(ClipboardCapture { text, lease });
            }
        }
        if started.elapsed() >= COPY_TIMEOUT {
            bail!("The app did not place the selected text on the clipboard");
        }
    }
}

pub fn read_current_text() -> anyhow::Result<SelectedText> {
    SelectedText::new(read_clipboard_text()?).map_err(Into::into)
}

/// Clears only the exact temporary value Mist captured. A newer clipboard
/// version or different content always belongs to the user and is preserved.
pub fn clear_if_owned(lease: &ClipboardLease) -> anyhow::Result<bool> {
    let mut clipboard = arboard::Clipboard::new().context("could not open the clipboard")?;
    let current = match clipboard.get_text() {
        Ok(current) => current,
        Err(_) => return Ok(false),
    };
    if !lease.still_owns(&current, clipboard_version()) {
        return Ok(false);
    }
    // Recheck the platform change token immediately before clearing. macOS
    // does not expose a compare-and-clear primitive, so the operation remains
    // best effort there; this closes the avoidable check/reopen window.
    if !lease.version_matches(clipboard_version()) {
        return Ok(false);
    }
    clipboard
        .clear()
        .context("could not clear Mist's temporary clipboard text")?;
    Ok(true)
}

fn read_clipboard_text() -> anyhow::Result<String> {
    arboard::Clipboard::new()
        .context("could not open the clipboard")?
        .get_text()
        .context("the clipboard does not contain text")
}

#[cfg(target_os = "macos")]
fn send_copy_shortcut() -> anyhow::Result<()> {
    use core_graphics::{
        event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
        event_source::{CGEventSource, CGEventSourceStateID},
    };

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|()| anyhow::anyhow!("could not create the macOS Copy event source"))?;
    let command_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::COMMAND, true)
        .map_err(|()| anyhow::anyhow!("could not create Command-down"))?;
    let copy_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_C, true)
        .map_err(|()| anyhow::anyhow!("could not create C-down"))?;
    let copy_up = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_C, false)
        .map_err(|()| anyhow::anyhow!("could not create C-up"))?;
    let command_up = CGEvent::new_keyboard_event(source, KeyCode::COMMAND, false)
        .map_err(|()| anyhow::anyhow!("could not create Command-up"))?;

    command_down.set_flags(CGEventFlags::CGEventFlagCommand);
    copy_down.set_flags(CGEventFlags::CGEventFlagCommand);
    copy_up.set_flags(CGEventFlags::CGEventFlagCommand);
    command_up.set_flags(CGEventFlags::empty());
    command_down.post(CGEventTapLocation::HID);
    copy_down.post(CGEventTapLocation::HID);
    copy_up.post(CGEventTapLocation::HID);
    command_up.post(CGEventTapLocation::HID);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn send_copy_shortcut() -> anyhow::Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .context("could not connect to the system input service")?;
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .context("could not press the Copy modifier")?;
    let copy_result = enigo.key(Key::Unicode('c'), Direction::Click);
    let release_result = enigo.key(modifier, Direction::Release);
    copy_result.context("could not send the Copy key")?;
    release_result.context("could not release the Copy modifier")?;
    Ok(())
}

fn fingerprint(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

#[cfg(target_os = "macos")]
fn clipboard_version() -> Option<u64> {
    use objc2_app_kit::NSPasteboard;

    u64::try_from(NSPasteboard::generalPasteboard().changeCount()).ok()
}

#[cfg(target_os = "windows")]
fn clipboard_version() -> Option<u64> {
    #[link(name = "User32")]
    unsafe extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }

    // SAFETY: This parameterless Win32 query only reads the global clipboard
    // sequence counter.
    Some(u64::from(unsafe { GetClipboardSequenceNumber() }))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn clipboard_version() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_clipboard_is_cleared_only_while_value_and_version_match() {
        let lease = ClipboardLease::new("Mist copied this", Some(17));

        assert!(lease.still_owns("Mist copied this", Some(17)));
        assert!(!lease.still_owns("The user copied something newer", Some(18)));
        assert!(!lease.still_owns("Mist copied this", Some(18)));
    }

    #[test]
    fn content_fingerprint_protects_platforms_without_clipboard_versions() {
        let lease = ClipboardLease::new("Selected text", None);

        assert!(lease.still_owns("Selected text", None));
        assert!(!lease.still_owns("New clipboard text", None));
    }
}
