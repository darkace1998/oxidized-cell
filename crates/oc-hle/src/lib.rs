//! HLE (High Level Emulation) modules for oxidized-cell
//!
//! This crate provides HLE implementations of PS3 system libraries.

pub mod module;
pub mod context;
pub mod dispatcher;

// Graphics Modules
pub mod cell_gcm_sys;
pub mod cell_gif_dec;
pub mod cell_png_dec;
pub mod cell_jpg_dec;
pub mod cell_resc;

// System Modules
pub mod cell_sysutil;
pub mod cell_game;
pub mod cell_save_data;

// Multimedia Modules
pub mod cell_dmux;
pub mod cell_vdec;
pub mod cell_adec;
pub mod cell_vpost;

// Network Modules
pub mod cell_net_ctl;
pub mod cell_http;
pub mod cell_ssl;

// Utilities Modules
pub mod cell_font;
pub mod cell_font_ft;
pub mod cell_spurs;
pub mod cell_spurs_jq;
pub mod libsre;
pub mod spu_runtime;

// Input Modules
pub mod cell_pad;
pub mod cell_kb;
pub mod cell_mouse;
pub mod cell_mic;

// Other System Modules
pub mod cell_audio;
pub mod cell_fs;

pub use module::ModuleRegistry;
pub use context::{HleContext, HLE_CONTEXT, get_hle_context, get_hle_context_mut, reset_hle_context};
pub use dispatcher::{
    HleDispatcher, HleCallContext, HleFunctionInfo, HleFn, HLE_DISPATCHER,
    get_dispatcher, get_dispatcher_mut, init_hle_dispatcher, dispatch_hle_call,
};
