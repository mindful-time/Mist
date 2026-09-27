//! Draggable queued-speech surface.

use crate::{MistPalette, QueueItemId, QueueItemState, QueuedSpeech};
use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind,
    Vec2, WidgetInfo, WidgetType,
};

use super::theme::{palette_color, with_alpha};

pub(super) const WIDTH: f32 = 266.0;
pub(super) const ROW_HEIGHT: f32 = 34.0;
pub(super) const ROW_GAP: f32 = 5.0;
pub(super) const TOP_GAP: f32 = 5.0;
pub(super) const MAX_VISIBLE_ITEMS: usize = 3;

const PILL_WIDTH: f32 = 194.0;
const CONTROL_SIZE: f32 = 32.0;
const CONTROL_GAP: f32 = 4.0;
const GLASS: Color32 = Color32::from_rgba_unmultiplied_const(225, 238, 245, 218);
const GLASS_HOVER: Color32 = Color32::from_rgba_unmultiplied_const(239, 247, 251, 235);
const INK: Color32 = Color32::from_rgb(50, 61, 71);
const INK_MUTED: Color32 = Color32::from_rgb(98, 111, 121);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueueAction {
    Play(QueueItemId),
    Pause(QueueItemId),
    Resume(QueueItemId),
    Delete(QueueItemId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct QueueTrayResponse {
    pub action: Option<QueueAction>,
    pub drag_started: bool,
}

pub(super) fn visible_rows(item_count: usize) -> usize {
    item_count.min(MAX_VISIBLE_ITEMS) + usize::from(item_count > MAX_VISIBLE_ITEMS)
}

pub(super) fn height(item_count: usize) -> f32 {
    let rows = visible_rows(item_count);
    if rows == 0 {
        return 0.0;
    }
    TOP_GAP + rows as f32 * ROW_HEIGHT + rows.saturating_sub(1) as f32 * ROW_GAP
}

pub(super) fn show(
    ui: &mut eframe::egui::Ui,
    items: &[QueuedSpeech],
    top: f32,
    palette: MistPalette,
    auto_play: bool,
) -> QueueTrayResponse {
    let left = ui.max_rect().center().x - WIDTH * 0.5;
    let accent = palette_color(palette.primary);
    let mut output = QueueTrayResponse::default();

    for (index, item) in items.iter().take(MAX_VISIBLE_ITEMS).enumerate() {
        let row_top = top + index as f32 * (ROW_HEIGHT + ROW_GAP);
        let pill = Rect::from_min_size(Pos2::new(left, row_top), Vec2::new(PILL_WIDTH, ROW_HEIGHT));
        let play = Rect::from_min_size(
            Pos2::new(pill.right() + CONTROL_GAP, row_top + 1.0),
            Vec2::splat(CONTROL_SIZE),
        );
        let delete = Rect::from_min_size(
            Pos2::new(play.right() + CONTROL_GAP, row_top + 1.0),
            Vec2::splat(CONTROL_SIZE),
        );

        let drag = ui.interact(
            pill,
            ui.id().with(("queue-drag", item.id.get())),
            Sense::drag(),
        );
        let play_response = ui.interact(
            play,
            ui.id().with(("queue-play", item.id.get())),
            Sense::click(),
        );
        let delete_response = ui.interact(
            delete,
            ui.id().with(("queue-delete", item.id.get())),
            Sense::click(),
        );

        let preview = item.text.preview(27).replace(['\r', '\n'], " ");
        drag.clone()
            .on_hover_cursor(if drag.dragged() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            })
            .on_hover_text("Drag Mist");
        play_response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                item.state != QueueItemState::Preparing,
                format!("{}: {preview}", playback_label(item.state, auto_play)),
            )
        });
        delete_response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::Button, true, format!("Delete: {preview}"))
        });

        paint_glass(ui, pill, drag.hovered(), item.state, accent);
        paint_glass_circle(ui, play, play_response.hovered(), accent, false);
        paint_glass_circle(ui, delete, delete_response.hovered(), accent, true);

        ui.painter().text(
            Pos2::new(pill.left() + 13.0, pill.center().y),
            Align2::LEFT_CENTER,
            preview,
            FontId::proportional(11.5),
            INK,
        );
        paint_state_dot(ui, pill, item.state, accent);
        paint_playback_icon(ui, play.center(), item.state);
        paint_delete_icon(ui, delete.center());

        let keyboard_play = play_response.has_focus()
            && ui.input_mut(|input| {
                input.consume_key(Modifiers::NONE, Key::Enter)
                    || input.consume_key(Modifiers::NONE, Key::Space)
            });
        let keyboard_delete = delete_response.has_focus()
            && ui.input_mut(|input| {
                input.consume_key(Modifiers::NONE, Key::Enter)
                    || input.consume_key(Modifiers::NONE, Key::Space)
            });

        if drag.drag_started() {
            output.drag_started = true;
        }
        if delete_response.clicked() || keyboard_delete {
            output.action = Some(QueueAction::Delete(item.id));
        } else if (play_response.clicked() || keyboard_play)
            && item.state != QueueItemState::Preparing
        {
            output.action = playback_action(item.id, item.state);
        }
    }

    if items.len() > MAX_VISIBLE_ITEMS {
        let index = MAX_VISIBLE_ITEMS;
        let rect = Rect::from_min_size(
            Pos2::new(left, top + index as f32 * (ROW_HEIGHT + ROW_GAP)),
            Vec2::new(PILL_WIDTH, ROW_HEIGHT),
        );
        let response = ui.interact(rect, ui.id().with("queue-overflow-drag"), Sense::drag());
        paint_glass(
            ui,
            rect,
            response.hovered(),
            QueueItemState::Waiting,
            accent,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("+{} more queued", items.len() - MAX_VISIBLE_ITEMS),
            FontId::proportional(11.0),
            INK_MUTED,
        );
        if response.drag_started() {
            output.drag_started = true;
        }
    }

    output
}

