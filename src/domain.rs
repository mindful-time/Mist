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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QueueItemId(u64);

impl QueueItemId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueItemState {
    Waiting,
    Preparing,
    Playing,
    Paused,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedSpeech {
    pub id: QueueItemId,
    pub text: SelectedText,
    pub state: QueueItemState,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum QueueError {
    #[error("The speech queue is full")]
    Full,
}

/// Ordered speech requested by the user. The queue owns text lifecycle while
/// OS adapters separately own any temporary clipboard lease.
#[derive(Debug, Default)]
pub struct SpeechQueue {
    items: Vec<QueuedSpeech>,
    next_id: u64,
}

impl SpeechQueue {
    pub const MAX_ITEMS: usize = 8;

    pub fn push(&mut self, text: SelectedText) -> Result<QueueItemId, QueueError> {
        if self.items.len() >= Self::MAX_ITEMS {
            return Err(QueueError::Full);
        }
        self.next_id = self.next_id.wrapping_add(1);
        let id = QueueItemId(self.next_id);
        self.items.push(QueuedSpeech {
            id,
            text,
            state: QueueItemState::Waiting,
        });
        Ok(id)
    }

    pub fn items(&self) -> &[QueuedSpeech] {
        &self.items
    }

    pub fn active_id(&self) -> Option<QueueItemId> {
        self.items
            .iter()
            .find(|item| {
                matches!(
                    item.state,
                    QueueItemState::Preparing | QueueItemState::Playing | QueueItemState::Paused
                )
            })
            .map(|item| item.id)
    }

    pub fn start_next(&mut self) -> Option<QueuedSpeech> {
        let id = self
            .items
            .iter()
            .find(|item| item.state == QueueItemState::Waiting)
            .map(|item| item.id)?;
        self.start(id)
    }

    pub fn start(&mut self, id: QueueItemId) -> Option<QueuedSpeech> {
        if self.active_id().is_some() {
            return None;
        }
        let position = self.items.iter().position(|item| item.id == id)?;
        let mut item = self.items.remove(position);
        item.state = QueueItemState::Preparing;
        self.items.insert(0, item.clone());
        Some(item)
    }

    pub fn mark_playing(&mut self, id: QueueItemId) -> bool {
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.id == id && item.state == QueueItemState::Preparing)
        else {
            return false;
        };
        item.state = QueueItemState::Playing;
        true
    }

    pub fn pause(&mut self, id: QueueItemId) -> bool {
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.id == id && item.state == QueueItemState::Playing)
        else {
            return false;
        };
        item.state = QueueItemState::Paused;
        true
    }

    pub fn resume(&mut self, id: QueueItemId) -> bool {
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.id == id && item.state == QueueItemState::Paused)
        else {
            return false;
        };
        item.state = QueueItemState::Playing;
        true
    }

    pub fn remove(&mut self, id: QueueItemId) -> Option<QueuedSpeech> {
        let position = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(position))
    }

    pub fn complete(&mut self, id: QueueItemId) -> Option<QueuedSpeech> {
        let position = self.items.iter().position(|item| {
            item.id == id
                && matches!(
                    item.state,
                    QueueItemState::Preparing | QueueItemState::Playing | QueueItemState::Paused
                )
        })?;
        Some(self.items.remove(position))
    }

    pub fn fail(&mut self, id: QueueItemId) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| {
            item.id == id
                && matches!(
                    item.state,
                    QueueItemState::Preparing | QueueItemState::Playing | QueueItemState::Paused
                )
        }) else {
            return false;
        };
        item.state = QueueItemState::Failed;
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceSettings {
    pub voice_id: VoiceId,
    pub speed: f32,
}

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
    pub const MAX_PERCENT: u16 = 200;

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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InferenceProviderId(String);

