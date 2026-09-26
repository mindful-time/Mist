use std::sync::mpsc::SyncSender;

use crate::{domain::SelectedText, worker::WorkerCommand};

pub enum PlatformEvent {
    Speak(SelectedText),
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

    pub fn registration_error(&self) -> Option<&str> {
        None
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
type CaptureMessage = (u64, Result<SelectedText, String>);

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct PlatformBridge {
    manager: Option<global_hotkey::GlobalHotKeyManager>,
    hotkey: global_hotkey::hotkey::HotKey,
    registration_error: Option<String>,
    capture_sender: std::sync::mpsc::Sender<CaptureMessage>,
    capture_receiver: std::sync::mpsc::Receiver<CaptureMessage>,
    capture_generation: u64,
    capture_in_flight: Option<(u64, std::time::Instant)>,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl PlatformBridge {
    pub fn new(_commands: SyncSender<WorkerCommand>) -> Self {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};

        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
        let (manager, registration_error) = if is_wayland_session() {
            (
                None,
                Some("Wayland blocks the global shortcut; use the pet menu".to_owned()),
            )
        } else {
            match global_hotkey::GlobalHotKeyManager::new() {
                Ok(manager) => match manager.register(hotkey) {
                    Ok(()) => (Some(manager), None),
                    Err(error) => (
                        None,
                        Some(format!("Could not register Ctrl+Alt+S: {error}")),
                    ),
                },
                Err(error) => (None, Some(format!("Global shortcut unavailable: {error}"))),
            }
        };
        let (capture_sender, capture_receiver) = std::sync::mpsc::channel();

        Self {
            manager,
            hotkey,
            registration_error,
            capture_sender,
            capture_receiver,
            capture_generation: 0,
            capture_in_flight: None,
        }
    }

    pub fn poll(&mut self) -> Option<PlatformEvent> {
        use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

        while let Ok((generation, message)) = self.capture_receiver.try_recv() {
            if self
                .capture_in_flight
                .is_some_and(|(active, _)| active == generation)
            {
                self.capture_in_flight = None;
                return Some(match message {
                    Ok(text) => PlatformEvent::Speak(text),
                    Err(error) => PlatformEvent::Error(error),
                });
            }
        }

        if self
            .capture_in_flight
            .is_some_and(|(_, started)| started.elapsed() > std::time::Duration::from_secs(3))
        {
            self.capture_in_flight = None;
            return Some(PlatformEvent::Error(
                "Selection capture timed out; try again".to_owned(),
            ));
        }

        if self.manager.is_none() || self.capture_in_flight.is_some() {
            return None;
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() && event.state == HotKeyState::Released {
                if let Err(error) = self.start_capture(capture_selected_text) {
                    return Some(PlatformEvent::Error(error));
                }
                break;
            }
        }
        None
    }

    pub fn usage_hint(&self) -> &'static str {
        if self.manager.is_some() {
            "Select text, then press Ctrl+Alt+S"
        } else {
            "Right-click to paste or type text"
        }
    }

    pub fn registration_error(&self) -> Option<&str> {
        self.registration_error.as_deref()
    }

    pub fn request_clipboard_text(&mut self) -> Result<(), String> {
        self.start_capture(capture_clipboard_text)
    }

    fn start_capture(
        &mut self,
        capture: fn() -> anyhow::Result<SelectedText>,
    ) -> Result<(), String> {
        if self.capture_in_flight.is_some() {
            return Err("Selection capture is already in progress".to_owned());
        }

        self.capture_generation = self.capture_generation.wrapping_add(1);
        let generation = self.capture_generation;
        self.capture_in_flight = Some((generation, std::time::Instant::now()));
        let sender = self.capture_sender.clone();
        std::thread::Builder::new()
            .name("selected-text-capture".to_owned())
            .spawn(move || {
                let message = capture().map_err(|error| format!("{error:#}"));
                let _ = sender.send((generation, message));
            })
            .map_err(|error| {
                self.capture_in_flight = None;
                format!("Could not start selection capture: {error}")
            })?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn is_wayland_session() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
}

/// Reads the focused control's selection without synthesizing Copy, so the
/// user's clipboard remains completely untouched.
#[cfg(target_os = "windows")]
fn capture_selected_text() -> anyhow::Result<SelectedText> {
    use anyhow::Context;
    use uiautomation::{UIAutomation, patterns::UITextPattern};

    let automation = UIAutomation::new().context("could not start Windows UI Automation")?;
    let focused = automation
        .get_focused_element()
        .context("could not inspect the focused control")?;
    let pattern: UITextPattern = focused
        .get_pattern()
        .context("the focused control does not expose selected text")?;
    let ranges = pattern
        .get_selection()
        .context("could not read the selected text")?;
    let mut text = String::new();
    for range in ranges {
        text.push_str(
            &range
                .get_text(-1)
                .context("could not read a selected text range")?,
        );
    }
    SelectedText::new(text).map_err(Into::into)
}

/// X11 publishes highlighted text through PRIMARY, independently of the normal
/// clipboard. Reading it is both faster and lossless for existing clipboard
/// contents.
#[cfg(target_os = "linux")]
fn capture_selected_text() -> anyhow::Result<SelectedText> {
    use std::time::Duration;

    use anyhow::{Context, bail};

    if is_wayland_session() {
        bail!("Wayland blocks global selection capture; copy the text and use the pet menu");
    }

    let clipboard =
        x11_clipboard::Clipboard::new().context("could not connect to the X11 selection")?;
    let utf8 = clipboard.load(
        clipboard.getter.atoms.primary,
        clipboard.getter.atoms.utf8_string,
        clipboard.getter.atoms.property,
        Duration::from_millis(500),
    );
    let text = match utf8 {
        Ok(bytes) => String::from_utf8(bytes).context("the selected text is not valid UTF-8")?,
        Err(_) => {
            let bytes = clipboard
                .load(
                    clipboard.getter.atoms.primary,
                    clipboard.getter.atoms.string,
                    clipboard.getter.atoms.property,
                    Duration::from_millis(500),
                )
                .context("the X11 primary selection does not contain text")?;
            bytes.into_iter().map(char::from).collect()
        }
    };
    SelectedText::new(text.trim_matches('\0')).map_err(Into::into)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn capture_clipboard_text() -> anyhow::Result<SelectedText> {
    let text = clipboard_text()?;
    SelectedText::new(text).map_err(Into::into)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn clipboard_text() -> anyhow::Result<String> {
    use anyhow::Context;

    clipboard_text_from(&mut arboard::Clipboard::new().context("could not open the clipboard")?)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn clipboard_text_from(clipboard: &mut arboard::Clipboard) -> anyhow::Result<String> {
    use anyhow::{Context, bail};

    let text = clipboard
        .get_text()
        .context("the clipboard does not contain text")?;
    if text.trim().is_empty() {
        bail!("the clipboard does not contain text");
    }
    Ok(text)
}
