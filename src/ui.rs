mod presentation;
mod system_settings;
mod theme;

use std::{
    sync::mpsc::{self, TrySendError},
    time::Duration,
};

use eframe::egui::{
    self, Align2, Button, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
};
use select_to_speak::{
    platform::{PlatformBridge, PlatformEvent},
    worker::{AppStatus, WorkerCommand},
};

use self::{
    presentation::{PanelKind, Presentation, PrimaryAction, Tone},
    system_settings::open_accessibility_settings,
    theme::{
        CARD_BACKGROUND, CARD_SURFACE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
        configure_interface, with_alpha,
    },
};

pub struct PetApp {
    commands: mpsc::SyncSender<WorkerCommand>,
    statuses: mpsc::Receiver<AppStatus>,
    status: AppStatus,
    platform: PlatformBridge,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    manual_text: String,
}

impl PetApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        commands: mpsc::SyncSender<WorkerCommand>,
        statuses: mpsc::Receiver<AppStatus>,
    ) -> Self {
        configure_interface(&creation_context.egui_ctx);
        let platform = PlatformBridge::new(commands.clone());
        Self {
            commands,
            statuses,
            status: AppStatus::CheckingModel,
            platform,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            manual_text: String::new(),
        }
    }

    fn drain_statuses(&mut self) {
        while let Ok(status) = self.statuses.try_recv() {
            self.status = status;
        }
    }

    fn enqueue(&mut self, command: WorkerCommand) {
        if let Err(error) = self.commands.try_send(command) {
            let message = match error {
                TrySendError::Full(_) => "The speech queue is full. Try again in a moment.",
                TrySendError::Disconnected(_) => "The speech worker stopped unexpectedly.",
            };
            self.status = AppStatus::Error(message.to_owned());
        }
    }

    fn paint_card(&self, ui: &egui::Ui, card: Rect, presentation: &Presentation) {
        let painter = ui.painter();
        painter.rect_filled(card, 24.0, CARD_BACKGROUND);
        painter.circle_filled(
            Pos2::new(card.left() + 80.0, card.top() + 130.0),
            72.0,
            Color32::from_rgba_unmultiplied(
                presentation.tone.color().r(),
                presentation.tone.color().g(),
                presentation.tone.color().b(),
                8,
            ),
        );
        painter.rect_stroke(
            card,
            24.0,
            Stroke::new(1.0, Color32::from_white_alpha(24)),
            StrokeKind::Inside,
        );
        painter.line_segment(
            [
                Pos2::new(card.left() + 18.0, card.top() + 46.0),
                Pos2::new(card.right() - 18.0, card.top() + 46.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(15)),
        );
    }

    fn paint_header(&self, ui: &egui::Ui, card: Rect, presentation: &Presentation) {
        let painter = ui.painter();
        let mark_center = Pos2::new(card.left() + 24.0, card.top() + 23.0);
        let accent = presentation.tone.color();
        painter.circle_filled(mark_center, 10.0, CARD_SURFACE);
        painter.circle_stroke(mark_center, 10.0, Stroke::new(1.0, accent));
        for (offset, height) in [(-3.5, 6.0), (0.0, 10.0), (3.5, 5.0)] {
            painter.rect_filled(
                Rect::from_center_size(
                    Pos2::new(mark_center.x + offset, mark_center.y),
                    Vec2::new(2.0, height),
                ),
                1.0,
                accent,
            );
        }
        painter.text(
            Pos2::new(card.left() + 43.0, card.top() + 16.0),
            Align2::LEFT_TOP,
            "SELECT TO SPEAK",
            FontId::proportional(11.0),
            TEXT_SECONDARY,
        );

        let badge_width = (presentation.badge.chars().count() as f32 * 6.5 + 28.0).max(66.0);
        let badge = Rect::from_min_size(
            Pos2::new(card.right() - badge_width - 45.0, card.top() + 11.0),
            Vec2::new(badge_width, 24.0),
        );
        painter.rect_filled(badge, 12.0, with_alpha(accent, 22));
        painter.rect_stroke(
            badge,
            12.0,
            Stroke::new(1.0, with_alpha(accent, 60)),
            StrokeKind::Inside,
        );
        painter.circle_filled(
            Pos2::new(badge.left() + 12.0, badge.center().y),
            3.0,
            accent,
        );
        painter.text(
            Pos2::new(badge.left() + 20.0, badge.center().y),
            Align2::LEFT_CENTER,
            presentation.badge,
            FontId::proportional(9.5),
            accent,
        );
    }

    fn paint_mascot(&self, ui: &egui::Ui, card: Rect, presentation: &Presentation, time: f32) {
        let painter = ui.painter();
        let center = Pos2::new(card.left() + 72.0, card.top() + 132.0);
        let accent = presentation.tone.color();
        let animated = matches!(presentation.kind, PanelKind::Busy | PanelKind::Speaking);
        let pulse = if animated {
            (time * 3.4).sin().mul_add(0.5, 0.5)
        } else {
            0.25
        };

        painter.circle_stroke(
            center,
            48.0 + pulse * 3.0,
            Stroke::new(1.0, with_alpha(accent, (32.0 + pulse * 36.0) as u8)),
        );
        painter.circle_filled(center, 42.0, CARD_SURFACE);
        painter.circle_stroke(center, 42.0, Stroke::new(1.5, with_alpha(accent, 165)));
        painter.circle_filled(
            Pos2::new(center.x - 16.0, center.y - 8.0),
            3.3,
            TEXT_PRIMARY,
        );
        painter.circle_filled(
            Pos2::new(center.x + 16.0, center.y - 8.0),
            3.3,
            TEXT_PRIMARY,
        );

        if presentation.kind == PanelKind::Speaking {
            for (index, offset) in [-8.0_f32, -4.0, 0.0, 4.0, 8.0].into_iter().enumerate() {
                let wave = (time * 7.0 + index as f32 * 0.9).sin().abs();
                let height = 4.0 + wave * 11.0;
                painter.rect_filled(
                    Rect::from_center_size(
                        Pos2::new(center.x + offset, center.y + 14.0),
                        Vec2::new(2.5, height),
                    ),
                    1.25,
                    accent,
                );
            }
        } else {
            painter.rect_filled(
                Rect::from_center_size(Pos2::new(center.x, center.y + 14.0), Vec2::new(17.0, 3.0)),
                1.5,
                if presentation.kind == PanelKind::Error {
                    with_alpha(accent, 210)
                } else {
                    with_alpha(TEXT_SECONDARY, 180)
                },
            );
        }
    }

    fn paint_body(
        &mut self,
        ui: &mut egui::Ui,
        card: Rect,
        presentation: &Presentation,
        time: f32,
    ) {
        let painter = ui.painter().clone();
        let content_left = card.left() + 132.0;
        let content_width = card.right() - 18.0 - content_left;
        let title_top = card.top() + 69.0;
        painter.text(
            Pos2::new(content_left, title_top),
            Align2::LEFT_TOP,
            presentation.title,
            FontId::proportional(21.0),
            TEXT_PRIMARY,
        );

        let detail_position = Pos2::new(content_left, card.top() + 103.0);
        let detail = painter.layout(
            presentation.detail.clone(),
            FontId::proportional(13.0),
            TEXT_SECONDARY,
            content_width,
        );
        let detail_bottom = detail_position.y + detail.size().y;
        painter.galley(detail_position, detail, TEXT_SECONDARY);

        match presentation.kind {
            PanelKind::Ready => {
                let shortcut_top = (detail_bottom + 12.0).min(card.bottom() - 67.0);
                self.paint_shortcut(
                    &painter,
                    Pos2::new(content_left, shortcut_top),
                    content_width,
                    presentation.tone.color(),
                );
            }
            PanelKind::Setup | PanelKind::Error if presentation.action != PrimaryAction::None => {
                let top = (detail_bottom + 12.0).min(card.bottom() - 68.0);
                let button_rect = Rect::from_min_size(
                    Pos2::new(content_left, top),
                    Vec2::new(content_width.min(190.0), 34.0),
                );
                let label = match presentation.action {
                    PrimaryAction::DownloadVoice => "Download voice · 93 MB",
                    PrimaryAction::OpenAccessibility => "Open Accessibility settings",
                    PrimaryAction::None => "",
                };
                let response = ui.put(
                    button_rect,
                    Button::new(RichText::new(label).size(12.5).strong().color(TEXT_PRIMARY))
                        .fill(with_alpha(presentation.tone.color(), 210))
                        .stroke(Stroke::new(1.0, with_alpha(Color32::WHITE, 30)))
                        .corner_radius(10.0),
                );
                if response.clicked() {
                    self.perform_action(presentation.action);
                }
            }
            PanelKind::Busy => {
                let track = Rect::from_min_size(
                    Pos2::new(
                        content_left,
                        (detail_bottom + 17.0).min(card.bottom() - 56.0),
                    ),
                    Vec2::new(content_width.min(190.0), 5.0),
                );
                painter.rect_filled(track, 3.0, Color32::from_white_alpha(16));
                let segment_width = track.width() * 0.34;
                let travel = track.width() - segment_width;
                let progress = ((time * 0.55).fract() * travel).max(0.0);
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(track.left() + progress, track.top()),
                        Vec2::new(segment_width, track.height()),
                    ),
                    3.0,
                    presentation.tone.color(),
                );
            }
            PanelKind::Speaking => {
                let label = "Kokoro 82M  •  af_heart";
                let galley = painter.layout_no_wrap(
                    label.to_owned(),
                    FontId::proportional(10.5),
                    presentation.tone.color(),
                );
                let chip = Rect::from_min_size(
                    Pos2::new(
                        content_left,
                        (detail_bottom + 13.0).min(card.bottom() - 62.0),
                    ),
                    galley.size() + Vec2::new(22.0, 12.0),
                );
                painter.rect_filled(chip, 10.0, with_alpha(presentation.tone.color(), 18));
                painter.galley(
                    Pos2::new(chip.left() + 11.0, chip.top() + 6.0),
                    galley,
                    presentation.tone.color(),
                );
            }
            PanelKind::Error | PanelKind::Setup => {}
        }
    }

    fn paint_shortcut(
        &self,
        painter: &egui::Painter,
        position: Pos2,
        max_width: f32,
        accent: Color32,
    ) {
        let label = self.platform.shortcut_label();
        let galley =
            painter.layout_no_wrap(label.to_owned(), FontId::proportional(12.0), TEXT_PRIMARY);
        let size = Vec2::new((galley.size().x + 28.0).min(max_width), 32.0);
        let chip = Rect::from_min_size(position, size);
        painter.rect_filled(chip, 10.0, Color32::from_white_alpha(10));
        painter.rect_stroke(
            chip,
            10.0,
            Stroke::new(1.0, with_alpha(accent, 70)),
            StrokeKind::Inside,
        );
        painter.circle_filled(Pos2::new(chip.left() + 13.0, chip.center().y), 3.0, accent);
        painter.galley(
            Pos2::new(chip.left() + 22.0, chip.center().y - galley.size().y / 2.0),
            galley,
            TEXT_PRIMARY,
        );
    }

    fn paint_footer(&self, ui: &egui::Ui, card: Rect) {
        let painter = ui.painter();
        let separator_y = card.bottom() - 31.0;
        painter.line_segment(
            [
                Pos2::new(card.left() + 18.0, separator_y),
                Pos2::new(card.right() - 18.0, separator_y),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(15)),
        );
        painter.circle_filled(
            Pos2::new(card.left() + 21.0, card.bottom() - 15.0),
            2.5,
            Tone::Mint.color(),
        );
        painter.text(
            Pos2::new(card.left() + 29.0, card.bottom() - 15.0),
            Align2::LEFT_CENTER,
            "Local speech · private by design",
            FontId::proportional(10.0),
            TEXT_MUTED,
        );
        painter.text(
            Pos2::new(card.right() - 18.0, card.bottom() - 15.0),
            Align2::RIGHT_CENTER,
            "Right-click for options",
            FontId::proportional(10.0),
            TEXT_MUTED,
        );
    }

    fn perform_action(&mut self, action: PrimaryAction) {
        match action {
            PrimaryAction::DownloadVoice => self.enqueue(WorkerCommand::InstallModel),
            PrimaryAction::OpenAccessibility => {
                if let Err(error) = open_accessibility_settings() {
                    self.status = AppStatus::Error(error);
                }
            }
            PrimaryAction::None => {}
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.set_min_width(245.0);
        ui.strong("Select to Speak");
        ui.label(
            RichText::new(self.platform.usage_hint())
                .size(11.5)
                .color(TEXT_SECONDARY),
        );

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            ui.separator();
            if ui.button("Speak copied text").clicked() {
                if let Err(error) = self.platform.request_clipboard_text() {
                    self.status = AppStatus::Error(error);
                }
                ui.close();
            }
            ui.label(RichText::new("Or type text").small().color(TEXT_MUTED));
            ui.add(
                egui::TextEdit::multiline(&mut self.manual_text)
                    .desired_rows(3)
                    .desired_width(220.0),
            );
            if ui.button("Speak typed text").clicked() {
                match select_to_speak::domain::SelectedText::new(&self.manual_text) {
                    Ok(text) => self.enqueue(WorkerCommand::Speak(text)),
                    Err(error) => self.status = AppStatus::Error(error.to_string()),
                }
                ui.close();
            }
        }

        if matches!(self.status, AppStatus::MissingModel | AppStatus::Error(_))
            && ui.button("Download Kokoro voice").clicked()
        {
            self.perform_action(PrimaryAction::DownloadVoice);
            ui.close();
        }

        #[cfg(target_os = "macos")]
        if ui.button("Open Accessibility settings").clicked() {
            self.perform_action(PrimaryAction::OpenAccessibility);
            ui.close();
        }

        if let Some(error) = self.platform.registration_error() {
            ui.separator();
            ui.label(RichText::new(error).size(11.0).color(Tone::Amber.color()));
        }
        ui.separator();
        if ui.button("Quit Select to Speak").clicked() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for PetApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_statuses();
        if let Some(event) = self.platform.poll() {
            match event {
                PlatformEvent::Speak(text) => self.enqueue(WorkerCommand::Speak(text)),
                PlatformEvent::Error(message) => self.status = AppStatus::Error(message),
            }
        }
        let refresh = if matches!(
            self.status,
            AppStatus::CheckingModel
                | AppStatus::Downloading
                | AppStatus::Loading
                | AppStatus::Speaking(_)
        ) {
            33
        } else {
            150
        };
        context.request_repaint_after(Duration::from_millis(refresh));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let available = ui.max_rect();
        let card = Rect::from_min_max(
            available.min + Vec2::new(12.0, 10.0),
            available.max - Vec2::new(12.0, 14.0),
        );
        let card_response = ui.interact(card, ui.id().with("pet-card"), Sense::click());
        let presentation = presentation::for_status(
            &self.status,
            self.platform.registration_error(),
            self.platform.accessibility_required(),
        );
        let time = context.input(|input| input.time as f32);

        self.paint_card(ui, card, &presentation);
        self.paint_header(ui, card, &presentation);
        self.paint_mascot(ui, card, &presentation, time);
        self.paint_body(ui, card, &presentation, time);
        self.paint_footer(ui, card);

        let drag_rect =
            Rect::from_min_max(card.min, Pos2::new(card.right() - 46.0, card.top() + 47.0));
        let drag = ui.interact(drag_rect, ui.id().with("drag-handle"), Sense::drag());
        if drag.drag_started() {
            context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let close_rect = Rect::from_center_size(
            Pos2::new(card.right() - 23.0, card.top() + 23.0),
            Vec2::splat(28.0),
        );
        let close = ui.put(
            close_rect,
            Button::new(RichText::new("×").size(17.0).color(TEXT_MUTED))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .corner_radius(9.0),
        );
        if close.on_hover_text("Quit Select to Speak").clicked() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        card_response.context_menu(|ui| self.context_menu(ui, &context));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
