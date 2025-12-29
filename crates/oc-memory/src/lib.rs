//! Memory management for oxidized-cell PS3 emulator
//!
//! This crate provides the virtual memory system that mimics the PS3's
//! address space, including reservation system for SPU atomics.

pub mod constants;
pub mod debug;
pub mod manager;
pub mod pages;
pub mod reservation;

pub use constants::*;
pub use debug::{
    CacheMode, CacheSimulator, CacheStats, MemoryProfiler, SmcDetector,
    Watchpoint, WatchpointCondition, WatchpointManager, WatchpointType,
};
pub use manager::{
    ExceptionHandlerResult, MemoryException, MemoryManager, MemoryRegion,
    RsxMemoryMapping, SharedMemoryRegion,
};
pub use pages::PageFlags;
pub use reservation::Reservation;
