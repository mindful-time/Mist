//! Playback coordination shared by application use cases and audio adapters.

use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use thiserror::Error;

use crate::core::domain::QueueItemId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlaybackToken(u64);

impl PlaybackToken {
    pub const PREVIEW: Self = Self(0);

    pub const fn queue_item(id: QueueItemId) -> Self {
        Self(id.get())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackPhase {
    Idle,
    Preparing,
    Playing,
    Paused,
    Cancelled,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("speech playback was stopped")]
pub struct PlaybackStopped;

#[derive(Debug)]
struct PlaybackControlState {
    token: Option<PlaybackToken>,
    phase: PlaybackPhase,
}

#[derive(Debug, Default)]
struct PlaybackControl {
    state: Mutex<PlaybackControlState>,
    changed: Condvar,
}

impl Default for PlaybackControlState {
    fn default() -> Self {
        Self {
            token: None,
            phase: PlaybackPhase::Idle,
        }
    }
}

/// Out-of-band control for the currently registered playback session.
///
/// The speech worker performs synthesis and playback synchronously, so pause
/// and cancellation cannot share its command queue. This handle is safe to
/// clone into the UI and audio adapter while keeping process details inside
/// the platform adapter.
#[derive(Clone, Debug, Default)]
pub struct PlaybackController {
    control: Arc<PlaybackControl>,
}

impl PlaybackController {
    pub fn register(&self, token: PlaybackToken) -> bool {
        let mut state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.token.is_some() {
            return false;
        }
        state.token = Some(token);
        state.phase = PlaybackPhase::Idle;
        self.control.changed.notify_all();
        true
    }

    pub fn begin_session(&self, token: PlaybackToken) -> Result<(), PlaybackStopped> {
        let mut state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.token.is_none() {
            state.token = Some(token);
        }
        if state.token != Some(token) || state.phase == PlaybackPhase::Cancelled {
            return Err(PlaybackStopped);
        }
        state.phase = PlaybackPhase::Preparing;
        self.control.changed.notify_all();
        Ok(())
    }

    pub fn begin_playback(&self) -> Result<(), PlaybackStopped> {
        let mut state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.phase == PlaybackPhase::Cancelled {
            return Err(PlaybackStopped);
        }
        if state.phase != PlaybackPhase::Paused {
            state.phase = PlaybackPhase::Playing;
            self.control.changed.notify_all();
        }
        Ok(())
    }

    pub fn pause(&self, token: PlaybackToken) -> bool {
        self.transition(token, PlaybackPhase::Playing, PlaybackPhase::Paused)
    }

    pub fn resume(&self, token: PlaybackToken) -> bool {
        self.transition(token, PlaybackPhase::Paused, PlaybackPhase::Playing)
    }

    pub fn cancel(&self, token: PlaybackToken) -> bool {
        let mut state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.token != Some(token) || state.phase == PlaybackPhase::Cancelled {
            return false;
        }
        state.phase = PlaybackPhase::Cancelled;
        self.control.changed.notify_all();
        true
    }

    pub fn finish_session(&self, token: PlaybackToken) {
        let mut state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.token == Some(token) {
            *state = PlaybackControlState::default();
            self.control.changed.notify_all();
        }
    }

    pub fn phase(&self) -> PlaybackPhase {
        self.control
            .state
            .lock()
            .expect("playback control was poisoned")
            .phase
    }

    /// Waits until playback control changes, or until the visual sampling
    /// interval expires. Audio adapters use this instead of sleeping so pause,
    /// resume, and cancellation reach the system player without polling lag.
    pub fn wait_for_phase_change(
        &self,
        observed: PlaybackPhase,
        timeout: Duration,
    ) -> PlaybackPhase {
        let state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.phase != observed {
            return state.phase;
        }
        let (state, _) = self
            .control
            .changed
            .wait_timeout_while(state, timeout, |state| state.phase == observed)
            .expect("playback control was poisoned while waiting");
        state.phase
    }

    fn transition(&self, token: PlaybackToken, from: PlaybackPhase, to: PlaybackPhase) -> bool {
        let mut state = self
            .control
            .state
            .lock()
            .expect("playback control was poisoned");
        if state.token != Some(token) || state.phase != from {
            return false;
        }
        state.phase = to;
        self.control.changed.notify_all();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_session_can_pause_resume_and_finish() {
        let control = PlaybackController::default();
        let token = PlaybackToken(7);
        assert!(control.register(token));
        control.begin_session(token).unwrap();
        control.begin_playback().unwrap();
        assert!(control.pause(token));
        assert_eq!(control.phase(), PlaybackPhase::Paused);
        assert!(control.resume(token));
        assert_eq!(control.phase(), PlaybackPhase::Playing);
        control.finish_session(token);
        assert_eq!(control.phase(), PlaybackPhase::Idle);
    }

    #[test]
    fn paused_session_stays_paused_when_the_next_audio_chunk_begins() {
        let control = PlaybackController::default();
        let token = PlaybackToken(8);
        assert!(control.register(token));
        control.begin_session(token).unwrap();
        control.begin_playback().unwrap();
        assert!(control.pause(token));

        control.begin_playback().unwrap();

        assert_eq!(control.phase(), PlaybackPhase::Paused);
        assert!(control.resume(token));
        assert_eq!(control.phase(), PlaybackPhase::Playing);
    }

    #[test]
    fn cancellation_before_worker_start_prevents_playback() {
        let control = PlaybackController::default();
        let token = PlaybackToken(42);
        assert!(control.register(token));
        assert!(control.cancel(token));
        assert_eq!(control.begin_session(token), Err(PlaybackStopped));
        control.finish_session(token);
        assert_eq!(control.phase(), PlaybackPhase::Idle);
    }

    #[test]
    fn stale_controls_cannot_affect_a_different_session() {
        let control = PlaybackController::default();
        let current = PlaybackToken(1);
        let stale = PlaybackToken(2);
        assert!(control.register(current));
        control.begin_session(current).unwrap();
        control.begin_playback().unwrap();

        assert!(!control.pause(stale));
        assert!(!control.cancel(stale));
        assert_eq!(control.phase(), PlaybackPhase::Playing);
    }
}
