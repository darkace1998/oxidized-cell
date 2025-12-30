//! HLE Global Context
//!
//! This module provides a global context that holds all HLE manager instances.
//! This enables HLE functions to share state and interact with each other properly.

use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use oc_core::{RsxBridgeSender, SpuBridgeSender};
use oc_vfs::VirtualFileSystem;

use crate::cell_sysutil::SysutilManager;
use crate::cell_game::GameManager;
use crate::cell_save_data::SaveDataManager;
use crate::cell_pad::PadManager;
use crate::cell_audio::AudioManager;
use crate::cell_fs::FsManager;
use crate::cell_vdec::VdecManager;
use crate::cell_adec::AdecManager;
use crate::cell_dmux::DmuxManager;
use crate::cell_png_dec::PngDecManager;
use crate::cell_jpg_dec::JpgDecManager;
use crate::cell_gif_dec::GifDecManager;
use crate::cell_vpost::VpostManager;
use crate::cell_net_ctl::NetCtlManager;
use crate::cell_http::HttpManager;
use crate::cell_ssl::SslManager;
use crate::cell_font::FontManager;
use crate::cell_font_ft::FontFtManager;
use crate::libsre::RegexManager;
use crate::cell_gcm_sys::GcmManager;
use crate::cell_spurs::SpursManager;
use crate::cell_spurs_jq::SpursJqManager;
use crate::cell_resc::RescManager;
use crate::cell_kb::KbManager;
use crate::cell_mouse::MouseManager;
use crate::cell_mic::MicManager;
use crate::spu_runtime::SpuRuntimeManager;

/// Global HLE context instance
pub static HLE_CONTEXT: Lazy<Arc<RwLock<HleContext>>> = Lazy::new(|| {
    Arc::new(RwLock::new(HleContext::new()))
});

/// HLE Context - holds all HLE manager instances
pub struct HleContext {
    /// System utilities manager
    pub sysutil: SysutilManager,
    /// Game data manager
    pub game: GameManager,
    /// Save data manager
    pub save_data: SaveDataManager,
    /// Controller input manager
    pub pad: PadManager,
    /// Audio output manager
    pub audio: AudioManager,
    /// File system manager
    pub fs: FsManager,
    /// Video decoder manager
    pub vdec: VdecManager,
    /// Audio decoder manager
    pub adec: AdecManager,
    /// Demuxer manager
    pub dmux: DmuxManager,
    /// PNG decoder manager
    pub png_dec: PngDecManager,
    /// JPEG decoder manager
    pub jpg_dec: JpgDecManager,
    /// GIF decoder manager
    pub gif_dec: GifDecManager,
    /// Video post-processor manager
    pub vpost: VpostManager,
    /// Network control manager
    pub net_ctl: NetCtlManager,
    /// HTTP client manager
    pub http: HttpManager,
    /// SSL/TLS manager
    pub ssl: SslManager,
    /// Font manager
    pub font: FontManager,
    /// FreeType font manager
    pub font_ft: FontFtManager,
    /// Regular expression manager
    pub regex: RegexManager,
    /// GCM (Graphics) manager
    pub gcm: GcmManager,
    /// SPURS manager
    pub spurs: SpursManager,
    /// SPURS Job Queue manager
    pub spurs_jq: SpursJqManager,
    /// Resolution scaler manager
    pub resc: RescManager,
    /// Keyboard input manager
    pub kb: KbManager,
    /// Mouse input manager
    pub mouse: MouseManager,
    /// Microphone input manager
    pub mic: MicManager,
    /// SPU Runtime manager
    pub spu_runtime: SpuRuntimeManager,
}

