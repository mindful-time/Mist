//! Desktop input boundary: global activation, selection capture, and clipboard fallback.

#[cfg(target_os = "linux")]
use crate::adapters::inbound::os::linux::{
    selection::is_wayland_session,
    shortcut::{self as wayland_shortcut, WaylandShortcutMessage},
};
#[cfg(target_os = "macos")]
use crate::adapters::inbound::os::macos::selection::frontmost_application_pid;
#[cfg(target_os = "macos")]
use crate::adapters::inbound::os::macos::shortcut::{self as macos_shortcut, MacShortcutMessage};
use crate::{
    adapters::inbound::selection::clipboard::{
        ClipboardLease, clear_if_owned, copy_selected_text, read_current_text,
    },
    domain::{SelectedText, SelectionCaptureError},
};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

const SHORTCUT_HINT: &str = "Select text, then press Ctrl+Space";
const NATIVE_CAPTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const FALLBACK_CAPTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);
type CaptureMessage = (u64, Result<CapturedSelection, String>);

#[derive(Clone, Copy, Debug)]
struct SelectionTarget {
    #[cfg(target_os = "macos")]
    pid: i32,
}

const CAPTURE_NATIVE: u8 = 0;
const CAPTURE_FALLBACK: u8 = 1;
const CAPTURE_CANCELLED: u8 = 2;

#[derive(Clone, Debug)]
struct CaptureControl(Arc<AtomicU8>);

