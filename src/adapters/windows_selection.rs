use anyhow::Context;
use uiautomation::{UIAutomation, patterns::UITextPattern};

use crate::domain::SelectedText;

/// Reads the focused control's selection without synthesizing Copy, so the
/// user's clipboard remains untouched.
pub fn capture_selected_text() -> anyhow::Result<SelectedText> {
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

pub fn capture_clipboard_text() -> anyhow::Result<SelectedText> {
    use anyhow::bail;

    let mut clipboard = arboard::Clipboard::new().context("could not open the clipboard")?;
    let text = clipboard
        .get_text()
        .context("the clipboard does not contain text")?;
    if text.trim().is_empty() {
        bail!("the clipboard does not contain text");
    }
    SelectedText::new(text).map_err(Into::into)
}
