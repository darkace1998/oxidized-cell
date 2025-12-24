//! cellSysutil HLE - System Utilities
//!
//! This module provides system utility functions including callback management,
//! system events, and game exit handling.

use std::collections::HashMap;
use tracing::{debug, trace};

/// System callback function type
pub type SysutilCallback = fn(status: u64, param: u64, userdata: u64);

/// System callback entry
#[derive(Debug, Clone, Copy)]
struct CallbackEntry {
    func: u32,      // Address of callback function
    userdata: u32,  // User data pointer
}

/// System utility manager
pub struct SysutilManager {
    /// Registered callbacks
    callbacks: HashMap<u32, CallbackEntry>,
    /// Next callback ID
    next_id: u32,
}

impl SysutilManager {
    /// Create a new system utility manager
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a callback
    pub fn register_callback(&mut self, func: u32, userdata: u32) -> i32 {
        let id = self.next_id;
        self.next_id += 1;

        self.callbacks.insert(
            id,
            CallbackEntry {
                func,
                userdata,
            },
        );

        debug!("Registered sysutil callback: id={}, func=0x{:08X}", id, func);
        0 // CELL_OK
    }

    /// Unregister a callback
    pub fn unregister_callback(&mut self, id: u32) -> i32 {
        if self.callbacks.remove(&id).is_some() {
            debug!("Unregistered sysutil callback: id={}", id);
            0 // CELL_OK
        } else {
            debug!("Failed to unregister callback: id={} not found", id);
            0x80010002u32 as i32 // CELL_SYSUTIL_ERROR_VALUE
        }
    }

    /// Check callbacks (should be called periodically by game)
    pub fn check_callback(&mut self) -> i32 {
        trace!("cellSysutilCheckCallback()");

        // TODO: Iterate through pending events and call callbacks
        // TODO: Handle system messages (exit request, etc.)

        0 // CELL_OK
    }
}

impl Default for SysutilManager {
    fn default() -> Self {
        Self::new()
    }
}

/// System event types
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSysutilEvent {
    /// Menu open
    MenuOpen = 0x0131,
    /// Menu close
    MenuClose = 0x0132,
    /// Drawing begin
    DrawBegin = 0x0121,
    /// Drawing end
    DrawEnd = 0x0122,
    /// XMB event
    XmbEvent = 0x0101,
    /// System message
    SystemMessage = 0x0141,
}

/// cellSysutilRegisterCallback - Register system callback
///
/// # Arguments
/// * `slot` - Callback slot (0-3)
/// * `func` - Callback function address
/// * `userdata` - User data to pass to callback
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_register_callback(slot: u32, func: u32, userdata: u32) -> i32 {
    debug!(
        "cellSysutilRegisterCallback(slot={}, func=0x{:08X}, userdata=0x{:08X})",
        slot, func, userdata
    );

    // TODO: Store callback in global manager
    // For now, just return success

    0 // CELL_OK
}

/// cellSysutilUnregisterCallback - Unregister system callback
///
/// # Arguments
/// * `slot` - Callback slot (0-3)
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_unregister_callback(slot: u32) -> i32 {
    debug!("cellSysutilUnregisterCallback(slot={})", slot);

    // TODO: Remove callback from global manager

    0 // CELL_OK
}

/// cellSysutilCheckCallback - Check and process callbacks
///
/// Should be called regularly by the game (typically once per frame)
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_check_callback() -> i32 {
    trace!("cellSysutilCheckCallback()");

    // TODO: Process pending system events
    // TODO: Call registered callbacks if needed

    0 // CELL_OK
}

/// cellSysutilGetSystemParamInt - Get system parameter (integer)
///
/// # Arguments
/// * `param_id` - Parameter ID
/// * `value_addr` - Address to write value to
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_get_system_param_int(param_id: u32, _value_addr: u32) -> i32 {
    debug!("cellSysutilGetSystemParamInt(param_id=0x{:X})", param_id);

    // TODO: Return appropriate system parameter
    // Common params: language, enter button assignment, etc.

    0 // CELL_OK
}

/// cellSysutilGetSystemParamString - Get system parameter (string)
///
/// # Arguments
/// * `param_id` - Parameter ID
/// * `buf_addr` - Buffer address to write string to
/// * `buf_size` - Buffer size
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_get_system_param_string(
    param_id: u32,
    _buf_addr: u32,
    buf_size: u32,
) -> i32 {
    debug!(
        "cellSysutilGetSystemParamString(param_id=0x{:X}, buf_size={})",
        param_id, buf_size
    );

    // TODO: Return appropriate system parameter string
    // Common params: nickname, current username, etc.

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sysutil_manager() {
        let mut manager = SysutilManager::new();
        assert_eq!(manager.register_callback(0x12345678, 0xABCDEF00), 0);
        assert_eq!(manager.check_callback(), 0);
    }

    #[test]
    fn test_register_callback() {
        let result = cell_sysutil_register_callback(0, 0x12345678, 0);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_check_callback() {
        let result = cell_sysutil_check_callback();
        assert_eq!(result, 0);
    }
}
