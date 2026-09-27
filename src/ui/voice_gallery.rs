use ::mist::VOICE_CATALOG;
use eframe::egui::{
    Align2, Button, Color32, Direction, FontId, Layout, Pos2, Rect, RichText, Stroke, StrokeKind,
    UiBuilder, Vec2,
};

use super::{
    mist::MistRenderer,
    theme::{PANEL_SURFACE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, palette_color, with_alpha},
};

pub(super) struct GalleryResponse {
    pub(super) preview_voice: Option<&'static str>,
    pub(super) bottom: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GalleryMode {
    Checking,
    DownloadRequired,
    Available,
    Busy,
}

impl GalleryMode {
    fn interaction_enabled(self) -> bool {
        self == Self::Available
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Checking => "Checking voices…",
            Self::DownloadRequired => "Download to preview",
            Self::Available => "Click to preview · on-device",
            Self::Busy => "Preview playing…",
        }
    }
}

pub(super) fn show(
    ui: &mut eframe::egui::Ui,
    mist: &MistRenderer,
    outer: Rect,
    time: f32,
    selected_voice: &str,
    mode: GalleryMode,
) -> GalleryResponse {
    let interaction_enabled = mode.interaction_enabled();
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
        mode.hint(),
        FontId::proportional(10.5),
        TEXT_MUTED,
    );

    let grid_left = outer.left() + 24.0;
    let grid_top = section_y + 24.0;
    let gap = 10.0;
    let card_width = (outer.width() - 48.0 - gap) * 0.5;
    let card_height = 78.0;
    let mut preview_voice_event = None;
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
        let response = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(card)
                    .layout(Layout::centered_and_justified(Direction::TopDown)),
                |ui| {
                    ui.add_enabled(
                        interaction_enabled,
                        Button::new(
                            RichText::new(format!(
                                "Preview and select {} voice, {}",
                                voice.display_name, voice.character
                            ))
                            .color(Color32::TRANSPARENT),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .corner_radius(17.0),
                    )
                },
            )
            .inner;
        let hovered = interaction_enabled && response.hovered();
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
        let play_center = Pos2::new(card.right() - 20.0, card.center().y);
        painter.circle_filled(
            play_center,
            10.0,
            if selected {
                with_alpha(palette_color(voice.palette.primary), 185)
            } else {
                Color32::from_white_alpha(if hovered { 28 } else { 14 })
            },
        );
        painter.add(eframe::egui::Shape::convex_polygon(
            vec![
                play_center + Vec2::new(-2.0, -4.0),
                play_center + Vec2::new(4.0, 0.0),
                play_center + Vec2::new(-2.0, 4.0),
            ],
            if interaction_enabled {
                TEXT_PRIMARY
            } else {
                TEXT_MUTED
            },
            Stroke::NONE,
        ));
        if interaction_enabled && response.clicked() {
            preview_voice_event = Some(voice.id);
        }
    }

    GalleryResponse {
        preview_voice: preview_voice_event,
        bottom: grid_top + 4.0 * (card_height + gap) + 2.0,
    }
}
