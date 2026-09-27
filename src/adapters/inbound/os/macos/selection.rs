//! macOS Accessibility selection-capture adapter.

use std::{error::Error, fmt};
use std::{ffi::c_void, ptr};

use anyhow::Context;
use core_foundation::{
    array::CFArray,
    base::{Boolean, CFRange, CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use objc2_app_kit::NSWorkspace;

use crate::domain::{SelectedText, SelectionCaptureError};

type AXError = i32;
type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;

const AX_ERROR_SUCCESS: AXError = 0;
const AX_ERROR_CANNOT_COMPLETE: AXError = -25_204;
const AX_ERROR_API_DISABLED: AXError = -25_211;
const AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25_205;
const AX_ERROR_NO_VALUE: AXError = -25_212;
const AX_VALUE_CF_RANGE_TYPE: u32 = 4;
const AX_MESSAGING_TIMEOUT_SECONDS: f32 = 1.0;

#[derive(Debug)]
struct AccessibilityAttributeError(AXError);

impl fmt::Display for AccessibilityAttributeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&accessibility_error(self.0))
    }
}

impl Error for AccessibilityAttributeError {}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;

    fn AXIsProcessTrusted() -> Boolean;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_in_seconds: f32) -> AXError;
    fn AXValueGetType(value: AXValueRef) -> u32;
    fn AXValueGetValue(value: AXValueRef, value_type: u32, value_ptr: *mut c_void) -> Boolean;
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
pub fn capture_selected_text(frontmost_pid: i32) -> anyhow::Result<SelectedText> {
    if !is_accessibility_trusted() {
        return Err(SelectionCaptureError::PermissionRequired.into());
    }

    // SAFETY: The create function returns an owned Core Foundation object.
    let system_wide =
        unsafe { CFType::wrap_under_create_rule(AXUIElementCreateSystemWide().cast::<c_void>()) };
    set_messaging_timeout(&system_wide);
    match optional_focused_element(&system_wide) {
        Ok(Some(focused)) => {
            ensure_not_protected(&focused)?;
            if let Some(selection) = selected_text_from_focus_chain(&focused)? {
                return Ok(selection);
            }
        }
        Ok(None) => {}
        // Electron apps can omit the system-wide focused element until their
        // app-specific accessibility tree is activated. Retry through that
        // stronger native handle before considering clipboard fallback.
        Err(error) if should_retry_accessibility_route(&error) => {}
        Err(error) => return Err(error),
    }
    let application = application_element(frontmost_pid);
    set_messaging_timeout(&application);
    match selected_text_from_application(&application) {
        Ok(Some(selection)) => return Ok(selection),
        Ok(None) => {}
        Err(error) if should_retry_accessibility_route(&error) => {}
        Err(error) => return Err(error),
    }
    if enable_manual_accessibility(&application) {
        std::thread::sleep(std::time::Duration::from_millis(80));
        if let Some(selection) = selected_text_from_application(&application)? {
            return Ok(selection);
        }
    }
    Err(SelectionCaptureError::NoSelection.into())
}

/// Captures the frontmost process identity before selection work moves to its
/// background thread. `NSWorkspace` reaches AppKit/HIToolbox state that macOS
/// requires callers to access from the main UI thread.
pub fn frontmost_application_pid() -> anyhow::Result<i32> {
    let application = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .context("macOS did not report a frontmost application")?;
    Ok(application.processIdentifier())
}

fn application_element(pid: i32) -> CFType {
    // SAFETY: The running application supplies a valid process identifier and
    // the create function returns an owned accessibility element.
    unsafe { CFType::wrap_under_create_rule(AXUIElementCreateApplication(pid).cast::<c_void>()) }
}

fn enable_manual_accessibility(application: &CFType) -> bool {
    let attribute = CFString::new("AXManualAccessibility");
    let enabled = CFBoolean::true_value();
    // SAFETY: All Core Foundation objects remain alive for the call. Unsupported
    // applications return an AX error and continue through the normal path.
    unsafe {
        AXUIElementSetAttributeValue(
            application.as_CFTypeRef().cast::<c_void>(),
            attribute.as_concrete_TypeRef(),
            enabled.as_CFTypeRef(),
        ) == AX_ERROR_SUCCESS
    }
}

fn set_messaging_timeout(element: &CFType) {
    // SAFETY: The element is a retained AXUIElement and the timeout is a finite,
    // positive duration accepted by the Accessibility API.
    let _ = unsafe {
        AXUIElementSetMessagingTimeout(
            element.as_CFTypeRef().cast::<c_void>(),
            AX_MESSAGING_TIMEOUT_SECONDS,
        )
    };
}

