use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use sha2::{Digest, Sha256};

use crate::ports::ModelProvisioner;

pub const MODEL_REVISION: &str = "1939ad2a8e416c0acfeecc08a694d14ef25f2231";
pub const MODEL_URL: &str = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/onnx/model_quantized.onnx?download=true";
pub const VOICE_URL: &str = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/voices/af_heart.bin?download=true";
const MIN_MODEL_BYTES: u64 = 80 * 1024 * 1024;
const MIN_VOICE_BYTES: u64 = 500 * 1024;
const MODEL_SHA256: &str = "fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478";
const VOICE_SHA256: &str = "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b";

#[derive(Clone, Debug)]
pub struct ModelStore {
    root: PathBuf,
}

impl ModelStore {
    pub fn discover() -> Result<Self> {
        if let Some(root) = env::var_os("SELECT_TO_SPEAK_MODEL_DIR") {
            return Ok(Self::at(root));
        }

        let directories = ProjectDirs::from("dev", "Akshobhya", "SelectToSpeak")
            .context("could not locate the platform application data directory")?;
        Ok(Self::at(directories.data_dir().join("models")))
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { root: path.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn model_path(&self) -> PathBuf {
        self.root.join("model_quantized.onnx")
    }

    pub fn voice_path(&self) -> PathBuf {
        self.root.join("af_heart.bin")
    }

    pub fn is_ready(&self) -> bool {
        artifact_is_valid(&self.model_path(), MIN_MODEL_BYTES, MODEL_SHA256)
            && artifact_is_valid(&self.voice_path(), MIN_VOICE_BYTES, VOICE_SHA256)
    }

    pub fn install(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        download(MODEL_URL, &self.model_path(), MIN_MODEL_BYTES, MODEL_SHA256)?;
        download(VOICE_URL, &self.voice_path(), MIN_VOICE_BYTES, VOICE_SHA256)?;
        Ok(())
    }
}

impl ModelProvisioner for ModelStore {
    fn is_ready(&self) -> bool {
        Self::is_ready(self)
    }

    fn install(&self) -> Result<()> {
        Self::install(self)
    }
}

fn artifact_is_valid(path: &Path, minimum_bytes: u64, expected_sha256: &str) -> bool {
    if !path
        .metadata()
        .is_ok_and(|metadata| metadata.len() >= minimum_bytes)
    {
        return false;
    }

    sha256(path).is_ok_and(|digest| digest == expected_sha256)
}

fn sha256(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("could not verify {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .context("could not verify model artifact")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(format!("{digest:x}"))
}

fn download(
    url: &str,
    destination: &Path,
    minimum_bytes: u64,
    expected_sha256: &str,
) -> Result<()> {
    if artifact_is_valid(destination, minimum_bytes, expected_sha256) {
        return Ok(());
    }

    let temporary = destination.with_extension("download");
    let mut response = ureq::get(url)
        .call()
        .with_context(|| format!("could not download {url}"))?;
    let mut reader = response.body_mut().as_reader();
    let mut file = fs::File::create(&temporary)
        .with_context(|| format!("could not create {}", temporary.display()))?;
    if let Err(error) = io::copy(&mut reader, &mut file) {
        let _ = fs::remove_file(&temporary);
        return Err(error).context("model download was interrupted");
    }

    drop(file);
    if !artifact_is_valid(&temporary, minimum_bytes, expected_sha256) {
        let _ = fs::remove_file(&temporary);
        bail!("downloaded model artifact failed its integrity check");
    }

    // Windows does not replace an existing file during rename. At this point
    // the complete replacement is safely present beside the known artifact.
    if destination.exists() {
        fs::remove_file(destination)
            .with_context(|| format!("could not replace {}", destination.display()))?;
    }

    fs::rename(&temporary, destination)
        .with_context(|| format!("could not save {}", destination.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_without_artifacts_is_not_ready() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelStore::at(temporary.path());
        assert!(!store.is_ready());
    }

    #[test]
    fn undersized_artifacts_are_not_ready() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelStore::at(temporary.path());
        fs::write(store.model_path(), b"not a model").unwrap();
        fs::write(store.voice_path(), b"not a voice").unwrap();
        assert!(!store.is_ready());
    }

    #[test]
    fn verifies_an_artifact_checksum() {
        let temporary = tempfile::NamedTempFile::new().unwrap();
        fs::write(temporary.path(), b"abc").unwrap();
        assert!(artifact_is_valid(
            temporary.path(),
            3,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));
    }

    #[test]
    fn download_urls_are_pinned_to_the_verified_revision() {
        assert!(MODEL_URL.contains(MODEL_REVISION));
        assert!(VOICE_URL.contains(MODEL_REVISION));
    }
}
