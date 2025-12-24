//! Core emulator logic for oxidized-cell PS3 emulator
//!
//! This crate provides the foundational types, error handling,
//! configuration, and logging infrastructure for the emulator.

pub mod config;
pub mod emulator;
pub mod error;
pub mod logging;
pub mod scheduler;

pub use config::Config;
pub use emulator::Emulator;
pub use error::{EmulatorError, Result};
pub use scheduler::{Scheduler, ThreadId, ThreadState, ThreadStats};