fn selected_text_from_application(application: &CFType) -> anyhow::Result<Option<SelectedText>> {
    let focused = focused_element(application)?;
    ensure_not_protected(&focused)?;
    if let Some(selection) = selected_text_from_focus_chain(&focused)? {
        return Ok(Some(selection));
    }
    if let Ok(window) = copy_attribute(application, "AXFocusedWindow")
        && let Some(selection) = selected_text_from_tree(&window)?
    {
        return Ok(Some(selection));
    }
    Ok(None)
}

fn selected_text(element: &CFType) -> anyhow::Result<Option<SelectedText>> {
    if let Some(selected) = selected_text_attribute(element) {
        return Ok(Some(selected));
    }
    if let Some(selected) = selected_text_from_ranges(element) {
        return Ok(Some(selected));
    }
    Ok(selected_text_from_range(element))
}

fn selected_text_attribute(element: &CFType) -> Option<SelectedText> {
    string_attribute(element, "AXSelectedText")
        .and_then(|value| SelectedText::new(value.to_string()).ok())
}

fn selected_text_from_ranges(element: &CFType) -> Option<SelectedText> {
    let values = array_attribute(element, "AXSelectedTextRanges")?;
    let selections = values
        .get_all_values()
        .into_iter()
        .filter_map(|value| selected_text_for_range_value(element, value))
        .collect();
    selected_text_from_fragments(selections)
}

fn selected_text_for_range_value(element: &CFType, value: CFTypeRef) -> Option<String> {
    // SAFETY: `AXSelectedTextRanges` contains borrowed AXValue objects. The
    // get rule retains the value for this wrapper's lifetime.
    let value = unsafe { CFType::wrap_under_get_rule(value) };
    accessibility_range(&value)
        .and_then(|range| text_for_accessibility_range(element, &value, range))
}

fn selected_text_from_range(element: &CFType) -> Option<SelectedText> {
    let range_value = copy_attribute(element, "AXSelectedTextRange").ok()?;
    accessibility_range(&range_value)
        .and_then(|range| text_for_accessibility_range(element, &range_value, range))
        .and_then(|selection| SelectedText::new(selection).ok())
}

fn accessibility_range(range_value: &CFType) -> Option<CFRange> {
    // SAFETY: AXSelectedTextRange values are AXValue objects. The type check
    // precedes copying the embedded Core Foundation range into valid storage.
    let is_range = unsafe { AXValueGetType(range_value.as_CFTypeRef().cast::<c_void>()) }
        == AX_VALUE_CF_RANGE_TYPE;
    is_range.then_some(())?;
    let mut range = CFRange {
        location: 0,
        length: 0,
    };
    // SAFETY: The type check above established a CFRange payload and `range`
    // is valid writable storage for the duration of the call.
    let copied = unsafe {
        AXValueGetValue(
            range_value.as_CFTypeRef().cast::<c_void>(),
            AX_VALUE_CF_RANGE_TYPE,
            (&raw mut range).cast::<c_void>(),
        ) != 0
    };
    copied.then_some(range)
}

fn text_for_accessibility_range(
    element: &CFType,
    range_value: &CFType,
    range: CFRange,
) -> Option<String> {
    parameterized_string_attribute(element, "AXStringForRange", range_value)
        .map(|value| value.to_string())
        .filter(|value| is_nonblank(value))
        .or_else(|| {
            string_attribute(element, "AXValue")
                .and_then(|value| utf16_range(&value.to_string(), range))
                .filter(|value| is_nonblank(value))
        })
}

fn string_attribute(element: &CFType, attribute: &str) -> Option<CFString> {
    copy_attribute(element, attribute)
        .ok()
        .and_then(|value| value.downcast_into::<CFString>())
}

fn array_attribute(element: &CFType, attribute: &str) -> Option<CFArray> {
    copy_attribute(element, attribute)
        .ok()
        .and_then(|value| value.downcast_into::<CFArray>())
}

fn parameterized_string_attribute(
    element: &CFType,
    attribute: &str,
    parameter: &CFType,
) -> Option<CFString> {
    copy_parameterized_attribute(element, attribute, parameter)
        .ok()
        .and_then(|value| value.downcast_into::<CFString>())
}

fn is_nonblank(value: &str) -> bool {
    !value.trim().is_empty()
}

