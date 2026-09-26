use std::sync::mpsc::SyncSender;

use crate::{domain::SelectedText, worker::WorkerCommand};

const SHORTCUT_HINT: &str = "Select text anywhere, then press Ctrl+Alt+S";
type CaptureMessage = (u64, Result<SelectedText, String>);

pub enum PlatformEvent {
    Speak(SelectedText),
    Error(String),
}

pub struct PlatformBridge {
    #[cfg(target_os = "macos")]
    _service_provider: objc2::rc::Retained<crate::adapters::macos_service::ServiceProvider>,
    manager: Option<global_hotkey::GlobalHotKeyManager>,
    hotkey: global_hotkey::hotkey::HotKey,
    registration_error: Option<String>,
    #[cfg(target_os = "macos")]
    shortcut_error: Option<String>,
    #[cfg(target_os = "macos")]
    accessibility_pending: bool,
    capture_sender: std::sync::mpsc::Sender<CaptureMessage>,
    capture_receiver: std::sync::mpsc::Receiver<CaptureMessage>,
    capture_generation: u64,
    capture_in_flight: Option<(u64, std::time::Instant)>,
    #[cfg(target_os = "linux")]
    wayland_shortcuts: Option<std::sync::mpsc::Receiver<WaylandShortcutMessage>>,
}

impl PlatformBridge {
    pub fn new(commands: SyncSender<WorkerCommand>) -> Self {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};

        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
        let (manager, registration_error) = if is_wayland_session() {
            (None, None)
        } else {
            register_hotkey(hotkey)
        };
        let (capture_sender, capture_receiver) = std::sync::mpsc::channel();

        #[cfg(target_os = "macos")]
        let shortcut_error = registration_error.clone();
        #[cfg(target_os = "macos")]
        let accessibility_pending =
            !crate::adapters::macos_selection::request_accessibility_permission();
        #[cfg(target_os = "macos")]
        let registration_error = if accessibility_pending {
            let permission_error =
                "Allow Accessibility access to read selections outside native Services";
            Some(match registration_error {
                Some(shortcut_error) => format!("{shortcut_error}; {permission_error}"),
                None => permission_error.to_owned(),
            })
        } else {
            registration_error
        };

        #[cfg(not(target_os = "macos"))]
        let _ = commands;

        Self {
            #[cfg(target_os = "macos")]
            _service_provider: crate::adapters::macos_service::register(commands),
            manager,
            hotkey,
            registration_error,
            #[cfg(target_os = "macos")]
            shortcut_error,
            #[cfg(target_os = "macos")]
            accessibility_pending,
            capture_sender,
            capture_receiver,
            capture_generation: 0,
            capture_in_flight: None,
            #[cfg(target_os = "linux")]
            wayland_shortcuts: is_wayland_session().then(spawn_wayland_shortcut_listener),
        }
    }

    pub fn poll(&mut self) -> Option<PlatformEvent> {
        use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

        #[cfg(target_os = "macos")]
        if self.accessibility_pending
            && crate::adapters::macos_selection::is_accessibility_trusted()
        {
            self.accessibility_pending = false;
            self.registration_error.clone_from(&self.shortcut_error);
        }

        #[cfg(target_os = "linux")]
        if let Some(message) = self
            .wayland_shortcuts
            .as_ref()
            .and_then(|messages| messages.try_recv().ok())
        {
            match message {
                WaylandShortcutMessage::Registered => self.registration_error = None,
                WaylandShortcutMessage::Activated => {
                    if let Err(error) = self.start_capture(capture_selected_text) {
                        return Some(PlatformEvent::Error(error));
                    }
                }
                WaylandShortcutMessage::Error(error) => {
                    self.registration_error = Some(error.clone());
                    return Some(PlatformEvent::Error(error));
                }
            }
        }

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
        SHORTCUT_HINT
    }

    pub fn registration_error(&self) -> Option<&str> {
        self.registration_error.as_deref()
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
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

fn register_hotkey(
    hotkey: global_hotkey::hotkey::HotKey,
) -> (Option<global_hotkey::GlobalHotKeyManager>, Option<String>) {
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
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn is_wayland_session() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
}

#[cfg(target_os = "linux")]
enum WaylandShortcutMessage {
    Registered,
    Activated,
    Error(String),
}

#[cfg(target_os = "linux")]
fn spawn_wayland_shortcut_listener() -> std::sync::mpsc::Receiver<WaylandShortcutMessage> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread_sender = sender.clone();
    let spawn_result = std::thread::Builder::new()
        .name("wayland-global-shortcut".to_owned())
        .spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| runtime.block_on(run_wayland_shortcut(thread_sender.clone())));
            if let Err(error) = result {
                let _ = thread_sender.send(WaylandShortcutMessage::Error(format!(
                    "Wayland shortcut unavailable: {error:#}"
                )));
            }
        });
    if let Err(error) = spawn_result {
        let _ = sender.send(WaylandShortcutMessage::Error(format!(
            "Could not start the Wayland shortcut listener: {error}"
        )));
    }
    receiver
}

#[cfg(target_os = "linux")]
async fn run_wayland_shortcut(
    sender: std::sync::mpsc::Sender<WaylandShortcutMessage>,
) -> anyhow::Result<()> {
    use anyhow::{Context, bail};
    use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    const SHORTCUT_ID: &str = "speak-selection";

    let shortcuts = GlobalShortcuts::new()
        .await
        .context("the desktop does not provide the GlobalShortcuts portal")?;
    let session = shortcuts
        .create_session(Default::default())
        .await
        .context("could not create a global-shortcut session")?;
    let requested = [
        NewShortcut::new(SHORTCUT_ID, "Speak selected text with Kokoro")
            .preferred_trigger("CTRL+ALT+S"),
    ];
    let bound = shortcuts
        .bind_shortcuts(&session, &requested, None, BindShortcutsOptions::default())
        .await?
        .response()?;
    if !bound
        .shortcuts()
        .iter()
        .any(|shortcut| shortcut.id() == SHORTCUT_ID)
    {
        bail!("the desktop did not grant the Speak Selection shortcut");
    }

    let mut activations = shortcuts.receive_activated().await?;
    let _ = sender.send(WaylandShortcutMessage::Registered);
    while let Some(event) = activations.next().await {
        if event.shortcut_id() == SHORTCUT_ID
            && sender.send(WaylandShortcutMessage::Activated).is_err()
        {
            break;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn capture_selected_text() -> anyhow::Result<SelectedText> {
    crate::adapters::macos_selection::capture_selected_text()
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
    use std::{io::Read, time::Duration};

    use anyhow::Context;

    if is_wayland_session() {
        use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

        let (mut pipe, _) = get_contents(ClipboardType::Primary, Seat::Unspecified, MimeType::Text)
            .context("the Wayland compositor does not expose the primary text selection")?;
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)
            .context("could not read the Wayland text selection")?;
        let text = String::from_utf8(bytes).context("the selected text is not valid UTF-8")?;
        return SelectedText::new(text.trim_matches('\0')).map_err(Into::into);
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

#[cfg(test)]
mod tests {
    #[test]
    fn shortcut_hint_explains_the_system_wide_activation() {
        assert_eq!(
            super::SHORTCUT_HINT,
            "Select text anywhere, then press Ctrl+Alt+S"
        );
    }
}
