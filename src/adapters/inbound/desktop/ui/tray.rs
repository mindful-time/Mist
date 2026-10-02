//! Native tray/menu-bar primary adapter.

use crate::{VoiceProfile, VoiceSettings, worker::AppStatus};
use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};

pub(super) enum TrayAction {
    OpenSettings,
    SelectVoice(String),
    OpenAccessibility,
    Quit,
}

pub(super) struct TrayAdapter {
    icon: TrayIcon,
    heading: MenuItem,
    settings: MenuItem,
    voices: Vec<(String, CheckMenuItem)>,
    accessibility: Option<MenuItem>,
    quit: MenuItem,
}

impl TrayAdapter {
    pub(super) fn new(selected: &VoiceSettings, catalog: &[VoiceProfile]) -> Result<Self> {
        let menu = Menu::new();
        let heading = MenuItem::new("Mist · waking up", false, None);
        let settings = MenuItem::new("Settings…", true, None);
        let voices_menu = Submenu::new("Voice", true);
        let mut voices = Vec::with_capacity(catalog.len());
        for voice in catalog {
            let item = CheckMenuItem::new(
                format!("{} · {}", voice.display_name, voice.character),
                true,
                voice.id == selected.voice_id.as_str(),
                None,
            );
            voices_menu
                .append(&item)
                .context("could not add a voice to the tray menu")?;
            voices.push((voice.id.to_owned(), item));
        }
        #[cfg(target_os = "macos")]
        let accessibility = Some(MenuItem::new("Accessibility settings…", true, None));
        #[cfg(not(target_os = "macos"))]
        let accessibility: Option<MenuItem> = None;
        let quit = MenuItem::new("Quit Mist", true, None);
        menu.append_items(&[
            &heading,
            &PredefinedMenuItem::separator(),
            &settings,
            &voices_menu,
        ])
        .context("could not assemble the tray menu")?;
        if let Some(accessibility) = &accessibility {
            menu.append_items(&[&PredefinedMenuItem::separator(), accessibility])
                .context("could not add the accessibility menu item")?;
        }
        menu.append_items(&[&PredefinedMenuItem::separator(), &quit])
            .context("could not complete the tray menu")?;

        let icon = mist_icon()?;
        let builder = TrayIconBuilder::new()
            .with_tooltip("Mist · local speech")
            .with_menu(Box::new(menu))
            .with_icon(icon);
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_as_template(true);
        #[cfg(target_os = "windows")]
        let builder = builder.with_menu_on_left_click(false);
        let icon = builder
            .build()
            .context("could not create the system tray icon")?;

        Ok(Self {
            icon,
            heading,
            settings,
            voices,
            accessibility,
            quit,
        })
    }

    pub(super) fn poll(&self) -> Option<TrayAction> {
        let event = MenuEvent::receiver().try_recv().ok()?;
        if event.id == *self.settings.id() {
            return Some(TrayAction::OpenSettings);
        }
        if self
            .accessibility
            .as_ref()
            .is_some_and(|item| event.id == *item.id())
        {
            return Some(TrayAction::OpenAccessibility);
        }
        if event.id == *self.quit.id() {
            return Some(TrayAction::Quit);
        }
        self.voices
            .iter()
            .find(|(_, item)| event.id == *item.id())
            .map(|(voice, _)| TrayAction::SelectVoice(voice.clone()))
    }

    pub(super) fn select_voice(&self, selected: &str) {
        // Native check items toggle themselves before the menu event arrives.
        // Clear every item first, then apply the single authoritative choice.
        for (_, item) in &self.voices {
            item.set_checked(false);
        }
        if let Some((_, item)) = self.voices.iter().find(|(voice, _)| voice == selected) {
            item.set_checked(true);
        }
    }

    pub(super) fn set_status(&self, status: &AppStatus) {
        let state = match status {
            AppStatus::CheckingModel => "waking up",
            AppStatus::MissingModel => "setup needed",
            AppStatus::Ready => "ready",
            AppStatus::Downloading => "downloading voices",
            AppStatus::Loading => "warming up",
            AppStatus::Synthesizing { .. } => "forming speech",
            AppStatus::Speaking { .. } => "speaking",
            AppStatus::Error(_) => "needs attention",
        };
        self.heading.set_text(format!("Mist · {state}"));
        let _ = self.icon.set_tooltip(Some(format!("Mist · {state}")));
    }

    pub(super) fn set_warning(&self, message: &str) {
        self.heading.set_text("Mist · needs attention");
        let _ = self.icon.set_tooltip(Some(format!("Mist · {message}")));
    }
}

fn mist_icon() -> Result<Icon> {
    let pixels = mist_icon_pixels()?;
    Icon::from_rgba(pixels.into_raw(), 32, 32).context("could not create the tray icon")
}

fn mist_icon_pixels() -> Result<image::RgbaImage> {
    let image =
        image::load_from_memory(include_bytes!("../../../../../assets/mist-orb-512-v1.png"))
            .context("the embedded app icon is invalid")?
            .into_rgba8();
    let pixels = image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Lanczos3);
    #[cfg(target_os = "macos")]
    let pixels = {
        // AppKit recolors template icons for light/dark menu bars; keep the fog's density.
        let mut pixels = pixels;
        for pixel in pixels.pixels_mut() {
            let luminance =
                ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3) as u8;
            pixel[3] = ((u16::from(pixel[3]) * u16::from(luminance)) / 255) as u8;
            pixel[0] = 255;
            pixel[1] = 255;
            pixel[2] = 255;
        }
        pixels
    };
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::mist_icon_pixels;
    use crate::{
        adapters::outbound::speech::kokoro::catalog::KokoroVoiceCatalog, ports::VoiceCatalog,
    };

    #[test]
    fn voice_catalog_resolves_exactly_one_native_check_item() {
        let checked = KokoroVoiceCatalog
            .voices()
            .iter()
            .filter(|voice| voice.id == "af_bella")
            .count();

        assert_eq!(checked, 1);
    }

    #[test]
    fn circular_tray_icon_stays_visible_at_small_size_without_a_backing_square() {
        let pixels = mist_icon_pixels().expect("the shipped icon must decode");

        assert_eq!(pixels.dimensions(), (32, 32));
        for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(pixels.get_pixel(x, y)[3], 0);
        }
        assert!(pixels.get_pixel(16, 16)[3] > 40);
        assert!(pixels.pixels().filter(|pixel| pixel[3] > 40).count() > 250);
        #[cfg(target_os = "macos")]
        assert!(pixels.pixels().all(|pixel| pixel.0[..3] == [255, 255, 255]));
    }
}
