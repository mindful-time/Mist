use anyhow::{Context, Result};
use select_to_speak::{VOICE_CATALOG, VoiceSettings, worker::AppStatus};
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
    voices: Vec<(&'static str, CheckMenuItem)>,
    accessibility: Option<MenuItem>,
    quit: MenuItem,
}

impl TrayAdapter {
    pub(super) fn new(selected: &VoiceSettings) -> Result<Self> {
        let menu = Menu::new();
        let heading = MenuItem::new("Select to Speak · waking up", false, None);
        let settings = MenuItem::new("Voice settings…", true, None);
        let voices_menu = Submenu::new("Voice", true);
        let mut voices = Vec::with_capacity(VOICE_CATALOG.len());
        for voice in VOICE_CATALOG {
            let item = CheckMenuItem::new(
                format!("{} · {}", voice.display_name, voice.character),
                true,
                selected.voice_id.as_str() == voice.id,
                None,
            );
            voices_menu
                .append(&item)
                .context("could not add a voice to the tray menu")?;
            voices.push((voice.id, item));
        }
        #[cfg(target_os = "macos")]
        let accessibility = Some(MenuItem::new("Accessibility settings…", true, None));
        #[cfg(not(target_os = "macos"))]
        let accessibility: Option<MenuItem> = None;
        let quit = MenuItem::new("Quit Select to Speak", true, None);
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
        let mut builder = TrayIconBuilder::new()
            .with_tooltip("Select to Speak · local Kokoro voice")
            .with_menu(Box::new(menu))
            .with_icon(icon);
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(true);
        }
        #[cfg(target_os = "windows")]
        {
            builder = builder.with_menu_on_left_click(false);
        }
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
            .map(|(voice, _)| TrayAction::SelectVoice((*voice).to_owned()))
    }

    pub(super) fn select_voice(&self, selected: &str) {
        for (voice, item) in &self.voices {
            item.set_checked(*voice == selected);
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
        self.heading.set_text(format!("Select to Speak · {state}"));
        let _ = self
            .icon
            .set_tooltip(Some(format!("Select to Speak · {state}")));
    }

    pub(super) fn set_warning(&self, message: &str) {
        self.heading.set_text("Select to Speak · needs attention");
        let _ = self
            .icon
            .set_tooltip(Some(format!("Select to Speak · {message}")));
    }
}

fn mist_icon() -> Result<Icon> {
    let image = image::load_from_memory(include_bytes!("../../assets/mist.png"))
        .context("the embedded mist texture is invalid")?
        .into_rgba8();
    let mut pixels = image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Lanczos3);
    for pixel in pixels.pixels_mut() {
        let luminance =
            ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3) as u8;
        pixel[3] = ((u16::from(pixel[3]) * u16::from(luminance)) / 255) as u8;
        pixel[0] = 255;
        pixel[1] = 255;
        pixel[2] = 255;
    }
    Icon::from_rgba(pixels.into_raw(), 32, 32).context("could not create the tray icon")
}
