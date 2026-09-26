mod mist;
mod presentation;
mod system_settings;
mod theme;
mod tray;
mod voice_gallery;

use std::{
    sync::mpsc::{self, TrySendError},
    time::Duration,
};

use eframe::egui::{
    self, Align2, Button, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
    WidgetInfo, WidgetType, text::LayoutJob,
};
use select_to_speak::{
    VOICE_CATALOG, VoiceSettings,
    platform::{PlatformBridge, PlatformEvent},
    voice_profile,
    worker::{AppStatus, WorkerCommand},
};

use self::{
    mist::MistRenderer,
    presentation::{PrimaryAction, copy_for_status, mist_for_status},
    system_settings::open_accessibility_settings,
    theme::{
        PANEL_BACKGROUND, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, configure_interface,
        palette_color, with_alpha,
    },
    tray::{TrayAction, TrayAdapter},
};

const MIST_WINDOW: Vec2 = Vec2::new(248.0, 248.0);
const PANEL_WINDOW: Vec2 = Vec2::new(600.0, 680.0);

pub struct PetApp {
    commands: mpsc::SyncSender<WorkerCommand>,
    statuses: mpsc::Receiver<AppStatus>,
    status: AppStatus,
    platform: PlatformBridge,
    mist: MistRenderer,
    tray: Option<TrayAdapter>,
    tray_error: Option<String>,
    last_platform_error: Option<String>,
    selected_voice: VoiceSettings,
    panel_open: bool,
    panel_visible_last_frame: bool,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    manual_text: String,
}

