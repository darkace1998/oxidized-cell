//! Core integration layer for oxidized-cell PS3 emulator
//!
//! This crate integrates all subsystems into a cohesive emulator runner.

pub mod loader;
pub mod pipeline;
pub mod runner;

pub use loader::{GameLoader, LoadedGame};
pub use pipeline::{
    GameInfo, GamePipeline, GameScanner, MemoryLayoutInfo, 
    ModuleDependency, ModuleLifecycleEvent, ModuleState, SystemModule
};
pub use runner::{EmulatorRunner, RunnerState};
