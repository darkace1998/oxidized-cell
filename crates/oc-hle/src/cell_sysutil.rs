//! cellSysutil HLE - System Utilities
//!
//! This module provides system utility functions including callback management,
//! system events, and game exit handling.

use std::collections::{HashMap, VecDeque};
use tracing::{debug, trace};

/// Maximum number of callback slots
pub const CELL_SYSUTIL_MAX_CALLBACK_SLOTS: usize = 4;

/// System callback function type
pub type SysutilCallback = fn(status: u64, param: u64, userdata: u64);

/// System callback entry
#[derive(Debug, Clone, Copy)]
struct CallbackEntry {
    func: u32,      // Address of callback function
    userdata: u32,  // User data pointer
}

/// System event
#[derive(Debug, Clone, Copy)]
pub struct SystemEvent {
    /// Event type
    pub event_type: u64,
    /// Event parameter
    pub param: u64,
}

/// System parameter IDs
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSysutilParamId {
    /// System language
    Language = 0x0111,
    /// Enter button assignment (Circle/Cross)
    EnterButtonAssign = 0x0112,
    /// Date format
    DateFormat = 0x0114,
    /// Time format
    TimeFormat = 0x0115,
    /// Time zone
    TimeZone = 0x0116,
    /// Summertime (DST)
    SummerTime = 0x0117,
    /// Game rating level
    GameRating = 0x0121,
    /// Nickname
    Nickname = 0x0131,
    /// Current username
    CurrentUsername = 0x0141,
}

/// System utility manager
pub struct SysutilManager {
    /// Registered callbacks by slot
    callbacks: [Option<CallbackEntry>; CELL_SYSUTIL_MAX_CALLBACK_SLOTS],
    /// Pending events queue
    pending_events: VecDeque<SystemEvent>,
    /// System parameters (integer)
    int_params: HashMap<u32, i32>,
    /// System parameters (string)
    string_params: HashMap<u32, String>,
}

impl SysutilManager {
    /// Create a new system utility manager
    pub fn new() -> Self {
        let mut manager = Self {
            callbacks: [None; CELL_SYSUTIL_MAX_CALLBACK_SLOTS],
            pending_events: VecDeque::new(),
            int_params: HashMap::new(),
            string_params: HashMap::new(),
        };
        
        // Initialize default system parameters
        manager.init_default_params();
        manager
    }

    /// Initialize default system parameters
    fn init_default_params(&mut self) {
        // Default language: English (0)
        self.int_params.insert(CellSysutilParamId::Language as u32, 0);
        
        // Default enter button: Cross (1)
        self.int_params.insert(CellSysutilParamId::EnterButtonAssign as u32, 1);
        
        // Default date format: YYYY/MM/DD (0)
        self.int_params.insert(CellSysutilParamId::DateFormat as u32, 0);
        
        // Default time format: 24-hour (0)
        self.int_params.insert(CellSysutilParamId::TimeFormat as u32, 0);
        
        // Default nickname
        self.string_params.insert(
            CellSysutilParamId::Nickname as u32,
            "Player".to_string(),
        );
        
        // Default username
        self.string_params.insert(
            CellSysutilParamId::CurrentUsername as u32,
            "User".to_string(),
        );
    }

    /// Register a callback
    pub fn register_callback(&mut self, slot: u32, func: u32, userdata: u32) -> i32 {
        if slot >= CELL_SYSUTIL_MAX_CALLBACK_SLOTS as u32 {
            return 0x80010002u32 as i32; // CELL_SYSUTIL_ERROR_VALUE
        }

        self.callbacks[slot as usize] = Some(CallbackEntry { func, userdata });

        debug!(
            "Registered sysutil callback: slot={}, func=0x{:08X}",
            slot, func
        );
        0 // CELL_OK
    }

    /// Unregister a callback
    pub fn unregister_callback(&mut self, slot: u32) -> i32 {
        if slot >= CELL_SYSUTIL_MAX_CALLBACK_SLOTS as u32 {
            return 0x80010002u32 as i32; // CELL_SYSUTIL_ERROR_VALUE
        }

        if self.callbacks[slot as usize].is_some() {
            self.callbacks[slot as usize] = None;
            debug!("Unregistered sysutil callback: slot={}", slot);
            0 // CELL_OK
        } else {
            debug!("Failed to unregister callback: slot={} not registered", slot);
            0x80010002u32 as i32 // CELL_SYSUTIL_ERROR_VALUE
        }
    }

    /// Check callbacks (should be called periodically by game)
    pub fn check_callback(&mut self) -> i32 {
        trace!("SysutilManager::check_callback()");

        // Process pending events
        while let Some(event) = self.pending_events.pop_front() {
            // TODO: Call registered callbacks with event
            trace!("Processing event: type=0x{:X}, param=0x{:X}", event.event_type, event.param);
            
            // For now, just process the event without calling actual callbacks
            // Real implementation would need to call PPU callback functions
        }

        0 // CELL_OK
    }

    /// Queue a system event
    pub fn queue_event(&mut self, event_type: u64, param: u64) {
        debug!("Queueing system event: type=0x{:X}, param=0x{:X}", event_type, param);
        self.pending_events.push_back(SystemEvent { event_type, param });
    }

