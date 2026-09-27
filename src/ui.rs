mod mist;
mod presentation;
mod queue_tray;
mod settings;
mod system_settings;
mod theme;
mod tray;
mod voice_gallery;

use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
    time::Duration,
};

use ::mist::{
    InferenceProviderId, PlaybackController, PlaybackPhase, PlaybackPreferences, PlaybackToken,
    ProviderCapability, QueueItemId, QueueItemState, SpeechQueue, VoiceSettings,
    adapters::{
        clipboard_fallback::ClipboardLease, model_preferences::ModelPreferencesStore,
        playback_preferences::PlaybackPreferencesStore,
    },
    platform::{CapturedSelection, PlatformBridge, PlatformEvent},
    ports::VoiceCatalog,
    worker::{
        AppStatus, PreviewDispatcher, WorkerCommand, WorkerCommands, WorkerHandle, WorkerSendError,
    },
};
use eframe::egui::{
    self, Align2, Button, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
    WidgetInfo, WidgetType, text::LayoutJob,
};

use self::{
    mist::MistRenderer,
    presentation::{MistSmoother, PrimaryAction, copy_for_status, mist_for_status},
    queue_tray::{QueueAction, QueueTrayResponse},
    settings::{SettingsAction, SettingsSection},
    system_settings::open_accessibility_settings,
    theme::{
        PANEL_BACKGROUND, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, configure_interface,
        palette_color, with_alpha,
    },
    tray::{TrayAction, TrayAdapter},
};

