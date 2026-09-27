//! Filesystem persistence for the selected adapter-owned voice identifier.

use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

use crate::{domain::VoiceSettings, ports::VoiceCatalog};

/// Filesystem adapter for the user's selected voice. Invalid or stale values
/// safely fall back to the product default.
#[derive(Clone, Debug)]
pub struct VoicePreferencesStore {
    root: PathBuf,
}

impl VoicePreferencesStore {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load(&self, catalog: &dyn VoiceCatalog) -> VoiceSettings {
        let fallback = catalog.default_settings();
        let Ok(contents) = fs::read_to_string(self.path()) else {
            return fallback;
        };
        contents
            .lines()
            .next()
            .and_then(|id| catalog.settings(id))
            .unwrap_or(fallback)
    }

    pub fn save(&self, settings: &VoiceSettings) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        let destination = self.path();
        let temporary = destination.with_extension("new");
        fs::write(&temporary, format!("{}\n", settings.voice_id.as_str()))
            .with_context(|| format!("could not write {}", temporary.display()))?;
        if destination.exists() {
            fs::remove_file(&destination)
                .with_context(|| format!("could not replace {}", destination.display()))?;
        }
        fs::rename(&temporary, &destination)
            .with_context(|| format!("could not save {}", destination.display()))
    }

    fn path(&self) -> PathBuf {
        self.root.join("voice-preferences")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{adapters::kokoro_catalog::KokoroVoiceCatalog, ports::VoiceCatalog};

    #[test]
    fn persists_only_the_voice_selection() {
        let temporary = tempfile::tempdir().unwrap();
        let preferences = VoicePreferencesStore::at(temporary.path());
        let catalog = KokoroVoiceCatalog;
        let mut selected = catalog.settings("am_michael").unwrap();
        selected.speed = 3.0;

        preferences.save(&selected).unwrap();

        assert_eq!(
            preferences.load(&catalog),
            catalog.settings("am_michael").unwrap()
        );
        assert_eq!(
            fs::read_to_string(preferences.path()).unwrap(),
            "am_michael\n"
        );
    }

    #[test]
    fn ignores_speed_from_legacy_preferences() {
        let temporary = tempfile::tempdir().unwrap();
        let preferences = VoicePreferencesStore::at(temporary.path());
        let catalog = KokoroVoiceCatalog;
        fs::write(preferences.path(), "am_michael\n2.0\n").unwrap();

        assert_eq!(
            preferences.load(&catalog),
            catalog.settings("am_michael").unwrap()
        );
    }

    #[test]
    fn invalid_or_missing_preferences_fall_back_to_default_voice() {
        let temporary = tempfile::tempdir().unwrap();
        let preferences = VoicePreferencesStore::at(temporary.path());
        let catalog = KokoroVoiceCatalog;
        assert_eq!(preferences.load(&catalog), catalog.default_settings());

        fs::write(preferences.path(), "not-a-real-voice\n9.0\n").unwrap();
        assert_eq!(preferences.load(&catalog), catalog.default_settings());
    }
}
