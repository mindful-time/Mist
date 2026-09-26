use std::time::Duration;

use thiserror::Error;

/// A validated piece of text received from an OS selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedText(String);

impl SelectedText {
    pub const MAX_CHARACTERS: usize = 12_000;

    pub fn new(value: impl Into<String>) -> Result<Self, SelectionError> {
        let value = value.into();
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(SelectionError::Empty);
        }

        let character_count = trimmed.chars().count();
        if character_count > Self::MAX_CHARACTERS {
            return Err(SelectionError::TooLong {
                actual: character_count,
                maximum: Self::MAX_CHARACTERS,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn preview(&self, maximum: usize) -> String {
        let mut preview: String = self.0.chars().take(maximum).collect();
        if self.0.chars().count() > maximum {
            preview.push('…');
        }
        preview
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SelectionError {
    #[error("Select some text first")]
    Empty,
    #[error("The selection has {actual} characters; the limit is {maximum}")]
    TooLong { actual: usize, maximum: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceSettings {
    pub voice_id: VoiceId,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VoiceId(&'static str);

impl VoiceId {
    pub fn parse(value: &str) -> Option<Self> {
        VOICE_CATALOG
            .iter()
            .find(|voice| voice.id == value)
            .map(|voice| Self(voice.id))
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MistPalette {
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
    pub glow: [u8; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoiceProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub character: &'static str,
    pub palette: MistPalette,
}

pub const VOICE_CATALOG: &[VoiceProfile] = &[
    VoiceProfile {
        id: "af_heart",
        display_name: "Heart",
        character: "Warm · bright",
        palette: MistPalette {
            primary: [255, 130, 173],
            secondary: [247, 182, 226],
            glow: [255, 224, 239],
        },
    },
    VoiceProfile {
        id: "af_bella",
        display_name: "Velvet",
        character: "Rich · expressive",
        palette: MistPalette {
            primary: [163, 102, 255],
            secondary: [239, 111, 192],
            glow: [230, 210, 255],
        },
    },
    VoiceProfile {
        id: "af_nicole",
        display_name: "Hush",
        character: "Low · intimate",
        palette: MistPalette {
            primary: [84, 113, 255],
            secondary: [118, 212, 255],
            glow: [202, 226, 255],
        },
    },
    VoiceProfile {
        id: "af_sarah",
        display_name: "Aurora",
        character: "Clear · composed",
        palette: MistPalette {
            primary: [82, 219, 203],
            secondary: [124, 157, 255],
            glow: [209, 255, 245],
        },
    },
    VoiceProfile {
        id: "am_adam",
        display_name: "Ember",
        character: "Deep · steady",
        palette: MistPalette {
            primary: [255, 132, 72],
            secondary: [255, 198, 92],
            glow: [255, 230, 190],
        },
    },
    VoiceProfile {
        id: "am_michael",
        display_name: "Tide",
        character: "Warm · grounded",
        palette: MistPalette {
            primary: [35, 175, 205],
            secondary: [91, 231, 190],
            glow: [190, 251, 245],
        },
    },
    VoiceProfile {
        id: "bf_emma",
        display_name: "Lilt",
        character: "British · luminous",
        palette: MistPalette {
            primary: [193, 113, 255],
            secondary: [118, 154, 255],
            glow: [234, 218, 255],
        },
    },
    VoiceProfile {
        id: "bm_daniel",
        display_name: "Vale",
        character: "British · measured",
        palette: MistPalette {
            primary: [101, 133, 164],
            secondary: [166, 199, 210],
            glow: [221, 236, 239],
        },
    },
];

pub fn voice_profile(id: &str) -> Option<&'static VoiceProfile> {
    VOICE_CATALOG.iter().find(|voice| voice.id == id)
}

impl VoiceSettings {
    pub fn from_voice_id(id: &str) -> Option<Self> {
        VoiceId::parse(id).map(|voice_id| Self {
            voice_id,
            speed: 1.0,
        })
    }
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            voice_id: VoiceId("af_heart"),
            speed: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioFeatures {
    pub energy: u8,
    pub brightness: u8,
}

impl Audio {
    pub const KOKORO_SAMPLE_RATE: u32 = 24_000;

    pub fn kokoro(samples: Vec<f32>) -> Self {
        Self {
            samples,
            sample_rate: Self::KOKORO_SAMPLE_RATE,
        }
    }

    /// Root-mean-square energy mapped to a compact presentation signal.
    pub fn energy(&self) -> u8 {
        features(&self.samples).energy
    }

    /// A short, playback-synchronised window. Energy follows loudness while
    /// brightness approximates high-frequency content from sample deltas.
    pub fn features_at(&self, elapsed: Duration, window: Duration) -> AudioFeatures {
        if self.samples.is_empty() || self.sample_rate == 0 {
            return AudioFeatures {
                energy: 0,
                brightness: 0,
            };
        }
        let center = (elapsed.as_secs_f64() * f64::from(self.sample_rate)) as usize;
        let width = (window.as_secs_f64() * f64::from(self.sample_rate)) as usize;
        let half = width.max(1) / 2;
        let start = center.saturating_sub(half).min(self.samples.len());
        let end = center.saturating_add(half).min(self.samples.len());
        features(&self.samples[start..end])
    }
}

fn features(samples: &[f32]) -> AudioFeatures {
    if samples.is_empty() {
        return AudioFeatures {
            energy: 0,
            brightness: 0,
        };
    }
    let mean_square = samples
        .iter()
        .map(|sample| sample.clamp(-1.0, 1.0).powi(2))
        .sum::<f32>()
        / samples.len() as f32;
    let rms = mean_square.sqrt().clamp(0.0, 1.0);
    let difference_rms = if samples.len() < 2 {
        0.0
    } else {
        (samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).powi(2))
            .sum::<f32>()
            / (samples.len() - 1) as f32)
            .sqrt()
    };
    let brightness = if rms <= f32::EPSILON {
        0.0
    } else {
        (difference_rms / (2.0 * rms)).clamp(0.0, 1.0)
    };
    AudioFeatures {
        energy: (rms * u8::MAX as f32).round() as u8,
        brightness: (brightness * u8::MAX as f32).round() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_trims_outer_whitespace() {
        let selected = SelectedText::new("  hello world\n").unwrap();
        assert_eq!(selected.as_str(), "hello world");
    }

    #[test]
    fn selection_rejects_blank_text() {
        assert_eq!(
            SelectedText::new(" \n\t ").unwrap_err(),
            SelectionError::Empty
        );
    }

    #[test]
    fn preview_is_unicode_safe() {
        let selected = SelectedText::new("你好世界").unwrap();
        assert_eq!(selected.preview(2), "你好…");
    }

    #[test]
    fn voice_catalog_has_stable_unique_ids_and_palettes() {
        assert_eq!(VOICE_CATALOG.len(), 8);
        for (index, voice) in VOICE_CATALOG.iter().enumerate() {
            assert!(voice.id.contains('_'));
            assert!(!voice.display_name.is_empty());
            assert_ne!(voice.palette.primary, voice.palette.secondary);
            assert!(
                VOICE_CATALOG[index + 1..]
                    .iter()
                    .all(|candidate| candidate.id != voice.id)
            );
        }
    }

    #[test]
    fn voice_settings_reject_unknown_catalog_entries() {
        assert!(VoiceSettings::from_voice_id("af_bella").is_some());
        assert!(VoiceSettings::from_voice_id("unknown").is_none());
    }

    #[test]
    fn audio_energy_tracks_silence_and_peak_signal() {
        assert_eq!(Audio::kokoro(vec![0.0; 8]).energy(), 0);
        assert_eq!(Audio::kokoro(vec![1.0, -1.0]).energy(), 255);
        assert!(Audio::kokoro(vec![0.25, -0.25]).energy() > 0);
    }

    #[test]
    fn audio_features_distinguish_soft_and_bright_windows() {
        let mut samples = vec![0.05; 2_400];
        samples.extend([1.0, -1.0].into_iter().cycle().take(2_400));
        let audio = Audio::kokoro(samples);
        let soft = audio.features_at(Duration::from_millis(25), Duration::from_millis(40));
        let bright = audio.features_at(Duration::from_millis(150), Duration::from_millis(40));
        assert!(bright.energy > soft.energy);
        assert!(bright.brightness > soft.brightness);
    }
}
