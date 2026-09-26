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
    MissingModel,
    Ready,
    Downloading,
    Loading,
    Speaking(String),
    Error(String),
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
    let mut speaker = None;
    send_status(
        &statuses,
        if store.is_ready() {
            AppStatus::Ready
        } else {
            AppStatus::MissingModel
        },
    );

    while let Ok(command) = commands.recv() {
        let result = match command {
            WorkerCommand::InstallModel => install_model(&store, &statuses),
            WorkerCommand::Speak(text) => speak(text, &store, &statuses, &mut speaker),
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
    store: &ModelStore,
    statuses: &Sender<AppStatus>,
    speaker: &mut Option<SpeakSelection<KokoroSynthesizer, SystemAudioPlayer>>,
) -> Result<()> {
    if !store.is_ready() {
        send_status(statuses, AppStatus::MissingModel);
        anyhow::bail!("Download Kokoro from the pet first");
    }

    if speaker.is_none() {
        send_status(statuses, AppStatus::Loading);
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voice_path())?;
        let audio_cache = store.root().join("audio-cache");
        let player = SystemAudioPlayer::new(&audio_cache)?;
        *speaker = Some(SpeakSelection::new(
            synthesizer,
            player,
            VoiceSettings::default(),
        ));
    }

    send_status(statuses, AppStatus::Speaking(text.preview(32)));
    speaker
        .as_mut()
        .context("speech engine was not initialized")?
        .execute(text)
}

fn send_status(statuses: &Sender<AppStatus>, status: AppStatus) {
    let _ = statuses.send(status);
}