fn utf16_range(value: &str, range: CFRange) -> Option<String> {
    let start = usize::try_from(range.location).ok()?;
    let length = usize::try_from(range.length).ok()?;
    let end = start.checked_add(length)?;
    let utf16: Vec<u16> = value.encode_utf16().collect();
    String::from_utf16(utf16.get(start..end)?).ok()
}

#[cfg(test)]
fn utf16_ranges(value: &str, ranges: &[CFRange]) -> Option<String> {
    let selections = ranges
        .iter()
        .filter_map(|range| utf16_range(value, *range))
        .filter(|selection| !selection.trim().is_empty())
        .collect();
    join_fragments(selections)
}

fn selected_text_from_fragments(selections: Vec<String>) -> Option<SelectedText> {
    join_fragments(selections).and_then(|selection| SelectedText::new(selection).ok())
}

fn join_fragments(selections: Vec<String>) -> Option<String> {
    (!selections.is_empty()).then(|| selections.join("\n"))
}

fn selected_text_from_tree(root: &CFType) -> anyhow::Result<Option<SelectedText>> {
    ensure_not_protected(root)?;
    if let Some(selection) = selected_text(root)? {
        return Ok(Some(selection));
    }
    selected_text_in_descendants(root)
}

fn selected_text_from_focus_chain(root: &CFType) -> anyhow::Result<Option<SelectedText>> {
    const MAX_ANCESTORS: usize = 24;
    let mut current = root.clone();
    for _ in 0..MAX_ANCESTORS {
        ensure_not_protected(&current)?;
        if let Some(selection) = selected_text(&current)? {
            return Ok(Some(selection));
        }
        let Ok(parent) = copy_attribute(&current, "AXParent") else {
            break;
        };
        current = parent;
    }
    Ok(None)
}

fn focused_element(element: &CFType) -> anyhow::Result<CFType> {
    optional_focused_element(element)?
        .ok_or_else(|| anyhow::Error::from(SelectionCaptureError::ProtectionUnknown))
}

fn optional_focused_element(element: &CFType) -> anyhow::Result<Option<CFType>> {
    match copy_attribute(element, "AXFocusedUIElement") {
        Ok(focused) => Ok(Some(focused)),
        Err(error) if attribute_unavailable(&error) => Ok(None),
        Err(error)
            if matches!(
                error.downcast_ref::<SelectionCaptureError>(),
                Some(
                    SelectionCaptureError::PermissionRequired
                        | SelectionCaptureError::ProviderTimeout
                )
            ) =>
        {
            Err(error)
        }
        Err(_) => Err(SelectionCaptureError::ProtectionUnknown.into()),
    }
}

fn ensure_not_protected(element: &CFType) -> anyhow::Result<()> {
    match copy_attribute(element, "AXSubrole") {
        Ok(value) => {
            let Some(subrole) = value.downcast_into::<CFString>() else {
                return Err(SelectionCaptureError::ProtectionUnknown.into());
            };
            if subrole == "AXSecureTextField" {
                return Err(SelectionCaptureError::ProtectedContent.into());
            }
            Ok(())
        }
        Err(error) if attribute_unavailable(&error) => {
            let role = copy_attribute(element, "AXRole")
                .map_err(|_| SelectionCaptureError::ProtectionUnknown)?;
            let Some(role) = role.downcast_into::<CFString>() else {
                return Err(SelectionCaptureError::ProtectionUnknown.into());
            };
            if role == "AXTextField" {
                return Err(SelectionCaptureError::ProtectionUnknown.into());
            }
            Ok(())
        }
        Err(_) => Err(SelectionCaptureError::ProtectionUnknown.into()),
    }
}

fn selected_text_in_descendants(root: &CFType) -> anyhow::Result<Option<SelectedText>> {
    const MAX_NODES: usize = 512;
    const MAX_DEPTH: usize = 16;

    let mut pending = std::collections::VecDeque::from([(root.clone(), 0)]);
    let mut visited = 0;
    while let Some((element, depth)) = pending.pop_front() {
        if visited >= MAX_NODES {
            break;
        }
        visited += 1;

        ensure_not_protected(&element)?;

        if depth > 0
            && let Some(selection) = selected_text(&element)?
        {
            return Ok(Some(selection));
        }

        if depth >= MAX_DEPTH {
            continue;
        }
        let Some(children) = copy_attribute(&element, "AXChildren")
            .ok()
            .and_then(|value| value.downcast_into::<CFArray>())
        else {
            continue;
        };
        for child in children.get_all_values() {
            // SAFETY: AXChildren contains borrowed accessibility elements. The
            // get rule retains each child for the queued CFType wrapper.
            pending.push_back((unsafe { CFType::wrap_under_get_rule(child) }, depth + 1));
        }
    }
    Ok(None)
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
    ensure_copy_succeeded(error)?;
    owned_copy_value(value, "macOS returned an empty accessibility value")
}

