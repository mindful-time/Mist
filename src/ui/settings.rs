use ::mist::{
    InferenceProviderId, LanguageId, PlaybackMode, PlaybackPreferences, PlaybackSpeed,
    ProviderCapability, ProviderPerformance, ports::VoiceCatalog,
};
use eframe::egui::{
    Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Slider, Stroke, StrokeKind, Vec2,
    WidgetInfo, WidgetType,
};

use super::theme::{PANEL_SURFACE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, with_alpha};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum SettingsSection {
    #[default]
    Voices,
    Playback,
    Privacy,
    Model,
}

impl SettingsSection {
    const ALL: [Self; 4] = [Self::Voices, Self::Playback, Self::Privacy, Self::Model];

    const fn label(self) -> &'static str {
        match self {
            Self::Voices => "Voices",
            Self::Playback => "Playback",
            Self::Privacy => "Privacy",
            Self::Model => "Model",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SettingsAction {
    PlayClipboard,
    OpenAccessibility,
}

pub(super) fn sidebar(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    current: SettingsSection,
    accent: Color32,
) -> Option<SettingsSection> {
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 18.0, Color32::from_white_alpha(5));
    painter.text(
        Pos2::new(rect.left() + 16.0, rect.top() + 17.0),
        Align2::LEFT_TOP,
        "SETTINGS",
        FontId::proportional(10.0),
        TEXT_MUTED,
    );

    let mut selected = None;
    for (index, section) in SettingsSection::ALL.into_iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 8.0, rect.top() + 42.0 + index as f32 * 47.0),
            Vec2::new(rect.width() - 16.0, 39.0),
        );
        let response = ui.interact(
            row,
            ui.id().with(("settings-section", index)),
            Sense::click(),
        );
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Button,
                true,
                current == section,
                section.label(),
            )
        });
        let active = current == section;
        let hovered = response.hovered();
        painter.rect_filled(
            row,
            11.0,
            if active {
                with_alpha(accent, 35)
            } else if hovered {
                Color32::from_white_alpha(10)
            } else {
                Color32::TRANSPARENT
            },
        );
        if active {
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(row.left() + 1.0, row.top() + 8.0),
                    Vec2::new(3.0, row.height() - 16.0),
                ),
                2.0,
                with_alpha(accent, 225),
            );
        }
        painter.text(
            Pos2::new(row.left() + 16.0, row.center().y),
            Align2::LEFT_CENTER,
            section.label(),
            FontId::proportional(13.0),
            if active { TEXT_PRIMARY } else { TEXT_SECONDARY },
        );
        if response.clicked() {
            selected = Some(section);
        }
    }
    selected
}

pub(super) fn playback(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    preferences: &mut PlaybackPreferences,
    accent: Color32,
) -> Option<SettingsAction> {
    let mut y = page_header(
        ui,
        rect,
        "PLAYBACK",
        "How Mist speaks",
        "Control queue flow and when generated speech becomes audible.",
    );
    toggle_row(
        ui,
        row(rect, y, 64.0),
        "Automatically play new queue items",
        "Turn this off to start each floating queue bubble yourself.",
        &mut preferences.auto_play_queue,
        accent,
    );
    y += 76.0;
    ui.painter().text(
        Pos2::new(rect.left(), y),
        Align2::LEFT_TOP,
        "WHEN SPEECH STARTS",
        FontId::proportional(9.5),
        TEXT_MUTED,
    );
    y += 20.0;
    if choice_row(
        ui,
        ChoiceRow {
            rect: row(rect, y, 52.0),
            title: "Real-time",
            detail: "Starts speaking as each sentence is generated · recommended",
            available: true,
            selected: preferences.mode == PlaybackMode::RealTime,
            performance: None,
        },
        accent,
    ) {
        preferences.mode = PlaybackMode::RealTime;
    }
    y += 59.0;
    if choice_row(
        ui,
        ChoiceRow {
            rect: row(rect, y, 52.0),
            title: "Complete audio",
            detail: "Generates the full selection before playback begins",
            available: true,
            selected: preferences.mode == PlaybackMode::CompleteAudio,
            performance: None,
        },
        accent,
    ) {
        preferences.mode = PlaybackMode::CompleteAudio;
    }
    y += 64.0;
    speed_row(ui, row(rect, y, 68.0), preferences, accent);
    y += 80.0;
    action_row(
        ui,
        row(rect, y, 68.0),
        "Play copied text from clipboard",
        "Adds the current plain-text clipboard value to the speech queue.",
        "Play now",
        accent,
    )
    .then_some(SettingsAction::PlayClipboard)
}

