use std::{
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    thread,
};

use anyhow::{Context, Result};

use crate::{
    InstallModel, PlaybackController, PlaybackStopped, PlaybackToken, SpeakSelection,
    VoiceSettings,
    adapters::{
        kokoro::KokoroSynthesizer, system_audio::SystemAudioPlayer,
        voice_preferences::VoicePreferencesStore,
    },
    application::VOICE_PREVIEW_TEXT,
    domain::{AudioFeatures, SelectedText},
    model_store::ModelStore,
};

#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Speak {
        token: PlaybackToken,
        text: SelectedText,
    },
    InstallModel,
    SelectVoice(VoiceSettings),
    PreviewVoice(VoiceSettings),
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
    speaker: SpeakSelection<KokoroSynthesizer, SystemAudioPlayer>,
    inference_policy: String,
}

struct SpeechEnvironment<'a> {
    store: &'a ModelStore,
    statuses: &'a Sender<AppStatus>,
    playback: &'a PlaybackController,
}

pub struct WorkerHandle {
    pub commands: SyncSender<WorkerCommand>,
    pub statuses: Receiver<AppStatus>,
    pub selected_voice: VoiceSettings,
    pub playback: PlaybackController,
}

pub fn spawn(store: ModelStore) -> WorkerHandle {
    let (command_tx, command_rx) = mpsc::sync_channel(8);
    let (status_tx, status_rx) = mpsc::channel();

    let preferences = VoicePreferencesStore::at(store.root());
    let selected_voice = preferences.load();
    let worker_voice = selected_voice.clone();
    let playback = PlaybackController::default();
    let worker_playback = playback.clone();
    thread::Builder::new()
        .name("kokoro-speech-worker".to_owned())
        .spawn(move || {
            run(
                command_rx,
                status_tx,
                store,
                preferences,
                worker_voice,
                worker_playback,
            )
        })
        .expect("speech worker thread should start");

    WorkerHandle {
        commands: command_tx,
        statuses: status_rx,
        selected_voice,
        playback,
    }
}

fn run(
    commands: Receiver<WorkerCommand>,
    statuses: Sender<AppStatus>,
    store: ModelStore,
    preferences: VoicePreferencesStore,
    mut selected_voice: VoiceSettings,
    playback: PlaybackController,
) {
    let mut session: Option<SpeechSession> = None;
    let mut model_ready = store.is_ready();
    send_status(
        &statuses,
        if model_ready {
            AppStatus::Ready
        } else {
            AppStatus::MissingModel
        },
    );

    while let Ok(command) = commands.recv() {
        let result = match command {
            WorkerCommand::InstallModel => {
                let result = install_model(&store, &statuses);
                if result.is_ok() {
                    model_ready = true;
                }
                result
            }
            WorkerCommand::SelectVoice(voice) => {
                select_voice(voice, &preferences, &mut selected_voice, &mut session)
            }
            WorkerCommand::PreviewVoice(voice) => {
                let result = select_voice(
                    voice.clone(),
                    &preferences,
                    &mut selected_voice,
                    &mut session,
                );
                if result.is_err() || !model_ready {
                    result
                } else {
                    preview_voice(voice, &store, &statuses, &playback, &mut session)
                }
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
                    &store,
                    &statuses,
                    &playback,
                    &mut session,
                )
            }
        };

        match result {
            Ok(()) => send_status(
                &statuses,
                if model_ready {
                    AppStatus::Ready
                } else {
                    AppStatus::MissingModel
                },
            ),
            Err(error) => send_status(&statuses, AppStatus::Error(format!("{error:#}"))),
        }
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

fn install_model(store: &ModelStore, statuses: &Sender<AppStatus>) -> Result<()> {
    send_status(statuses, AppStatus::Downloading);
    InstallModel::new(store.clone())
        .execute()
        .context("Kokoro setup failed")
}

fn speak(
    token: PlaybackToken,
    text: SelectedText,
    selected_voice: &VoiceSettings,
    store: &ModelStore,
    statuses: &Sender<AppStatus>,
    playback: &PlaybackController,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    let preview = text.preview(32);
    let environment = SpeechEnvironment {
        store,
        statuses,
        playback,
    };
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
    store: &ModelStore,
    statuses: &Sender<AppStatus>,
    playback: &PlaybackController,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    let preview = SelectedText::new(VOICE_PREVIEW_TEXT)
        .expect("the built-in voice preview copy must remain valid")
        .preview(32);
    let session_voice = voice.clone();
    let environment = SpeechEnvironment {
        store,
        statuses,
        playback,
    };
    run_speech(
        preview,
        &session_voice,
        environment,
        PlaybackToken::PREVIEW,
        session,
        move |speaker, on_playback| speaker.preview_voice_with_playback_cues(voice, on_playback),
    )
}

fn run_speech(
    preview: String,
    voice: &VoiceSettings,
    environment: SpeechEnvironment<'_>,
    token: PlaybackToken,
    session: &mut Option<SpeechSession>,
    execute: impl FnOnce(
        &mut SpeakSelection<KokoroSynthesizer, SystemAudioPlayer>,
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
            environment.store,
            environment.statuses,
            environment.playback,
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
    store: &ModelStore,
    statuses: &Sender<AppStatus>,
    playback: &PlaybackController,
    session: &'a mut Option<SpeechSession>,
) -> Result<&'a mut SpeechSession> {
    if session.is_none() {
        send_status(statuses, AppStatus::Loading);
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voices_path())?;
        let inference_policy = synthesizer.inference_policy_label().to_owned();
        let audio_cache = store.root().join("audio-cache");
        let player = SystemAudioPlayer::new(&audio_cache, playback.clone())?;
        *session = Some(SpeechSession {
            speaker: SpeakSelection::new(synthesizer, player, voice.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_voice_is_persisted_by_the_worker() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelStore::at(temporary.path());
        let handle = spawn(store.clone());
        assert_eq!(handle.selected_voice, VoiceSettings::default());
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);

        let voice = VoiceSettings::from_voice_id("af_bella").unwrap();
        handle
            .commands
            .send(WorkerCommand::SelectVoice(voice.clone()))
            .unwrap();
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);
        assert_eq!(VoicePreferencesStore::at(store.root()).load(), voice);
    }

    #[test]
    fn speaking_before_setup_keeps_onboarding_actionable() {
        let temporary = tempfile::tempdir().unwrap();
        let handle = spawn(ModelStore::at(temporary.path()));
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);

        handle
            .commands
            .send(WorkerCommand::Speak {
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
        let handle = spawn(store.clone());
        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);

        let voice = VoiceSettings::from_voice_id("bm_daniel").unwrap();
        handle
            .commands
            .send(WorkerCommand::PreviewVoice(voice.clone()))
            .unwrap();

        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);
        assert_eq!(VoicePreferencesStore::at(store.root()).load(), voice);
    }
}
