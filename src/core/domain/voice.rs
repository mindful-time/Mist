#[derive(Clone, Debug, PartialEq)]
pub struct VoiceSettings {
    pub voice_id: VoiceId,
    pub speed: f32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VoiceId(String);

impl VoiceId {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
    pub language: LanguageId,
    pub palette: MistPalette,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LanguageId(&'static str);

impl LanguageId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageProfile {
    pub id: LanguageId,
    pub display_name: &'static str,
}

impl VoiceSettings {
    pub fn new(id: impl Into<String>) -> Option<Self> {
        VoiceId::new(id).map(|voice_id| Self {
            voice_id,
            speed: 1.0,
        })
    }
}
