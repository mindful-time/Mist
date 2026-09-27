use ::mist::{LanguageId, VoiceProfile};
use eframe::egui::{
    Align2, Button, Color32, Direction, FontId, Layout, Pos2, Rect, RichText, Stroke, StrokeKind,
    UiBuilder, Vec2,
};

use super::{
    mist::MistRenderer,
    theme::{PANEL_SURFACE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, palette_color, with_alpha},
};

pub(super) struct GalleryResponse {
    pub(super) select_voice: Option<String>,
}

pub(super) struct GalleryContent<'a> {
    pub(super) time: f32,
    pub(super) selected_voice: &'a str,
    pub(super) voices: &'a [VoiceProfile],
    pub(super) language: LanguageId,
    pub(super) language_name: &'a str,
    pub(super) mode: GalleryMode,
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
            Self::DownloadRequired => "Download the local model to select a voice",
            Self::Available => "Select a voice · its local preview plays automatically",
            Self::Busy => "Current voice is speaking…",
        }
    }
}

pub(super) fn show(
    ui: &mut eframe::egui::Ui,
    mist: &MistRenderer,
    content: Rect,
    gallery: GalleryContent<'_>,
) -> GalleryResponse {
    let GalleryContent {
        time,
        selected_voice,
        voices: catalog,
        language,
        language_name,
        mode,
    } = gallery;
    let interaction_enabled = mode.interaction_enabled();
    let painter = ui.painter().clone();
    let section_y = content.top();
    painter.text(
        Pos2::new(content.left(), section_y),
        Align2::LEFT_TOP,
        format!("VOICE · {}", language_name.to_uppercase()),
        FontId::proportional(9.5),
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(content.left(), section_y + 18.0),
        Align2::LEFT_TOP,
        "Choose your mist",
        FontId::proportional(20.0),
        TEXT_PRIMARY,
    );
    painter.text(
        Pos2::new(content.left(), section_y + 45.0),
        Align2::LEFT_TOP,
        mode.hint(),
        FontId::proportional(11.0),
        TEXT_SECONDARY,
    );

    let grid_left = content.left();
    let grid_top = section_y + 66.0;
    let gap = 10.0;
    let card_width = (content.width() - gap) * 0.5;
    let card_height = 74.0;
    let voices = catalog
        .iter()
        .filter(|voice| voice.language == language)
        .collect::<Vec<_>>();
    let row_count = voices.len().div_ceil(2);
    let grid_view = Rect::from_min_max(
        Pos2::new(content.left(), grid_top),
        Pos2::new(content.right(), content.bottom()),
    );
    let content_height = (row_count as f32 * (card_height + gap) - gap).max(0.0);
    let max_scroll = (content_height - grid_view.height()).max(0.0);
    let scroll_id = ui.id().with(("voice-grid-scroll", language));
    let mut scroll = ui
        .data_mut(|data| data.get_temp::<f32>(scroll_id))
        .unwrap_or_default();
    let scroll_delta = ui.input(|input| {
        input
            .pointer
            .hover_pos()
            .filter(|position| grid_view.contains(*position))
            .map(|_| input.smooth_scroll_delta.y)
            .unwrap_or_default()
    });
    scroll = (scroll - scroll_delta).clamp(0.0, max_scroll);
    ui.data_mut(|data| data.insert_temp(scroll_id, scroll));
    ui.interact(
        grid_view,
        scroll_id.with("wheel-target"),
        eframe::egui::Sense::hover(),
    );
    let previous_clip = ui.clip_rect();
    ui.set_clip_rect(grid_view);
    let card_painter = ui.painter().clone();
    let mut selected_voice_event = None;
    for (index, voice) in voices.into_iter().enumerate() {
        let column = index % 2;
        let row = index / 2;
        let card = Rect::from_min_size(
            Pos2::new(
                grid_left + column as f32 * (card_width + gap),
                grid_top + row as f32 * (card_height + gap) - scroll,
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
                                "Select {} voice, {}",
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
        card_painter.rect_filled(
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
        card_painter.rect_stroke(
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
            Pos2::new(card.left() + 35.0, card.center().y),
            Vec2::splat(58.0),
        );
        mist.paint_swatch(ui, swatch, time, voice.palette, index as f32 * 0.83);
        card_painter.text(
            Pos2::new(card.left() + 68.0, card.top() + 18.0),
            Align2::LEFT_TOP,
            voice.display_name,
            FontId::proportional(16.0),
            TEXT_PRIMARY,
        );
        card_painter.text(
            Pos2::new(card.left() + 68.0, card.top() + 43.0),
            Align2::LEFT_TOP,
            voice.character,
            FontId::proportional(11.0),
            TEXT_SECONDARY,
        );
        let selected_center = Pos2::new(card.right() - 18.0, card.center().y);
        card_painter.circle_stroke(
            selected_center,
            7.0,
            Stroke::new(
                1.0,
                if selected {
                    with_alpha(palette_color(voice.palette.primary), 210)
                } else {
                    Color32::from_white_alpha(if hovered { 65 } else { 22 })
                },
            ),
        );
        if selected {
            card_painter.circle_filled(
                selected_center,
                3.5,
                with_alpha(palette_color(voice.palette.primary), 230),
            );
        }
        if interaction_enabled && response.clicked() {
            selected_voice_event = Some(voice.id.to_owned());
        }
    }
    ui.set_clip_rect(previous_clip);
    if max_scroll > 0.0 {
        let track = Rect::from_min_size(
            Pos2::new(grid_view.right() - 3.0, grid_view.top() + 4.0),
            Vec2::new(2.0, grid_view.height() - 8.0),
        );
        painter.rect_filled(track, 1.0, Color32::from_white_alpha(14));
        let thumb_height = (grid_view.height() / content_height * track.height()).max(28.0);
        let travel = track.height() - thumb_height;
        let thumb_top = track.top() + travel * scroll / max_scroll;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(track.left(), thumb_top),
                Vec2::new(track.width(), thumb_height),
            ),
            1.0,
            Color32::from_white_alpha(70),
        );
    }

    GalleryResponse {
        select_voice: selected_voice_event,
    }
}
