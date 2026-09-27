use mist::{
    SelectedText, SpeechSynthesizer, VoiceSettings, adapters::kokoro::KokoroSynthesizer,
    model_store::ModelStore,
};

/// Optional end-to-end model check. It is ignored in normal CI because the
/// Kokoro model is downloaded data, not a repository fixture.
#[test]
#[ignore = "requires MIST_MODEL_DIR with downloaded Kokoro files"]
fn local_kokoro_model_produces_audio() {
    let store = ModelStore::discover().expect("model store");
    assert!(store.is_ready(), "run mist --install-model first");

    let mut synthesizer =
        KokoroSynthesizer::load(&store.model_path(), &store.voices_path()).expect("load Kokoro");
    for voice in ["af_heart", "bm_daniel"] {
        let mut chunks = Vec::new();
        synthesizer
            .synthesize_streaming(
                &SelectedText::new("Hello from Kokoro.").unwrap(),
                &VoiceSettings::from_voice_id(voice).unwrap(),
                &mut |audio| {
                    chunks.push(audio);
                    Ok(())
                },
            )
            .expect("synthesize speech");

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|audio| audio.sample_rate == 24_000));
        assert!(
            chunks
                .iter()
                .flat_map(|audio| &audio.samples)
                .any(|sample| sample.abs() > 0.0001)
        );
    }
}
