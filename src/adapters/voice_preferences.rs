use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::{VoiceId, VoiceSettings};

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

    pub fn load(&self) -> VoiceSettings {
        let fallback = VoiceSettings::default();
        let Ok(contents) = fs::read_to_string(self.path()) else {
            return fallback;
        };
        let mut lines = contents.lines();
        let Some(voice_id) = lines.next().and_then(VoiceId::parse) else {
            return fallback;
        };
        let Some(speed) = lines.next().and_then(|value| value.parse::<f32>().ok()) else {
            return fallback;
        };
        if !(0.5..=2.0).contains(&speed) {
            return fallback;
        }
        VoiceSettings { voice_id, speed }
    }

    pub fn save(&self, settings: &VoiceSettings) -> Result<()> {
        if !(0.5..=2.0).contains(&settings.speed) {
            bail!("voice speed must be between 0.5 and 2.0");
        }
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        let destination = self.path();
        let temporary = destination.with_extension("new");
        fs::write(
            &temporary,
            format!("{}\n{}\n", settings.voice_id.as_str(), settings.speed),
        )
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

    #[test]
    fn persists_a_valid_voice_selection() {
        let temporary = tempfile::tempdir().unwrap();
        let preferences = VoicePreferencesStore::at(temporary.path());
        let selected = VoiceSettings::from_voice_id("am_michael").unwrap();

        preferences.save(&selected).unwrap();

        assert_eq!(preferences.load(), selected);
    }

    #[test]
    fn invalid_or_missing_preferences_fall_back_to_default_voice() {
        let temporary = tempfile::tempdir().unwrap();
        let preferences = VoicePreferencesStore::at(temporary.path());
        assert_eq!(preferences.load(), VoiceSettings::default());

        fs::write(preferences.path(), "not-a-real-voice\n9.0\n").unwrap();
        assert_eq!(preferences.load(), VoiceSettings::default());
    }
}
