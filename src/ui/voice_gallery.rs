use eframe::egui::{
    Align2, Button, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Vec2,
};
use select_to_speak::VOICE_CATALOG;

use super::{
    mist::MistRenderer,
    theme::{PANEL_SURFACE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, palette_color, with_alpha},
};

pub(super) struct GalleryResponse {
    pub(super) selected_voice: Option<&'static str>,
    pub(super) bottom: f32,
}

pub(super) fn show(
    ui: &mut eframe::egui::Ui,
    mist: &MistRenderer,
    outer: Rect,
    time: f32,
    selected_voice: &str,
) -> GalleryResponse {
    let painter = ui.painter().clone();
    let section_y = outer.top() + 171.0;
    painter.text(
        Pos2::new(outer.left() + 27.0, section_y),
        Align2::LEFT_TOP,
        "VOICE MISTS",
        FontId::proportional(10.5),
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(outer.right() - 27.0, section_y),
        Align2::RIGHT_TOP,
        "Kokoro · on-device",
        FontId::proportional(10.5),
        TEXT_MUTED,
    );

    let grid_left = outer.left() + 24.0;
    let grid_top = section_y + 24.0;
    let gap = 10.0;
    let card_width = (outer.width() - 48.0 - gap) * 0.5;
    let card_height = 78.0;
    let mut selected_voice_event = None;
    for (index, voice) in VOICE_CATALOG.iter().enumerate() {
        let column = index % 2;
        let row = index / 2;
        let card = Rect::from_min_size(
            Pos2::new(
                grid_left + column as f32 * (card_width + gap),
                grid_top + row as f32 * (card_height + gap),
            ),
            Vec2::new(card_width, card_height),
        );
        let selected = selected_voice == voice.id;
        let response = ui.put(
            card,
            Button::new(
                RichText::new(format!(
                    "Select {} voice, {}",
                    voice.display_name, voice.character
                ))
                .color(Color32::TRANSPARENT),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(17.0),
        );
        let hovered = response.hovered();
        let focused = response.has_focus();
        painter.rect_filled(
            card,
            17.0,
            if selected {
                with_alpha(palette_color(voice.palette.primary), 31)
            } else if hovered {
                Color32::from_white_alpha(14)
            } else {
                PANEL_SURFACE
            },
        );
        painter.rect_stroke(
            card,
            17.0,
            Stroke::new(
                if selected || focused { 1.5 } else { 1.0 },
                if selected {
                    with_alpha(palette_color(voice.palette.primary), 150)
                } else if focused {
                    Color32::from_white_alpha(135)
                } else {
                    Color32::from_white_alpha(18)
                },
            ),
            StrokeKind::Inside,
        );
        let swatch = Rect::from_center_size(
            Pos2::new(card.left() + 42.0, card.center().y),
            Vec2::splat(66.0),
        );
        mist.paint_swatch(ui, swatch, time, voice.palette, index as f32 * 0.83);
        painter.text(
            Pos2::new(card.left() + 78.0, card.top() + 20.0),
            Align2::LEFT_TOP,
            voice.display_name,
            FontId::proportional(16.0),
            TEXT_PRIMARY,
        );
        painter.text(
            Pos2::new(card.left() + 78.0, card.top() + 45.0),
            Align2::LEFT_TOP,
            voice.character,
            FontId::proportional(11.0),
            TEXT_SECONDARY,
        );
        if selected {
            painter.circle_filled(
                Pos2::new(card.right() - 18.0, card.top() + 18.0),
                3.5,
                palette_color(voice.palette.primary),
            );
        }
        if response.clicked() {
            selected_voice_event = Some(voice.id);
        }
    }

    GalleryResponse {
        selected_voice: selected_voice_event,
        bottom: grid_top + 4.0 * (card_height + gap) + 2.0,
    }
}
