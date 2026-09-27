//! Model-independent business rules and the ports they require.

pub mod application;
pub mod domain;
pub mod ports;

pub use application::playback;