fn speed_row(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    preferences: &mut PlaybackPreferences,
    accent: Color32,
) {
    card_background(ui, rect, false);
    row_text(
        ui,
        rect,
        "Playback speed",
        "Applies to every language and voice.",
        true,
    );
    let mut percent = preferences.speed.percent();
    let slider = Rect::from_center_size(
        Pos2::new(rect.right() - 88.0, rect.center().y),
        Vec2::new(150.0, 28.0),
    );
    let response = ui.put(
        slider,
        Slider::new(
            &mut percent,
            PlaybackSpeed::MIN_PERCENT..=PlaybackSpeed::MAX_PERCENT,
        )
        .step_by(5.0)
        .suffix("%"),
    );
    if response.changed() {
        preferences.speed = PlaybackSpeed::from_percent(percent)
            .expect("the playback slider range must stay inside the domain range");
    }
    if response.hovered() || response.dragged() {
        ui.painter().rect_stroke(
            slider.expand(3.0),
            10.0,
            Stroke::new(1.0, with_alpha(accent, 90)),
            StrokeKind::Inside,
        );
    }
}

pub(super) fn privacy(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    preferences: &mut PlaybackPreferences,
    show_accessibility: bool,
    accent: Color32,
) -> Option<SettingsAction> {
    let mut y = page_header(
        ui,
        rect,
        "PRIVACY",
        "Selection and clipboard",
        "Speech synthesis stays on this device. Mist does not upload selected text.",
    );
    toggle_row(
        ui,
        row(rect, y, 78.0),
        "Use Copy when selection access fails",
        "Mist restores or clears only its own temporary clipboard value when it still matches.",
        &mut preferences.automatic_clipboard_fallback,
        accent,
    );
    y += 92.0;
    info_row(
        ui,
        row(rect, y, 68.0),
        "Local speech processing",
        "Voice generation and playback remain inside Mist on this computer.",
        "ON DEVICE",
        accent,
    );
    y += 82.0;
    if show_accessibility
        && action_row(
            ui,
            row(rect, y, 68.0),
            "macOS Accessibility",
            "Required to read selections exposed by other applications.",
            "Open settings",
            accent,
        )
    {
        Some(SettingsAction::OpenAccessibility)
    } else {
        None
    }
}

pub(super) const VOICE_LIST_OFFSET: f32 = 190.0;

pub(super) fn voice_language(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    catalog: &dyn VoiceCatalog,
    selected: LanguageId,
    accent: Color32,
) -> Option<LanguageId> {
    let painter = ui.painter();
    painter.text(
        rect.min,
        Align2::LEFT_TOP,
        "VOICES",
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(rect.left(), rect.top() + 18.0),
        Align2::LEFT_TOP,
        "Language & voice",
        FontId::proportional(23.0),
        TEXT_PRIMARY,
    );
    painter.text(
        Pos2::new(rect.left(), rect.top() + 51.0),
        Align2::LEFT_TOP,
        "Choose a language, then select its voice below.",
        FontId::proportional(12.0),
        TEXT_SECONDARY,
    );
    painter.text(
        Pos2::new(rect.left(), rect.top() + 78.0),
        Align2::LEFT_TOP,
        "LANGUAGE",
        FontId::proportional(9.5),
        TEXT_MUTED,
    );

    let gap = 6.0;
    let card_width = (rect.width() - gap * 2.0) / 3.0;
    let mut changed = None;
    for (index, language) in catalog.languages().iter().enumerate() {
        let column = index % 3;
        let card_row = index / 3;
        let card = Rect::from_min_size(
            Pos2::new(
                rect.left() + column as f32 * (card_width + gap),
                rect.top() + 98.0 + card_row as f32 * 29.0,
            ),
            Vec2::new(card_width, 24.0),
        );
        let voice_count = catalog.voice_count(language.id);
        if language_chip(
            ui,
            card,
            language.display_name,
            voice_count,
            selected == language.id,
            accent,
        ) && selected != language.id
        {
            changed = Some(language.id);
        }
    }
    changed
}

