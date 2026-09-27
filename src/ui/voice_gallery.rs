use ::mist::{LanguageId, LanguageProfile, VoiceProfile};
use eframe::egui::{
    Align2, Button, Color32, Direction, FontId, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, UiBuilder, Vec2, WidgetInfo, WidgetType,
};

use super::{
    mist::MistRenderer,
    theme::{PANEL_SURFACE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, palette_color, with_alpha},
};

pub(super) struct GalleryResponse {
    pub(super) select_voice: Option<String>,
    pub(super) select_language: Option<LanguageId>,
}

pub(super) struct GalleryContent<'a> {
    pub(super) time: f32,
    pub(super) selected_voice: &'a str,
    pub(super) voices: &'a [VoiceProfile],
    pub(super) languages: &'a [LanguageProfile],
    pub(super) selected_language: LanguageId,
    pub(super) mode: GalleryMode,
    pub(super) accent: Color32,
}

const HEADER_HEIGHT: f32 = 42.0;
const HEADER_GAP: f32 = 9.0;
const CARD_HEIGHT: f32 = 78.0;
const CARD_GAP: f32 = 12.0;

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
        languages,
        selected_language,
        mode,
        accent,
    } = gallery;
    let interaction_enabled = mode.interaction_enabled();
    let painter = ui.painter().clone();
    let section_y = content.top();
    painter.text(
        Pos2::new(content.left(), section_y),
        Align2::LEFT_TOP,
        "VOICES",
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(content.left(), section_y + 18.0),
        Align2::LEFT_TOP,
        "Language & voice",
        FontId::proportional(23.0),
        TEXT_PRIMARY,
    );
    painter.text(
        Pos2::new(content.left(), section_y + 45.0),
        Align2::LEFT_TOP,
        format!("{} · Open a language to see its voices", mode.hint()),
        FontId::proportional(11.0),
        TEXT_SECONDARY,
    );

    let list_view = Rect::from_min_max(
        Pos2::new(content.left(), section_y + 80.0),
        Pos2::new(content.right() - 16.0, content.bottom()),
    );
    let expanded_voice_count = catalog
        .iter()
        .filter(|voice| voice.language == selected_language)
        .count();
    let content_height = accordion_height(languages.len(), expanded_voice_count);
    let max_scroll = (content_height - list_view.height()).max(0.0);
    let scroll_id = ui.id().with("voice-accordion-scroll");
    let mut scroll = ui
        .data_mut(|data| data.get_temp::<f32>(scroll_id))
        .unwrap_or_default();
    let scroll_delta = ui.input(|input| {
        input
            .pointer
            .hover_pos()
            .filter(|position| list_view.contains(*position))
            .map(|_| input.smooth_scroll_delta.y)
            .unwrap_or_default()
    });
    scroll = (scroll - scroll_delta).clamp(0.0, max_scroll);
    ui.data_mut(|data| data.insert_temp(scroll_id, scroll));
    ui.interact(
        list_view,
        scroll_id.with("wheel-target"),
        eframe::egui::Sense::hover(),
    );
    let previous_clip = ui.clip_rect();
    ui.set_clip_rect(list_view);
    let mut cursor_y = list_view.top() - scroll;
    let mut selected_voice_event = None;
    let mut selected_language_event = None;
    for language in languages {
        let expanded = language.id == selected_language;
        let voice_count = catalog
            .iter()
            .filter(|voice| voice.language == language.id)
            .count();
        let header = Rect::from_min_size(
            Pos2::new(list_view.left(), cursor_y),
            Vec2::new(list_view.width(), HEADER_HEIGHT),
        );
        if language_header(ui, header, language, voice_count, expanded, accent) && !expanded {
            selected_language_event = Some(language.id);
        }
        cursor_y += HEADER_HEIGHT + HEADER_GAP;
        if expanded {
            let card_width = (list_view.width() - CARD_GAP) * 0.5;
            for (index, voice) in catalog
                .iter()
                .filter(|voice| voice.language == language.id)
                .enumerate()
            {
                let column = index % 2;
                let row = index / 2;
                let card = Rect::from_min_size(
                    Pos2::new(
                        list_view.left() + column as f32 * (card_width + CARD_GAP),
                        cursor_y + row as f32 * (CARD_HEIGHT + CARD_GAP),
                    ),
                    Vec2::new(card_width, CARD_HEIGHT),
                );
                if voice_card(
                    ui,
                    mist,
                    card,
                    VoiceCardContent {
                        voice,
                        time,
                        index,
                        selected: selected_voice == voice.id,
                        interaction_enabled,
                    },
                ) {
                    selected_voice_event = Some(voice.id.to_owned());
                }
            }
            cursor_y += expanded_height(voice_count);
        }
    }
    ui.set_clip_rect(previous_clip);
    if max_scroll > 0.0 {
        let track = Rect::from_min_size(
            Pos2::new(content.right() - 7.0, list_view.top() + 4.0),
            Vec2::new(5.0, list_view.height() - 8.0),
        );
        painter.rect_filled(track, 2.5, Color32::from_white_alpha(28));
        let thumb_height = (list_view.height() / content_height * track.height()).max(38.0);
        let travel = track.height() - thumb_height;
        let thumb_top = track.top() + travel * scroll / max_scroll;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(track.left(), thumb_top),
                Vec2::new(track.width(), thumb_height),
            ),
            2.5,
            Color32::from_white_alpha(145),
        );
    }

    GalleryResponse {
        select_voice: selected_voice_event,
        select_language: selected_language_event,
    }
}

