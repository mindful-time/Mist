//! Domain values and state machines owned by Mist's model-independent core.

mod audio;
mod inference;
mod playback;
mod queue;
mod selection;
mod voice;

pub use audio::{Audio, AudioFeatures};
pub use inference::{InferenceProviderId, ProviderCapability, ProviderPerformance};
pub use playback::{PlaybackMode, PlaybackPreferences, PlaybackSpeed};
pub use queue::{QueueError, QueueItemId, QueueItemState, QueuedSpeech, SpeechQueue};
pub use selection::{SelectedText, SelectionCaptureError, SelectionError};
pub use voice::{LanguageId, LanguageProfile, MistPalette, VoiceId, VoiceProfile, VoiceSettings};

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
    fn playback_speed_covers_half_to_triple_speed() {
        assert_eq!(PlaybackSpeed::from_percent(50).unwrap().multiplier(), 0.5);
        assert_eq!(PlaybackSpeed::from_percent(125).unwrap().multiplier(), 1.25);
        assert_eq!(PlaybackSpeed::from_percent(300).unwrap().multiplier(), 3.0);
        assert!(PlaybackSpeed::from_percent(49).is_none());
        assert!(PlaybackSpeed::from_percent(301).is_none());
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
