use mist::{
    SelectedText, SpeechSynthesizer,
    adapters::{kokoro::KokoroSynthesizer, kokoro_catalog::KokoroVoiceCatalog},
    model_store::ModelStore,
    ports::VoiceCatalog,
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
    let catalog = KokoroVoiceCatalog;
    for (voice, sample) in [
        ("af_heart", "Hello from Mist."),
        ("ef_dora", "Hola desde Mist."),
        ("jf_alpha", "こんにちは、ミストです。"),
        ("zf_xiaoni", "你好，这里是 Mist。"),
    ] {
        let mut chunks = Vec::new();
        synthesizer
            .synthesize_streaming(
                &SelectedText::new(sample).unwrap(),
                &catalog.settings(voice).unwrap(),
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
