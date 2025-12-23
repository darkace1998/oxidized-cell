//! RSX (Reality Synthesizer) graphics emulation for oxidized-cell
//!
//! The RSX is based on NVIDIA G70/G71 architecture.

pub mod backend;
pub mod buffer;
pub mod fifo;
pub mod methods;
pub mod shader;
pub mod state;
pub mod texture;
pub mod thread;
pub mod vertex;

pub use state::RsxState;
pub use thread::RsxThread;
