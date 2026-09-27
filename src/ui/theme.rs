use std::{fs, sync::Arc};

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Vec2};

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

    let mut fonts = FontDefinitions::default();
    let mut loaded = Vec::new();
    for (name, path) in candidates {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let name = name.to_owned();
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
        loaded.push(name);
    }
    if !loaded.is_empty() {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .splice(0..0, loaded);
        context.set_fonts(fonts);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{FontFamily, FontId, RawInput};

    use super::*;

    #[test]
    fn configured_interface_covers_every_supported_writing_system() {
        let context = egui::Context::default();
        configure_interface(&context);
        context.begin_pass(RawInput::default());

        let font = FontId::new(16.0, FontFamily::Proportional);
        for sample in [
            "Mist speaks clearly",
            "ミストが話します",
            "薄雾会说话",
            "मिस्ट बोलता है",
        ] {
            assert!(
                context.fonts_mut(|fonts| fonts.has_glyphs(&font, sample)),
                "configured UI fonts do not cover {sample:?}"
            );
        }

        let mut output = context.end_pass();
        output.textures_delta.clear();
    }
}