fn language_chip(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    title: &str,
    voice_count: usize,
    selected: bool,
    accent: Color32,
) -> bool {
    let response = ui.interact(rect, ui.id().with(("language", title)), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::RadioButton,
            true,
            selected,
            format!("{title}, {voice_count} voices"),
        )
    });
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        9.0,
        if selected {
            with_alpha(accent, 32)
        } else if response.hovered() {
            Color32::from_white_alpha(12)
        } else {
            PANEL_SURFACE
        },
    );
    painter.rect_stroke(
        rect,
        9.0,
        Stroke::new(
            if selected { 1.5 } else { 1.0 },
            if selected {
                with_alpha(accent, 155)
            } else {
                Color32::from_white_alpha(16)
            },
        ),
        StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(10.5),
        if selected {
            TEXT_PRIMARY
        } else {
            TEXT_SECONDARY
        },
    );
    painter.text(
        Pos2::new(rect.right() - 9.0, rect.center().y),
        Align2::RIGHT_CENTER,
        voice_count,
        FontId::proportional(9.0),
        if selected {
            with_alpha(accent, 225)
        } else {
            TEXT_MUTED
        },
    );
    response.clicked()
}

pub(super) fn model(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    capabilities: &[ProviderCapability],
    selected: &InferenceProviderId,
    active: &InferenceProviderId,
    restart_required: bool,
    accent: Color32,
) -> Option<InferenceProviderId> {
    page_header(
        ui,
        rect,
        "MODEL",
        "Performance",
        "Mist enables only runtimes validated on this device. Technical details stay visible below.",
    );
    let mut changed = None;
    for (capability, provider_rect) in capabilities
        .iter()
        .zip(model_provider_rows(rect, capabilities.len()))
    {
        if choice_row(
            ui,
            ChoiceRow {
                rect: provider_rect,
                title: capability.display_name,
                detail: capability.detail,
                available: capability.available,
                selected: selected == &capability.id,
                performance: Some(capability.performance),
            },
            accent,
        ) && &capability.id != selected
        {
            changed = Some(capability.id.clone());
        }
    }
    if restart_required {
        let notice = model_restart_notice_rect(rect);
        ui.painter()
            .rect_filled(notice, notice.height() * 0.5, with_alpha(accent, 30));
        ui.painter().text(
            notice.center(),
            Align2::CENTER_CENTER,
            format!(
                "RESTART TO APPLY · {} → {}",
                provider_name(capabilities, active),
                provider_name(capabilities, selected)
            ),
            FontId::proportional(9.5),
            with_alpha(accent, 225),
        );
    }
    changed
}

fn provider_name<'a>(capabilities: &'a [ProviderCapability], id: &InferenceProviderId) -> &'a str {
    capabilities
        .iter()
        .find(|capability| &capability.id == id)
        .map(|capability| capability.display_name)
        .unwrap_or("Unknown")
}

fn model_provider_rows(page: Rect, count: usize) -> impl Iterator<Item = Rect> {
    let first = page.top() + 86.0;
    (0..count).map(move |index| row(page, first + index as f32 * 59.0, 52.0))
}