pub(crate) const MIST_WINDOW: Vec2 = Vec2::new(164.0, 164.0);
const SPEAKING_MIST_WINDOW: Vec2 = Vec2::new(232.0, 232.0);
const QUEUE_WINDOW_WIDTH: f32 = 282.0;
const CONTEXT_MENU_WINDOW: Vec2 = Vec2::new(280.0, 420.0);
const PANEL_WINDOW: Vec2 = Vec2::new(860.0, 760.0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewportMode {
    Mist,
    Speaking,
    ContextMenu,
    Panel,
}

impl ViewportMode {
    fn size(self) -> Vec2 {
        match self {
            Self::Mist => MIST_WINDOW,
            Self::Speaking => SPEAKING_MIST_WINDOW,
            Self::ContextMenu => CONTEXT_MENU_WINDOW,
            Self::Panel => PANEL_WINDOW,
        }
    }

    fn mist_center(self) -> Vec2 {
        self.size() * 0.5
    }
}

pub struct PetApp {
    commands: WorkerCommands,
    statuses: mpsc::Receiver<AppStatus>,
    playback: PlaybackController,
    previews: PreviewDispatcher,
    status: AppStatus,
    platform: PlatformBridge,
    mist: MistRenderer,
    mist_smoother: MistSmoother,
    tray: Option<TrayAdapter>,
    tray_error: Option<String>,
    last_platform_error: Option<String>,
    selected_voice: VoiceSettings,
    voice_catalog: Arc<dyn VoiceCatalog>,
    voices_ready: bool,
    voice_preview: VoicePreviewActivity,
    panel_open: bool,
    viewport_mode: ViewportMode,
    mist_screen_center: Option<Pos2>,
    queue_screen_position: Option<Pos2>,
    speech_queue: SpeechQueue,
    clipboard_leases: HashMap<QueueItemId, ClipboardLease>,
    deferred_clipboard_cleanup: Vec<ClipboardLease>,
    queue_playback_started: bool,
    playback_preferences: PlaybackPreferences,
    playback_preferences_store: PlaybackPreferencesStore,
    settings_section: SettingsSection,
    model_provider: InferenceProviderId,
    active_model_provider: InferenceProviderId,
    model_preferences_store: ModelPreferencesStore,
    provider_capabilities: Vec<ProviderCapability>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    manual_text: String,
}

pub(crate) struct AppSettings {
    pub(crate) playback: PlaybackPreferences,
    pub(crate) playback_store: PlaybackPreferencesStore,
    pub(crate) model_provider: InferenceProviderId,
    pub(crate) model_store: ModelPreferencesStore,
    pub(crate) provider_capabilities: Vec<ProviderCapability>,
    pub(crate) open_panel: bool,
}

impl PetApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        worker: WorkerHandle,
        voice_catalog: Arc<dyn VoiceCatalog>,
        settings: AppSettings,
    ) -> Self {
        let WorkerHandle {
            commands,
            statuses,
            selected_voice,
            playback,
            previews,
        } = worker;
        let AppSettings {
            playback: playback_preferences,
            playback_store: playback_preferences_store,
            model_provider,
            model_store: model_preferences_store,
            provider_capabilities,
            open_panel,
        } = settings;
        configure_interface(&creation_context.egui_ctx);
        let mut platform = PlatformBridge::new();
        platform
            .set_automatic_clipboard_fallback(playback_preferences.automatic_clipboard_fallback);
        let mist = MistRenderer::new(&creation_context.egui_ctx)
            .expect("the embedded living-mist texture should decode");
        let (tray, tray_error) = match TrayAdapter::new(&selected_voice, voice_catalog.voices()) {
            Ok(tray) => (Some(tray), None),
            Err(error) => (None, Some(format!("Menu-bar setup failed: {error:#}"))),
        };
        let last_platform_error = platform.registration_error().map(str::to_owned);
        let panel_open = open_panel || tray_error.is_some();
        let app = Self {
            commands,
            statuses,
            playback,
            previews,
            status: AppStatus::CheckingModel,
            platform,
            mist,
            mist_smoother: MistSmoother::default(),
            tray,
            tray_error,
            last_platform_error,
            selected_voice,
            voice_catalog,
            voices_ready: false,
            voice_preview: VoicePreviewActivity::Idle,
            panel_open,
            viewport_mode: ViewportMode::Mist,
            mist_screen_center: None,
            queue_screen_position: None,
            speech_queue: SpeechQueue::default(),
            clipboard_leases: HashMap::new(),
            deferred_clipboard_cleanup: Vec::new(),
            queue_playback_started: false,
            playback_preferences,
            playback_preferences_store,
            settings_section: SettingsSection::default(),
            model_provider: model_provider.clone(),
            active_model_provider: model_provider,
            model_preferences_store,
            provider_capabilities,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            manual_text: String::new(),
        };
        if let (Some(tray), Some(error)) = (&app.tray, app.last_platform_error.as_deref()) {
            tray.set_warning(error);
        }
        app
    }

    fn drain_statuses(&mut self) {
        let mut cleanup_error = None;
        let mut retry_cleanup = false;
        while let Ok(status) = self.statuses.try_recv() {
            retry_cleanup |= matches!(status, AppStatus::Ready | AppStatus::Error(_));
            if self.speech_queue.active_id().is_some()
                && matches!(
                    status,
                    AppStatus::Loading
                        | AppStatus::Synthesizing { .. }
                        | AppStatus::Speaking { .. }
                )
            {
                self.queue_playback_started = true;
            }
            if matches!(status, AppStatus::Speaking { .. })
                && let Some(active) = self.speech_queue.active_id()
            {
                self.speech_queue.mark_playing(active);
            }
            if self.queue_playback_started {
                match status {
                    AppStatus::Ready => {
                        cleanup_error = self.complete_active_queue();
                    }
                    AppStatus::Error(_) => {
                        if let Some(active) = self.speech_queue.active_id() {
                            self.speech_queue.fail(active);
                            cleanup_error = self.clear_clipboard_lease(active);
                        }
                        self.queue_playback_started = false;
                    }
                    _ => {}
                }
            }
            self.set_status(status);
        }
        if let Some(error) = cleanup_error {
            self.set_error(error);
        } else if retry_cleanup {
            self.retry_deferred_clipboard_cleanup();
        }
        self.maybe_start_automatic_queue();
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
        if status_opens_panel(&status) {
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
                WorkerSendError::Full => "The speech queue is full. Try again in a moment.",
                WorkerSendError::Disconnected => "The speech worker stopped unexpectedly.",
            };
            self.set_error(message);
            false
        } else {
            true
        }
    }

    fn queue_capture(&mut self, capture: CapturedSelection) {
        let lease = capture.clipboard_lease;
        match self.speech_queue.push(capture.text) {
            Ok(id) => {
                self.panel_open = false;
                if let Some(lease) = lease {
                    self.clipboard_leases.insert(id, lease);
                }
                self.maybe_start_automatic_queue();
            }
            Err(error) => {
                let mut message = error.to_string();
                if let Some(lease) = lease
                    && let Err(cleanup_error) = self.platform.clear_temporary_clipboard(&lease)
                {
                    self.deferred_clipboard_cleanup.push(lease);
                    message.push_str(&format!(
                        ". Mist will retry clearing its temporary clipboard text: {cleanup_error}"
                    ));
                }
                self.set_error(message);
            }
        }
    }

    fn worker_available_for_queue(&self) -> bool {
        queue_worker_available(
            &self.status,
            self.voices_ready,
            self.voice_preview,
            self.playback.phase(),
        )
    }

    fn maybe_start_automatic_queue(&mut self) {
        if !self.playback_preferences.auto_play_queue
            || !self.worker_available_for_queue()
            || self.speech_queue.active_id().is_some()
        {
            return;
        }
        if let Some(item) = self.speech_queue.start_next() {
            self.dispatch_queue_item(item.id, item.text);
        }
    }

    fn play_queue_item(&mut self, id: QueueItemId) {
        if !self.worker_available_for_queue() {
            return;
        }
        if let Some(item) = self.speech_queue.start(id) {
            self.dispatch_queue_item(item.id, item.text);
        }
    }

    fn pause_queue_item(&mut self, id: QueueItemId) {
        if self.playback.pause(PlaybackToken::queue_item(id)) {
            self.speech_queue.pause(id);
        }
    }

    fn resume_queue_item(&mut self, id: QueueItemId) {
        if self.playback.resume(PlaybackToken::queue_item(id)) {
            self.speech_queue.resume(id);
        }
    }

    fn delete_queue_item(&mut self, id: QueueItemId) {
        let was_active = self.speech_queue.active_id() == Some(id);
        if was_active {
            self.playback.cancel(PlaybackToken::queue_item(id));
            self.queue_playback_started = false;
        }
        if self.speech_queue.remove(id).is_none() {
            return;
        }
        if let Some(error) = self.clear_clipboard_lease(id) {
            self.set_error(error);
        }
        self.maybe_start_automatic_queue();
    }

    fn handle_queue_action(&mut self, action: QueueAction) {
        match action {
            QueueAction::Play(id) => self.play_queue_item(id),
            QueueAction::Pause(id) => self.pause_queue_item(id),
            QueueAction::Resume(id) => self.resume_queue_item(id),
            QueueAction::Delete(id) => self.delete_queue_item(id),
        }
    }

    fn dispatch_queue_item(&mut self, id: QueueItemId, text: ::mist::SelectedText) {
        self.queue_playback_started = false;
        let token = PlaybackToken::queue_item(id);
        if !self.playback.register(token) {
            self.speech_queue.fail(id);
            if let Some(error) = self.clear_clipboard_lease(id) {
                self.set_error(error);
                return;
            }
            self.set_error("The audio player is still finishing the previous queue item.");
            return;
        }
        if !self.enqueue(WorkerCommand::Speak { token, text }) {
            self.playback.finish_session(token);
            self.speech_queue.fail(id);
            if let Some(error) = self.clear_clipboard_lease(id) {
                self.set_error(error);
            }
        }
    }

    fn complete_active_queue(&mut self) -> Option<String> {
        let active = self.speech_queue.active_id()?;
        self.speech_queue.complete(active)?;
        self.queue_playback_started = false;
        self.clear_clipboard_lease(active)
    }

    fn clear_clipboard_lease(&mut self, id: QueueItemId) -> Option<String> {
        let lease = self.clipboard_leases.get(&id)?.clone();
        match self.platform.clear_temporary_clipboard(&lease) {
            Ok(_) => {
                self.clipboard_leases.remove(&id);
                None
            }
            Err(error) => {
                self.clipboard_leases.remove(&id);
                self.deferred_clipboard_cleanup.push(lease);
                Some(format!(
                    "Speech finished, but Mist could not clear its temporary clipboard text: {error}"
                ))
            }
        }
    }

    fn retry_deferred_clipboard_cleanup(&mut self) {
        self.deferred_clipboard_cleanup
            .retain(|lease| self.platform.clear_temporary_clipboard(lease).is_err());
    }

    fn persist_playback_preferences(&mut self) {
        self.platform.set_automatic_clipboard_fallback(
            self.playback_preferences.automatic_clipboard_fallback,
        );
        if let Err(error) = self
            .playback_preferences_store
            .save(self.playback_preferences)
        {
            self.set_error(format!("Could not save playback settings: {error:#}"));
        }
    }

    fn select_voice(&mut self, voice_id: &str) {
        let Some(mut settings) = self.voice_catalog.settings(voice_id) else {
            self.set_error(format!("Unknown voice: {voice_id}"));
            return;
        };
        settings.speed = self.playback_preferences.speed.multiplier();
        self.enqueue_voice_command(settings.clone(), WorkerCommand::SelectVoice(settings));
    }

    fn preview_voice(&mut self, voice_id: &str) {
        if !self.voices_ready || self.speech_queue.active_id().is_some() {
            return;
        }
        let mut settings = match voice_preview_settings(self.voice_catalog.as_ref(), voice_id) {
            Ok(settings) => settings,
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        settings.speed = self.playback_preferences.speed.multiplier();
        let replacing = self.voice_preview != VoicePreviewActivity::Idle;
        if replacing {
            self.playback.cancel(PlaybackToken::PREVIEW);
        } else if !self.playback.register(PlaybackToken::PREVIEW) {
            return;
        }
        if self.enqueue_preview(settings) {
            self.voice_preview = VoicePreviewActivity::Pending;
        } else if !replacing {
            self.playback.finish_session(PlaybackToken::PREVIEW);
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

    fn enqueue_preview(&mut self, settings: VoiceSettings) -> bool {
        if self.previews.request(&settings).is_err() {
            self.set_error("The speech worker stopped unexpectedly.");
            return false;
        }
        if let Some(tray) = &self.tray {
            tray.select_voice(settings.voice_id.as_str());
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
                if voice_preview_available(
                    &self.status,
                    self.voices_ready,
                    self.voice_preview,
                    self.speech_queue.active_id().is_none(),
                ) {
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
            let position = viewport_origin_for_mist_center(center, mode);
            context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        }
        self.viewport_mode = mode;
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(mode.size()));
    }

    fn mist_rect(&mut self, ui: &egui::Ui, mist_size: Vec2) -> Rect {
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
                ui.max_rect().min + self.viewport_mode.mist_center()
            }
            _ => ui.max_rect().center(),
        };
        Rect::from_center_size(center, mist_size).shrink(2.0)
    }

    fn paint_mist_only(
        &mut self,
        ui: &mut egui::Ui,
        time: f32,
        presentation: presentation::MistPresentation,
    ) -> bool {
        let context = ui.ctx().clone();
        let mist_size = if presentation.activity == presentation::MistActivity::Speaking {
            SPEAKING_MIST_WINDOW
        } else {
            MIST_WINDOW
        };
        let rect = self.mist_rect(ui, mist_size);
        let response = ui.interact(rect, ui.id().with("living-mist"), Sense::click_and_drag());
        let selected = self.selected_voice.voice_id.as_str();
        let profile = self.voice_catalog.profile(selected).unwrap_or_else(|| {
            self.voice_catalog
                .voices()
                .first()
                .expect("catalog is not empty")
        });
        let seed = selected_voice_index(self.voice_catalog.as_ref(), selected) as f32 * 0.83;
        self.mist
            .paint(ui, rect, time, presentation, profile.palette, seed);

        if response.drag_started() {
            context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        response.context_menu(|ui| self.context_menu(ui, &context));
        response.context_menu_opened()
    }

    fn paint_queue_window(&mut self, context: &egui::Context) -> QueueTrayResponse {
        if self.speech_queue.items().is_empty() {
            return QueueTrayResponse::default();
        }

        let viewport_id = queue_viewport_id();
        if let Some(position) = context.input(|input| {
            input
                .raw
                .viewports
                .get(&viewport_id)
                .and_then(|viewport| viewport.outer_rect)
                .map(|rect| rect.min)
        }) {
            self.queue_screen_position = Some(position);
        }

        let size = queue_window_size(self.speech_queue.items().len());
        let position = self.queue_screen_position.or_else(|| {
            context.input(|input| {
                input
                    .viewport()
                    .outer_rect
                    .map(|root| queue_position_below(root, size))
            })
        });
        self.queue_screen_position = position;

        let mut builder = egui::ViewportBuilder::default()
            .with_title("Mist queue")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_taskbar(false)
            .with_always_on_top();
        if let Some(position) = position {
            builder = builder.with_position(position);
        }

        let items = self.speech_queue.items().to_vec();
        let selected = self.selected_voice.voice_id.as_str();
        let palette = self
            .voice_catalog
            .profile(selected)
            .unwrap_or_else(|| {
                self.voice_catalog
                    .voices()
                    .first()
                    .expect("catalog is not empty")
            })
            .palette;
        let auto_play = self.playback_preferences.auto_play_queue;
        context.show_viewport_immediate(viewport_id, builder, move |ui, _class| {
            let response = queue_tray::show(ui, &items, queue_tray::TOP_GAP, palette, auto_play);
            if response.drag_started {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            response
        })
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
        let profile = self
            .voice_catalog
            .profile(selected_voice)
            .unwrap_or_else(|| {
                self.voice_catalog
                    .voices()
                    .first()
                    .expect("catalog is not empty")
            });
        let accent = palette_color(profile.palette.primary);
        let preview = Rect::from_min_size(outer.min + Vec2::new(18.0, 16.0), Vec2::splat(142.0));
        self.mist.paint(
            ui,
            preview,
            time,
            presentation,
            profile.palette,
            selected_voice_index(self.voice_catalog.as_ref(), selected_voice) as f32 * 0.83,
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
        detail.wrap.max_rows = 2;
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

        if copy.action != PrimaryAction::None {
            let (label, enabled) = match (&self.status, copy.action) {
                (AppStatus::Downloading, _) => ("Gathering voices…", false),
                (_, PrimaryAction::InstallVoices) => ("Download voices · ~338 MiB", true),
                (_, PrimaryAction::OpenAccessibility) => ("Open Accessibility", true),
                _ => ("Continue", false),
            };
            let response = ui.put(
                Rect::from_min_size(
                    Pos2::new(outer.right() - 216.0, outer.top() + 126.0),
                    Vec2::new(184.0, 32.0),
                ),
                Button::new(RichText::new(label).size(11.5).strong().color(TEXT_PRIMARY))
                    .fill(if enabled {
                        with_alpha(accent, 180)
                    } else {
                        Color32::from_white_alpha(10)
                    })
                    .stroke(Stroke::new(1.0, Color32::from_white_alpha(25)))
                    .corner_radius(12.0),
            );
            if enabled && response.clicked() {
                self.perform_action(copy.action);
            }
        }

        let drag_rect = Rect::from_min_max(
            Pos2::new(outer.left() + 10.0, outer.top() + 6.0),
            Pos2::new(outer.right() - 48.0, outer.top() + 26.0),
        );
        if ui
            .interact(drag_rect, ui.id().with("settings-drag"), Sense::drag())
            .drag_started()
        {
            context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let body = Rect::from_min_max(
            Pos2::new(outer.left() + 18.0, outer.top() + 178.0),
            Pos2::new(outer.right() - 18.0, outer.bottom() - 18.0),
        );
        let sidebar = Rect::from_min_max(body.min, Pos2::new(body.left() + 164.0, body.bottom()));
        let content = Rect::from_min_max(
            Pos2::new(sidebar.right() + 30.0, body.top() + 4.0),
            Pos2::new(body.right() - 10.0, body.bottom() - 4.0),
        );
        painter.line_segment(
            [
                Pos2::new(sidebar.right() + 14.0, body.top() + 4.0),
                Pos2::new(sidebar.right() + 14.0, body.bottom() - 4.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(15)),
        );
        if let Some(section) = settings::sidebar(ui, sidebar, self.settings_section, accent) {
            self.settings_section = section;
        }

        let preferences_before = self.playback_preferences;
        let settings_action = match self.settings_section {
            SettingsSection::Voices => {
                let gallery_mode = gallery_mode(
                    &self.status,
                    self.voices_ready,
                    self.voice_preview,
                    self.speech_queue.active_id().is_none(),
                );
                let language = self
                    .voice_catalog
                    .language_for(self.selected_voice.voice_id.as_str())
                    .unwrap_or_else(|| {
                        self.voice_catalog
                            .languages()
                            .first()
                            .expect("catalog is not empty")
                            .id
                    });
                let gallery = voice_gallery::show(
                    ui,
                    &self.mist,
                    content,
                    voice_gallery::GalleryContent {
                        time,
                        selected_voice: self.selected_voice.voice_id.as_str(),
                        voices: self.voice_catalog.voices(),
                        languages: self.voice_catalog.languages(),
                        selected_language: language,
                        mode: gallery_mode,
                        accent,
                    },
                );
                if let Some(language) = gallery.select_language {
                    match language_voice_id(self.voice_catalog.as_ref(), language) {
                        Ok(default_voice) => {
                            if should_preview_language(
                                self.voices_ready,
                                self.speech_queue.active_id().is_none(),
                            ) {
                                self.preview_voice(&default_voice);
                            } else {
                                self.select_voice(&default_voice);
                            }
                        }
                        Err(error) => self.set_error(error),
                    }
                }
                if let Some(voice) = gallery.select_voice {
                    self.preview_voice(&voice);
                }
                None
            }
            SettingsSection::Playback => {
                settings::playback(ui, content, &mut self.playback_preferences, accent)
            }
            SettingsSection::Privacy => settings::privacy(
                ui,
                content,
                &mut self.playback_preferences,
                cfg!(target_os = "macos"),
                accent,
            ),
            SettingsSection::Model => {
                if let Some(provider) = settings::model(
                    ui,
                    content,
                    &self.provider_capabilities,
                    &self.model_provider,
                    &self.active_model_provider,
                    self.model_provider != self.active_model_provider,
                    accent,
                ) {
                    if let Err(error) = self.model_preferences_store.save(&provider) {
                        self.set_error(format!("Could not save model settings: {error:#}"));
                    } else {
                        self.model_provider = provider;
                    }
                }
                None
            }
        };
        apply_playback_preferences(self, preferences_before);
        match settings_action {
            Some(SettingsAction::PlayClipboard) => {
                if let Err(error) = self.platform.request_clipboard_text() {
                    self.set_error(error);
                }
            }
            Some(SettingsAction::OpenAccessibility) => {
                self.perform_action(PrimaryAction::OpenAccessibility);
            }
            None => {}
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.set_min_width(238.0);
        let profile = self
            .voice_catalog
            .profile(self.selected_voice.voice_id.as_str())
            .unwrap_or_else(|| {
                self.voice_catalog
                    .voices()
                    .first()
                    .expect("catalog is not empty")
            });
        ui.label(RichText::new("MIST").small().color(TEXT_MUTED));
        ui.strong(format!("{} mist", profile.display_name));
        ui.label(
            RichText::new(self.platform.usage_hint())
                .size(11.0)
                .color(TEXT_SECONDARY),
        );
        ui.separator();
        if ui.button("Settings…").clicked() {
            self.panel_open = true;
            ui.close();
        }

        if ui.button("Speak copied text").clicked() {
            if let Err(error) = self.platform.request_clipboard_text() {
                self.set_error(error);
            }
            ui.close();
        }
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            ui.collapsing("Speak typed text", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.manual_text)
                        .desired_rows(3)
                        .desired_width(210.0),
                );
                if ui.button("Speak").clicked() {
                    match ::mist::SelectedText::new(&self.manual_text) {
                        Ok(text) => {
                            self.queue_capture(CapturedSelection {
                                text,
                                clipboard_lease: None,
                            });
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
                PlatformEvent::Captured(selection) => self.queue_capture(selection),
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
        let mut presentation = mist_for_status(
            &self.status,
            self.platform.accessibility_required(),
            shell_error,
        );
        if self
            .speech_queue
            .items()
            .iter()
            .any(|item| item.state == QueueItemState::Paused)
        {
            presentation.activity = presentation::MistActivity::Idle;
            presentation.features = ::mist::AudioFeatures {
                energy: 10,
                brightness: 16,
            };
        }
        let panel_visible = self.panel_open
            || required_panel_visible(
                presentation.requires_panel,
                self.voices_ready,
                self.speech_queue.items().len(),
            );
        let time = context.input(|input| input.time as f32);
        let presentation = self.mist_smoother.update(presentation, time);
        let context_menu_visible = if panel_visible {
            self.paint_panel(ui, time, presentation);
            false
        } else {
            self.paint_mist_only(ui, time, presentation)
        };
        let queue_response = self.paint_queue_window(&context);
        if let Some(action) = queue_response.action {
            self.handle_queue_action(action);
        }
        let mode = viewport_mode(
            panel_visible,
            context_menu_visible,
            presentation.activity == presentation::MistActivity::Speaking,
        );
        self.sync_window_size(&context, mode);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

impl Drop for PetApp {
    fn drop(&mut self) {
        for (_, lease) in self.clipboard_leases.drain() {
            let _ = self.platform.clear_temporary_clipboard(&lease);
        }
        for lease in self.deferred_clipboard_cleanup.drain(..) {
            let _ = self.platform.clear_temporary_clipboard(&lease);
        }
    }
}

fn apply_playback_preferences(app: &mut PetApp, previous: PlaybackPreferences) {
    if app.playback_preferences != previous {
        dispatch_playback_preferences(app, previous);
    }
}

fn dispatch_playback_preferences(app: &mut PetApp, previous: PlaybackPreferences) {
    if app
        .commands
        .configure_playback(app.playback_preferences)
        .is_err()
    {
        app.playback_preferences = previous;
        app.set_error("The speech worker stopped unexpectedly.");
        return;
    }
    app.persist_playback_preferences();
    app.selected_voice.speed = app.playback_preferences.speed.multiplier();
}

fn selected_voice_index(catalog: &dyn VoiceCatalog, selected: &str) -> usize {
    catalog
        .voices()
        .iter()
        .position(|voice| voice.id == selected)
        .unwrap_or(0)
}

fn viewport_mode(panel_visible: bool, context_menu_visible: bool, speaking: bool) -> ViewportMode {
    if panel_visible {
        ViewportMode::Panel
    } else if context_menu_visible {
        ViewportMode::ContextMenu
    } else if speaking {
        ViewportMode::Speaking
    } else {
        ViewportMode::Mist
    }
}

fn queue_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("mist-queue")
}

fn queue_window_size(item_count: usize) -> Vec2 {
    Vec2::new(QUEUE_WINDOW_WIDTH, queue_tray::height(item_count))
}

fn queue_position_below(root: Rect, queue_size: Vec2) -> Pos2 {
    Pos2::new(
        root.center().x - queue_size.x * 0.5,
        root.bottom() + queue_tray::TOP_GAP,
    )
}

fn required_panel_visible(requires_panel: bool, voices_ready: bool, queued_items: usize) -> bool {
    requires_panel && !(voices_ready && queued_items > 0)
}

fn status_opens_panel(status: &AppStatus) -> bool {
    matches!(status, AppStatus::MissingModel)
}

fn viewport_origin_for_mist_center(center: Pos2, mode: ViewportMode) -> Pos2 {
    center - mode.mist_center()
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
    queue_idle: bool,
) -> voice_gallery::GalleryMode {
    if voice_preview_available(status, voices_ready, preview, queue_idle) {
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
    queue_idle: bool,
) -> bool {
    voices_ready
        && queue_idle
        && match preview {
            VoicePreviewActivity::Idle => matches!(status, AppStatus::Ready),
            VoicePreviewActivity::Pending | VoicePreviewActivity::Active => matches!(
                status,
                AppStatus::Ready
                    | AppStatus::Loading
                    | AppStatus::Synthesizing { .. }
                    | AppStatus::Speaking { .. }
            ),
        }
}

fn voice_preview_settings(
    catalog: &dyn VoiceCatalog,
    voice_id: &str,
) -> Result<VoiceSettings, String> {
    catalog
        .settings(voice_id)
        .ok_or_else(|| format!("Unknown voice: {voice_id}"))
}

fn language_voice_id(
    catalog: &dyn VoiceCatalog,
    language: ::mist::LanguageId,
) -> Result<String, &'static str> {
    catalog
        .default_voice_for(language)
        .map(str::to_owned)
        .ok_or("The active speech model has no voice for that language")
}

fn queue_worker_available(
    status: &AppStatus,
    voices_ready: bool,
    preview: VoicePreviewActivity,
    playback: PlaybackPhase,
) -> bool {
    voices_ready
        && preview == VoicePreviewActivity::Idle
        && playback == PlaybackPhase::Idle
        && matches!(status, AppStatus::Ready | AppStatus::Error(_))
}

fn should_preview_language(voices_ready: bool, queue_idle: bool) -> bool {
    voices_ready && queue_idle
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::mist::adapters::kokoro_catalog::KokoroVoiceCatalog;

    #[test]
    fn voice_card_action_dispatches_preview_for_the_exact_voice() {
        let catalog = KokoroVoiceCatalog;
        let settings = voice_preview_settings(&catalog, "bf_emma").unwrap();
        assert_eq!(settings.voice_id.as_str(), "bf_emma");
    }

    #[test]
    fn active_preview_keeps_cards_available_for_immediate_replacement() {
        let pending = VoicePreviewActivity::Pending;

        assert_eq!(
            gallery_mode(&AppStatus::Ready, true, pending, true),
            voice_gallery::GalleryMode::Available
        );
        assert_eq!(pending.after_status(&AppStatus::Ready), pending);
        let active = pending.after_status(&AppStatus::Synthesizing {
            text: "preview".to_owned(),
            inference_policy: "CPU".to_owned(),
        });
        assert_eq!(active, VoicePreviewActivity::Active);
        assert_eq!(
            gallery_mode(&AppStatus::Ready, true, active, true),
            voice_gallery::GalleryMode::Available
        );
        assert_eq!(
            active.after_status(&AppStatus::Ready),
            VoicePreviewActivity::Idle
        );
    }

    #[test]
    fn voice_cards_require_downloaded_model_artifacts() {
        assert_eq!(
            gallery_mode(
                &AppStatus::MissingModel,
                false,
                VoicePreviewActivity::Idle,
                true,
            ),
            voice_gallery::GalleryMode::DownloadRequired
        );
    }

    #[test]
    fn language_change_persists_without_preview_while_queue_owns_playback() {
        assert!(!should_preview_language(true, false));
        assert!(should_preview_language(true, true));
        assert!(!should_preview_language(false, true));
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
            VoicePreviewActivity::Idle,
            true,
        ));
        assert!(voice_preview_available(
            &AppStatus::Ready,
            true,
            VoicePreviewActivity::Idle,
            true,
        ));
        assert!(!voice_preview_available(
            &AppStatus::Ready,
            true,
            VoicePreviewActivity::Idle,
            false,
        ));
    }

    #[test]
    fn cancelled_item_must_acknowledge_before_the_next_item_starts() {
        assert!(!queue_worker_available(
            &AppStatus::Ready,
            true,
            VoicePreviewActivity::Idle,
            PlaybackPhase::Cancelled,
        ));
        assert!(queue_worker_available(
            &AppStatus::Ready,
            true,
            VoicePreviewActivity::Idle,
            PlaybackPhase::Idle,
        ));
    }

    #[test]
    fn failed_voice_enqueue_restores_the_authoritative_tray_selection() {
        let current = VoiceSettings::new("af_heart").unwrap();
        let requested = VoiceSettings::new("af_bella").unwrap();

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
        assert_eq!(viewport_mode(true, true, true), ViewportMode::Panel);
        assert_eq!(viewport_mode(false, true, true), ViewportMode::ContextMenu);
        assert_eq!(viewport_mode(false, false, false), ViewportMode::Mist);
    }

    #[test]
    fn speaking_grows_then_returns_to_compact_mist() {
        assert_eq!(viewport_mode(false, false, true), ViewportMode::Speaking);
        assert_eq!(viewport_mode(false, false, false), ViewportMode::Mist);
        assert!(ViewportMode::Speaking.size().x > ViewportMode::Mist.size().x);
    }

    #[test]
    fn queue_does_not_expand_or_drag_the_mist_surface() {
        let mode = viewport_mode(false, false, false);
        let queue = queue_window_size(1);
        assert_eq!(mode, ViewportMode::Mist);
        assert_eq!(mode.size(), MIST_WINDOW);
        assert_eq!(queue.x, QUEUE_WINDOW_WIDTH);
        assert_eq!(queue.y, queue_tray::height(1));
    }

    #[test]
    fn selection_error_does_not_force_voice_settings_open() {
        assert!(!status_opens_panel(&AppStatus::Error(
            "Select some text first".to_owned()
        )));
        assert!(status_opens_panel(&AppStatus::MissingModel));
    }

    #[test]
    fn captured_service_text_can_show_its_queue_over_an_accessibility_notice() {
        assert!(!required_panel_visible(true, true, 1));
        assert!(required_panel_visible(true, false, 1));
        assert!(required_panel_visible(true, true, 0));
    }

    #[test]
    fn expanded_viewport_preserves_center_on_secondary_monitors() {
        assert_eq!(
            viewport_origin_for_mist_center(Pos2::new(-300.0, -200.0), ViewportMode::ContextMenu),
            Pos2::new(-440.0, -410.0)
        );
    }
}
