//! User interface for oxidized-cell

pub mod app;
pub mod debugger;
pub mod game_list;
pub mod log_viewer;
pub mod memory_viewer;
pub mod settings;
pub mod themes;

pub use app::OxidizedCellApp;
pub use log_viewer::{LogViewer, LogLevel, LogEntry, SharedLogBuffer, create_log_buffer};
pub use memory_viewer::MemoryViewer;
