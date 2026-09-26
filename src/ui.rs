use std::{
    sync::mpsc::{self, TrySendError},
    time::Duration,
};

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use select_to_speak::{
    platform::{PlatformBridge, PlatformEvent},
    worker::{AppStatus, WorkerCommand},
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
        creation_context.egui_ctx.set_pixels_per_point(1.1);
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
                TrySendError::Full(_) => "The speech queue is full; try again in a moment",
                TrySendError::Disconnected(_) => "The speech worker stopped unexpectedly",
            };
            self.status = AppStatus::Error(message.to_owned());
        }
    }

    fn status_text(&self) -> &str {
        match &self.status {
            AppStatus::CheckingModel => "Checking Kokoro voice…",
            AppStatus::MissingModel => "Kokoro needs a nap-sized download",
            AppStatus::Ready => self.platform.usage_hint(),
            AppStatus::Downloading => "Fetching my voice…",
            AppStatus::Loading => "Warming up Kokoro…",
            AppStatus::Speaking(text) => text,
            AppStatus::Error(message) => message,
        }
    }

    fn pet_color(&self) -> Color32 {
        match self.status {
            AppStatus::Error(_) => Color32::from_rgb(235, 115, 112),
            AppStatus::Speaking(_) => Color32::from_rgb(89, 198, 165),
            AppStatus::CheckingModel | AppStatus::Downloading | AppStatus::Loading => {
                Color32::from_rgb(247, 190, 84)
            }
            _ => Color32::from_rgb(112, 158, 235),
        }
    }

    fn draw_pet(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter();
        let center = Pos2::new(rect.center().x, rect.top() + 68.0);
        let color = self.pet_color();
        let dark = Color32::from_rgb(33, 42, 58);
        let cream = Color32::from_rgb(255, 248, 224);

        let left_ear = vec![
            Pos2::new(center.x - 42.0, center.y - 25.0),
            Pos2::new(center.x - 31.0, center.y - 58.0),
            Pos2::new(center.x - 12.0, center.y - 33.0),
        ];
        let right_ear = vec![
            Pos2::new(center.x + 42.0, center.y - 25.0),
            Pos2::new(center.x + 31.0, center.y - 58.0),
            Pos2::new(center.x + 12.0, center.y - 33.0),
        ];
        painter.add(Shape::convex_polygon(left_ear, color, Stroke::NONE));
        painter.add(Shape::convex_polygon(right_ear, color, Stroke::NONE));
        painter.circle_filled(center, 50.0, color);
        painter.circle_filled(Pos2::new(center.x, center.y + 11.0), 28.0, cream);

        let blink = matches!(
            self.status,
            AppStatus::CheckingModel | AppStatus::Loading | AppStatus::Downloading
        );
        if blink {
            painter.line_segment(
                [
                    Pos2::new(center.x - 23.0, center.y - 7.0),
                    Pos2::new(center.x - 11.0, center.y - 7.0),
                ],
                Stroke::new(3.0, dark),
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 11.0, center.y - 7.0),
                    Pos2::new(center.x + 23.0, center.y - 7.0),
                ],
                Stroke::new(3.0, dark),
            );
        } else {
            painter.circle_filled(Pos2::new(center.x - 17.0, center.y - 7.0), 3.8, dark);
            painter.circle_filled(Pos2::new(center.x + 17.0, center.y - 7.0), 3.8, dark);
        }

        painter.add(Shape::convex_polygon(
            vec![
                Pos2::new(center.x - 5.0, center.y + 7.0),
                Pos2::new(center.x + 5.0, center.y + 7.0),
                Pos2::new(center.x, center.y + 13.0),
            ],
            dark,
            Stroke::NONE,
        ));

        if matches!(self.status, AppStatus::Speaking(_)) {
            for radius in [10.0, 17.0, 24.0] {
                let wave_center = Pos2::new(center.x + 44.0, center.y + 10.0);
                painter.circle_stroke(wave_center, radius, Stroke::new(2.0, cream));
            }
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(center.x + 38.0, center.y - 18.0),
                    Pos2::new(center.x + 70.0, center.y + 38.0),
                ),
                0.0,
                Color32::TRANSPARENT,
            );
        }

        let galley = painter.layout(
            self.status_text().to_owned(),
            FontId::proportional(12.0),
            Color32::WHITE,
            rect.width() - 18.0,
        );
        let galley_position = Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - 35.0 - galley.size().y / 2.0,
        );
        painter.galley(galley_position, galley, Color32::WHITE);
    }
}

impl eframe::App for PetApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_statuses();
        if let Some(event) = self.platform.poll() {
            match event {
                PlatformEvent::Speak(text) => {
                    self.enqueue(WorkerCommand::Speak(text));
                }
                PlatformEvent::Error(message) => self.status = AppStatus::Error(message),
            }
        }
        context.request_repaint_after(Duration::from_millis(150));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let available = ui.max_rect();
        let response = ui.allocate_rect(available, Sense::click_and_drag());
        if response.drag_started() {
            context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        self.draw_pet(ui, available);

        response.context_menu(|ui| {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                if ui.button("Speak copied text").clicked() {
                    if let Err(error) = self.platform.request_clipboard_text() {
                        self.status = AppStatus::Error(error);
                    }
                    ui.close();
                }
                ui.label("Or type text:");
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
                && ui.button("Download Kokoro voice (~93 MB)").clicked()
            {
                self.enqueue(WorkerCommand::InstallModel);
                ui.close();
            }
            ui.label(self.platform.usage_hint());
            if let Some(error) = self.platform.registration_error() {
                ui.colored_label(Color32::from_rgb(247, 190, 84), error);
            }
            ui.separator();
            if ui.button("Quit Select to Speak").clicked() {
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        if matches!(self.status, AppStatus::MissingModel) {
            let button_rect = Rect::from_center_size(
                Pos2::new(available.center().x, available.bottom() - 13.0),
                Vec2::new(112.0, 26.0),
            );
            let button = ui.put(button_rect, egui::Button::new("Download voice"));
            if button.clicked() {
                self.enqueue(WorkerCommand::InstallModel);
            }
        }

        ui.painter().rect_stroke(
            available.shrink(1.0),
            22.0,
            Stroke::new(1.0, Color32::from_white_alpha(24)),
            StrokeKind::Inside,
        );
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
