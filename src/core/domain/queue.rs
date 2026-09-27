use thiserror::Error;

use super::SelectedText;

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
