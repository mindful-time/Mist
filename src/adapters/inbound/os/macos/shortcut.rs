//! macOS global Ctrl+Space shortcut adapter.

use std::{
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicPtr, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use core_foundation::{
    base::TCFType,
    runloop::{CFRunLoop, kCFRunLoopCommonModes},
};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField, KeyCode,
};

const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub enum MacShortcutMessage {
    Registered,
    Activated,
    Error(String),
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
}

struct ShortcutState {
    sender: mpsc::Sender<MacShortcutMessage>,
    tap: AtomicPtr<c_void>,
    armed: AtomicBool,
    space_down: AtomicBool,
}

impl ShortcutState {
    fn new(sender: mpsc::Sender<MacShortcutMessage>) -> Self {
        Self {
            sender,
            tap: AtomicPtr::new(std::ptr::null_mut()),
            armed: AtomicBool::new(false),
            space_down: AtomicBool::new(false),
        }
    }

    fn finish_if_released(&self, flags: CGEventFlags) {
        if self.space_down.load(Ordering::Acquire)
            || flags.contains(CGEventFlags::CGEventFlagControl)
        {
            return;
        }
        let disallowed = CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand;
        if self.armed.swap(false, Ordering::AcqRel) && !flags.intersects(disallowed) {
            let _ = self.sender.send(MacShortcutMessage::Activated);
        }
    }

    fn reenable_tap(&self) {
        let tap = self.tap.load(Ordering::Acquire);
        if tap.is_null() {
            let _ = self.sender.send(MacShortcutMessage::Error(
                "Control-Space listener stopped unexpectedly".to_owned(),
            ));
            return;
        }
        // SAFETY: The event-tap object owns this mach-port pointer for the
        // entire run-loop lifetime. Core Graphics explicitly permits
        // re-enabling a tap from its disabled callback.
        unsafe { CGEventTapEnable(tap, true) };
    }
}

/// Installs an active session event tap for Control-Space. macOS commonly
/// reserves this chord for input-source switching, which makes Carbon hot-key
/// registration fail. The event tap is covered by the Accessibility permission
/// Mist already needs for semantic selection capture.
pub fn spawn() -> mpsc::Receiver<MacShortcutMessage> {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("macos-control-space".to_owned())
        .spawn(move || run(sender))
        .expect("the macOS shortcut listener thread should start");
    receiver
}

fn run(sender: mpsc::Sender<MacShortcutMessage>) {
    let mut reported_error = false;
    loop {
        let state = Arc::new(ShortcutState::new(sender.clone()));
        let callback_state = Arc::clone(&state);
        let event_tap = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            vec![
                CGEventType::KeyDown,
                CGEventType::KeyUp,
                CGEventType::FlagsChanged,
            ],
            move |_proxy, event_type, event| {
                if matches!(
                    event_type,
                    CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
                ) {
                    callback_state.reenable_tap();
                    return CallbackResult::Keep;
                }
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                let flags = event.get_flags();
                if matches!(event_type, CGEventType::KeyDown) && is_control_space(key_code, flags) {
                    let is_repeat =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;
                    if !is_repeat {
                        callback_state.armed.store(true, Ordering::Release);
                        callback_state.space_down.store(true, Ordering::Release);
                    }
                    return CallbackResult::Drop;
                }
                if matches!(event_type, CGEventType::KeyUp)
                    && key_code == i64::from(KeyCode::SPACE)
                    && callback_state.armed.load(Ordering::Acquire)
                {
                    callback_state.space_down.store(false, Ordering::Release);
                    callback_state.finish_if_released(flags);
                    return CallbackResult::Drop;
                }
                if matches!(event_type, CGEventType::FlagsChanged)
                    && callback_state.armed.load(Ordering::Acquire)
                {
                    callback_state.finish_if_released(flags);
                }
                CallbackResult::Keep
            },
        );
        match event_tap {
            Ok(event_tap) => {
                let tap = event_tap.mach_port().as_concrete_TypeRef().cast::<c_void>();
                state.tap.store(tap, Ordering::Release);
                let Ok(loop_source) = event_tap.mach_port().create_runloop_source(0) else {
                    let _ = sender.send(MacShortcutMessage::Error(
                        "Could not attach the Control-Space listener".to_owned(),
                    ));
                    thread::sleep(RETRY_DELAY);
                    continue;
                };
                CFRunLoop::get_current().add_source(&loop_source, unsafe { kCFRunLoopCommonModes });
                event_tap.enable();
                let _ = sender.send(MacShortcutMessage::Registered);
                CFRunLoop::run_current();
                state.tap.store(std::ptr::null_mut(), Ordering::Release);
                reported_error = false;
            }
            Err(()) if !reported_error => {
                let _ = sender.send(MacShortcutMessage::Error(
                    "Control-Space needs Accessibility access".to_owned(),
                ));
                reported_error = true;
            }
            Err(()) => {}
        }
        thread::sleep(RETRY_DELAY);
    }
}

fn is_control_space(key_code: i64, flags: CGEventFlags) -> bool {
    let disallowed = CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagAlternate
        | CGEventFlags::CGEventFlagCommand;
    key_code == i64::from(KeyCode::SPACE)
        && flags.contains(CGEventFlags::CGEventFlagControl)
        && !flags.intersects(disallowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_space_requires_the_exact_primary_modifiers() {
        assert!(is_control_space(
            i64::from(KeyCode::SPACE),
            CGEventFlags::CGEventFlagControl
        ));
        assert!(!is_control_space(
            i64::from(KeyCode::SPACE),
            CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagAlternate
        ));
        assert!(!is_control_space(
            i64::from(KeyCode::RETURN),
            CGEventFlags::CGEventFlagControl
        ));
    }

    #[test]
    fn activation_waits_until_space_and_control_are_both_released() {
        let (sender, receiver) = mpsc::channel();
        let state = ShortcutState::new(sender);
        state.armed.store(true, Ordering::Release);
        state.space_down.store(false, Ordering::Release);

        state.finish_if_released(CGEventFlags::CGEventFlagControl);
        assert!(receiver.try_recv().is_err());

        state.finish_if_released(CGEventFlags::empty());
        assert!(matches!(
            receiver.try_recv(),
            Ok(MacShortcutMessage::Activated)
        ));

        state.armed.store(true, Ordering::Release);
        state.space_down.store(true, Ordering::Release);
        state.finish_if_released(CGEventFlags::empty());
        assert!(receiver.try_recv().is_err());

        state.space_down.store(false, Ordering::Release);
        state.finish_if_released(CGEventFlags::empty());
        assert!(matches!(
            receiver.try_recv(),
            Ok(MacShortcutMessage::Activated)
        ));
    }
}
