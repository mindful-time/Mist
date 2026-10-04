use mist::{
    SelectedText, SpeechSynthesizer,
    adapters::{kokoro::KokoroSynthesizer, kokoro_catalog::KokoroVoiceCatalog},
    model_store::ModelStore,
    ports::VoiceCatalog,
};

/// End-to-end model check. Native Intel CI explicitly downloads and verifies
/// the model before running this; ordinary unit-test runs do not download it.
#[test]
#[ignore = "requires MIST_MODEL_DIR with downloaded Kokoro files"]
fn local_kokoro_model_produces_audio() {
    let store = ModelStore::discover().expect("model store");
    assert!(store.is_ready(), "run mist --install-model first");

    let mut synthesizer =
        KokoroSynthesizer::load(&store.model_path(), &store.voices_path()).expect("load Kokoro");
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    assert_eq!(
        synthesizer.runtime_backend_label(),
        "CoreMLExecutionProvider (device 0)",
        "Apple Silicon must try the packaged Core ML provider before CPU"
    );
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    assert_eq!(
        synthesizer.runtime_backend_label(),
        "CPU",
        "Intel macOS uses the pinned, statically linked CPU runtime"
    );
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
                .all(|sample| sample.is_finite()),
            "voice {voice} produced non-finite audio"
        );
        assert!(
            chunks
                .iter()
                .flat_map(|audio| &audio.samples)
                .any(|sample| sample.abs() > 0.0001)
        );
    }
}