fn model_restart_notice_rect(page: Rect) -> Rect {
    Rect::from_min_size(
        Pos2::new(page.right() - 218.0, page.top()),
        Vec2::new(218.0, 22.0),
    )
}

fn page_header(ui: &eframe::egui::Ui, rect: Rect, eyebrow: &str, title: &str, detail: &str) -> f32 {
    let painter = ui.painter();
    painter.text(
        rect.min,
        Align2::LEFT_TOP,
        eyebrow,
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(rect.left(), rect.top() + 20.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(23.0),
        TEXT_PRIMARY,
    );
    painter.text(
        Pos2::new(rect.left(), rect.top() + 55.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(12.0),
        TEXT_SECONDARY,
    );
    rect.top() + 86.0
}

fn row(page: Rect, top: f32, height: f32) -> Rect {
    Rect::from_min_size(Pos2::new(page.left(), top), Vec2::new(page.width(), height))
}

fn toggle_row(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    title: &str,
    detail: &str,
    value: &mut bool,
    accent: Color32,
) {
    let response = ui.interact(rect, ui.id().with(("toggle", title)), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, *value, title));
    if response.clicked() {
        *value = !*value;
    }
    card_background(ui, rect, response.hovered());
    row_text(ui, rect, title, detail, true);
    let track = Rect::from_center_size(
        Pos2::new(rect.right() - 29.0, rect.center().y),
        Vec2::new(38.0, 22.0),
    );
    ui.painter().rect_filled(
        track,
        11.0,
        if *value {
            with_alpha(accent, 195)
        } else {
            Color32::from_white_alpha(20)
        },
    );
    ui.painter().circle_filled(
        Pos2::new(
            if *value {
                track.right() - 11.0
            } else {
                track.left() + 11.0
            },
            track.center().y,
        ),
        8.0,
        TEXT_PRIMARY,
    );
}

fn action_row(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    title: &str,
    detail: &str,
    action: &str,
    accent: Color32,
) -> bool {
    let response = ui.interact(rect, ui.id().with(("action", title)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, title));
    card_background(ui, rect, response.hovered());
    row_text(ui, rect, title, detail, true);
    let action_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 54.0, rect.center().y),
        Vec2::new(88.0, 30.0),
    );
    ui.painter()
        .rect_filled(action_rect, 11.0, with_alpha(accent, 155));
    ui.painter().text(
        action_rect.center(),
        Align2::CENTER_CENTER,
        action,
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    response.clicked()
}

fn info_row(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    title: &str,
    detail: &str,
    badge: &str,
    accent: Color32,
) {
    card_background(ui, rect, false);
    row_text(ui, rect, title, detail, true);
    ui.painter().text(
        Pos2::new(rect.right() - 16.0, rect.center().y),
        Align2::RIGHT_CENTER,
        badge,
        FontId::proportional(9.5),
        with_alpha(accent, 220),
    );
}

struct ChoiceRow<'a> {
    rect: Rect,
    title: &'a str,
    detail: &'a str,
    available: bool,
    selected: bool,
    performance: Option<ProviderPerformance>,
}

