use select_to_speak::{
    SelectedText, SpeechSynthesizer, VoiceSettings, adapters::kokoro::KokoroSynthesizer,
    model_store::ModelStore,
};

/// Optional end-to-end model check. It is ignored in normal CI because the
/// Kokoro model is downloaded data, not a repository fixture.
#[test]
#[ignore = "requires SELECT_TO_SPEAK_MODEL_DIR with downloaded Kokoro files"]
fn local_kokoro_model_produces_audio() {
    let store = ModelStore::discover().expect("model store");
    assert!(
        store.is_ready(),
        "run select-to-speak --install-model first"
    );

    let mut synthesizer =
        KokoroSynthesizer::load(&store.model_path(), &store.voice_path()).expect("load Kokoro");
    let audio = synthesizer
        .synthesize(
            &SelectedText::new("Hello from Kokoro.").unwrap(),
            &VoiceSettings::default(),
        )
        .expect("synthesize speech");

    assert_eq!(audio.sample_rate, 24_000);
    assert!(!audio.samples.is_empty());
    assert!(audio.samples.iter().any(|sample| sample.abs() > 0.0001));
}
