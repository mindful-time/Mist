use std::{ffi::c_void, ptr};

use anyhow::{Context, bail};
use core_foundation::{
    base::{Boolean, CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};

use crate::domain::SelectedText;

type AXError = i32;
type AXUIElementRef = *const c_void;

const AX_ERROR_SUCCESS: AXError = 0;
const AX_ERROR_API_DISABLED: AXError = -25_211;
const AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25_205;
const AX_ERROR_NO_VALUE: AXError = -25_212;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;

    fn AXIsProcessTrusted() -> Boolean;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
}

/// Requests the one permission needed to inspect selection in applications
/// whose custom context menus do not expose macOS Services.
pub fn request_accessibility_permission() -> bool {
    // SAFETY: kAXTrustedCheckOptionPrompt is an immutable system-owned CFString.
    let prompt_key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let prompt_value = CFBoolean::true_value();
    let options = CFDictionary::from_CFType_pairs(&[(prompt_key, prompt_value)]);

    // SAFETY: `options` remains alive for the duration of the call.
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

pub fn is_accessibility_trusted() -> bool {
    // SAFETY: This function has no parameters and only reads this process's TCC state.
    unsafe { AXIsProcessTrusted() != 0 }
}

/// Reads the selected text from the currently focused macOS accessibility
/// element without touching the user's clipboard.
pub fn capture_selected_text() -> anyhow::Result<SelectedText> {
    if !is_accessibility_trusted() {
        bail!(
            "Allow Select to Speak in System Settings → Privacy & Security → Accessibility, then try again"
        );
    }

    // SAFETY: The create function returns an owned Core Foundation object.
    let system_wide =
        unsafe { CFType::wrap_under_create_rule(AXUIElementCreateSystemWide().cast::<c_void>()) };
    let focused = copy_attribute(&system_wide, "AXFocusedUIElement")
        .context("could not inspect the focused application")?;
    let selected = copy_attribute(&focused, "AXSelectedText")
        .context("the focused control does not expose selected text")?;
    let selected = selected
        .downcast_into::<CFString>()
        .context("the selected accessibility value is not text")?;

    SelectedText::new(selected.to_string()).map_err(Into::into)
}

fn copy_attribute(element: &CFType, attribute: &str) -> anyhow::Result<CFType> {
    let attribute = CFString::new(attribute);
    let mut value = ptr::null();
    // SAFETY: Both Core Foundation objects are alive for the call and `value`
    // is a valid out-pointer. A successful Copy call transfers ownership.
    let error = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef().cast::<c_void>(),
            attribute.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if error != AX_ERROR_SUCCESS {
        bail!(accessibility_error(error));
    }
    if value.is_null() {
        bail!("macOS returned an empty accessibility value");
    }

    // SAFETY: A successful Copy call returned a non-null owned CFTypeRef.
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

fn accessibility_error(error: AXError) -> String {
    match error {
        AX_ERROR_API_DISABLED => {
            "Accessibility permission is required to read selected text".to_owned()
        }
        AX_ERROR_ATTRIBUTE_UNSUPPORTED | AX_ERROR_NO_VALUE => {
            "This control does not expose selected text through Accessibility".to_owned()
        }
        _ => format!("macOS could not read the selection (Accessibility error {error})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_errors_are_actionable() {
        assert!(accessibility_error(AX_ERROR_API_DISABLED).contains("permission"));
        assert!(accessibility_error(AX_ERROR_NO_VALUE).contains("does not expose"));
    }
}