fn choice_row(ui: &mut eframe::egui::Ui, row: ChoiceRow<'_>, accent: Color32) -> bool {
    let ChoiceRow {
        rect,
        title,
        detail,
        available,
        selected,
        performance,
    } = row;
    let response = ui.interact(
        rect,
        ui.id().with(("choice", title)),
        if available {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, available, selected, title));
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        14.0,
        if selected {
            with_alpha(accent, 24)
        } else if available && response.hovered() {
            Color32::from_white_alpha(12)
        } else {
            PANEL_SURFACE
        },
    );
    painter.rect_stroke(
        rect,
        14.0,
        Stroke::new(
            if selected { 1.5 } else { 1.0 },
            if selected {
                with_alpha(accent, 145)
            } else {
                Color32::from_white_alpha(16)
            },
        ),
        StrokeKind::Inside,
    );
    let color = if available { TEXT_PRIMARY } else { TEXT_MUTED };
    let text_left = if performance.is_some() { 43.0 } else { 17.0 };
    if let Some(performance) = performance {
        performance_icon(
            painter,
            Pos2::new(rect.left() + 22.0, rect.center().y),
            performance,
            color,
        );
    }
    painter.text(
        Pos2::new(rect.left() + text_left, rect.top() + 12.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(13.5),
        color,
    );
    painter.text(
        Pos2::new(rect.left() + text_left, rect.top() + 35.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(10.5),
        if available {
            TEXT_SECONDARY
        } else {
            TEXT_MUTED
        },
    );
    let center = Pos2::new(rect.right() - 22.0, rect.center().y);
    painter.circle_stroke(
        center,
        7.0,
        Stroke::new(
            1.0,
            if available {
                with_alpha(accent, 180)
            } else {
                Color32::from_white_alpha(28)
            },
        ),
    );
    if selected {
        painter.circle_filled(center, 3.5, with_alpha(accent, 230));
    }
    available && response.clicked()
}

fn performance_icon(
    painter: &eframe::egui::Painter,
    center: Pos2,
    performance: ProviderPerformance,
    color: Color32,
) {
    let stroke = Stroke::new(1.5, color);
    match performance {
        ProviderPerformance::Recommended => {
            painter.line_segment(
                [center - Vec2::new(0.0, 7.0), center + Vec2::new(0.0, 7.0)],
                stroke,
            );
            painter.line_segment(
                [center - Vec2::new(7.0, 0.0), center + Vec2::new(7.0, 0.0)],
                stroke,
            );
            painter.circle_filled(center, 2.5, color);
        }
        ProviderPerformance::Standard => {
            painter.circle_filled(center, 5.0, color);
        }
        ProviderPerformance::Accelerated => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(1.0, -7.0),
                    center + Vec2::new(-5.0, 0.5),
                    center,
                ],
                color,
                Stroke::NONE,
            ));
            painter.add(Shape::convex_polygon(
                vec![
                    center,
                    center + Vec2::new(5.0, -0.5),
                    center + Vec2::new(-1.0, 7.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        ProviderPerformance::Planned => {
            let top = center - Vec2::new(0.0, 6.0);
            let right = center + Vec2::new(6.0, 0.0);
            let bottom = center + Vec2::new(0.0, 6.0);
            let left = center - Vec2::new(6.0, 0.0);
            painter.line_segment([top, right], stroke);
            painter.line_segment([right, bottom], stroke);
            painter.line_segment([bottom, left], stroke);
            painter.line_segment([left, top], stroke);
        }
    }
}

fn card_background(ui: &eframe::egui::Ui, rect: Rect, hovered: bool) {
    ui.painter().rect_filled(
        rect,
        15.0,
        if hovered {
            Color32::from_white_alpha(12)
        } else {
            PANEL_SURFACE
        },
    );
    ui.painter().rect_stroke(
        rect,
        15.0,
        Stroke::new(1.0, Color32::from_white_alpha(16)),
        StrokeKind::Inside,
    );
}

fn row_text(ui: &eframe::egui::Ui, rect: Rect, title: &str, detail: &str, enabled: bool) {
    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.left() + 17.0, rect.top() + 13.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(13.5),
        if enabled { TEXT_PRIMARY } else { TEXT_MUTED },
    );
    painter.text(
        Pos2::new(rect.left() + 17.0, rect.top() + 38.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(10.5),
        if enabled { TEXT_SECONDARY } else { TEXT_MUTED },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_notice_never_intersects_a_model_provider_row() {
        let page = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 450.0));
        let notice = model_restart_notice_rect(page);

        for provider in model_provider_rows(page, 6) {
            assert!(
                !provider.intersects(notice),
                "restart notice {notice:?} overlaps provider row {provider:?}"
            );
        }
    }
}
