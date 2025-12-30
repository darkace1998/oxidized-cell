//! RSX shader translation (RSX → SPIR-V)
//!
//! This module handles translation of RSX vertex and fragment programs
//! to Vulkan SPIR-V shaders.
//!
//! The RSX GPU uses a custom instruction set based on NVIDIA NV40/G70:
//! - Vertex programs: 128-bit instructions with vector and scalar co-issue
//! - Fragment programs: 128-bit instructions with byte-swapped encoding

pub mod vp_decode;
pub mod fp_decode;
pub mod spirv_gen;
pub mod types;
pub mod cache;

pub use vp_decode::*;
pub use fp_decode::*;
pub use spirv_gen::*;
pub use types::*;
pub use cache::*;
