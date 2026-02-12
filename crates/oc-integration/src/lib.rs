//! Core integration layer for oxidized-cell PS3 emulator
//!
//! This crate integrates all subsystems into a cohesive emulator runner.

pub mod loader;
pub mod pipeline;
pub mod runner;

pub use loader::{GameLoader, LoadedGame};
pub use pipeline::{
    BootSequenceInfo, GameInfo, GamePipeline, GameScanner, KernelObjectsInfo, 
    MainThreadInfo, MainThreadState, MemoryLayoutInfo, ModuleDependency, 
    ModuleLifecycleEvent, ModuleState, RegisterState, SystemModule, 
    ThreadStackInfo, TlsLayoutInfo, TlsThreadArea
};
pub use runner::{EmulatorRunner, RunnerState};
