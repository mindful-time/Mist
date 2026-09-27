#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackMode {
    RealTime,
    CompleteAudio,
}

impl PlaybackMode {
    pub const fn streams_audio(self) -> bool {
        matches!(self, Self::RealTime)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackSpeed(u16);

impl PlaybackSpeed {
    pub const MIN_PERCENT: u16 = 50;
    pub const MAX_PERCENT: u16 = 300;
    pub const STEP_PERCENT: u16 = 25;

    pub const fn from_percent(percent: u16) -> Option<Self> {
        if percent >= Self::MIN_PERCENT && percent <= Self::MAX_PERCENT {
            Some(Self(percent))
        } else {
            None
        }
    }

    pub const fn percent(self) -> u16 {
        self.0
    }

    pub fn multiplier(self) -> f32 {
        f32::from(self.0) / 100.0
    }
}

impl Default for PlaybackSpeed {
    fn default() -> Self {
        Self(100)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackPreferences {
    pub auto_play_queue: bool,
    pub automatic_clipboard_fallback: bool,
    pub mode: PlaybackMode,
    pub speed: PlaybackSpeed,
}

impl Default for PlaybackPreferences {
    fn default() -> Self {
        Self {
            auto_play_queue: true,
            automatic_clipboard_fallback: true,
            mode: PlaybackMode::RealTime,
            speed: PlaybackSpeed::default(),
        }
    }
}
