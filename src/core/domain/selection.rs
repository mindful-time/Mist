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

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SelectionCaptureError {
    #[error("Accessibility permission is required to read selected text")]
    PermissionRequired,
    #[error("Mist will not read or copy text from a protected field")]
    ProtectedContent,
    #[error("Mist could not verify that the focused content is safe to copy")]
    ProtectionUnknown,
    #[error("Select some text first")]
    NoSelection,
}
