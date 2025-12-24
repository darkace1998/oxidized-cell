//! Core integration layer for oxidized-cell PS3 emulator
//!
//! This crate integrates all subsystems into a cohesive emulator runner.

pub mod loader;
pub mod runner;

pub use loader::{GameLoader, LoadedGame};
pub use runner::{EmulatorRunner, RunnerState};
