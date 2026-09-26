use std::sync::mpsc::SyncSender;

#[cfg(target_os = "macos")]
use crate::adapters::macos_selection::capture_selected_text;
#[cfg(target_os = "windows")]
use crate::adapters::windows_selection::{capture_clipboard_text, capture_selected_text};
#[cfg(target_os = "linux")]
use crate::adapters::{
    linux_selection::{capture_clipboard_text, capture_selected_text, is_wayland_session},
    wayland_shortcut::{self, WaylandShortcutMessage},
};
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
    shortcut_label: String,
    usage_hint: String,
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
        let (manager, registration_error) = if is_wayland() {
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
            shortcut_label: "Ctrl+Alt+S".to_owned(),
            usage_hint: SHORTCUT_HINT.to_owned(),
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
            wayland_shortcuts: is_wayland_session().then(wayland_shortcut::spawn),
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
                WaylandShortcutMessage::Registered(trigger) => {
                    self.usage_hint = format!("Select text anywhere, then press {trigger}");
                    self.shortcut_label = trigger;
                    self.registration_error = None;
                }
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

    pub fn usage_hint(&self) -> &str {
        &self.usage_hint
    }

    pub fn shortcut_label(&self) -> &str {
        &self.shortcut_label
    }

    #[cfg(target_os = "macos")]
    pub fn accessibility_required(&self) -> bool {
        self.accessibility_pending
    }

    #[cfg(not(target_os = "macos"))]
    pub fn accessibility_required(&self) -> bool {
        false
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
fn is_wayland() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    is_wayland_session()
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
