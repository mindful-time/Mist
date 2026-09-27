//! Windows UI Automation selection-capture adapter.

use anyhow::Context;
use uiautomation::{UIAutomation, UIElement, UITreeWalker, patterns::UITextPattern};

use crate::domain::{SelectedText, SelectionCaptureError, SelectionError};

/// Reads the focused control's selection without synthesizing Copy, so the
/// user's clipboard remains untouched.
pub fn capture_selected_text() -> anyhow::Result<SelectedText> {
    UIAutomation::new()
        .context("could not start Windows UI Automation")
        .and_then(capture_with_automation)
}

fn capture_with_automation(automation: UIAutomation) -> anyhow::Result<SelectedText> {
    focused_element(&automation).and_then(|focused| capture_for_focus(&automation, focused))
}

fn capture_for_focus(
    automation: &UIAutomation,
    focused: UIElement,
) -> anyhow::Result<SelectedText> {
    ensure_not_password(&focused)
        .and_then(|()| control_view_walker(automation))
        .and_then(|walker| selected_text_in_focus_chain(&walker, &focused, 16))
        .and_then(require_selection)
        .and_then(|text| verify_focus_and_return(automation, &focused, text))
}

fn require_selection(selection: Option<SelectedText>) -> anyhow::Result<SelectedText> {
    selection.ok_or_else(|| SelectionCaptureError::NoSelection.into())
}

fn focused_element(automation: &UIAutomation) -> anyhow::Result<UIElement> {
    automation
        .get_focused_element()
        .map_err(protection_unknown_error)
}

fn protection_unknown_error(error: uiautomation::Error) -> anyhow::Error {
    anyhow::Error::new(SelectionCaptureError::ProtectionUnknown)
        .context(format!("could not verify the focused UIA element: {error}"))
}

fn ensure_not_password(element: &UIElement) -> anyhow::Result<()> {
    element
        .is_password()
        .map_err(protection_unknown_error)
        .and_then(ensure_safe_control)
}

fn ensure_safe_control(is_password: bool) -> anyhow::Result<()> {
    (!is_password)
        .then_some(())
        .ok_or_else(|| SelectionCaptureError::ProtectedContent.into())
}

fn control_view_walker(automation: &UIAutomation) -> anyhow::Result<UITreeWalker> {
    automation.get_control_view_walker().map_err(|error| {
        anyhow::Error::new(SelectionCaptureError::ProviderUnsupported).context(format!(
            "could not inspect the focused UIA hierarchy: {error}"
        ))
    })
}

fn selected_text_in_focus_chain(
    walker: &UITreeWalker,
    candidate: &UIElement,
    remaining: usize,
) -> anyhow::Result<Option<SelectedText>> {
    if remaining == 0 {
        return Ok(None);
    }
    selected_text_candidate(candidate).and_then(|selection| {
        selection.map_or_else(
            || selected_text_from_parent(walker, candidate, remaining - 1),
            |text| Ok(Some(text)),
        )
    })
}

fn selected_text_candidate(element: &UIElement) -> anyhow::Result<Option<SelectedText>> {
    ensure_not_password(element).and_then(|()| selected_text_from(element))
}

fn selected_text_from_parent(
    walker: &UITreeWalker,
    candidate: &UIElement,
    remaining: usize,
) -> anyhow::Result<Option<SelectedText>> {
    walker
        .get_parent(candidate)
        .map(|parent| selected_text_in_focus_chain(walker, &parent, remaining))
        .unwrap_or_else(|_| Ok(None))
}

fn ensure_focus_unchanged(automation: &UIAutomation, original: &UIElement) -> anyhow::Result<()> {
    focused_element(automation)
        .and_then(|current| compare_focus(automation, original, &current))
        .and_then(ensure_same_focus)
}

fn ensure_same_focus(unchanged: bool) -> anyhow::Result<()> {
    unchanged
        .then_some(())
        .ok_or_else(|| SelectionCaptureError::FocusChanged.into())
}

fn verify_focus_and_return(
    automation: &UIAutomation,
    original: &UIElement,
    text: SelectedText,
) -> anyhow::Result<SelectedText> {
    ensure_focus_unchanged(automation, original).map(|()| text)
}

fn compare_focus(
    automation: &UIAutomation,
    original: &UIElement,
    current: &UIElement,
) -> anyhow::Result<bool> {
    automation
        .compare_elements(original, current)
        .map_err(protection_unknown_error)
}

fn selected_text_from(element: &UIElement) -> anyhow::Result<Option<SelectedText>> {
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
