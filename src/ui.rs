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

use ::mist::{
    VOICE_CATALOG, VoiceSettings,
    platform::{PlatformBridge, PlatformEvent},
    voice_profile,
    worker::{AppStatus, WorkerCommand},
};
use eframe::egui::{
    self, Align2, Button, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
    WidgetInfo, WidgetType, text::LayoutJob,
};

use self::{
    mist::MistRenderer,
    presentation::{MistSmoother, PrimaryAction, copy_for_status, mist_for_status},
    system_settings::open_accessibility_settings,
    theme::{
        PANEL_BACKGROUND, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, configure_interface,
        palette_color, with_alpha,
    },
    tray::{TrayAction, TrayAdapter},
};

pub(crate) const MIST_WINDOW: Vec2 = Vec2::new(164.0, 164.0);
const CONTEXT_MENU_WINDOW: Vec2 = Vec2::new(280.0, 420.0);
const PANEL_WINDOW: Vec2 = Vec2::new(600.0, 680.0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewportMode {
    Mist,
    ContextMenu,
    Panel,
}

impl ViewportMode {
    fn size(self) -> Vec2 {
        match self {
            Self::Mist => MIST_WINDOW,
            Self::ContextMenu => CONTEXT_MENU_WINDOW,
            Self::Panel => PANEL_WINDOW,
        }
    }
}

pub struct PetApp {
    commands: mpsc::SyncSender<WorkerCommand>,
    statuses: mpsc::Receiver<AppStatus>,
    status: AppStatus,
    platform: PlatformBridge,
    mist: MistRenderer,
    mist_smoother: MistSmoother,
    tray: Option<TrayAdapter>,
    tray_error: Option<String>,
    last_platform_error: Option<String>,
    selected_voice: VoiceSettings,
    voices_ready: bool,
    voice_preview: VoicePreviewActivity,
    panel_open: bool,
    viewport_mode: ViewportMode,
    mist_screen_center: Option<Pos2>,
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
            mist_smoother: MistSmoother::default(),
            tray,
            tray_error,
            last_platform_error,
            selected_voice,
            voices_ready: false,
            voice_preview: VoicePreviewActivity::Idle,
            panel_open,
            viewport_mode: ViewportMode::Mist,
            mist_screen_center: None,
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
        match status {
            AppStatus::Ready => self.voices_ready = true,
            AppStatus::MissingModel | AppStatus::Downloading => self.voices_ready = false,
            _ => {}
        }
        self.voice_preview = self.voice_preview.after_status(&status);
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
        self.enqueue_voice_command(settings.clone(), WorkerCommand::SelectVoice(settings));
    }

    fn preview_voice(&mut self, voice_id: &str) {
        if self.voice_preview != VoicePreviewActivity::Idle || !self.voices_ready {
            return;
        }
        let (settings, command) = match voice_preview_command(voice_id) {
            Ok(command) => command,
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        if self.enqueue_voice_command(settings, command) {
            self.voice_preview = VoicePreviewActivity::Pending;
        }
    }

    fn enqueue_voice_command(&mut self, settings: VoiceSettings, command: WorkerCommand) -> bool {
        let queued = self.enqueue(command);
        let authoritative =
            authoritative_voice_after_enqueue(&self.selected_voice, &settings, queued);
        if let Some(tray) = &self.tray {
            tray.select_voice(authoritative.voice_id.as_str());
        }
        if !queued {
            return false;
        }
        self.selected_voice = settings;
        true
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
            Some(TrayAction::SelectVoice(voice)) => {
                if voice_preview_available(&self.status, self.voices_ready, self.voice_preview) {
                    self.preview_voice(&voice);
                } else {
                    self.select_voice(&voice);
                }
            }
            Some(TrayAction::OpenAccessibility) => {
                self.perform_action(PrimaryAction::OpenAccessibility)
            }
            Some(TrayAction::Quit) => context.send_viewport_cmd(egui::ViewportCommand::Close),
            None => {}
        }
    }

    fn sync_window_size(&mut self, context: &egui::Context, mode: ViewportMode) {
        if mode == self.viewport_mode {
            return;
        }
        let outer_rect = context.input(|input| input.viewport().outer_rect);
        if self.mist_screen_center.is_none() {
            self.mist_screen_center = outer_rect.map(|rect| rect.center());
        }
        if let Some(center) = self.mist_screen_center {
            let position = viewport_origin_for_center(center, mode.size());
            context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        }
        self.viewport_mode = mode;
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(mode.size()));
    }

    fn mist_rect(&mut self, ui: &egui::Ui) -> Rect {
        let outer_rect = ui.ctx().input(|input| input.viewport().outer_rect);
        if self.viewport_mode == ViewportMode::Mist
            && outer_rect.is_some_and(|rect| {
                (rect.width() - MIST_WINDOW.x).abs() <= 2.0
                    && (rect.height() - MIST_WINDOW.y).abs() <= 2.0
            })
        {
            self.mist_screen_center = None;
        }
        let center = match (self.mist_screen_center, outer_rect) {
            (Some(screen_center), Some(viewport)) => screen_center - viewport.min.to_vec2(),
            (None, _) if self.viewport_mode != ViewportMode::Mist => {
                ui.max_rect().min + MIST_WINDOW * 0.5
            }
            _ => ui.max_rect().center(),
        };
        Rect::from_center_size(center, MIST_WINDOW).shrink(2.0)
    }

    fn paint_mist_only(
        &mut self,
        ui: &mut egui::Ui,
        time: f32,
        presentation: presentation::MistPresentation,
    ) -> bool {
        let context = ui.ctx().clone();
        let rect = self.mist_rect(ui);
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
        response.context_menu_opened()
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

        let gallery_mode = gallery_mode(&self.status, self.voices_ready, self.voice_preview);
        let gallery = voice_gallery::show(
            ui,
            &self.mist,
            outer,
            time,
            self.selected_voice.voice_id.as_str(),
            gallery_mode,
        );
        if let Some(voice) = gallery.preview_voice {
            self.preview_voice(voice);
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
        ui.label(RichText::new("MIST").small().color(TEXT_MUTED));
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
                    match ::mist::SelectedText::new(&self.manual_text) {
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
        if ui.button("Quit Mist").clicked() {
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
            AppStatus::Ready | AppStatus::Speaking { .. } => 16,
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
        let time = context.input(|input| input.time as f32);
        let presentation = self.mist_smoother.update(presentation, time);
        let context_menu_visible = if panel_visible {
            self.paint_panel(ui, time, presentation);
            false
        } else {
            self.paint_mist_only(ui, time, presentation)
        };
        let mode = viewport_mode(
            self.panel_open || presentation.requires_panel,
            context_menu_visible,
        );
        self.sync_window_size(&context, mode);
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

fn viewport_mode(panel_visible: bool, context_menu_visible: bool) -> ViewportMode {
    if panel_visible {
        ViewportMode::Panel
    } else if context_menu_visible {
        ViewportMode::ContextMenu
    } else {
        ViewportMode::Mist
    }
}

fn viewport_origin_for_center(center: Pos2, size: Vec2) -> Pos2 {
    center - size * 0.5
}

fn authoritative_voice_after_enqueue<'a>(
    current: &'a VoiceSettings,
    requested: &'a VoiceSettings,
    queued: bool,
) -> &'a VoiceSettings {
    if queued { requested } else { current }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VoicePreviewActivity {
    Idle,
    Pending,
    Active,
}

impl VoicePreviewActivity {
    fn after_status(self, status: &AppStatus) -> Self {
        match (self, status) {
            (
                Self::Pending,
                AppStatus::Loading | AppStatus::Synthesizing { .. } | AppStatus::Speaking { .. },
            ) => Self::Active,
            (Self::Active, AppStatus::Ready | AppStatus::MissingModel | AppStatus::Error(_))
            | (Self::Pending, AppStatus::MissingModel | AppStatus::Error(_)) => Self::Idle,
            (activity, _) => activity,
        }
    }
}

fn gallery_mode(
    status: &AppStatus,
    voices_ready: bool,
    preview: VoicePreviewActivity,
) -> voice_gallery::GalleryMode {
    if voice_preview_available(status, voices_ready, preview) {
        return voice_gallery::GalleryMode::Available;
    }
    if preview != VoicePreviewActivity::Idle {
        return voice_gallery::GalleryMode::Busy;
    }
    match status {
        AppStatus::CheckingModel => voice_gallery::GalleryMode::Checking,
        AppStatus::MissingModel | AppStatus::Downloading => {
            voice_gallery::GalleryMode::DownloadRequired
        }
        AppStatus::Loading | AppStatus::Synthesizing { .. } | AppStatus::Speaking { .. } => {
            voice_gallery::GalleryMode::Busy
        }
        AppStatus::Ready | AppStatus::Error(_) if voices_ready => voice_gallery::GalleryMode::Busy,
        AppStatus::Ready | AppStatus::Error(_) => voice_gallery::GalleryMode::DownloadRequired,
    }
}

fn voice_preview_available(
    status: &AppStatus,
    voices_ready: bool,
    preview: VoicePreviewActivity,
) -> bool {
    voices_ready && preview == VoicePreviewActivity::Idle && matches!(status, AppStatus::Ready)
}

fn voice_preview_command(voice_id: &str) -> Result<(VoiceSettings, WorkerCommand), String> {
    let settings = VoiceSettings::from_voice_id(voice_id)
        .ok_or_else(|| format!("Unknown Kokoro voice: {voice_id}"))?;
    Ok((settings.clone(), WorkerCommand::PreviewVoice(settings)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_card_action_dispatches_preview_for_the_exact_voice() {
        let (settings, command) = voice_preview_command("bf_emma").unwrap();

        assert_eq!(settings.voice_id.as_str(), "bf_emma");
        let WorkerCommand::PreviewVoice(preview) = command else {
            panic!("voice cards must dispatch the preview path");
        };
        assert_eq!(preview.voice_id.as_str(), "bf_emma");
    }

    #[test]
    fn pending_preview_disables_cards_until_its_terminal_status() {
        let pending = VoicePreviewActivity::Pending;

        assert_eq!(
            gallery_mode(&AppStatus::Ready, true, pending),
            voice_gallery::GalleryMode::Busy
        );
        assert_eq!(pending.after_status(&AppStatus::Ready), pending);
        let active = pending.after_status(&AppStatus::Synthesizing {
            text: "preview".to_owned(),
            inference_policy: "CPU".to_owned(),
        });
        assert_eq!(active, VoicePreviewActivity::Active);
        assert_eq!(
            active.after_status(&AppStatus::Ready),
            VoicePreviewActivity::Idle
        );
    }

    #[test]
    fn voice_cards_require_downloaded_model_artifacts() {
        assert_eq!(
            gallery_mode(&AppStatus::MissingModel, false, VoicePreviewActivity::Idle,),
            voice_gallery::GalleryMode::DownloadRequired
        );
    }

    #[test]
    fn tray_voice_selection_does_not_queue_preview_while_speaking() {
        let speaking = AppStatus::Speaking {
            text: "Already speaking".to_owned(),
            inference_policy: "CoreML → CPU".to_owned(),
            features: ::mist::AudioFeatures {
                energy: 120,
                brightness: 96,
            },
        };

        assert!(!voice_preview_available(
            &speaking,
            true,
            VoicePreviewActivity::Idle
        ));
        assert!(voice_preview_available(
            &AppStatus::Ready,
            true,
            VoicePreviewActivity::Idle
        ));
    }

    #[test]
    fn failed_voice_enqueue_restores_the_authoritative_tray_selection() {
        let current = VoiceSettings::from_voice_id("af_heart").unwrap();
        let requested = VoiceSettings::from_voice_id("af_bella").unwrap();

        assert_eq!(
            authoritative_voice_after_enqueue(&current, &requested, false)
                .voice_id
                .as_str(),
            "af_heart"
        );
        assert_eq!(
            authoritative_voice_after_enqueue(&current, &requested, true)
                .voice_id
                .as_str(),
            "af_bella"
        );
    }

    #[test]
    fn settings_panel_wins_over_context_menu_viewport_size() {
        assert_eq!(viewport_mode(true, true), ViewportMode::Panel);
        assert_eq!(viewport_mode(false, true), ViewportMode::ContextMenu);
        assert_eq!(viewport_mode(false, false), ViewportMode::Mist);
    }

    #[test]
    fn expanded_viewport_preserves_center_on_secondary_monitors() {
        assert_eq!(
            viewport_origin_for_center(Pos2::new(-300.0, -200.0), CONTEXT_MENU_WINDOW),
            Pos2::new(-440.0, -410.0)
        );
    }
}
