#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod ui;

fn main() -> eframe::Result {
    use std::env;

    use select_to_speak::{
        SelectedText, SpeakSelection, VoiceSettings,
        adapters::{kokoro::KokoroSynthesizer, system_audio::SystemAudioPlayer},
        model_store::ModelStore,
        worker,
    };

    let store = ModelStore::discover().expect("could not find the application data directory");
    let arguments: Vec<String> = env::args().skip(1).collect();

    if arguments
        .first()
        .is_some_and(|value| value == "--install-model")
    {
        store.install().expect("Kokoro model download failed");
        println!("Kokoro is ready in {}", store.root().display());
        return Ok(());
    }

    if arguments.first().is_some_and(|value| value == "--speak") {
        let text = SelectedText::new(arguments[1..].join(" ")).expect("provide text after --speak");
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voice_path())
            .expect("Kokoro is not ready; run --install-model first");
        let player = SystemAudioPlayer::new(&store.root().join("audio-cache"))
            .expect("could not create the audio cache");
        SpeakSelection::new(synthesizer, player, VoiceSettings::default())
            .execute(text)
            .expect("could not speak the text");
        return Ok(());
    }

    let worker::WorkerHandle { commands, statuses } = worker::spawn(store);
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Select to Speak")
            .with_inner_size([392.0, 272.0])
            .with_min_inner_size([392.0, 272.0])
            .with_max_inner_size([392.0, 272.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "Select to Speak",
        native_options,
        Box::new(move |creation_context| {
            Ok(Box::new(ui::PetApp::new(
                creation_context,
                commands,
                statuses,
            )))
        }),
    )
}
