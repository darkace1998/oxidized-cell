//! Core emulator logic for oxidized-cell PS3 emulator
//!
//! This crate provides the foundational types, error handling,
//! configuration, and logging infrastructure for the emulator.

pub mod config;
pub mod emulator;
pub mod error;
pub mod logging;
pub mod rsx_bridge;
pub mod scheduler;
pub mod spu_bridge;

pub use config::Config;
pub use emulator::Emulator;
pub use error::{EmulatorError, Result};
pub use rsx_bridge::{
    create_rsx_bridge, BridgeCommand, BridgeDisplayBuffer, BridgeFlipRequest,
    BridgeMessage, FlipStatus, RsxBridgeReceiver, RsxBridgeSender,
};
pub use spu_bridge::{
    create_spu_bridge, SpuBridgeMessage, SpuBridgeReceiver, SpuBridgeSender,
    SpuDmaRequest, SpuEvent, SpuEventType, SpuGroupRequest, SpuThreadRequest,
    SpuWorkload,
};
pub use scheduler::{Scheduler, ThreadId, ThreadState, ThreadStats};