impl HleContext {
    /// Create a new HLE context with default manager instances
    pub fn new() -> Self {
        Self {
            sysutil: SysutilManager::new(),
            game: GameManager::new(),
            save_data: SaveDataManager::new(),
            pad: PadManager::new(),
            audio: AudioManager::new(),
            fs: FsManager::new(),
            vdec: VdecManager::new(),
            adec: AdecManager::new(),
            dmux: DmuxManager::new(),
            png_dec: PngDecManager::new(),
            jpg_dec: JpgDecManager::new(),
            gif_dec: GifDecManager::new(),
            vpost: VpostManager::new(),
            net_ctl: NetCtlManager::new(),
            http: HttpManager::new(),
            ssl: SslManager::new(),
            font: FontManager::new(),
            font_ft: FontFtManager::new(),
            regex: RegexManager::new(),
            gcm: GcmManager::new(),
            spurs: SpursManager::new(),
            spurs_jq: SpursJqManager::new(),
            resc: RescManager::new(),
            kb: KbManager::new(),
            mouse: MouseManager::new(),
            mic: MicManager::new(),
            spu_runtime: SpuRuntimeManager::new(),
        }
    }

    /// Reset all managers to their initial state
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for HleContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Get read access to the global HLE context
/// 
/// # Panics
/// 
/// Panics if the lock is poisoned (another thread panicked while holding the lock)
/// or if a deadlock is detected. This is a critical system failure that indicates
/// a bug in the HLE implementation.
pub fn get_hle_context() -> std::sync::RwLockReadGuard<'static, HleContext> {
    HLE_CONTEXT.read().expect(
        "CRITICAL: Failed to acquire HLE context read lock. \
         This indicates either a poisoned lock (thread panic while holding lock) \
         or a potential deadlock in HLE module code."
    )
}

/// Get write access to the global HLE context
/// 
/// # Panics
/// 
/// Panics if the lock is poisoned (another thread panicked while holding the lock)
/// or if a deadlock is detected. This is a critical system failure that indicates
/// a bug in the HLE implementation.
pub fn get_hle_context_mut() -> std::sync::RwLockWriteGuard<'static, HleContext> {
    HLE_CONTEXT.write().expect(
        "CRITICAL: Failed to acquire HLE context write lock. \
         This indicates either a poisoned lock (thread panic while holding lock) \
         or a potential deadlock in HLE module code."
    )
}

/// Reset the global HLE context to its initial state
pub fn reset_hle_context() {
    get_hle_context_mut().reset();
}

/// Set the RSX bridge sender on the GCM manager
/// 
/// This connects the GCM HLE module to the RSX graphics thread,
/// allowing commands to flow from cellGcmSys to the RSX backend.
pub fn set_rsx_bridge(bridge: RsxBridgeSender) {
    get_hle_context_mut().gcm.set_rsx_bridge(bridge);
}

/// Check if RSX bridge is connected
pub fn has_rsx_bridge() -> bool {
    get_hle_context().gcm.has_rsx_bridge()
}

/// Set the SPU bridge sender on the SPURS manager
/// 
/// This connects the SPURS HLE module to the SPU thread runtime,
/// allowing workloads to flow from cellSpurs to the SPU interpreter.
pub fn set_spu_bridge(bridge: SpuBridgeSender) {
    get_hle_context_mut().spurs.set_spu_bridge(bridge);
}

/// Check if SPU bridge is connected
pub fn has_spu_bridge() -> bool {
    get_hle_context().spurs.has_spu_bridge()
}

/// Pop a pending sysutil callback that needs to be invoked on PPU
/// 
/// Returns None if there are no pending callbacks. The runner should
/// invoke the returned callback by calling the function at `func` with
/// arguments (status, param, userdata).
pub fn pop_sysutil_callback() -> Option<crate::cell_sysutil::PendingCallback> {
    get_hle_context_mut().sysutil.pop_pending_callback()
}

/// Check if there are pending sysutil callbacks
pub fn has_pending_sysutil_callbacks() -> bool {
    get_hle_context().sysutil.has_pending_callbacks()
}

/// Queue a sysutil system event (e.g., XMB open, disc eject, etc.)
/// 
/// This will generate callbacks for all registered callback slots when
/// `cellSysutilCheckCallback` is next called.
pub fn queue_sysutil_event(event_type: u64, param: u64) {
    get_hle_context_mut().sysutil.queue_event(event_type, param);
}

