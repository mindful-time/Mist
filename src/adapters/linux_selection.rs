use std::{io::Read, time::Duration};

use anyhow::Context;

use crate::domain::SelectedText;

pub fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
}

/// Reads the compositor's primary selection without modifying the regular
/// clipboard.
pub fn capture_selected_text() -> anyhow::Result<SelectedText> {
    if is_wayland_session() {
        return capture_wayland_selection();
    }

    capture_x11_selection()
}

fn capture_wayland_selection() -> anyhow::Result<SelectedText> {
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

    let (mut pipe, _) = get_contents(ClipboardType::Primary, Seat::Unspecified, MimeType::Text)
        .context("the Wayland compositor does not expose the primary text selection")?;
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)
        .context("could not read the Wayland text selection")?;
    let text = String::from_utf8(bytes).context("the selected text is not valid UTF-8")?;
    SelectedText::new(text.trim_matches('\0')).map_err(Into::into)
}

/// X11 publishes highlighted text through PRIMARY, independently of the
/// regular clipboard.
fn capture_x11_selection() -> anyhow::Result<SelectedText> {
    let clipboard =
        x11_clipboard::Clipboard::new().context("could not connect to the X11 selection")?;
    let utf8 = clipboard.load(
        clipboard.getter.atoms.primary,
        clipboard.getter.atoms.utf8_string,
        clipboard.getter.atoms.property,
        Duration::from_millis(500),
    );
    let text = match utf8 {
        Ok(bytes) => String::from_utf8(bytes).context("the selected text is not valid UTF-8")?,
        Err(_) => {
            let bytes = clipboard
                .load(
                    clipboard.getter.atoms.primary,
                    clipboard.getter.atoms.string,
                    clipboard.getter.atoms.property,
                    Duration::from_millis(500),
                )
                .context("the X11 primary selection does not contain text")?;
            bytes.into_iter().map(char::from).collect()
        }
    };
    SelectedText::new(text.trim_matches('\0')).map_err(Into::into)
}
