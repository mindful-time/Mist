use std::sync::mpsc::SyncSender;

use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};

use crate::worker::WorkerCommand;

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
            _error: *mut *mut NSString,
        ) {
            // SAFETY: NSPasteboardTypeString is an immutable AppKit global.
            let pasteboard_type = unsafe { NSPasteboardTypeString };
            let Some(value) = pasteboard.stringForType(pasteboard_type) else {
                return;
            };
            let _ = self
                .ivars()
                .commands
                .try_send(WorkerCommand::Speak(value.to_string()));
        }
    }
);

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