/// Close any open sysutil dialog with OK result
/// 
/// This simulates the user pressing OK/Yes on a dialog.
pub fn close_sysutil_dialog_ok() -> i32 {
    get_hle_context_mut().sysutil.close_dialog_ok()
}

/// Close any open sysutil dialog with Cancel result
/// 
/// This simulates the user pressing Cancel/No on a dialog.
pub fn close_sysutil_dialog_cancel() -> i32 {
    get_hle_context_mut().sysutil.close_dialog_cancel()
}

/// Check if a sysutil dialog is currently open
pub fn is_sysutil_dialog_open() -> bool {
    get_hle_context().sysutil.is_dialog_open()
}

// ============================================================================
// Virtual File System (VFS) Integration
// ============================================================================

/// Set the VFS backend on the file system manager
/// 
/// This connects the cellFs HLE module to the virtual file system,
/// enabling actual file I/O operations through mounted devices.
/// 
/// # Example
/// ```ignore
/// use oc_vfs::VirtualFileSystem;
/// use std::sync::Arc;
/// 
/// let vfs = Arc::new(VirtualFileSystem::new());
/// vfs.mount("/dev_hdd0", std::path::PathBuf::from("/path/to/dev_hdd0"));
/// oc_hle::context::set_vfs(vfs);
/// ```
pub fn set_vfs(vfs: Arc<VirtualFileSystem>) {
    get_hle_context_mut().fs.set_vfs(vfs);
}

/// Check if VFS is connected
pub fn has_vfs() -> bool {
    get_hle_context().fs.has_vfs()
}

// ============================================================================
// Input Backend (oc-input) Integration
// ============================================================================

/// Set the input backend on the pad manager
/// 
/// This connects the cellPad HLE module to the oc-input DualShock3Manager,
/// enabling actual controller polling from connected gamepads.
/// 
/// # Example
/// ```ignore
/// use oc_input::DualShock3Manager;
/// use std::sync::{Arc, RwLock};
/// 
/// let input_manager = Arc::new(RwLock::new(DualShock3Manager::default()));
/// oc_hle::context::set_input_backend(input_manager);
/// ```
pub fn set_input_backend(backend: Arc<std::sync::RwLock<oc_input::DualShock3Manager>>) {
    get_hle_context_mut().pad.set_input_backend(backend);
}

/// Check if input backend is connected
pub fn has_input_backend() -> bool {
    get_hle_context().pad.has_input_backend()
}

/// Poll all controllers and update pad data
/// 
/// Should be called each frame to update controller state from oc-input.
pub fn poll_input() -> i32 {
    get_hle_context_mut().pad.poll_input()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hle_context_creation() {
        let ctx = HleContext::new();
        // Verify managers are initialized
        assert!(!ctx.sysutil.has_callbacks());
        assert!(!ctx.game.is_initialized());
        assert_eq!(ctx.save_data.directory_count(), 0);
        assert_eq!(ctx.fs.open_count(), 0);
    }

    #[test]
    fn test_hle_context_reset() {
        let mut ctx = HleContext::new();
        
        // Make some changes
        ctx.sysutil.register_callback(0, 0x12345678, 0);
        assert!(ctx.sysutil.has_callbacks());
        
        // Reset and verify
        ctx.reset();
        assert!(!ctx.sysutil.has_callbacks());
    }

    #[test]
    fn test_global_context_access() {
        // Test read access
        {
            let ctx = get_hle_context();
            assert!(!ctx.sysutil.has_callbacks());
        }
        
        // Test write access
        {
            let mut ctx = get_hle_context_mut();
            ctx.sysutil.register_callback(0, 0x12345678, 0);
        }
        
        // Verify changes persisted
        {
            let ctx = get_hle_context();
            assert!(ctx.sysutil.has_callbacks());
        }
        
        // Reset for other tests
        reset_hle_context();
    }
}
