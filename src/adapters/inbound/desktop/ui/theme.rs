//! Desktop visual tokens and font configuration.

use std::{fs, sync::Arc};

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Vec2};

const BUNDLED_DEVANAGARI: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/NotoSansDevanagariUI-Regular.ttf"
));

pub(super) const PANEL_BACKGROUND: Color32 = Color32::from_rgba_premultiplied(14, 16, 23, 248);
pub(super) const PANEL_SURFACE: Color32 = Color32::from_rgba_premultiplied(7, 7, 7, 7);
pub(super) const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 246, 250);
pub(super) const TEXT_SECONDARY: Color32 = Color32::from_rgb(169, 175, 193);
pub(super) const TEXT_MUTED: Color32 = Color32::from_rgb(119, 126, 146);

pub(super) fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub(super) fn palette_color(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

pub(super) fn configure_interface(context: &egui::Context) {
    install_system_font(context);
    context.set_theme(egui::Theme::Dark);
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = PANEL_BACKGROUND;
    style.visuals.override_text_color = Some(TEXT_PRIMARY);
    style.visuals.widgets.noninteractive.bg_fill = PANEL_SURFACE;
    style.visuals.widgets.inactive.bg_fill = Color32::from_white_alpha(9);
    style.visuals.widgets.hovered.bg_fill = Color32::from_white_alpha(18);
    style.visuals.widgets.active.bg_fill = Color32::from_white_alpha(24);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.animation_time = 0.16;
    context.set_style_of(egui::Theme::Dark, style);
}

fn install_system_font(context: &egui::Context) {
    let loaded = load_system_fonts();
    if loaded.is_empty() {
        return;
    }

    let mut fonts = FontDefinitions::default();
    let mut names = Vec::with_capacity(loaded.len());
    for (name, data) in loaded {
        fonts.font_data.insert(name.clone(), Arc::new(data));
        names.push(name);
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .splice(0..0, names);
    context.set_fonts(fonts);
}

fn load_system_fonts() -> Vec<(String, FontData)> {
    #[cfg(target_os = "macos")]
    let candidates = [
        ("system-ui", "/System/Library/Fonts/SFNS.ttf"),
        ("system-cjk", "/System/Library/Fonts/Hiragino Sans GB.ttc"),
        (
            "system-devanagari",
            "/System/Library/Fonts/Supplemental/Devanagari Sangam MN.ttc",
        ),
    ];
    #[cfg(target_os = "windows")]
    let candidates = [
        ("system-ui", "C:\\Windows\\Fonts\\segoeui.ttf"),
        ("system-japanese", "C:\\Windows\\Fonts\\meiryo.ttc"),
        ("system-chinese", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("system-devanagari", "C:\\Windows\\Fonts\\Nirmala.ttf"),
    ];
    #[cfg(target_os = "linux")]
    let candidates = [
        (
            "system-ui",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ),
        (
            "system-cjk",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ),
        (
            "system-devanagari",
            "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
        ),
        (
            "system-ui-fallback",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        ),
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let candidates: [(&str, &str); 0] = [];

    let mut loaded: Vec<_> = candidates
        .into_iter()
        .filter_map(|(name, path)| {
            fs::read(path)
                .ok()
                .map(|bytes| (name.to_owned(), FontData::from_owned(bytes)))
        })
        .collect();
    loaded.push((
        "bundled-devanagari".to_owned(),
        FontData::from_static(BUNDLED_DEVANAGARI),
    ));
    loaded
}

#[cfg(test)]
mod tests {
    use eframe::egui::RawInput;
    use skrifa::{FontRef, MetadataProvider};

    use super::*;

    fn font_covers(data: &FontData, character: char) -> bool {
        FontRef::from_index(data.font.as_ref(), data.index)
            .ok()
            .is_some_and(|font| font.charmap().map(character).is_some())
    }

    #[test]
    fn bundled_fallback_covers_the_hindi_sample() {
        let font = FontData::from_static(BUNDLED_DEVANAGARI);

        for character in "मिस्ट बोलता है"
            .chars()
            .filter(|character| !character.is_whitespace())
        {
            assert!(font_covers(&font, character));
        }
    }

    #[test]
    fn configured_interface_covers_every_supported_writing_system() {
        let context = egui::Context::default();
        configure_interface(&context);
        context.begin_pass(RawInput::default());

        let system_fonts = load_system_fonts();
        assert!(!system_fonts.is_empty(), "no supported system fonts found");
        for sample in [
            "Mist speaks clearly",
            "ミストが話します",
            "薄雾会说话",
            "मिस्ट बोलता है",
        ] {
            for character in sample
                .chars()
                .filter(|character| !character.is_whitespace())
            {
                let covered = system_fonts
                    .iter()
                    .any(|(_, data)| font_covers(data, character));
                assert!(covered, "configured UI fonts do not cover {character:?}");
            }
        }

        let mut output = context.end_pass();
        output.textures_delta.clear();
    }
}