impl PetApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        commands: mpsc::SyncSender<WorkerCommand>,
        statuses: mpsc::Receiver<AppStatus>,
        selected_voice: VoiceSettings,
    ) -> Self {
        configure_interface(&creation_context.egui_ctx);
        let platform = PlatformBridge::new(commands.clone());
        let mist = MistRenderer::new(&creation_context.egui_ctx)
            .expect("the embedded living-mist texture should decode");
        let (tray, tray_error) = match TrayAdapter::new(&selected_voice) {
            Ok(tray) => (Some(tray), None),
            Err(error) => (None, Some(format!("Menu-bar setup failed: {error:#}"))),
        };
        let last_platform_error = platform.registration_error().map(str::to_owned);
        let panel_open = tray_error.is_some();
        let app = Self {
            commands,
            statuses,
            status: AppStatus::CheckingModel,
            platform,
            mist,
            tray,
            tray_error,
            last_platform_error,
            selected_voice,
            panel_open,
            panel_visible_last_frame: false,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            manual_text: String::new(),
        };
        if let (Some(tray), Some(error)) = (&app.tray, app.last_platform_error.as_deref()) {
            tray.set_warning(error);
        }
        app
    }

    fn drain_statuses(&mut self) {
        while let Ok(status) = self.statuses.try_recv() {
            self.set_status(status);
        }
    }

    fn set_status(&mut self, status: AppStatus) {
        let lifecycle_changed =
            std::mem::discriminant(&self.status) != std::mem::discriminant(&status);
        if matches!(status, AppStatus::MissingModel | AppStatus::Error(_)) {
            self.panel_open = true;
        }
        if lifecycle_changed && let Some(tray) = &self.tray {
            tray.set_status(&status);
            if matches!(status, AppStatus::Ready)
                && let Some(error) = self.platform.registration_error()
            {
                tray.set_warning(error);
            }
        }
        self.status = status;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.set_status(AppStatus::Error(message.into()));
    }

    fn sync_platform_status(&mut self) {
        let current = self.platform.registration_error().map(str::to_owned);
        if current == self.last_platform_error {
            return;
        }
        self.last_platform_error.clone_from(&current);
        if let Some(tray) = &self.tray {
            if let Some(error) = current {
                tray.set_warning(&error);
            } else {
                tray.set_status(&self.status);
            }
        }
    }

    fn enqueue(&mut self, command: WorkerCommand) -> bool {
        if let Err(error) = self.commands.try_send(command) {
            let message = match error {
                TrySendError::Full(_) => "The speech queue is full. Try again in a moment.",
                TrySendError::Disconnected(_) => "The speech worker stopped unexpectedly.",
            };
            self.set_error(message);
            false
        } else {
            true
        }
    }

    fn select_voice(&mut self, voice_id: &str) {
        let Some(settings) = VoiceSettings::from_voice_id(voice_id) else {
            self.set_error(format!("Unknown Kokoro voice: {voice_id}"));
            return;
        };
        if self.enqueue(WorkerCommand::SelectVoice(settings.clone())) {
            self.selected_voice = settings;
            if let Some(tray) = &self.tray {
                tray.select_voice(voice_id);
            }
        }
    }

    fn perform_action(&mut self, action: PrimaryAction) {
        match action {
            PrimaryAction::InstallVoices => {
                self.enqueue(WorkerCommand::InstallModel);
            }
            PrimaryAction::OpenAccessibility => {
                if let Err(error) = open_accessibility_settings() {
                    self.set_error(error);
                }
            }
            PrimaryAction::None => {}
        }
    }

    fn handle_tray(&mut self, context: &egui::Context) {
        let action = self.tray.as_ref().and_then(TrayAdapter::poll);
        match action {
            Some(TrayAction::OpenSettings) => {
                self.panel_open = true;
                context.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            Some(TrayAction::SelectVoice(voice)) => self.select_voice(&voice),
            Some(TrayAction::OpenAccessibility) => {
                self.perform_action(PrimaryAction::OpenAccessibility)
            }
            Some(TrayAction::Quit) => context.send_viewport_cmd(egui::ViewportCommand::Close),
            None => {}
        }
    }

    fn sync_window_size(&mut self, context: &egui::Context, panel_visible: bool) {
        if panel_visible == self.panel_visible_last_frame {
            return;
        }
        self.panel_visible_last_frame = panel_visible;
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(if panel_visible {
            PANEL_WINDOW
        } else {
            MIST_WINDOW
        }));
    }

    fn paint_mist_only(
        &mut self,
        ui: &mut egui::Ui,
        time: f32,
        presentation: presentation::MistPresentation,
    ) {
        let context = ui.ctx().clone();
        let rect = ui.max_rect().shrink(7.0);
        let response = ui.interact(rect, ui.id().with("living-mist"), Sense::click_and_drag());
        let selected = self.selected_voice.voice_id.as_str();
        let profile = voice_profile(selected).unwrap_or(&VOICE_CATALOG[0]);
        let seed = selected_voice_index(selected) as f32 * 0.83;
        self.mist
            .paint(ui, rect, time, presentation, profile.palette, seed);

        if response.drag_started() {
            context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        response.context_menu(|ui| self.context_menu(ui, &context));
    }

    fn paint_panel(
        &mut self,
        ui: &mut egui::Ui,
        time: f32,
        presentation: presentation::MistPresentation,
    ) {
        let context = ui.ctx().clone();
        let outer = ui.max_rect().shrink(12.0);
        let painter = ui.painter().clone();
        painter.rect_filled(outer, 30.0, PANEL_BACKGROUND);
        painter.rect_stroke(
            outer,
            30.0,
            Stroke::new(1.0, Color32::from_white_alpha(28)),
            StrokeKind::Inside,
        );

        let selected_voice = self.selected_voice.voice_id.as_str();
        let profile = voice_profile(selected_voice).unwrap_or(&VOICE_CATALOG[0]);
        let accent = palette_color(profile.palette.primary);
        let preview = Rect::from_min_size(outer.min + Vec2::new(18.0, 16.0), Vec2::splat(142.0));
        self.mist.paint(
            ui,
            preview,
            time,
            presentation,
            profile.palette,
            selected_voice_index(selected_voice) as f32 * 0.83,
        );

        let copy = copy_for_status(
            &self.status,
            self.platform.accessibility_required(),
            self.platform.registration_error(),
            self.tray_error.as_deref(),
        );
        let status_semantics = ui.interact(
            Rect::from_min_max(
                Pos2::new(outer.left() + 166.0, outer.top() + 24.0),
                Pos2::new(outer.right() - 24.0, outer.top() + 148.0),
            ),
            ui.id().with("status-description"),
            Sense::hover(),
        );
        status_semantics.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Label,
                true,
                format!("{}. {}. {}", copy.eyebrow, copy.title, copy.detail),
            )
        });
        let copy_left = outer.left() + 170.0;
        painter.text(
            Pos2::new(copy_left, outer.top() + 31.0),
            Align2::LEFT_TOP,
            copy.eyebrow,
            FontId::proportional(10.5),
            with_alpha(accent, 225),
        );
        painter.text(
            Pos2::new(copy_left, outer.top() + 51.0),
            Align2::LEFT_TOP,
            copy.title,
            FontId::proportional(25.0),
            TEXT_PRIMARY,
        );
        let mut detail = LayoutJob::simple(
            copy.detail.clone(),
            FontId::proportional(13.0),
            TEXT_SECONDARY,
            outer.right() - 26.0 - copy_left,
        );
        detail.wrap.max_rows = 3;
        detail.wrap.overflow_character = Some('…');
        let detail = painter.layout_job(detail);
        painter.galley(
            Pos2::new(copy_left, outer.top() + 89.0),
            detail,
            TEXT_SECONDARY,
        );

        let close_rect = Rect::from_center_size(
            Pos2::new(outer.right() - 24.0, outer.top() + 24.0),
            Vec2::splat(30.0),
        );
        if ui
            .put(
                close_rect,
                Button::new(RichText::new("×").size(18.0).color(TEXT_MUTED))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .corner_radius(15.0),
            )
            .on_hover_text("Return to the floating mist")
            .clicked()
        {
            self.panel_open = false;
        }

        let drag_rect = Rect::from_min_max(
            Pos2::new(outer.left() + 10.0, outer.top() + 8.0),
            Pos2::new(outer.right() - 48.0, outer.top() + 158.0),
        );
        if ui
            .interact(drag_rect, ui.id().with("settings-drag"), Sense::drag())
            .drag_started()
        {
            context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let gallery = voice_gallery::show(
            ui,
            &self.mist,
            outer,
            time,
            self.selected_voice.voice_id.as_str(),
        );
        if let Some(voice) = gallery.selected_voice {
            self.select_voice(voice);
        }

        let footer_top = gallery.bottom;
        painter.line_segment(
            [
                Pos2::new(outer.left() + 25.0, footer_top),
                Pos2::new(outer.right() - 25.0, footer_top),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(16)),
        );
        painter.text(
            Pos2::new(outer.left() + 27.0, footer_top + 18.0),
            Align2::LEFT_TOP,
            "Private by design",
            FontId::proportional(12.0),
            TEXT_PRIMARY,
        );
        painter.text(
            Pos2::new(outer.left() + 27.0, footer_top + 38.0),
            Align2::LEFT_TOP,
            "Selected text and speech never leave this device.",
            FontId::proportional(10.5),
            TEXT_MUTED,
        );

        let button = Rect::from_min_size(
            Pos2::new(outer.right() - 218.0, footer_top + 17.0),
            Vec2::new(191.0, 44.0),
        );
        let (label, action, enabled) = match (&self.status, copy.action) {
            (AppStatus::Downloading, _) => ("Gathering voices…", PrimaryAction::None, false),
            (_, PrimaryAction::InstallVoices) => (
                "Download voices · ~96 MB",
                PrimaryAction::InstallVoices,
                true,
            ),
            (_, PrimaryAction::OpenAccessibility) => {
                ("Open Accessibility", PrimaryAction::OpenAccessibility, true)
            }
            _ => ("Let it float", PrimaryAction::None, true),
        };
        let response = ui.put(
            button,
            Button::new(RichText::new(label).size(12.0).strong().color(TEXT_PRIMARY))
                .fill(if enabled {
                    with_alpha(accent, 190)
                } else {
                    Color32::from_white_alpha(10)
                })
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(25)))
                .corner_radius(14.0),
        );
        if enabled && response.clicked() {
            if action == PrimaryAction::None {
                self.panel_open = false;
            } else {
                self.perform_action(action);
            }
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.set_min_width(238.0);
        let profile =
            voice_profile(self.selected_voice.voice_id.as_str()).unwrap_or(&VOICE_CATALOG[0]);
        ui.label(RichText::new("SELECT TO SPEAK").small().color(TEXT_MUTED));
        ui.strong(format!("{} mist", profile.display_name));
        ui.label(
            RichText::new(self.platform.usage_hint())
                .size(11.0)
                .color(TEXT_SECONDARY),
        );
        ui.separator();
        if ui.button("Voice settings…").clicked() {
            self.panel_open = true;
            ui.close();
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            if ui.button("Speak copied text").clicked() {
                if let Err(error) = self.platform.request_clipboard_text() {
                    self.set_error(error);
                }
                ui.close();
            }
            ui.collapsing("Speak typed text", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.manual_text)
                        .desired_rows(3)
                        .desired_width(210.0),
                );
                if ui.button("Speak").clicked() {
                    match select_to_speak::SelectedText::new(&self.manual_text) {
                        Ok(text) => {
                            self.enqueue(WorkerCommand::Speak(text));
                        }
                        Err(error) => self.set_error(error.to_string()),
                    }
                    ui.close();
                }
            });
        }

        #[cfg(target_os = "macos")]
        if ui.button("Accessibility settings…").clicked() {
            self.perform_action(PrimaryAction::OpenAccessibility);
            ui.close();
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
        self.handle_tray(context);
        if let Some(event) = self.platform.poll() {
            match event {
                PlatformEvent::Speak(text) => {
                    self.enqueue(WorkerCommand::Speak(text));
                }
                PlatformEvent::AccessibilityPermissionRequired => self.panel_open = true,
                PlatformEvent::Error(message) => {
                    self.set_error(message);
                }
            }
        }
        self.sync_platform_status();
        let refresh = match self.status {
            AppStatus::Speaking { .. } => 16,
            AppStatus::CheckingModel
            | AppStatus::Downloading
            | AppStatus::Loading
            | AppStatus::Synthesizing { .. } => 33,
            _ => 90,
        };
        context.request_repaint_after(Duration::from_millis(refresh));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let shell_error = self
            .tray_error
            .as_deref()
            .or_else(|| self.platform.registration_error());
        let presentation = mist_for_status(
            &self.status,
            self.platform.accessibility_required(),
            shell_error,
        );
        let panel_visible = self.panel_open || presentation.requires_panel;
        self.sync_window_size(&context, panel_visible);
        let time = context.input(|input| input.time as f32);
        if panel_visible {
            self.paint_panel(ui, time, presentation);
        } else {
            self.paint_mist_only(ui, time, presentation);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

fn selected_voice_index(selected: &str) -> usize {
    VOICE_CATALOG
        .iter()
        .position(|voice| voice.id == selected)
        .unwrap_or(0)
}
