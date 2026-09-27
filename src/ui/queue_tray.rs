use ::mist::{MistPalette, QueueItemId, QueueItemState, QueuedSpeech};
use eframe::egui::{
    Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
    WidgetInfo, WidgetType,
};

use super::theme::{TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, palette_color, with_alpha};

pub(super) const WIDTH: f32 = 276.0;
pub(super) const ROW_HEIGHT: f32 = 42.0;
pub(super) const ROW_GAP: f32 = 6.0;
pub(super) const TOP_GAP: f32 = 7.0;
pub(super) const MAX_VISIBLE_ITEMS: usize = 3;

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
) -> Option<QueueItemId> {
    let left = ui.max_rect().center().x - WIDTH * 0.5;
    let accent = palette_color(palette.primary);
    let mut requested = None;

    for (index, item) in items.iter().take(MAX_VISIBLE_ITEMS).enumerate() {
        let rect = Rect::from_min_size(
            Pos2::new(left, top + index as f32 * (ROW_HEIGHT + ROW_GAP)),
            Vec2::new(WIDTH, ROW_HEIGHT),
        );
        let response = ui.interact(
            rect,
            ui.id().with(("queue-item", item.id.get())),
            Sense::click(),
        );
        let active = matches!(
            item.state,
            QueueItemState::Preparing | QueueItemState::Playing
        );
        let failed = item.state == QueueItemState::Failed;
        let preview = item.text.preview(42).replace(['\r', '\n'], " ");
        let accessible_action = if active {
            "Currently speaking"
        } else if failed {
            "Retry speech"
        } else {
            "Play queued speech"
        };
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                !active,
                format!("{accessible_action}: {preview}"),
            )
        });
        ui.painter().rect_filled(
            rect,
            ROW_HEIGHT * 0.5,
            if active {
                with_alpha(accent, 54)
            } else if response.hovered() {
                Color32::from_rgba_unmultiplied(19, 22, 31, 226)
            } else {
                Color32::from_rgba_unmultiplied(11, 13, 20, 205)
            },
        );
        ui.painter().rect_stroke(
            rect,
            ROW_HEIGHT * 0.5,
            Stroke::new(
                1.0,
                if active {
                    with_alpha(accent, 126)
                } else if failed {
                    Color32::from_rgba_unmultiplied(255, 144, 108, 105)
                } else {
                    Color32::from_white_alpha(22)
                },
            ),
            StrokeKind::Inside,
        );

        let action_center = Pos2::new(rect.left() + 22.0, rect.center().y);
        ui.painter().circle_filled(
            action_center,
            11.0,
            if active {
                with_alpha(accent, 205)
            } else {
                Color32::from_white_alpha(if response.hovered() { 30 } else { 18 })
            },
        );
        if active {
            for offset in [-3.0, 1.0, 5.0] {
                let height = if offset == 1.0 { 10.0 } else { 6.0 };
                ui.painter().line_segment(
                    [
                        Pos2::new(action_center.x + offset, action_center.y - height * 0.5),
                        Pos2::new(action_center.x + offset, action_center.y + height * 0.5),
                    ],
                    Stroke::new(1.4, TEXT_PRIMARY),
                );
            }
        } else {
            ui.painter().add(eframe::egui::Shape::convex_polygon(
                vec![
                    action_center + Vec2::new(-2.5, -4.5),
                    action_center + Vec2::new(4.0, 0.0),
                    action_center + Vec2::new(-2.5, 4.5),
                ],
                if failed {
                    Color32::from_rgb(255, 182, 156)
                } else {
                    TEXT_PRIMARY
                },
                Stroke::NONE,
            ));
        }

        let label = match item.state {
            QueueItemState::Preparing => "PREPARING",
            QueueItemState::Playing => "SPEAKING",
            QueueItemState::Failed => "RETRY",
            QueueItemState::Waiting if auto_play => "QUEUED",
            QueueItemState::Waiting => "CLICK TO PLAY",
        };
        ui.painter().text(
            Pos2::new(rect.left() + 42.0, rect.top() + 7.0),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(8.5),
            if active {
                with_alpha(accent, 235)
            } else {
                TEXT_MUTED
            },
        );
        ui.painter().text(
            Pos2::new(rect.left() + 42.0, rect.top() + 20.0),
            Align2::LEFT_TOP,
            preview,
            FontId::proportional(11.5),
            if active { TEXT_PRIMARY } else { TEXT_SECONDARY },
        );

        let keyboard_activated = response.has_focus()
            && ui.input_mut(|input| {
                input.consume_key(Modifiers::NONE, Key::Enter)
                    || input.consume_key(Modifiers::NONE, Key::Space)
            });
        if (response.clicked() || keyboard_activated) && !active {
            requested = Some(item.id);
        }
    }

    if items.len() > MAX_VISIBLE_ITEMS {
        let index = MAX_VISIBLE_ITEMS;
        let rect = Rect::from_min_size(
            Pos2::new(left, top + index as f32 * (ROW_HEIGHT + ROW_GAP)),
            Vec2::new(WIDTH, ROW_HEIGHT),
        );
        ui.painter().rect_filled(
            rect,
            ROW_HEIGHT * 0.5,
            Color32::from_rgba_unmultiplied(11, 13, 20, 188),
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("+{} more in Mist", items.len() - MAX_VISIBLE_ITEMS),
            FontId::proportional(11.0),
            TEXT_MUTED,
        );
    }

    requested
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_tray_stays_bounded_as_items_accumulate() {
        assert_eq!(visible_rows(0), 0);
        assert_eq!(visible_rows(2), 2);
        assert_eq!(visible_rows(8), 4);
        assert!(height(8) < 210.0);
    }
}
