use std::{
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    thread,
};

use anyhow::{Context, Result};

use crate::{
    InstallModel, SpeakSelection, VoiceSettings,
    adapters::{kokoro::KokoroSynthesizer, system_audio::SystemAudioPlayer},
    domain::SelectedText,
    model_store::ModelStore,
};

#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Speak(SelectedText),
    InstallModel,
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
}

pub fn spawn(store: ModelStore) -> WorkerHandle {
    let (command_tx, command_rx) = mpsc::sync_channel(8);
    let (status_tx, status_rx) = mpsc::channel();

    thread::Builder::new()
        .name("kokoro-speech-worker".to_owned())
        .spawn(move || run(command_rx, status_tx, store))
        .expect("speech worker thread should start");

    WorkerHandle {
        commands: command_tx,
        statuses: status_rx,
    }
}

fn run(commands: Receiver<WorkerCommand>, statuses: Sender<AppStatus>, store: ModelStore) {
    let mut session = None;
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
            WorkerCommand::Speak(text) => speak(text, model_ready, &store, &statuses, &mut session),
        };

        match result {
            Ok(()) => send_status(&statuses, AppStatus::Ready),
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
    model_ready: bool,
    store: &ModelStore,
    statuses: &Sender<AppStatus>,
    session: &mut Option<SpeechSession>,
) -> Result<()> {
    if !model_ready {
        send_status(statuses, AppStatus::MissingModel);
        anyhow::bail!("Download Kokoro from the pet first");
    }

    if session.is_none() {
        send_status(statuses, AppStatus::Loading);
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voice_path())?;
        let inference_policy = synthesizer.inference_policy_label().to_owned();
        let audio_cache = store.root().join("audio-cache");
        let player = SystemAudioPlayer::new(&audio_cache)?;
        *session = Some(SpeechSession {
            speaker: SpeakSelection::new(synthesizer, player, VoiceSettings::default()),
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
    session.speaker.execute_with_playback_started(text, || {
        send_status(
            statuses,
            AppStatus::Speaking {
                text: preview,
                inference_policy,
            },
        );
    })
}

fn send_status(statuses: &Sender<AppStatus>, status: AppStatus) {
    let _ = statuses.send(status);
}
