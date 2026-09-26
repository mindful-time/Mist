use std::sync::mpsc::SyncSender;

use crate::worker::WorkerCommand;

pub enum PlatformEvent {
    Speak(String),
    Error(String),
}

#[cfg(target_os = "macos")]
pub struct PlatformBridge {
    _service_provider: objc2::rc::Retained<crate::adapters::macos_service::ServiceProvider>,
}

#[cfg(target_os = "macos")]
impl PlatformBridge {
    pub fn new(commands: SyncSender<WorkerCommand>) -> Self {
        Self {
            _service_provider: crate::adapters::macos_service::register(commands),
        }
    }

    pub fn poll(&mut self) -> Option<PlatformEvent> {
        None
    }

    pub fn usage_hint(&self) -> &'static str {
        "Select text, then right-click"
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct PlatformBridge {
    _commands: SyncSender<WorkerCommand>,
    manager: Option<global_hotkey::GlobalHotKeyManager>,
    hotkey: global_hotkey::hotkey::HotKey,
    registration_error: Option<String>,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl PlatformBridge {
    pub fn new(commands: SyncSender<WorkerCommand>) -> Self {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};

        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
        let (manager, registration_error) = match global_hotkey::GlobalHotKeyManager::new() {
            Ok(manager) => match manager.register(hotkey) {
                Ok(()) => (Some(manager), None),
                Err(error) => (
                    None,
                    Some(format!("Could not register Ctrl+Alt+S: {error}")),
                ),
            },
            Err(error) => (None, Some(format!("Global shortcut unavailable: {error}"))),
        };

        Self {
            _commands: commands,
            manager,
            hotkey,
            registration_error,
        }
    }

    pub fn poll(&mut self) -> Option<PlatformEvent> {
        use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

        if self.manager.is_none() {
            return None;
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() && event.state == HotKeyState::Released {
                return Some(match capture_selected_text() {
                    Ok(text) => PlatformEvent::Speak(text),
                    Err(error) => PlatformEvent::Error(format!("{error:#}")),
                });
            }
        }
        None
    }

    pub fn usage_hint(&self) -> &'static str {
        if self.manager.is_some() {
            "Select text, then press Ctrl+Alt+S"
        } else {
            "Copy text, then right-click me"
        }
    }

    pub fn registration_error(&self) -> Option<&str> {
        self.registration_error.as_deref()
    }

    pub fn clipboard_text(&self) -> anyhow::Result<String> {
        clipboard_text()
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn capture_selected_text() -> anyhow::Result<String> {
    use std::{thread, time::Duration};

    use anyhow::{Context, bail};
    use arboard::Clipboard;
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let marker = format!("select-to-speak-copy-marker-{}", std::process::id());
    Clipboard::new()
        .context("could not open the clipboard")?
        .set_text(&marker)
        .context("could not prepare the clipboard")?;

    let mut keyboard = Enigo::new(&Settings::default()).context("could not access the keyboard")?;
    keyboard
        .key(Key::Control, Direction::Press)
        .context("could not press Control")?;
    let copy_result = keyboard.key(Key::Unicode('c'), Direction::Click);
    let release_result = keyboard.key(Key::Control, Direction::Release);
    copy_result.context("could not send Copy to the focused app")?;
    release_result.context("could not release Control")?;

    thread::sleep(Duration::from_millis(180));
    let text = clipboard_text()?;
    if text == marker {
        bail!("the focused app did not provide selected text");
    }
    Ok(text)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn clipboard_text() -> anyhow::Result<String> {
    use anyhow::{Context, bail};
    use arboard::Clipboard;

    let text = Clipboard::new()
        .context("could not open the clipboard")?
        .get_text()
        .context("the clipboard does not contain text")?;
    if text.trim().is_empty() {
        bail!("the clipboard does not contain text");
    }
    Ok(text)
}
