//! Audio backends

pub mod cpal_backend;
pub mod null;

pub use cpal_backend::CpalAudioBackend;
pub use null::NullAudioBackend;
