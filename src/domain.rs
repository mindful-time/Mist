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
    pub name: String,
    pub speed: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            name: "af_heart".to_owned(),
            speed: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Audio {
    pub const KOKORO_SAMPLE_RATE: u32 = 24_000;

    pub fn kokoro(samples: Vec<f32>) -> Self {
        Self {
            samples,
            sample_rate: Self::KOKORO_SAMPLE_RATE,
        }
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
}
