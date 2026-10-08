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
pub(super) const CONTEXT_MENU_GAP: f32 = 12.0;

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

pub(super) fn context_menu_popup(response: &egui::Response) -> egui::Popup<'_> {
    egui::Popup::context_menu(response)
        .anchor(response.rect)
        .align(egui::RectAlign::LEFT)
        .align_alternatives(&[])
        .gap(CONTEXT_MENU_GAP)
        .style(|style: &mut egui::Style| {
            egui::containers::menu::menu_style(style);
            style.spacing.button_padding = Vec2::new(12.0, 8.0);
            style.spacing.interact_size.y = 32.0;
            style.spacing.item_spacing.y = 6.0;
            style.spacing.menu_margin = egui::Margin::same(10);
        })
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
    fn right_click_menu_has_roomy_rows_without_clipping() {
        let context = egui::Context::default();
        configure_interface(&context);
        let mode = super::super::ViewportMode::ContextMenu;
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, mode.size());
        let mist_rect =
            egui::Rect::from_center_size(mode.mist_center().to_pos2(), super::super::MIST_WINDOW);
        let input = RawInput {
            screen_rect: Some(viewport),
            ..Default::default()
        };
        let mut rows = Vec::new();
        let mut menu_rect = egui::Rect::NOTHING;

        // Let the popup complete its initial sizing pass before measuring.
        for _ in 0..2 {
            let mut output = context.run_ui(input.clone(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let anchor = ui.interact(mist_rect, ui.id().with("mist"), egui::Sense::click());
                    let popup = context_menu_popup(&anchor)
                        .open(true)
                        .show(|ui| {
                            ui.set_min_width(238.0);
                            ui.strong("Aoede mist");
                            ui.label(
                                egui::RichText::new("Select text, then press Ctrl+Space")
                                    .size(11.0),
                            );
                            ui.separator();
                            rows.clear();
                            for label in [
                                "Settings…",
                                "Speak copied text",
                                "Accessibility settings…",
                                "Quit Mist",
                            ] {
                                if label == "Quit Mist" {
                                    ui.separator();
                                }
                                rows.push(ui.button(label).rect);
                            }
                        })
                        .expect("the menu is open");
                    menu_rect = popup.response.rect;
                });
            });
            output.textures_delta.clear();
        }

        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert!(row.height() >= 32.0, "cramped menu row: {row:?}");
            assert!(row.left() - menu_rect.left() >= 10.0);
            assert!(menu_rect.right() - row.right() >= 10.0);
            assert!(menu_rect.contains_rect(*row), "menu clips a row");
        }
        for pair in rows.windows(2) {
            assert!(pair[1].top() - pair[0].bottom() >= 6.0);
        }
        assert!(
            viewport.contains_rect(menu_rect),
            "menu exceeds its viewport"
        );
    }

    #[test]
    fn right_click_menu_stays_to_the_left_of_the_mist() {
        for mist_size in [
            super::super::MIST_WINDOW,
            super::super::SPEAKING_MIST_WINDOW,
        ] {
            let context = egui::Context::default();
            configure_interface(&context);
            let mode = super::super::ViewportMode::ContextMenu;
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, mode.size());
            let mist_rect = egui::Rect::from_center_size(mode.mist_center().to_pos2(), mist_size);
            let input = RawInput {
                screen_rect: Some(viewport),
                events: vec![egui::Event::PointerMoved(mist_rect.center())],
                ..Default::default()
            };
            let mut menu_rect = egui::Rect::NOTHING;
            for _ in 0..2 {
                let mut output = context.run_ui(input.clone(), |ui| {
                    let response =
                        ui.interact(mist_rect, ui.id().with("mist"), egui::Sense::click());
                    let popup = context_menu_popup(&response)
                        .open_memory(Some(egui::SetOpenCommand::Bool(true)))
                        .show(|ui| {
                            ui.set_min_width(238.0);
                            let _ = ui.button("Settings…");
                        })
                        .expect("the right-click menu is open");
                    menu_rect = popup.response.rect;
                });
                output.textures_delta.clear();
            }

            assert!(
                menu_rect.right() + 12.0 <= mist_rect.left(),
                "menu covers the Mist instead of sitting to its left: {menu_rect:?}, {mist_rect:?}"
            );
            assert!(viewport.contains_rect(menu_rect), "menu is clipped");
            assert!(viewport.contains_rect(mist_rect), "Mist is clipped");
        }
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
