//! SPU (Synergistic Processing Unit) emulation for oxidized-cell
//!
//! The Cell BE has 8 SPUs (6 usable on PS3), each with:
//! - 256 KB local storage
//! - 128 x 128-bit registers
//! - MFC (Memory Flow Controller) for DMA

pub mod atomics;
pub mod channels;
pub mod decoder;
pub mod instructions;
pub mod interpreter;
pub mod mfc;
pub mod thread;

pub use decoder::SpuDecoder;
pub use interpreter::SpuInterpreter;
pub use mfc::Mfc;
pub use thread::{
    SpuThread, SpuThreadState, SpuThreadGroup, SpuPriority, SpuAffinity,
    SpuExceptionType, SpuExceptionState, SpuEventType, SpuEventQueue,
    SPU_LS_SIZE, MAX_SPU_THREADS_PER_GROUP,
};
