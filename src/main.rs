#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod ui;

fn main() -> eframe::Result {
    use std::env;

    use mist::{
        SelectedText, SpeakSelection,
        adapters::{
            kokoro::KokoroSynthesizer, playback_preferences::PlaybackPreferencesStore,
            system_audio::SystemAudioPlayer, voice_preferences::VoicePreferencesStore,
        },
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
        let voice = VoicePreferencesStore::at(store.root()).load();
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voices_path())
            .expect("Kokoro is not ready; run --install-model first");
        let player = SystemAudioPlayer::new(&store.root().join("audio-cache"))
            .expect("could not create the audio cache");
        SpeakSelection::new(synthesizer, player, voice)
            .execute(text)
            .expect("could not speak the text");
        return Ok(());
    }

    let playback_preferences_store = PlaybackPreferencesStore::at(store.root());
    let playback_preferences = playback_preferences_store.load();
    let worker::WorkerHandle {
        commands,
        statuses,
        selected_voice,
    } = worker::spawn(store);
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Mist")
            .with_inner_size(ui::MIST_WINDOW)
            .with_min_inner_size(ui::MIST_WINDOW)
            .with_max_inner_size([600.0, 680.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "Mist",
        native_options,
        Box::new(move |creation_context| {
            Ok(Box::new(ui::PetApp::new(
                creation_context,
                commands,
                statuses,
                selected_voice,
                playback_preferences,
                playback_preferences_store,
            )))
        }),
    )
}
