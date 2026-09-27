use std::sync::mpsc::{SyncSender, TrySendError};

use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};

use crate::{domain::SelectedText, worker::WorkerCommand};

#[derive(Debug)]
pub struct ServiceProviderIvars {
    commands: SyncSender<WorkerCommand>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this type is main-thread-only.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ServiceProviderIvars]
    pub struct ServiceProvider;

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for ServiceProvider {}

    impl ServiceProvider {
        // AppKit constructs this selector from NSMessage="speakSelection" in Info.plist.
        #[unsafe(method(speakSelection:userData:error:))]
        fn speak_selection(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            error: *mut *mut NSString,
        ) {
            // SAFETY: NSPasteboardTypeString is an immutable AppKit global.
            let pasteboard_type = unsafe { NSPasteboardTypeString };
            let Some(value) = pasteboard.stringForType(pasteboard_type) else {
                set_service_error(error, "The selected item is not text");
                return;
            };
            let text = match SelectedText::new(value.to_string()) {
                Ok(text) => text,
                Err(selection_error) => {
                    set_service_error(error, &selection_error.to_string());
                    return;
                }
            };
            if let Err(queue_error) = self.ivars().commands.try_send(WorkerCommand::Speak(text)) {
                let message = match queue_error {
                    TrySendError::Full(_) => "Mist is busy; try again in a moment",
                    TrySendError::Disconnected(_) => "The speech worker is not running",
                };
                set_service_error(error, message);
            }
        }
    }
);

fn set_service_error(error: *mut *mut NSString, message: &str) {
    if error.is_null() {
        return;
    }
    // SAFETY: AppKit supplies a valid nullable NSString out-parameter for this
    // callback and expects an autoreleased object.
    unsafe {
        *error = Retained::autorelease_ptr(NSString::from_str(message));
    }
}

impl ServiceProvider {
    fn new(mtm: MainThreadMarker, commands: SyncSender<WorkerCommand>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ServiceProviderIvars { commands });
        // SAFETY: NSObject's `init` signature is correct for this subclass.
        unsafe { msg_send![super(this), init] }
    }
}

/// Registers the object AppKit calls when the user chooses the text Service.
pub fn register(commands: SyncSender<WorkerCommand>) -> Retained<ServiceProvider> {
    let mtm = MainThreadMarker::new().expect("the pet UI must run on the macOS main thread");
    let provider = ServiceProvider::new(mtm, commands);
    let application = NSApplication::sharedApplication(mtm);
    // SAFETY: ServiceProvider implements the selector advertised in this app's Info.plist.
    unsafe { application.setServicesProvider(Some(&provider)) };
    provider
}
