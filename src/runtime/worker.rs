//! Long-lived desktop runtime coordinating core use cases and outbound adapters.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, SyncSender, TrySendError},
    },
    thread,
};

use anyhow::{Context, Result};

use crate::{
    PlaybackController, PlaybackPreferences, PlaybackStopped, PlaybackToken, SpeakSelection,
    VoiceSettings,
    adapters::outbound::{
        audio::system::SystemAudioPlayer,
        persistence::{playback::PlaybackPreferencesStore, voice::VoicePreferencesStore},
    },
    domain::{AudioFeatures, SelectedText},
    ports::{DynSpeechSynthesizer, ModelProvisioner, SpeechEngineFactory, VoiceCatalog},
};

#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Speak {
        token: PlaybackToken,
        text: SelectedText,
    },
    InstallModel,
    SelectVoice(VoiceSettings),
}

#[derive(Clone, Debug)]
enum WorkerMessage {
    Command(WorkerCommand),
    PreviewWake,
    PlaybackConfigurationWake,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerSendError {
    Full,
    Disconnected,
}

#[derive(Clone, Debug)]
pub struct WorkerCommands {
    sender: SyncSender<WorkerMessage>,
    playback_requests: LatestRequest<PlaybackPreferences>,
}

impl WorkerCommands {
    pub fn try_send(&self, command: WorkerCommand) -> Result<(), WorkerSendError> {
        match self.sender.try_send(WorkerMessage::Command(command)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(WorkerSendError::Full),
            Err(TrySendError::Disconnected(_)) => Err(WorkerSendError::Disconnected),
        }
    }