impl InferenceProviderId {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCapability {
    pub id: InferenceProviderId,
    pub display_name: &'static str,
    pub performance: ProviderPerformance,
    pub available: bool,
    pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderPerformance {
    Recommended,
    Standard,
    Accelerated,
    Planned,
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
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
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
    fn speech_queue_plays_in_order_and_removes_only_completed_items() {
        let mut queue = SpeechQueue::default();
        let first = queue
            .push(SelectedText::new("First selection").unwrap())
            .unwrap();
        let second = queue
            .push(SelectedText::new("Second selection").unwrap())
            .unwrap();

        assert_eq!(queue.start_next().map(|item| item.id), Some(first));
        assert_eq!(
            queue.complete(first).unwrap().text.as_str(),
            "First selection"
        );
        assert_eq!(queue.items().len(), 1);
        assert_eq!(queue.items()[0].id, second);
        assert_eq!(queue.items()[0].state, QueueItemState::Waiting);
    }

    #[test]
    fn speech_queue_can_play_an_exact_item_and_keeps_failed_items_for_retry() {
        let mut queue = SpeechQueue::default();
        let first = queue
            .push(SelectedText::new("First selection").unwrap())
            .unwrap();
        let second = queue
            .push(SelectedText::new("Play this one").unwrap())
            .unwrap();

        let started = queue.start(second).unwrap();
        assert_eq!(started.id, second);
        assert_eq!(started.state, QueueItemState::Preparing);
        assert!(queue.start(first).is_none());
        assert!(queue.mark_playing(second));
        assert!(queue.fail(second));
        assert_eq!(
            queue
                .items()
                .iter()
                .find(|item| item.id == second)
                .unwrap()
                .state,
            QueueItemState::Failed
        );
        assert_eq!(queue.start(second).map(|item| item.id), Some(second));
    }

    #[test]
    fn choosing_a_queued_item_moves_it_to_the_top_for_playback() {
        let mut queue = SpeechQueue::default();
        let first = queue
            .push(SelectedText::new("First selection").unwrap())
            .unwrap();
        let second = queue
            .push(SelectedText::new("Second selection").unwrap())
            .unwrap();
        let third = queue
            .push(SelectedText::new("Play this one now").unwrap())
            .unwrap();

        assert_eq!(queue.start(third).map(|item| item.id), Some(third));

        assert_eq!(
            queue.items().iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![third, first, second]
        );
        assert_eq!(queue.items()[0].state, QueueItemState::Preparing);
    }

    #[test]
    fn speech_queue_pauses_and_resumes_only_the_playing_item() {
        let mut queue = SpeechQueue::default();
        let item = queue
            .push(SelectedText::new("Pause this selection").unwrap())
            .unwrap();

        assert!(!queue.pause(item));
        queue.start(item).unwrap();
        assert!(queue.mark_playing(item));
        assert!(queue.pause(item));
        assert_eq!(queue.active_id(), Some(item));
        assert_eq!(queue.items()[0].state, QueueItemState::Paused);
        assert!(queue.resume(item));
        assert_eq!(queue.items()[0].state, QueueItemState::Playing);
    }

    #[test]
    fn speech_queue_removes_waiting_and_active_items_by_identity() {
        let mut queue = SpeechQueue::default();
        let waiting = queue
            .push(SelectedText::new("Remove while waiting").unwrap())
            .unwrap();
        let active = queue
            .push(SelectedText::new("Remove while active").unwrap())
            .unwrap();

        assert_eq!(queue.remove(waiting).unwrap().id, waiting);
        queue.start(active).unwrap();
        assert_eq!(queue.remove(active).unwrap().id, active);
        assert!(queue.items().is_empty());
        assert!(queue.remove(active).is_none());
    }

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
    fn voice_settings_require_a_non_empty_adapter_voice_id() {
        assert!(VoiceSettings::new("adapter-voice").is_some());
        assert!(VoiceSettings::new(" ").is_none());
    }

    #[test]
    fn audio_energy_tracks_silence_and_peak_signal() {
        assert_eq!(Audio::new(vec![0.0; 8], 24_000).energy(), 0);
        assert_eq!(Audio::new(vec![1.0, -1.0], 24_000).energy(), 255);
        assert!(Audio::new(vec![0.25, -0.25], 24_000).energy() > 0);
    }

    #[test]
    fn audio_features_distinguish_soft_and_bright_windows() {
        let mut samples = vec![0.05; 2_400];
        samples.extend([1.0, -1.0].into_iter().cycle().take(2_400));
        let audio = Audio::new(samples, 24_000);
        let soft = audio.features_at(Duration::from_millis(25), Duration::from_millis(40));
        let bright = audio.features_at(Duration::from_millis(150), Duration::from_millis(40));
        assert!(bright.energy > soft.energy);
        assert!(bright.brightness > soft.brightness);
    }
}