fn accordion_height(language_count: usize, expanded_voice_count: usize) -> f32 {
    language_count as f32 * (HEADER_HEIGHT + HEADER_GAP) + expanded_height(expanded_voice_count)
}

fn expanded_height(voice_count: usize) -> f32 {
    let rows = voice_count.div_ceil(2);
    rows as f32 * (CARD_HEIGHT + CARD_GAP)
}

fn language_header(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    language: &LanguageProfile,
    voice_count: usize,
    expanded: bool,
    accent: Color32,
) -> bool {
    let response = ui.interact(
        rect,
        ui.id().with(("language-accordion", language.id)),
        Sense::click(),
    );
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::Button,
            true,
            expanded,
            format!("{}, {voice_count} voices", language.display_name),
        )
    });
    let style = language_header_style(expanded, response.hovered(), accent);
    let painter = ui.painter();
    painter.rect_filled(rect, 12.0, style.fill);
    painter.rect_stroke(
        rect,
        12.0,
        Stroke::new(f32::from(style.stroke_width), style.stroke),
        StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        language.display_name,
        FontId::proportional(13.0),
        style.title,
    );
    painter.text(
        Pos2::new(rect.right() - 35.0, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{voice_count}"),
        FontId::proportional(10.0),
        style.meta,
    );
    painter.add(Shape::line(
        chevron_points(expanded)
            .into_iter()
            .map(|offset| Pos2::new(rect.right() - 15.0, rect.center().y) + offset)
            .collect(),
        Stroke::new(1.5, style.meta),
    ));
    response.clicked()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LanguageHeaderStyle {
    fill: Color32,
    stroke: Color32,
    stroke_width: u8,
    title: Color32,
    meta: Color32,
}

fn language_header_style(expanded: bool, hovered: bool, accent: Color32) -> LanguageHeaderStyle {
    match (expanded, hovered) {
        (true, _) => LanguageHeaderStyle {
            fill: with_alpha(accent, 34),
            stroke: with_alpha(accent, 155),
            stroke_width: 2,
            title: TEXT_PRIMARY,
            meta: with_alpha(accent, 225),
        },
        (false, true) => LanguageHeaderStyle {
            fill: Color32::from_white_alpha(12),
            stroke: Color32::from_white_alpha(17),
            stroke_width: 1,
            title: TEXT_SECONDARY,
            meta: TEXT_MUTED,
        },
        (false, false) => LanguageHeaderStyle {
            fill: PANEL_SURFACE,
            stroke: Color32::from_white_alpha(17),
            stroke_width: 1,
            title: TEXT_SECONDARY,
            meta: TEXT_MUTED,
        },
    }
}

fn chevron_points(expanded: bool) -> [Vec2; 3] {
    if expanded {
        [
            Vec2::new(-4.0, -2.0),
            Vec2::new(0.0, 2.0),
            Vec2::new(4.0, -2.0),
        ]
    } else {
        [
            Vec2::new(-2.0, -4.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(-2.0, 4.0),
        ]
    }
}

struct VoiceCardContent<'a> {
    voice: &'a VoiceProfile,
    time: f32,
    index: usize,
    selected: bool,
    interaction_enabled: bool,
}

fn voice_card(
    ui: &mut eframe::egui::Ui,
    mist: &MistRenderer,
    card: Rect,
    content: VoiceCardContent<'_>,
) -> bool {
    let VoiceCardContent {
        voice,
        time,
        index,
        selected,
        interaction_enabled,
    } = content;
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
                    .corner_radius(16.0),
                )
            },
        )
        .inner;
    let style = voice_card_style(
        selected,
        response.has_focus(),
        interaction_enabled,
        response.hovered(),
        palette_color(voice.palette.primary),
    );
    let painter = ui.painter();
    painter.rect_filled(card, 16.0, style.fill);
    painter.rect_stroke(
        card,
        16.0,
        Stroke::new(f32::from(style.stroke_width), style.stroke),
        StrokeKind::Inside,
    );
    let swatch = Rect::from_center_size(
        Pos2::new(card.left() + 32.0, card.center().y),
        Vec2::splat(52.0),
    );
    mist.paint_swatch(ui, swatch, time, voice.palette, index as f32 * 0.83);
    painter.text(
        Pos2::new(card.left() + 62.0, card.top() + 15.0),
        Align2::LEFT_TOP,
        voice.display_name,
        FontId::proportional(15.0),
        TEXT_PRIMARY,
    );
    painter.text(
        Pos2::new(card.left() + 62.0, card.top() + 39.0),
        Align2::LEFT_TOP,
        voice.character,
        FontId::proportional(10.5),
        TEXT_SECONDARY,
    );
    let selected_center = Pos2::new(card.right() - 17.0, card.center().y);
    painter.circle_stroke(selected_center, 6.5, Stroke::new(1.0, style.selection_ring));
    painter.circle_filled(selected_center, 3.2, style.selection_dot);
    active_click(interaction_enabled, response.clicked())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VoiceCardStyle {
    fill: Color32,
    stroke: Color32,
    stroke_width: u8,
    selection_ring: Color32,
    selection_dot: Color32,
}

