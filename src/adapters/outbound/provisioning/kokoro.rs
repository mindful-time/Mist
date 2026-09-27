//! Kokoro model bundle provisioning adapter.

use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use sha2::{Digest, Sha256};

use crate::ports::ModelProvisioner;

pub const MODEL_REVISION: &str = "d9d564ee264fcd95459552767c8c713ed385c999";
pub const MODEL_URL: &str = "https://media.githubusercontent.com/media/8b-is/kokoro-tiny/d9d564ee264fcd95459552767c8c713ed385c999/models/0.onnx";
pub const VOICES_URL: &str = "https://media.githubusercontent.com/media/8b-is/kokoro-tiny/d9d564ee264fcd95459552767c8c713ed385c999/models/0.bin";
const MIN_MODEL_BYTES: u64 = 300 * 1024 * 1024;
const MIN_VOICES_BYTES: u64 = 25 * 1024 * 1024;
const MODEL_SHA256: &str = "7d5df8ecf7d4b1878015a32686053fd0eebe2bc377234608764cc0ef3636a6c5";
const VOICES_SHA256: &str = "bca610b8308e8d99f32e6fe4197e7ec01679264efed0cac9140fe9c29f1fbf7d";

#[derive(Clone, Debug)]
pub struct ModelStore {
    root: PathBuf,
}

impl ModelStore {
    pub fn discover() -> Result<Self> {
        if let Some(root) =
            env::var_os("MIST_MODEL_DIR").or_else(|| env::var_os("SELECT_TO_SPEAK_MODEL_DIR"))
        {
            return Ok(Self::at(root));
        }

        // Keep the established data directory so the Mist rename preserves
        // the stable application identity and model location.
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
        self.root.join("kokoro-v1.0.onnx")
    }

    pub fn voices_path(&self) -> PathBuf {
        self.root.join("kokoro-voices-v1.0.npz")
    }

    pub fn is_ready(&self) -> bool {
        artifact_is_valid(&self.model_path(), MIN_MODEL_BYTES, MODEL_SHA256)
            && artifact_is_valid(&self.voices_path(), MIN_VOICES_BYTES, VOICES_SHA256)
    }

    pub fn install(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        download(MODEL_URL, &self.model_path(), MIN_MODEL_BYTES, MODEL_SHA256)?;
        download(
            VOICES_URL,
            &self.voices_path(),
            MIN_VOICES_BYTES,
            VOICES_SHA256,
        )?;
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
        fs::write(store.voices_path(), b"not voices").unwrap();
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
        assert!(VOICES_URL.contains(MODEL_REVISION));
    }

    #[test]
    fn multilingual_bundle_has_one_verified_model_and_voice_pack() {
        assert_minimum_bundle_size(MIN_MODEL_BYTES, MIN_VOICES_BYTES);
        assert_ne!(MODEL_SHA256, VOICES_SHA256);
    }

    fn assert_minimum_bundle_size(model_bytes: u64, voices_bytes: u64) {
        assert!(model_bytes >= 300 * 1024 * 1024);
        assert!(voices_bytes >= 25 * 1024 * 1024);
    }
}
