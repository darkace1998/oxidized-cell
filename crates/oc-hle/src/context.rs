//! HLE Global Context
//!
//! This module provides a global context that holds all HLE manager instances.
//! This enables HLE functions to share state and interact with each other properly.

use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;

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
use crate::libsre::RegexManager;

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
    /// Regular expression manager
    pub regex: RegexManager,
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
            regex: RegexManager::new(),
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
