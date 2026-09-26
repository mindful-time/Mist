use std::{
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    thread,
};

use anyhow::{Context, Result};

use crate::{
    InstallModel, SpeakSelection, VoiceSettings,
    adapters::{
        kokoro::KokoroSynthesizer, system_audio::SystemAudioPlayer,
        voice_preferences::VoicePreferencesStore,
    },
    domain::{AudioFeatures, SelectedText},
    model_store::ModelStore,
};

#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Speak(SelectedText),
    InstallModel,
    SelectVoice(VoiceSettings),
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

pub struct WorkerHandle {
    pub commands: SyncSender<WorkerCommand>,
    pub statuses: Receiver<AppStatus>,
    pub selected_voice: VoiceSettings,
}

pub fn spawn(store: ModelStore) -> WorkerHandle {
    let (command_tx, command_rx) = mpsc::sync_channel(8);
    let (status_tx, status_rx) = mpsc::channel();

    let preferences = VoicePreferencesStore::at(store.root());
    let selected_voice = preferences.load();
    let worker_voice = selected_voice.clone();
    thread::Builder::new()
        .name("kokoro-speech-worker".to_owned())
        .spawn(move || run(command_rx, status_tx, store, preferences, worker_voice))
        .expect("speech worker thread should start");

    WorkerHandle {
        commands: command_tx,
        statuses: status_rx,
        selected_voice,
    }
}

fn run(
    commands: Receiver<WorkerCommand>,
    statuses: Sender<AppStatus>,
    store: ModelStore,
    preferences: VoicePreferencesStore,
    mut selected_voice: VoiceSettings,
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
                let result = preferences.save(&voice);
                if result.is_ok() {
                    selected_voice = voice;
                    if let Some(session) = session.as_mut() {
                        session.speaker.set_voice(selected_voice.clone());
                    }
                }
                result
            }
            WorkerCommand::Speak(text) => {
                if !model_ready {
                    send_status(&statuses, AppStatus::MissingModel);
                    continue;
                }
                speak(text, &selected_voice, &store, &statuses, &mut session)
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

fn install_model(store: &ModelStore, statuses: &Sender<AppStatus>) -> Result<()> {
    send_status(statuses, AppStatus::Downloading);
    InstallModel::new(store.clone())
        .execute()
        .context("Kokoro setup failed")
}

fn speak(
    text: SelectedText,
    selected_voice: &VoiceSettings,
    store: &ModelStore,
    statuses: &Sender<AppStatus>,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    if session.is_none() {
        send_status(statuses, AppStatus::Loading);
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voices_path())?;
        let inference_policy = synthesizer.inference_policy_label().to_owned();
        let audio_cache = store.root().join("audio-cache");
        let player = SystemAudioPlayer::new(&audio_cache)?;
        *session = Some(SpeechSession {
            speaker: SpeakSelection::new(synthesizer, player, selected_voice.clone()),
            inference_policy,
        });
    }

    let session = session
        .as_mut()
        .context("speech engine was not initialized")?;
    let preview = text.preview(32);
    let inference_policy = session.inference_policy.clone();
    send_status(
        statuses,
        AppStatus::Synthesizing {
            text: preview.clone(),
            inference_policy: inference_policy.clone(),
        },
    );
    session
        .speaker
        .execute_with_playback_cues(text, |features| {
            send_status(
                statuses,
                AppStatus::Speaking {
                    text: preview.clone(),
                    inference_policy: inference_policy.clone(),
                    features,
                },
            );
        })
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
            .send(WorkerCommand::Speak(
                SelectedText::new("Not installed yet").unwrap(),
            ))
            .unwrap();

        assert_eq!(handle.statuses.recv().unwrap(), AppStatus::MissingModel);
    }
}
