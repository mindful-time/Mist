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
type CaptureMessage = Result<CaptureOutcome, String>;

#[cfg(any(target_os = "windows", target_os = "linux"))]
struct CaptureOutcome {
    result: Result<SelectedText, String>,
    // Linux clipboards may require the writing process to remain the owner.
    clipboard_owner: arboard::Clipboard,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct PlatformBridge {
    manager: Option<global_hotkey::GlobalHotKeyManager>,
    hotkey: global_hotkey::hotkey::HotKey,
    registration_error: Option<String>,
    capture_sender: std::sync::mpsc::Sender<CaptureMessage>,
    capture_receiver: std::sync::mpsc::Receiver<CaptureMessage>,
    capture_in_flight: bool,
    _clipboard_owner: Option<arboard::Clipboard>,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl PlatformBridge {
    pub fn new(_commands: SyncSender<WorkerCommand>) -> Self {
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
        let (capture_sender, capture_receiver) = std::sync::mpsc::channel();

        Self {
            manager,
            hotkey,
            registration_error,
            capture_sender,
            capture_receiver,
            capture_in_flight: false,
            _clipboard_owner: None,
        }
    }

    pub fn poll(&mut self) -> Option<PlatformEvent> {
        use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

        if let Ok(message) = self.capture_receiver.try_recv() {
            self.capture_in_flight = false;
            return Some(match message {
                Ok(outcome) => {
                    self._clipboard_owner = Some(outcome.clipboard_owner);
                    match outcome.result {
                        Ok(text) => PlatformEvent::Speak(text),
                        Err(error) => PlatformEvent::Error(error),
                    }
                }
                Err(error) => PlatformEvent::Error(error),
            });
        }

        if self.manager.is_none() || self.capture_in_flight {
            return None;
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() && event.state == HotKeyState::Released {
                self.capture_in_flight = true;
                let sender = self.capture_sender.clone();
                std::thread::Builder::new()
                    .name("selected-text-capture".to_owned())
                    .spawn(move || {
                        let message = capture_selected_text().map_err(|error| format!("{error:#}"));
                        let _ = sender.send(message);
                    })
                    .expect("selection capture thread should start");
                break;
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

    pub fn clipboard_text(&self) -> anyhow::Result<SelectedText> {
        let text = clipboard_text()?;
        SelectedText::new(text).map_err(Into::into)
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
enum ClipboardBackup {
    Text(String),
    Image(arboard::ImageData<'static>),
    Empty,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl ClipboardBackup {
    fn read(clipboard: &mut arboard::Clipboard) -> Self {
        if let Ok(text) = clipboard.get_text() {
            Self::Text(text)
        } else if let Ok(image) = clipboard.get_image() {
            Self::Image(image)
        } else {
            Self::Empty
        }
    }

    fn restore(self, clipboard: &mut arboard::Clipboard) -> anyhow::Result<()> {
        use anyhow::Context;

        match self {
            Self::Text(text) => clipboard
                .set_text(text)
                .context("could not restore clipboard text"),
            Self::Image(image) => clipboard
                .set_image(image)
                .context("could not restore clipboard image"),
            Self::Empty => clipboard.clear().context("could not clear the clipboard"),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn capture_selected_text() -> anyhow::Result<CaptureOutcome> {
    use std::{thread, time::Duration};

    use anyhow::{Context, bail};
    use arboard::Clipboard;
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    // Wait for Ctrl+Alt to be released, so the copy does not become Ctrl+Alt+C.
    thread::sleep(Duration::from_millis(160));

    let mut clipboard = Clipboard::new().context("could not open the clipboard")?;
    let backup = ClipboardBackup::read(&mut clipboard);
    let marker = format!("select-to-speak-copy-marker-{}", std::process::id());
    clipboard
        .set_text(&marker)
        .context("could not prepare the clipboard")?;

    let capture_result = (|| -> anyhow::Result<SelectedText> {
        let mut keyboard =
            Enigo::new(&Settings::default()).context("could not access the keyboard")?;
        keyboard
            .key(Key::Control, Direction::Press)
            .context("could not press Control")?;
        let copy_result = keyboard.key(Key::Unicode('c'), Direction::Click);
        let release_result = keyboard.key(Key::Control, Direction::Release);
        copy_result.context("could not send Copy to the focused app")?;
        release_result.context("could not release Control")?;

        thread::sleep(Duration::from_millis(180));
        let text = clipboard_text_from(&mut clipboard)?;
        if text == marker {
            bail!("the focused app did not provide selected text");
        }
        SelectedText::new(text).map_err(Into::into)
    })();

    let restore_result = backup.restore(&mut clipboard);
    let result = match (capture_result, restore_result) {
        (Ok(text), Ok(())) => Ok(text),
        (Err(error), Ok(())) => Err(format!("{error:#}")),
        (_, Err(error)) => Err(format!("{error:#}")),
    };

    Ok(CaptureOutcome {
        result,
        clipboard_owner: clipboard,
    })
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
