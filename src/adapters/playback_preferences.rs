use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

use crate::domain::{PlaybackMode, PlaybackPreferences, PlaybackSpeed};

#[derive(Clone, Debug)]
pub struct PlaybackPreferencesStore {
    root: PathBuf,
}

impl PlaybackPreferencesStore {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load(&self) -> PlaybackPreferences {
        let Ok(contents) = fs::read_to_string(self.path()) else {
            return PlaybackPreferences::default();
        };
        let mut lines = contents.lines();
        let Some(auto_play_queue) = lines.next().and_then(parse_bool) else {
            return PlaybackPreferences::default();
        };
        let Some(automatic_clipboard_fallback) = lines.next().and_then(parse_bool) else {
            return PlaybackPreferences::default();
        };
        let mode = match lines.next() {
            Some(value) => match parse_bool(value) {
                Some(true) => PlaybackMode::RealTime,
                Some(false) => PlaybackMode::CompleteAudio,
                None => return PlaybackPreferences::default(),
            },
            None => PlaybackMode::RealTime,
        };
        let speed = lines
            .next()
            .and_then(|value| value.parse::<u16>().ok())
            .and_then(PlaybackSpeed::from_percent)
            .unwrap_or_default();
        PlaybackPreferences {
            auto_play_queue,
            automatic_clipboard_fallback,
            mode,
            speed,
        }
    }

    pub fn save(&self, preferences: PlaybackPreferences) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        let destination = self.path();
        let temporary = destination.with_extension("new");
        fs::write(
            &temporary,
            format!(
                "{}\n{}\n{}\n{}\n",
                preferences.auto_play_queue,
                preferences.automatic_clipboard_fallback,
                preferences.mode.streams_audio(),
                preferences.speed.percent(),
            ),
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
        self.root.join("playback-preferences")
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_preferences_round_trip_and_default_to_the_seamless_flow() {
        let temporary = tempfile::tempdir().unwrap();
        let store = PlaybackPreferencesStore::at(temporary.path());

        assert_eq!(store.load(), PlaybackPreferences::default());

        let preferences = PlaybackPreferences {
            auto_play_queue: false,
            automatic_clipboard_fallback: false,
            mode: PlaybackMode::CompleteAudio,
            speed: PlaybackSpeed::from_percent(125).unwrap(),
        };
        store.save(preferences).unwrap();

        assert_eq!(store.load(), preferences);

        fs::write(store.path(), "maybe\nfalse\n").unwrap();
        assert_eq!(store.load(), PlaybackPreferences::default());
    }

    #[test]
    fn legacy_preferences_gain_streaming_as_the_default() {
        let temporary = tempfile::tempdir().unwrap();
        let store = PlaybackPreferencesStore::at(temporary.path());

        fs::write(store.path(), "false\nfalse\n").unwrap();

        assert_eq!(
            store.load(),
            PlaybackPreferences {
                auto_play_queue: false,
                automatic_clipboard_fallback: false,
                mode: PlaybackMode::RealTime,
                speed: PlaybackSpeed::default(),
            }
        );
    }

    #[test]
    fn legacy_streaming_preference_gains_normal_speed() {
        let temporary = tempfile::tempdir().unwrap();
        let store = PlaybackPreferencesStore::at(temporary.path());

        fs::write(store.path(), "true\ntrue\nfalse\n").unwrap();

        assert_eq!(store.load().mode, PlaybackMode::CompleteAudio);
        assert_eq!(store.load().speed, PlaybackSpeed::default());
    }
}