fn copy_parameterized_attribute(
    element: &CFType,
    attribute: &str,
    parameter: &CFType,
) -> anyhow::Result<CFType> {
    let attribute = CFString::new(attribute);
    let mut value = ptr::null();
    // SAFETY: The element, attribute, and parameter are valid Core Foundation
    // objects for the duration of the call. A successful Copy call transfers
    // ownership of a non-null result to the returned wrapper.
    let error = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element.as_CFTypeRef().cast::<c_void>(),
            attribute.as_concrete_TypeRef(),
            parameter.as_CFTypeRef(),
            &mut value,
        )
    };
    ensure_copy_succeeded(error)?;
    owned_copy_value(
        value,
        "macOS returned an empty parameterized accessibility value",
    )
}

fn ensure_copy_succeeded(error: AXError) -> anyhow::Result<()> {
    (error == AX_ERROR_SUCCESS)
        .then_some(())
        .ok_or_else(|| accessibility_attribute_error(error))
}

fn owned_copy_value(value: CFTypeRef, empty_message: &str) -> anyhow::Result<CFType> {
    (!value.is_null())
        // SAFETY: A successful Copy call returned a non-null owned CFTypeRef.
        .then(|| unsafe { CFType::wrap_under_create_rule(value) })
        .ok_or_else(|| anyhow::anyhow!(empty_message.to_owned()))
}

fn attribute_error_code(error: &anyhow::Error) -> Option<AXError> {
    error
        .downcast_ref::<AccessibilityAttributeError>()
        .map(|error| error.0)
}

fn accessibility_attribute_error(error: AXError) -> anyhow::Error {
    typed_accessibility_error(error)
        .map(anyhow::Error::new)
        .unwrap_or_else(|| AccessibilityAttributeError(error).into())
}

fn typed_accessibility_error(error: AXError) -> Option<SelectionCaptureError> {
    match error {
        AX_ERROR_API_DISABLED => Some(SelectionCaptureError::PermissionRequired),
        AX_ERROR_CANNOT_COMPLETE => Some(SelectionCaptureError::ProviderTimeout),
        _ => None,
    }
}

fn attribute_unavailable(error: &anyhow::Error) -> bool {
    matches!(
        attribute_error_code(error),
        Some(AX_ERROR_ATTRIBUTE_UNSUPPORTED | AX_ERROR_NO_VALUE)
    )
}

fn should_retry_accessibility_route(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<SelectionCaptureError>(),
        Some(SelectionCaptureError::ProtectionUnknown)
    )
}

fn accessibility_error(error: AXError) -> String {
    match error {
        AX_ERROR_API_DISABLED => {
            "Accessibility permission is required to read selected text".to_owned()
        }
        AX_ERROR_CANNOT_COMPLETE => {
            "The focused application did not answer before the selection timeout".to_owned()
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
        assert_eq!(
            typed_accessibility_error(AX_ERROR_CANNOT_COMPLETE),
            Some(SelectionCaptureError::ProviderTimeout)
        );
    }

    #[test]
    fn accessibility_ranges_use_utf16_offsets() {
        let selection = utf16_range(
            "A mist 🌫 speaks",
            CFRange {
                location: 7,
                length: 2,
            },
        );

        assert_eq!(selection.as_deref(), Some("🌫"));
    }

    #[test]
    fn accessibility_multiple_ranges_preserve_order_and_utf16_offsets() {
        let selection = utf16_ranges(
            "First 🌫 second mist",
            &[
                CFRange {
                    location: 6,
                    length: 2,
                },
                CFRange {
                    location: 16,
                    length: 4,
                },
            ],
        );

        assert_eq!(selection.as_deref(), Some("🌫\nmist"));
    }

    #[test]
    fn uncertain_system_focus_retries_through_the_frontmost_application() {
        assert!(should_retry_accessibility_route(
            &SelectionCaptureError::ProtectionUnknown.into()
        ));
        assert!(!should_retry_accessibility_route(
            &SelectionCaptureError::PermissionRequired.into()
        ));
        assert!(!should_retry_accessibility_route(
            &SelectionCaptureError::ProtectedContent.into()
        ));
    }
}
