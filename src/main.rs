#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod ui;

fn main() -> eframe::Result {
    use std::{env, sync::Arc};

    use mist::{
        PlaybackController, SelectedText, SpeakSelection,
        adapters::{
            kokoro::{
                KokoroEngineFactory, KokoroSynthesizer, configure_inference_provider,
                provider_capabilities,
            },
            kokoro_catalog::KokoroVoiceCatalog,
            model_preferences::ModelPreferencesStore,
            playback_preferences::PlaybackPreferencesStore,
            system_audio::SystemAudioPlayer,
            voice_preferences::VoicePreferencesStore,
        },
        model_store::ModelStore,
        ports::VoiceCatalog,
        worker,
    };

    let store = ModelStore::discover().expect("could not find the application data directory");
    let model_preferences_store = ModelPreferencesStore::at(store.root());
    let provider_capabilities = provider_capabilities();
    let default_provider = provider_capabilities
        .first()
        .expect("the speech adapter must expose a default provider")
        .id
        .clone();
    let requested_provider =
        model_preferences_store.load(&default_provider, &provider_capabilities);
    let model_provider = provider_capabilities
        .iter()
        .find(|capability| capability.id == requested_provider && capability.available)
        .map(|capability| capability.id.clone())
        .unwrap_or(default_provider);
    configure_inference_provider(&model_provider);
    let voice_catalog: Arc<dyn VoiceCatalog> = Arc::new(KokoroVoiceCatalog);
    let arguments: Vec<String> = env::args().skip(1).collect();
    let open_settings = arguments.first().is_some_and(|value| value == "--settings");

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
        let preferences = PlaybackPreferencesStore::at(store.root()).load();
        let mut voice = VoicePreferencesStore::at(store.root()).load(voice_catalog.as_ref());
        voice.speed = preferences.speed.multiplier();
        let synthesizer = KokoroSynthesizer::load(&store.model_path(), &store.voices_path())
            .expect("Kokoro is not ready; run --install-model first");
        let player = SystemAudioPlayer::new(
            &store.root().join("audio-cache"),
            PlaybackController::default(),
        )
        .expect("could not create the audio cache");
        let mut speaker = SpeakSelection::new(synthesizer, player, voice);
        speaker.set_playback_mode(preferences.mode);
        speaker.execute(text).expect("could not speak the text");
        return Ok(());
    }

    let playback_preferences_store = PlaybackPreferencesStore::at(store.root());
    let playback_preferences = playback_preferences_store.load();
    let engine_factory = Arc::new(KokoroEngineFactory::new(
        store.model_path(),
        store.voices_path(),
    ));
    let speech_worker = worker::spawn(
        store.root().to_owned(),
        Arc::new(store),
        engine_factory,
        voice_catalog.clone(),
    );
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Mist")
            .with_inner_size(ui::MIST_WINDOW)
            .with_min_inner_size(ui::MIST_WINDOW)
            .with_max_inner_size([860.0, 760.0])
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
                speech_worker,
                voice_catalog,
                ui::AppSettings {
                    playback: playback_preferences,
                    playback_store: playback_preferences_store,
                    model_provider,
                    model_store: model_preferences_store,
                    provider_capabilities,
                    open_panel: open_settings,
                },
            )))
        }),
    )
}