fn playback_label(state: QueueItemState, auto_play: bool) -> &'static str {
    match state {
        QueueItemState::Preparing => "Preparing speech",
        QueueItemState::Playing => "Pause speech",
        QueueItemState::Paused => "Resume speech",
        QueueItemState::Failed => "Retry speech",
        QueueItemState::Waiting if auto_play => "Queued speech",
        QueueItemState::Waiting => "Play queued speech",
    }
}

fn playback_action(id: QueueItemId, state: QueueItemState) -> Option<QueueAction> {
    match state {
        QueueItemState::Playing => Some(QueueAction::Pause(id)),
        QueueItemState::Paused => Some(QueueAction::Resume(id)),
        QueueItemState::Waiting | QueueItemState::Failed => Some(QueueAction::Play(id)),
        QueueItemState::Preparing => None,
    }
}

fn paint_glass(
    ui: &eframe::egui::Ui,
    rect: Rect,
    hovered: bool,
    state: QueueItemState,
    accent: Color32,
) {
    let active = matches!(
        state,
        QueueItemState::Preparing | QueueItemState::Playing | QueueItemState::Paused
    );
    ui.painter().rect_filled(
        rect,
        ROW_HEIGHT * 0.5,
        if active {
            tinted_glass(accent, if hovered { 238 } else { 220 })
        } else if hovered {
            GLASS_HOVER
        } else {
            GLASS
        },
    );
    ui.painter().rect_stroke(
        rect,
        ROW_HEIGHT * 0.5,
        Stroke::new(1.0, Color32::from_white_alpha(105)),
        StrokeKind::Inside,
    );
}

