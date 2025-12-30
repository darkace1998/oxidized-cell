//! PPU (PowerPC Processing Unit) emulation for oxidized-cell
//!
//! This crate implements the Cell BE PPU, which is based on the PowerPC 970
//! architecture with VMX/AltiVec SIMD support.
//!
//! ## Execution Modes
//!
//! The PPU can operate in three modes:
//! - **Interpreter**: Pure software interpretation (default, always available)
//! - **JIT**: Just-In-Time compilation for maximum performance (requires C++ backend)
//! - **Hybrid**: Uses JIT for hot paths, interpreter for cold code (best balance)
//!
//! ## Usage
//!
//! ```ignore
//! use oc_ppu::{PpuInterpreter, PpuThread, JitMode};
//! use std::sync::Arc;
//!
//! let memory = Arc::new(MemoryManager::new());
//! let interpreter = PpuInterpreter::new(memory.clone());
//!
//! // Enable JIT compilation (hybrid mode)
//! interpreter.enable_jit();
//!
//! // Or set a specific mode
//! interpreter.set_jit_mode(JitMode::Hybrid);
//!
//! // Check JIT availability
//! if interpreter.is_jit_available() {
//!     println!("JIT compiler is available");
//! }
//! ```

pub mod decoder;
pub mod instructions;
pub mod interpreter;
pub mod thread;
pub mod vmx;

pub use decoder::PpuDecoder;
pub use interpreter::{PpuInterpreter, JitMode, JitStats};
pub use thread::PpuThread;