fn voice_card_style(
    selected: bool,
    focused: bool,
    enabled: bool,
    pointer_hovered: bool,
    accent: Color32,
) -> VoiceCardStyle {
    let hovered = enabled && pointer_hovered;
    let (selection_ring, selection_dot) = selection_indicator(selected, hovered, accent);
    VoiceCardStyle {
        fill: card_fill(selected, hovered, accent),
        stroke: card_stroke(selected, focused, accent),
        stroke_width: u8::from(selected || focused) + 1,
        selection_ring,
        selection_dot,
    }
}

fn card_fill(selected: bool, hovered: bool, accent: Color32) -> Color32 {
    match (selected, hovered) {
        (true, _) => with_alpha(accent, 31),
        (false, true) => Color32::from_white_alpha(14),
        (false, false) => PANEL_SURFACE,
    }
}

fn card_stroke(selected: bool, focused: bool, accent: Color32) -> Color32 {
    match (selected, focused) {
        (true, _) => with_alpha(accent, 150),
        (false, true) => Color32::from_white_alpha(135),
        (false, false) => Color32::from_white_alpha(18),
    }
}

fn selection_indicator(selected: bool, hovered: bool, accent: Color32) -> (Color32, Color32) {
    match (selected, hovered) {
        (true, _) => (with_alpha(accent, 210), with_alpha(accent, 230)),
        (false, true) => (Color32::from_white_alpha(65), Color32::TRANSPARENT),
        (false, false) => (Color32::from_white_alpha(22), Color32::TRANSPARENT),
    }
}

fn active_click(enabled: bool, clicked: bool) -> bool {
    enabled && clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accordion_reserves_headers_and_only_one_expanded_voice_grid() {
        assert_eq!(accordion_height(9, 1), 549.0);
        assert_eq!(accordion_height(9, 4), 639.0);
        assert_eq!(accordion_height(9, 5), 729.0);
    }

    #[test]
    fn accordion_header_styles_cover_open_hovered_and_resting_states() {
        let accent = Color32::from_rgb(10, 20, 30);
        let open = language_header_style(true, false, accent);
        assert_eq!(open.stroke_width, 2);
        assert_eq!(open.title, TEXT_PRIMARY);

        let hovered = language_header_style(false, true, accent);
        assert_eq!(hovered.fill, Color32::from_white_alpha(12));

        let resting = language_header_style(false, false, accent);
        assert_eq!(resting.fill, PANEL_SURFACE);
        assert_eq!(resting.stroke_width, 1);
        assert_eq!(chevron_points(true)[1], Vec2::new(0.0, 2.0));
        assert_eq!(chevron_points(false)[1], Vec2::new(2.0, 0.0));
    }

    #[test]
    fn voice_card_styles_cover_selection_focus_hover_and_disabled_states() {
        let accent = Color32::from_rgb(40, 50, 60);
        let selected = voice_card_style(true, false, true, false, accent);
        assert_eq!(selected.stroke_width, 2);
        assert_ne!(selected.selection_dot, Color32::TRANSPARENT);

        let focused = voice_card_style(false, true, true, false, accent);
        assert_eq!(focused.stroke, Color32::from_white_alpha(135));
        assert_eq!(focused.stroke_width, 2);

        let hovered = voice_card_style(false, false, true, true, accent);
        assert_eq!(hovered.fill, Color32::from_white_alpha(14));
        assert_eq!(hovered.selection_ring, Color32::from_white_alpha(65));

        let disabled = voice_card_style(false, false, false, true, accent);
        assert_eq!(disabled.fill, PANEL_SURFACE);
        assert_eq!(disabled.selection_dot, Color32::TRANSPARENT);
        assert!(!active_click(false, true));
        assert!(active_click(true, true));
    }
}
