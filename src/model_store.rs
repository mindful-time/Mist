use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;

pub const MODEL_URL: &str = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/onnx/model_quantized.onnx?download=true";
pub const VOICE_URL: &str = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/voices/af_heart.bin?download=true";

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
            .context("could not locate the macOS Application Support directory")?;
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
        file_is_nonempty(&self.model_path()) && file_is_nonempty(&self.voice_path())
    }

    pub fn install(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        download(MODEL_URL, &self.model_path())?;
        download(VOICE_URL, &self.voice_path())?;
        Ok(())
    }
}

fn file_is_nonempty(path: &Path) -> bool {
    path.metadata().is_ok_and(|metadata| metadata.len() > 0)
}

fn download(url: &str, destination: &Path) -> Result<()> {
    if file_is_nonempty(destination) {
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

    fs::rename(&temporary, destination)
        .with_context(|| format!("could not save {}", destination.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_is_ready_only_when_both_files_exist() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelStore::at(temporary.path());
        assert!(!store.is_ready());

        fs::write(store.model_path(), b"model").unwrap();
        assert!(!store.is_ready());

        fs::write(store.voice_path(), b"voice").unwrap();
        assert!(store.is_ready());
    }
}
