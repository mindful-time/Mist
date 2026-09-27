//! Windows UI Automation selection-capture adapter.

use anyhow::Context;
use uiautomation::{UIAutomation, patterns::UITextPattern};

use crate::domain::{SelectedText, SelectionCaptureError, SelectionError};

/// Reads the focused control's selection without synthesizing Copy, so the
/// user's clipboard remains untouched.
pub fn capture_selected_text() -> anyhow::Result<SelectedText> {
    let automation = UIAutomation::new().context("could not start Windows UI Automation")?;
    let focused = automation
        .get_focused_element()
        .map_err(|_| SelectionCaptureError::ProtectionUnknown)?;
    if focused
        .is_password()
        .map_err(|_| SelectionCaptureError::ProtectionUnknown)?
    {
        return Err(SelectionCaptureError::ProtectedContent.into());
    }
    let walker = automation.get_control_view_walker().map_err(|error| {
        anyhow::Error::new(SelectionCaptureError::ProviderUnsupported).context(format!(
            "could not inspect the focused UIA hierarchy: {error}"
        ))
    })?;
    let mut candidate = focused;
    for _ in 0..16 {
        if candidate
            .is_password()
            .map_err(|_| SelectionCaptureError::ProtectionUnknown)?
        {
            return Err(SelectionCaptureError::ProtectedContent.into());
        }
        if let Some(text) = selected_text_from(&candidate)? {
            return Ok(text);
        }
        let Ok(parent) = walker.get_parent(&candidate) else {
            break;
        };
        candidate = parent;
    }
    Err(SelectionCaptureError::NoSelection.into())
}

fn selected_text_from(element: &uiautomation::UIElement) -> anyhow::Result<Option<SelectedText>> {
    let Ok(pattern) = element.get_pattern::<UITextPattern>() else {
        return Ok(None);
    };
    let ranges = pattern.get_selection().map_err(|error| {
        anyhow::Error::new(SelectionCaptureError::ProviderUnsupported)
            .context(format!("could not read the selected text: {error}"))
    })?;
    let mut text = String::new();
    for range in ranges {
        let range = range
            .get_text(-1)
            .context("could not read a selected text range")?;
        if range.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&range);
    }
    if text.trim().is_empty() {
        return Ok(None);
    }
    SelectedText::new(text)
        .map(Some)
        .map_err(|error| match error {
            SelectionError::Empty => SelectionCaptureError::NoSelection.into(),
            error => anyhow::Error::new(error),
        })
}