    /// Coalesces rapid UI changes. A full channel already guarantees another
    /// worker iteration, where the latest configuration is read from the slot.
    pub fn configure_playback(
        &self,
        preferences: PlaybackPreferences,
    ) -> Result<(), WorkerSendError> {
        self.playback_requests.request(preferences);
        match self
            .sender
            .try_send(WorkerMessage::PlaybackConfigurationWake)
        {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
            Err(TrySendError::Disconnected(_)) => Err(WorkerSendError::Disconnected),
        }
    }
}

#[derive(Debug)]
struct LatestRequest<T> {
    latest: Arc<Mutex<Option<T>>>,
}

impl<T> Clone for LatestRequest<T> {
    fn clone(&self) -> Self {
        Self {
            latest: self.latest.clone(),
        }
    }
}

impl<T> Default for LatestRequest<T> {
    fn default() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T> LatestRequest<T> {
    fn request(&self, value: T) {
        *self.latest.lock().expect("latest request was poisoned") = Some(value);
    }

    fn take_latest(&self) -> Option<T> {
        self.latest
            .lock()
            .expect("latest request was poisoned")
            .take()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppStatus {
    CheckingModel,
    MissingModel,
    Ready,
    Downloading,
    Loading,
    Synthesizing {
        text: String,
        inference_policy: String,
    },
    Speaking {
        text: String,
        inference_policy: String,
        features: AudioFeatures,
    },
    Error(String),
}

struct SpeechSession {
    speaker: SpeakSelection<DynSpeechSynthesizer, SystemAudioPlayer>,
    inference_policy: String,
}

struct SpeechEnvironment<'a> {
    data_root: &'a Path,
    engine_factory: &'a dyn SpeechEngineFactory,
    catalog: &'a dyn VoiceCatalog,
    statuses: &'a Sender<AppStatus>,
    playback: &'a PlaybackController,
    playback_preferences: PlaybackPreferences,
}

pub struct WorkerHandle {
    pub commands: WorkerCommands,
    pub statuses: Receiver<AppStatus>,
    pub selected_voice: VoiceSettings,
    pub playback: PlaybackController,
    pub previews: PreviewDispatcher,
}

struct WorkerRuntime {
    commands: Receiver<WorkerMessage>,
    statuses: Sender<AppStatus>,
    data_root: PathBuf,
    models: Arc<dyn ModelProvisioner>,
    preferences: VoicePreferencesStore,
    selected_voice: VoiceSettings,
    playback_preferences: PlaybackPreferences,
    playback: PlaybackController,
    preview_requests: PreviewRequests,
    playback_requests: LatestRequest<PlaybackPreferences>,
    engine_factory: Arc<dyn SpeechEngineFactory>,
    catalog: Arc<dyn VoiceCatalog>,
}

/// Latest-wins coordination for voice-card previews.
///
/// Preview synthesis and playback run synchronously on the speech worker. The
/// UI uses this shared slot plus the out-of-band playback controller so rapid
/// card changes cancel the current sample and stale queued samples are skipped.
#[derive(Clone, Debug, Default)]
pub struct PreviewRequests {
    latest: LatestRequest<VoiceSettings>,
}

#[derive(Clone, Debug)]
pub struct PreviewDispatcher {
    sender: SyncSender<WorkerMessage>,
    requests: PreviewRequests,
}

impl PreviewDispatcher {
    fn new(sender: SyncSender<WorkerMessage>, requests: PreviewRequests) -> Self {
        Self { sender, requests }
    }

    /// Replaces any pending sample and wakes the worker. A full channel is
    /// already carrying a wake-up, so the coalesced latest voice remains valid.
    pub fn request(&self, voice: &VoiceSettings) -> Result<(), WorkerSendError> {
        self.requests.request(voice);
        match self.sender.try_send(WorkerMessage::PreviewWake) {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
            Err(TrySendError::Disconnected(_)) => Err(WorkerSendError::Disconnected),
        }
    }
}

impl PreviewRequests {
    pub fn request(&self, voice: &VoiceSettings) {
        self.latest.request(voice.clone());
    }

    fn take_latest(&self) -> Option<VoiceSettings> {
        self.latest.take_latest()
    }
}

pub fn spawn(
    data_root: PathBuf,
    models: Arc<dyn ModelProvisioner>,
    engine_factory: Arc<dyn SpeechEngineFactory>,
    catalog: Arc<dyn VoiceCatalog>,
) -> WorkerHandle {
    let (command_tx, command_rx) = mpsc::sync_channel(8);
    let (status_tx, status_rx) = mpsc::channel();

    let preferences = VoicePreferencesStore::at(&data_root);
    let playback_preferences = PlaybackPreferencesStore::at(&data_root).load();
    let mut selected_voice = preferences.load(catalog.as_ref());
    selected_voice.speed = playback_preferences.speed.multiplier();
    let worker_voice = selected_voice.clone();
    let playback = PlaybackController::default();
    let worker_playback = playback.clone();
    let preview_requests = PreviewRequests::default();
    let worker_preview_requests = preview_requests.clone();
    let playback_requests = LatestRequest::default();
    let worker_playback_requests = playback_requests.clone();
    let commands = WorkerCommands {
        sender: command_tx.clone(),
        playback_requests,
    };
    let previews = PreviewDispatcher::new(command_tx, preview_requests);
    thread::Builder::new()
        .name("mist-speech-worker".to_owned())
        .spawn(move || {
            run(WorkerRuntime {
                commands: command_rx,
                statuses: status_tx,
                data_root,
                models,
                preferences,
                selected_voice: worker_voice,
                playback_preferences,
                playback: worker_playback,
                preview_requests: worker_preview_requests,
                playback_requests: worker_playback_requests,
                engine_factory,
                catalog,
            })
        })
        .expect("speech worker thread should start");

    WorkerHandle {
        commands,
        statuses: status_rx,
        selected_voice,
        playback,
        previews,
    }
}

fn run(runtime: WorkerRuntime) {
    let WorkerRuntime {
        commands,
        statuses,
        data_root,
        models,
        preferences,
        mut selected_voice,
        mut playback_preferences,
        playback,
        preview_requests,
        playback_requests,
        engine_factory,
        catalog,
    } = runtime;
    let mut session: Option<SpeechSession> = None;
    let mut model_ready = models.is_ready();
    send_status(
        &statuses,
        if model_ready {
            AppStatus::Ready
        } else {
            AppStatus::MissingModel
        },
    );

    loop {
        if let Some(preferences) = playback_requests.take_latest() {
            configure_playback(
                preferences,
                &mut playback_preferences,
                &mut selected_voice,
                &mut session,
            );
        }
        if let Some(voice) = preview_requests.take_latest() {
            // A cancelled request may never have reached the worker. Reset its
            // shared preview session before starting the coalesced latest one.
            playback.finish_session(PlaybackToken::PREVIEW);
            let result = select_voice(
                voice.clone(),
                &preferences,
                &mut selected_voice,
                &mut session,
            );
            let result = if result.is_err() || !model_ready {
                result
            } else {
                preview_voice(
                    voice,
                    SpeechEnvironment {
                        data_root: &data_root,
                        engine_factory: engine_factory.as_ref(),
                        catalog: catalog.as_ref(),
                        statuses: &statuses,
                        playback: &playback,
                        playback_preferences,
                    },
                    &mut session,
                )
            };
            send_result_status(&statuses, model_ready, result);
            continue;
        }

        let Ok(message) = commands.recv() else {
            break;
        };
        let WorkerMessage::Command(command) = message else {
            continue;
        };
        let result = match command {
            WorkerCommand::InstallModel => {
                let result = install_model(models.as_ref(), &statuses);
                if result.is_ok() {
                    model_ready = true;
                }
                result
            }
            WorkerCommand::SelectVoice(voice) => {
                select_voice(voice, &preferences, &mut selected_voice, &mut session)
            }
            WorkerCommand::Speak { token, text } => {
                if !model_ready {
                    send_status(&statuses, AppStatus::MissingModel);
                    continue;
                }
                speak(
                    token,
                    text,
                    &selected_voice,
                    SpeechEnvironment {
                        data_root: &data_root,
                        engine_factory: engine_factory.as_ref(),
                        catalog: catalog.as_ref(),
                        statuses: &statuses,
                        playback: &playback,
                        playback_preferences,
                    },
                    &mut session,
                )
            }
        };

        send_result_status(&statuses, model_ready, result);
    }
}

fn configure_playback(
    preferences: PlaybackPreferences,
    current: &mut PlaybackPreferences,
    selected_voice: &mut VoiceSettings,
    session: &mut Option<SpeechSession>,
) {
    *current = preferences;
    selected_voice.speed = preferences.speed.multiplier();
    if let Some(session) = session.as_mut() {
        session.speaker.set_playback_mode(preferences.mode);
        session.speaker.set_voice(selected_voice.clone());
    }
}

fn select_voice(
    voice: VoiceSettings,
    preferences: &VoicePreferencesStore,
    selected_voice: &mut VoiceSettings,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    preferences.save(&voice)?;
    *selected_voice = voice;
    if let Some(session) = session.as_mut() {
        session.speaker.set_voice(selected_voice.clone());
    }
    Ok(())
}

fn install_model(models: &dyn ModelProvisioner, statuses: &Sender<AppStatus>) -> Result<()> {
    send_status(statuses, AppStatus::Downloading);
    models.install().context("speech model setup failed")
}

fn speak(
    token: PlaybackToken,
    text: SelectedText,
    selected_voice: &VoiceSettings,
    environment: SpeechEnvironment<'_>,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    let preview = text.preview(32);
    run_speech(
        preview,
        selected_voice,
        environment,
        token,
        session,
        move |speaker, on_playback| speaker.execute_with_playback_cues(text, on_playback),
    )
}

fn preview_voice(
    voice: VoiceSettings,
    environment: SpeechEnvironment<'_>,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    let language = environment
        .catalog
        .language_for(voice.voice_id.as_str())
        .context("the selected voice is not present in the active catalog")?;
    let sample = SelectedText::new(
        environment
            .catalog
            .preview_text(language)
            .context("the active catalog has no preview for this language")?,
    )
    .expect("the built-in voice preview copy must remain valid");
    let preview = sample.preview(32);
    let session_voice = voice.clone();
    run_speech(
        preview,
        &session_voice,
        environment,
        PlaybackToken::PREVIEW,
        session,
        move |speaker, on_playback| {
            speaker.preview_voice_with_playback_cues(voice, sample, on_playback)
        },
    )
}

fn run_speech(
    preview: String,
    voice: &VoiceSettings,
    environment: SpeechEnvironment<'_>,
    token: PlaybackToken,
    session: &mut Option<SpeechSession>,
    execute: impl FnOnce(
        &mut SpeakSelection<DynSpeechSynthesizer, SystemAudioPlayer>,
        &mut dyn FnMut(AudioFeatures),
    ) -> Result<()>,
) -> Result<()> {
    if let Err(error) = environment.playback.begin_session(token) {
        environment.playback.finish_session(token);
        debug_assert_eq!(error, PlaybackStopped);
        return Ok(());
    }
    let result = (|| {
        let session = ensure_session(
            voice,
            environment.data_root,
            environment.engine_factory,
            environment.statuses,
            environment.playback,
            environment.playback_preferences.mode,
            session,
        )?;
        let inference_policy = session.inference_policy.clone();
        send_status(
            environment.statuses,
            AppStatus::Synthesizing {
                text: preview.clone(),
                inference_policy: inference_policy.clone(),
            },
        );
        execute(&mut session.speaker, &mut |features| {
            send_status(
                environment.statuses,
                AppStatus::Speaking {
                    text: preview.clone(),
                    inference_policy: inference_policy.clone(),
                    features,
                },
            );
        })
    })();
    environment.playback.finish_session(token);
    match result {
        Err(error) if playback_was_stopped(&error) => Ok(()),
        result => result,
    }
}

fn ensure_session<'a>(
    voice: &VoiceSettings,
    data_root: &Path,
    engine_factory: &dyn SpeechEngineFactory,
    statuses: &Sender<AppStatus>,
    playback: &PlaybackController,
    playback_mode: crate::PlaybackMode,
    session: &'a mut Option<SpeechSession>,
) -> Result<&'a mut SpeechSession> {
    if session.is_none() {
        send_status(statuses, AppStatus::Loading);
        let engine = engine_factory.load()?;
        let inference_policy = engine.inference_policy;
        let audio_cache = data_root.join("audio-cache");
        let player = SystemAudioPlayer::new(&audio_cache, playback.clone())?;
        let mut speaker = SpeakSelection::new(engine.synthesizer, player, voice.clone());
        speaker.set_playback_mode(playback_mode);
        *session = Some(SpeechSession {
            speaker,
            inference_policy,
        });
    }