    /// Get integer system parameter
    pub fn get_system_param_int(&self, param_id: u32) -> Option<i32> {
        self.int_params.get(&param_id).copied()
    }

    /// Set integer system parameter
    pub fn set_system_param_int(&mut self, param_id: u32, value: i32) {
        self.int_params.insert(param_id, value);
    }

    /// Get string system parameter
    pub fn get_system_param_string(&self, param_id: u32) -> Option<&str> {
        self.string_params.get(&param_id).map(|s| s.as_str())
    }

    /// Set string system parameter
    pub fn set_system_param_string(&mut self, param_id: u32, value: String) {
        self.string_params.insert(param_id, value);
    }

    /// Check if any callbacks are registered
    pub fn has_callbacks(&self) -> bool {
        self.callbacks.iter().any(|c| c.is_some())
    }

    /// Get number of pending events
    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
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

    if slot >= CELL_SYSUTIL_MAX_CALLBACK_SLOTS as u32 {
        return 0x80010002u32 as i32; // CELL_SYSUTIL_ERROR_VALUE
    }

    // TODO: Store callback in global manager

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

    if slot >= CELL_SYSUTIL_MAX_CALLBACK_SLOTS as u32 {
        return 0x80010002u32 as i32; // CELL_SYSUTIL_ERROR_VALUE
    }

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

    // TODO: Process pending system events through global manager
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

    // TODO: Return appropriate system parameter from global manager
    // TODO: Write value to memory

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

    // TODO: Return appropriate system parameter string from global manager
    // TODO: Write string to memory

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sysutil_manager() {
        let mut manager = SysutilManager::new();
        assert_eq!(manager.register_callback(0, 0x12345678, 0xABCDEF00), 0);
        assert_eq!(manager.check_callback(), 0);
    }

    #[test]
    fn test_sysutil_manager_callbacks() {
        let mut manager = SysutilManager::new();
        
        // Register callbacks in different slots
        assert_eq!(manager.register_callback(0, 0x12345678, 0), 0);
        assert_eq!(manager.register_callback(1, 0x87654321, 0), 0);
        assert!(manager.has_callbacks());
        
        // Unregister callback
        assert_eq!(manager.unregister_callback(0), 0);
        
        // Try to unregister again (should fail)
        assert!(manager.unregister_callback(0) != 0);
        
        // Invalid slot
        assert!(manager.register_callback(10, 0x12345678, 0) != 0);
    }

    #[test]
    fn test_sysutil_manager_events() {
        let mut manager = SysutilManager::new();
        
        // Queue some events
        manager.queue_event(CellSysutilEvent::MenuOpen as u64, 0);
        manager.queue_event(CellSysutilEvent::DrawBegin as u64, 1);
        
        assert_eq!(manager.pending_event_count(), 2);
        
        // Process events
        assert_eq!(manager.check_callback(), 0);
        
        // Events should be processed
        assert_eq!(manager.pending_event_count(), 0);
    }

    #[test]
    fn test_sysutil_manager_params() {
        let manager = SysutilManager::new();
        
        // Check default integer parameters
        let lang = manager.get_system_param_int(CellSysutilParamId::Language as u32);
        assert_eq!(lang, Some(0)); // English
        
        let button = manager.get_system_param_int(CellSysutilParamId::EnterButtonAssign as u32);
        assert_eq!(button, Some(1)); // Cross
        
        // Check default string parameters
        let nickname = manager.get_system_param_string(CellSysutilParamId::Nickname as u32);
        assert_eq!(nickname, Some("Player"));
        
        let username = manager.get_system_param_string(CellSysutilParamId::CurrentUsername as u32);
        assert_eq!(username, Some("User"));
    }

    #[test]
    fn test_sysutil_manager_param_mutation() {
        let mut manager = SysutilManager::new();
        
        // Change integer parameter
        manager.set_system_param_int(CellSysutilParamId::Language as u32, 1);
        assert_eq!(manager.get_system_param_int(CellSysutilParamId::Language as u32), Some(1));
        
        // Change string parameter
        manager.set_system_param_string(CellSysutilParamId::Nickname as u32, "TestUser".to_string());
        assert_eq!(manager.get_system_param_string(CellSysutilParamId::Nickname as u32), Some("TestUser"));
    }

    #[test]
    fn test_register_callback() {
        let result = cell_sysutil_register_callback(0, 0x12345678, 0);
        assert_eq!(result, 0);
        
        // Invalid slot
        let result = cell_sysutil_register_callback(10, 0x12345678, 0);
        assert!(result != 0);
    }

    #[test]
    fn test_unregister_callback() {
        let result = cell_sysutil_unregister_callback(0);
        assert_eq!(result, 0);
        
        // Invalid slot
        let result = cell_sysutil_unregister_callback(10);
        assert!(result != 0);
    }

    #[test]
    fn test_check_callback() {
        let result = cell_sysutil_check_callback();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_param_id_values() {
        assert_eq!(CellSysutilParamId::Language as u32, 0x0111);
        assert_eq!(CellSysutilParamId::EnterButtonAssign as u32, 0x0112);
        assert_eq!(CellSysutilParamId::Nickname as u32, 0x0131);
    }
}