impl CaptureControl {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(CAPTURE_NATIVE)))
    }

    fn begin_fallback(&self) -> bool {
        self.0
            .compare_exchange(
                CAPTURE_NATIVE,
                CAPTURE_FALLBACK,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn cancel_native(&self) -> bool {
        self.0
            .compare_exchange(
                CAPTURE_NATIVE,
                CAPTURE_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn fallback_started(&self) -> bool {
        self.0.load(Ordering::Acquire) == CAPTURE_FALLBACK
    }
}

#[derive(Clone, Debug)]
pub struct CapturedSelection {
    pub text: SelectedText,
    pub clipboard_lease: Option<ClipboardLease>,
}

pub enum PlatformEvent {
    Captured(CapturedSelection),
    AccessibilityPermissionRequired,
    Error(String),
}

pub struct PlatformBridge {
    #[cfg(target_os = "macos")]
    _service_provider:
        objc2::rc::Retained<crate::adapters::inbound::os::macos::service::ServiceProvider>,
    #[cfg(target_os = "macos")]
    service_receiver: std::sync::mpsc::Receiver<SelectedText>,
    #[cfg(target_os = "macos")]
    macos_shortcuts: Option<std::sync::mpsc::Receiver<MacShortcutMessage>>,
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
    capture_in_flight: Option<(u64, std::time::Instant, CaptureControl)>,
    deferred_clipboard_cleanup: Vec<ClipboardLease>,
    automatic_clipboard_fallback: bool,
    #[cfg(target_os = "linux")]
    wayland_shortcuts: Option<std::sync::mpsc::Receiver<WaylandShortcutMessage>>,
}

impl Default for PlatformBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBridge {
    pub fn new() -> Self {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};

        let hotkey = HotKey::new(Some(Modifiers::CONTROL), Code::Space);
        #[cfg(target_os = "macos")]
        let (manager, registration_error) = (None, None);
        #[cfg(not(target_os = "macos"))]
        let (manager, registration_error) = if is_wayland() {
            (None, None)
        } else {
            register_hotkey(hotkey)
        };
        let (capture_sender, capture_receiver) = std::sync::mpsc::channel();
        #[cfg(target_os = "macos")]
        let (service_sender, service_receiver) = std::sync::mpsc::channel();
        #[cfg(target_os = "macos")]
        let macos_shortcuts = manager.is_none().then(macos_shortcut::spawn);

        #[cfg(target_os = "macos")]
        let shortcut_error = registration_error.clone();
        #[cfg(target_os = "macos")]
        let accessibility_pending =
            !crate::adapters::inbound::os::macos::selection::request_accessibility_permission();
        #[cfg(target_os = "macos")]
        let registration_error =
            macos_registration_error(registration_error.as_deref(), accessibility_pending);

        Self {
            #[cfg(target_os = "macos")]
            _service_provider: crate::adapters::inbound::os::macos::service::register(
                service_sender,
            ),
            #[cfg(target_os = "macos")]
            service_receiver,
            #[cfg(target_os = "macos")]
            macos_shortcuts,
            manager,
            hotkey,
            shortcut_label: "Ctrl+Space".to_owned(),
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
            deferred_clipboard_cleanup: Vec::new(),
            automatic_clipboard_fallback: true,
            #[cfg(target_os = "linux")]
            wayland_shortcuts: is_wayland_session().then(wayland_shortcut::spawn),
        }
    }

    pub fn poll(&mut self) -> Option<PlatformEvent> {
        use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

        #[cfg(target_os = "macos")]
        if self.accessibility_pending
            && crate::adapters::inbound::os::macos::selection::is_accessibility_trusted()
        {
            self.accessibility_pending = false;
            self.registration_error =
                macos_registration_error(self.shortcut_error.as_deref(), false);
        }

        #[cfg(target_os = "macos")]
        if let Some(message) = self
            .macos_shortcuts
            .as_ref()
            .and_then(|messages| messages.try_recv().ok())
        {
            match message {
                MacShortcutMessage::Registered => {
                    self.shortcut_error = None;
                    self.accessibility_pending = false;
                    self.registration_error = None;
                }
                MacShortcutMessage::Activated => {
                    if let Err(error) = self.start_selection_capture() {
                        return Some(PlatformEvent::Error(error));
                    }
                }
                MacShortcutMessage::Error(error) => {
                    self.shortcut_error = Some(error);
                    self.registration_error = macos_registration_error(
                        self.shortcut_error.as_deref(),
                        self.accessibility_pending,
                    );
                }
            }
        }

        #[cfg(target_os = "macos")]
        if let Ok(text) = self.service_receiver.try_recv() {
            return Some(PlatformEvent::Captured(CapturedSelection {
                text,
                clipboard_lease: None,
            }));
        }

        #[cfg(target_os = "linux")]
        if let Some(message) = self
            .wayland_shortcuts
            .as_ref()
            .and_then(|messages| messages.try_recv().ok())
        {
            match message {
                WaylandShortcutMessage::Registered(trigger) => {
                    self.usage_hint = format!("Select text, then press {trigger}");
                    self.shortcut_label = trigger;
                    self.registration_error = None;
                }
                WaylandShortcutMessage::Activated => {
                    if let Err(error) = self.start_selection_capture() {
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
                .as_ref()
                .is_some_and(|(active, _, _)| *active == generation)
            {
                self.capture_in_flight = None;
                return Some(match message {
                    Ok(selection) => PlatformEvent::Captured(selection),
                    Err(error) => self.capture_error(error),
                });
            }
            if let Ok(selection) = message
                && let Some(lease) = selection.clipboard_lease
                && clear_if_owned(&lease).is_err()
            {
                self.deferred_clipboard_cleanup.push(lease);
            }
        }

        if let Some((_, started, control)) = &self.capture_in_flight {
            let timed_out = if control.fallback_started() {
                started.elapsed() > FALLBACK_CAPTURE_TIMEOUT
            } else {
                started.elapsed() > NATIVE_CAPTURE_TIMEOUT && control.cancel_native()
            };
            if timed_out {
                self.capture_in_flight = None;
                return Some(PlatformEvent::Error(
                    "Selection capture timed out; try again".to_owned(),
                ));
            }
        }

        if self.manager.is_none() || self.capture_in_flight.is_some() {
            return None;
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() && event.state == HotKeyState::Released {
                if let Err(error) = self.start_selection_capture() {
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

    pub fn set_automatic_clipboard_fallback(&mut self, enabled: bool) {
        self.automatic_clipboard_fallback = enabled;
    }

    pub fn request_clipboard_text(&mut self) -> Result<(), String> {
        self.start_capture(CaptureRequest::ExistingClipboard)
    }

    pub fn clear_temporary_clipboard(&self, lease: &ClipboardLease) -> Result<bool, String> {
        clear_if_owned(lease).map_err(|error| format!("{error:#}"))
    }

    fn start_selection_capture(&mut self) -> Result<(), String> {
        let target = current_selection_target()?;
        self.start_capture(CaptureRequest::Selection {
            clipboard_fallback: self.automatic_clipboard_fallback,
            target,
        })
    }

    fn start_capture(&mut self, request: CaptureRequest) -> Result<(), String> {
        self.retry_deferred_clipboard_cleanup();
        if self.capture_in_flight.is_some() {
            return Err("Selection capture is already in progress".to_owned());
        }

        self.capture_generation = self.capture_generation.wrapping_add(1);
        let generation = self.capture_generation;
        let control = CaptureControl::new();
        self.capture_in_flight = Some((generation, std::time::Instant::now(), control.clone()));
        let sender = self.capture_sender.clone();
        std::thread::Builder::new()
            .name("selected-text-capture".to_owned())
            .spawn(move || {
                let message = capture(request, &control).map_err(|error| format!("{error:#}"));
                let _ = sender.send((generation, message));
            })
            .map_err(|error| {
                self.capture_in_flight = None;
                format!("Could not start selection capture: {error}")
            })?;
        Ok(())
    }

    fn retry_deferred_clipboard_cleanup(&mut self) {
        self.deferred_clipboard_cleanup
            .retain(|lease| clear_if_owned(lease).is_err());
    }

    #[cfg(target_os = "macos")]
    fn capture_error(&mut self, error: String) -> PlatformEvent {
        if !crate::adapters::inbound::os::macos::selection::is_accessibility_trusted() {
            self.accessibility_pending = true;
            PlatformEvent::AccessibilityPermissionRequired
        } else {
            PlatformEvent::Error(error)
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn capture_error(&mut self, error: String) -> PlatformEvent {
        PlatformEvent::Error(error)
    }
}

impl Drop for PlatformBridge {
    fn drop(&mut self) {
        for lease in self.deferred_clipboard_cleanup.drain(..) {
            let _ = clear_if_owned(&lease);
        }
    }
}

#[derive(Clone, Copy)]
enum CaptureRequest {
    Selection {
        clipboard_fallback: bool,
        target: SelectionTarget,
    },
    ExistingClipboard,
}

fn capture(request: CaptureRequest, control: &CaptureControl) -> anyhow::Result<CapturedSelection> {
    match request {
        CaptureRequest::Selection {
            clipboard_fallback,
            target,
        } => match capture_native_selected_text(target) {
            Ok(text) => Ok(CapturedSelection {
                text,
                clipboard_lease: None,
            }),
            Err(selection_error)
                if clipboard_fallback
                    && allows_clipboard_fallback(&selection_error)
                    && control.begin_fallback() =>
            {
                let capture = copy_selected_text().map_err(|clipboard_error| {
                    anyhow::anyhow!(
                        "Selection access failed: {selection_error:#}. Clipboard fallback failed: {clipboard_error:#}"
                    )
                })?;
                Ok(CapturedSelection {
                    text: capture.text,
                    clipboard_lease: Some(capture.lease),
                })
            }
            Err(error) => Err(error),
        },
        CaptureRequest::ExistingClipboard => Ok(CapturedSelection {
            text: read_current_text()?,
            clipboard_lease: None,
        }),
    }
}

#[cfg(target_os = "macos")]
fn current_selection_target() -> Result<SelectionTarget, String> {
    frontmost_application_pid()
        .map(|pid| SelectionTarget { pid })
        .map_err(|error| format!("Could not identify the selected app: {error:#}"))
}

#[cfg(not(target_os = "macos"))]
fn current_selection_target() -> Result<SelectionTarget, String> {
    Ok(SelectionTarget {})
}

#[cfg(target_os = "macos")]
fn capture_native_selected_text(target: SelectionTarget) -> anyhow::Result<SelectedText> {
    crate::adapters::inbound::os::macos::selection::capture_selected_text(target.pid)
}

#[cfg(target_os = "windows")]
fn capture_native_selected_text(_target: SelectionTarget) -> anyhow::Result<SelectedText> {
    crate::adapters::inbound::os::windows::selection::capture_selected_text()
}

#[cfg(target_os = "linux")]
fn capture_native_selected_text(_target: SelectionTarget) -> anyhow::Result<SelectedText> {
    crate::adapters::inbound::os::linux::selection::capture_selected_text()
}

fn allows_clipboard_fallback(error: &anyhow::Error) -> bool {
    !matches!(
        error.downcast_ref::<SelectionCaptureError>(),
        Some(
            SelectionCaptureError::PermissionRequired
                | SelectionCaptureError::ProtectedContent
                | SelectionCaptureError::ProtectionUnknown
                | SelectionCaptureError::FocusChanged
                | SelectionCaptureError::ShortcutConflict
                | SelectionCaptureError::IntegrityBoundary
                | SelectionCaptureError::PortalDenied
        )
    )
}

#[cfg(not(target_os = "macos"))]
fn register_hotkey(
    hotkey: global_hotkey::hotkey::HotKey,
) -> (Option<global_hotkey::GlobalHotKeyManager>, Option<String>) {
    match global_hotkey::GlobalHotKeyManager::new() {
        Ok(manager) => match manager.register(hotkey) {
            Ok(()) => (Some(manager), None),
            Err(error) => (
                None,
                Some(format!("Could not register Ctrl+Space: {error}")),
            ),
        },
        Err(error) => (None, Some(format!("Global shortcut unavailable: {error}"))),
    }
}

#[cfg(target_os = "windows")]
fn is_wayland() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    is_wayland_session()
}

#[cfg(target_os = "macos")]
fn macos_registration_error(
    shortcut_error: Option<&str>,
    accessibility_pending: bool,
) -> Option<String> {
    let permission_error = accessibility_pending
        .then_some("Allow Accessibility access to read selections and enable Control-Space");
    match (shortcut_error, permission_error) {
        (Some(shortcut), Some(permission)) => Some(format!("{shortcut}; {permission}")),
        (Some(shortcut), None) => Some(shortcut.to_owned()),
        (None, Some(permission)) => Some(permission.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SelectionCaptureError;

    #[test]
    fn shortcut_hint_explains_the_system_wide_activation() {
        assert_eq!(super::SHORTCUT_HINT, "Select text, then press Ctrl+Space");
    }

    #[test]
    fn clipboard_fallback_never_bypasses_permission_or_protected_content() {
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::PermissionRequired.into()
        ));
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::ProtectedContent.into()
        ));
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::ProtectionUnknown.into()
        ));
        assert!(super::allows_clipboard_fallback(
            &SelectionCaptureError::NoSelection.into()
        ));
        assert!(super::allows_clipboard_fallback(
            &SelectionCaptureError::ProviderUnsupported.into()
        ));
        assert!(super::allows_clipboard_fallback(
            &SelectionCaptureError::ProviderTimeout.into()
        ));
        assert!(super::allows_clipboard_fallback(
            &SelectionCaptureError::CompositorProtocolMissing.into()
        ));
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::ShortcutConflict.into()
        ));
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::IntegrityBoundary.into()
        ));
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::PortalDenied.into()
        ));
        assert!(!super::allows_clipboard_fallback(
            &SelectionCaptureError::FocusChanged.into()
        ));
    }

    #[test]
    fn timed_out_native_capture_cannot_start_a_late_copy() {
        let control = super::CaptureControl::new();

        assert!(control.cancel_native());
        assert!(!control.begin_fallback());
    }
}