    session
        .as_mut()
        .context("speech engine was not initialized")
}

fn playback_was_stopped(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<PlaybackStopped>().is_some())
}

fn send_status(statuses: &Sender<AppStatus>, status: AppStatus) {
    let _ = statuses.send(status);
}

fn send_result_status(statuses: &Sender<AppStatus>, model_ready: bool, result: Result<()>) {
    match result {
        Ok(()) => send_status(
            statuses,
            if model_ready {
                AppStatus::Ready
            } else {
                AppStatus::MissingModel
            },
        ),
        Err(error) => send_status(statuses, AppStatus::Error(format!("{error:#}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::outbound::provisioning::kokoro::ModelStore;
    use crate::adapters::outbound::speech::kokoro::{
        KokoroEngineFactory, catalog::KokoroVoiceCatalog,
    };

    fn test_worker(store: ModelStore) -> WorkerHandle {
        let factory = KokoroEngineFactory::new(store.model_path(), store.voices_path());
        spawn(
            store.root().to_owned(),
            Arc::new(store),
            Arc::new(factory),
            Arc::new(KokoroVoiceCatalog),
        )
    }

    #[test]
    fn selected_voice_is_persisted_by_the_worker() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelStore::at(temporary.path());
        let catalog = KokoroVoiceCatalog;
        let handle = test_worker(store.clone());
        assert_eq!(handle.selected_voice, catalog.default_settings());
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);

        let voice = catalog.settings("af_bella").unwrap();
        handle
            .commands
            .try_send(WorkerCommand::SelectVoice(voice.clone()))
            .unwrap();
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);
        assert_eq!(
            VoicePreferencesStore::at(store.root()).load(&catalog),
            voice
        );
    }

    #[test]
    fn speaking_before_setup_keeps_onboarding_actionable() {
        let temporary = tempfile::tempdir().unwrap();
        let handle = test_worker(ModelStore::at(temporary.path()));
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);

        handle
            .commands
            .try_send(WorkerCommand::Speak {
                token: PlaybackToken::PREVIEW,
                text: SelectedText::new("Not installed yet").unwrap(),
            })
            .unwrap();

        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);
    }

    #[test]
    fn previewing_before_setup_selects_the_clicked_voice() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelStore::at(temporary.path());
        let catalog = KokoroVoiceCatalog;
        let handle = test_worker(store.clone());
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);

        let voice = catalog.settings("bm_daniel").unwrap();
        handle.previews.request(&voice).unwrap();

        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);
        assert_eq!(
            VoicePreferencesStore::at(store.root()).load(&catalog),
            voice
        );
    }

    #[test]
    fn voice_preview_requests_coalesce_to_the_latest_voice() {
        let requests = PreviewRequests::default();
        let catalog = KokoroVoiceCatalog;
        let first = catalog.settings("af_bella").unwrap();
        let latest = catalog.settings("bf_emma").unwrap();

        requests.request(&first);
        requests.request(&latest);

        assert_eq!(requests.take_latest(), Some(latest));
        assert_eq!(requests.take_latest(), None);
    }

    #[test]
    fn full_worker_channel_keeps_the_coalesced_latest_preview_pending() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        sender
            .try_send(WorkerMessage::Command(WorkerCommand::InstallModel))
            .unwrap();
        let requests = PreviewRequests::default();
        let previews = PreviewDispatcher::new(sender, requests.clone());
        let voice = VoiceSettings::new("latest-voice").unwrap();

        assert_eq!(previews.request(&voice), Ok(()));
        assert_eq!(requests.take_latest(), Some(voice));
    }

    #[test]
    fn full_worker_channel_keeps_the_latest_playback_configuration_pending() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        sender
            .try_send(WorkerMessage::Command(WorkerCommand::InstallModel))
            .unwrap();
        let requests = LatestRequest::default();
        let commands = WorkerCommands {
            sender,
            playback_requests: requests.clone(),
        };
        let latest = PlaybackPreferences {
            speed: crate::PlaybackSpeed::from_percent(175).unwrap(),
            ..PlaybackPreferences::default()
        };

        assert_eq!(commands.configure_playback(latest), Ok(()));
        assert_eq!(requests.take_latest(), Some(latest));
    }

    #[test]
    fn stale_preview_releases_a_cancelled_pre_start_session_for_its_replacement() {
        let playback = PlaybackController::default();
        let requests = PreviewRequests::default();
        let catalog = KokoroVoiceCatalog;
        let stale = catalog.settings("af_bella").unwrap();
        let latest = catalog.settings("bf_emma").unwrap();

        assert!(playback.register(PlaybackToken::PREVIEW));
        requests.request(&stale);
        requests.request(&latest);
        assert_eq!(requests.take_latest(), Some(latest));
        assert!(playback.cancel(PlaybackToken::PREVIEW));
        assert_eq!(playback.phase(), crate::PlaybackPhase::Cancelled);

        playback.finish_session(PlaybackToken::PREVIEW);
        assert_eq!(playback.phase(), crate::PlaybackPhase::Idle);
        assert!(playback.register(PlaybackToken::PREVIEW));
    }
}