fn paint_glass_circle(
    ui: &eframe::egui::Ui,
    rect: Rect,
    hovered: bool,
    accent: Color32,
    destructive: bool,
) {
    let fill = if hovered && destructive {
        Color32::from_rgba_unmultiplied(255, 211, 205, 238)
    } else if hovered {
        tinted_glass(accent, 235)
    } else {
        GLASS
    };
    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.5, fill);
    ui.painter().circle_stroke(
        rect.center(),
        rect.width() * 0.5,
        Stroke::new(1.0, Color32::from_white_alpha(105)),
    );
}

fn tinted_glass(accent: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(
        ((u16::from(accent.r()) + 225) / 2) as u8,
        ((u16::from(accent.g()) + 238) / 2) as u8,
        ((u16::from(accent.b()) + 245) / 2) as u8,
        alpha,
    )
}

fn paint_state_dot(ui: &eframe::egui::Ui, rect: Rect, state: QueueItemState, accent: Color32) {
    let color = match state {
        QueueItemState::Playing => accent,
        QueueItemState::Paused => Color32::from_rgb(241, 173, 79),
        QueueItemState::Failed => Color32::from_rgb(219, 105, 88),
        QueueItemState::Preparing => with_alpha(accent, 160),
        QueueItemState::Waiting => Color32::from_rgba_unmultiplied(80, 94, 105, 95),
    };
    ui.painter()
        .circle_filled(Pos2::new(rect.right() - 12.0, rect.center().y), 2.6, color);
}

fn paint_playback_icon(ui: &eframe::egui::Ui, center: Pos2, state: QueueItemState) {
    if state == QueueItemState::Playing {
        for offset in [-2.4, 2.4] {
            ui.painter().line_segment(
                [
                    Pos2::new(center.x + offset, center.y - 4.2),
                    Pos2::new(center.x + offset, center.y + 4.2),
                ],
                Stroke::new(1.8, INK),
            );
        }
    } else if state == QueueItemState::Preparing {
        ui.painter().circle_stroke(
            center,
            5.0,
            Stroke::new(1.5, Color32::from_rgba_unmultiplied(50, 61, 71, 105)),
        );
    } else {
        ui.painter().add(eframe::egui::Shape::convex_polygon(
            vec![
                center + Vec2::new(-2.5, -4.5),
                center + Vec2::new(4.0, 0.0),
                center + Vec2::new(-2.5, 4.5),
            ],
            INK,
            Stroke::NONE,
        ));
    }
}

fn paint_delete_icon(ui: &eframe::egui::Ui, center: Pos2) {
    for (from, to) in [
        (Vec2::new(-3.5, -3.5), Vec2::new(3.5, 3.5)),
        (Vec2::new(3.5, -3.5), Vec2::new(-3.5, 3.5)),
    ] {
        ui.painter()
            .line_segment([center + from, center + to], Stroke::new(1.7, INK_MUTED));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_tray_stays_compact_and_bounded_as_items_accumulate() {
        assert_eq!(visible_rows(0), 0);
        assert_eq!(visible_rows(2), 2);
        assert_eq!(visible_rows(8), 4);
        assert_eq!(height(1), 39.0);
        assert!(height(8) < 165.0);
    }

    #[test]
    fn each_state_exposes_the_expected_playback_action() {
        assert_eq!(
            playback_label(QueueItemState::Playing, true),
            "Pause speech"
        );
        assert_eq!(
            playback_label(QueueItemState::Paused, true),
            "Resume speech"
        );
        assert_eq!(playback_label(QueueItemState::Failed, true), "Retry speech");
        let mut queue = crate::SpeechQueue::default();
        let id = queue
            .push(crate::SelectedText::new("Control this item").unwrap())
            .unwrap();
        assert_eq!(
            playback_action(id, QueueItemState::Playing),
            Some(QueueAction::Pause(id))
        );
        assert_eq!(
            playback_action(id, QueueItemState::Paused),
            Some(QueueAction::Resume(id))
        );
        assert_eq!(playback_action(id, QueueItemState::Preparing), None);
    }
}
