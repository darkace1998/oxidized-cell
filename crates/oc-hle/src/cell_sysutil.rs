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

/// Dialog status
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogStatus {
    /// No dialog active
    #[default]
    None = 0,
    /// Dialog is open
    Open = 1,
    /// Dialog closed with OK
    Ok = 2,
    /// Dialog closed with Cancel
    Cancel = 3,
    /// Dialog closed with error
    Error = 4,
}

/// Dialog type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    /// Message dialog
    Message = 0,
    /// Error dialog
    Error = 1,
    /// Progress dialog
    Progress = 2,
    /// Save data list dialog
    SaveDataList = 3,
    /// Game data dialog
    GameData = 4,
    /// Trophy dialog
    Trophy = 5,
}

/// Dialog state
#[derive(Debug, Clone)]
pub struct DialogState {
    /// Current dialog type
    pub dialog_type: Option<DialogType>,
    /// Dialog status
    pub status: DialogStatus,
    /// Dialog message
    pub message: String,
    /// Progress value (0-100)
    pub progress: u32,
    /// User selection (for list dialogs)
    pub selection: i32,
}

impl Default for DialogState {
    fn default() -> Self {
        Self {
            dialog_type: None,
            status: DialogStatus::None,
            message: String::new(),
            progress: 0,
            selection: -1,
        }
    }
}

/// PSID (PlayStation Identifier) - 16 bytes
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellSysutilPsid {
    /// High 64 bits
    pub high: u64,
    /// Low 64 bits
    pub low: u64,
}

/// Account information
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// Account ID
    pub account_id: u64,
    /// Username
    pub username: String,
    /// Online ID (PSN name)
    pub online_id: String,
    /// Region code
    pub region: u32,
    /// Language code
    pub language: u32,
}

impl Default for AccountInfo {
    fn default() -> Self {
        Self {
            account_id: 0x0001000000000001, // Default account ID
            username: "User".to_string(),
            online_id: "Player".to_string(),
            region: 1, // US
            language: 1, // English
        }
    }
}

/// Disc status
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscStatus {
    /// No disc
    #[default]
    NoDisc = 0,
    /// Disc inserted
    Inserted = 1,
    /// Reading disc
    Reading = 2,
    /// Disc ready
    Ready = 3,
    /// Disc error
    Error = 4,
}

/// Disc information
#[derive(Debug, Clone, Default)]
pub struct DiscInfo {
    /// Disc status
    pub status: DiscStatus,
    /// Disc type (0=Unknown, 1=PS3, 2=DVD, 3=BD)
    pub disc_type: u32,
    /// Game ID (e.g., "BLUS00000")
    pub game_id: String,
    /// Disc label/title
    pub label: String,
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
    /// Dialog state
    dialog: DialogState,
    /// PSID
    psid: CellSysutilPsid,
    /// Account information
    account: AccountInfo,
    /// Disc information
    disc: DiscInfo,
}

impl SysutilManager {
    /// Create a new system utility manager
    pub fn new() -> Self {
        let mut manager = Self {
            callbacks: [None; CELL_SYSUTIL_MAX_CALLBACK_SLOTS],
            pending_events: VecDeque::new(),
            int_params: HashMap::new(),
            string_params: HashMap::new(),
            dialog: DialogState::default(),
            psid: CellSysutilPsid {
                high: 0x0123456789ABCDEF,
                low: 0xFEDCBA9876543210,
            },
            account: AccountInfo::default(),
            disc: DiscInfo::default(),
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

    // ========================================================================
    // Dialog Support
    // ========================================================================

    /// Open a message dialog
    pub fn open_message_dialog(&mut self, message: &str) -> i32 {
        if self.dialog.dialog_type.is_some() {
            return 0x80010003u32 as i32; // Dialog already open
        }

        debug!("SysutilManager::open_message_dialog: {}", message);
        
        self.dialog.dialog_type = Some(DialogType::Message);
        self.dialog.status = DialogStatus::Open;
        self.dialog.message = message.to_string();
        
        0 // CELL_OK
    }

    /// Open an error dialog
    pub fn open_error_dialog(&mut self, error_code: u32, message: &str) -> i32 {
        if self.dialog.dialog_type.is_some() {
            return 0x80010003u32 as i32; // Dialog already open
        }

        debug!("SysutilManager::open_error_dialog: error=0x{:08X}, {}", error_code, message);
        
        self.dialog.dialog_type = Some(DialogType::Error);
        self.dialog.status = DialogStatus::Open;
        self.dialog.message = format!("Error 0x{:08X}: {}", error_code, message);
        
        0 // CELL_OK
    }

    /// Open a progress dialog
    pub fn open_progress_dialog(&mut self, message: &str) -> i32 {
        if self.dialog.dialog_type.is_some() {
            return 0x80010003u32 as i32; // Dialog already open
        }

        debug!("SysutilManager::open_progress_dialog: {}", message);
        
        self.dialog.dialog_type = Some(DialogType::Progress);
        self.dialog.status = DialogStatus::Open;
        self.dialog.message = message.to_string();
        self.dialog.progress = 0;
        
        0 // CELL_OK
    }

    /// Update progress dialog
    pub fn update_progress(&mut self, progress: u32) -> i32 {
        if self.dialog.dialog_type != Some(DialogType::Progress) {
            return 0x80010002u32 as i32; // No progress dialog open
        }

        self.dialog.progress = progress.min(100);
        trace!("SysutilManager::update_progress: {}%", self.dialog.progress);
        
        0 // CELL_OK
    }

    /// Close the current dialog
    pub fn close_dialog(&mut self, result: DialogStatus) -> i32 {
        if self.dialog.dialog_type.is_none() {
            return 0x80010002u32 as i32; // No dialog open
        }

        debug!("SysutilManager::close_dialog: result={:?}", result);
        
        self.dialog.status = result;
        self.dialog.dialog_type = None;
        self.dialog.message.clear();
        self.dialog.progress = 0;
        
        0 // CELL_OK
    }

    /// Get current dialog status
    pub fn get_dialog_status(&self) -> DialogStatus {
        self.dialog.status
    }

    /// Check if a dialog is currently open
    pub fn is_dialog_open(&self) -> bool {
        self.dialog.dialog_type.is_some()
    }

    /// Get dialog selection (for list dialogs)
    pub fn get_dialog_selection(&self) -> i32 {
        self.dialog.selection
    }

    /// Set dialog selection (for simulating user input)
    pub fn set_dialog_selection(&mut self, selection: i32) {
        self.dialog.selection = selection;
    }

    // ========================================================================
    // PSID/Account Handling
    // ========================================================================

    /// Get the system PSID
    pub fn get_psid(&self) -> CellSysutilPsid {
        self.psid
    }

    /// Set the system PSID (for testing)
    pub fn set_psid(&mut self, high: u64, low: u64) {
        self.psid.high = high;
        self.psid.low = low;
        debug!("SysutilManager::set_psid: {:016X}{:016X}", high, low);
    }

    /// Get the current account ID
    pub fn get_account_id(&self) -> u64 {
        self.account.account_id
    }

    /// Get the current account information
    pub fn get_account_info(&self) -> &AccountInfo {
        &self.account
    }

    /// Set account information
    pub fn set_account_info(&mut self, info: AccountInfo) {
        debug!("SysutilManager::set_account_info: user={}, online_id={}", 
               info.username, info.online_id);
        self.account = info;
    }

    /// Get the current online ID (PSN name)
    pub fn get_online_id(&self) -> &str {
        &self.account.online_id
    }

    /// Check if the user is signed in to PSN
    pub fn is_signed_in(&self) -> bool {
        self.account.account_id != 0
    }

    // ========================================================================
    // Disc Detection
    // ========================================================================

    /// Get the current disc status
    pub fn get_disc_status(&self) -> DiscStatus {
        self.disc.status
    }

    /// Get disc information
    pub fn get_disc_info(&self) -> &DiscInfo {
        &self.disc
    }

    /// Check if a disc is inserted
    pub fn is_disc_inserted(&self) -> bool {
        matches!(self.disc.status, DiscStatus::Inserted | DiscStatus::Reading | DiscStatus::Ready)
    }

    /// Check if disc is ready
    pub fn is_disc_ready(&self) -> bool {
        self.disc.status == DiscStatus::Ready
    }

    /// Set disc status (for simulating disc insertion/removal)
    pub fn set_disc_status(&mut self, status: DiscStatus) {
        debug!("SysutilManager::set_disc_status: {:?}", status);
        self.disc.status = status;
        
        // Queue appropriate system event
        let event_type = match status {
            DiscStatus::NoDisc => 0x0201, // Disc removed
            DiscStatus::Inserted | DiscStatus::Reading => 0x0200, // Disc inserted
            DiscStatus::Ready => 0x0202, // Disc ready
            DiscStatus::Error => 0x0203, // Disc error
        };
        self.queue_event(event_type, status as u64);
    }

    /// Set disc information
    pub fn set_disc_info(&mut self, info: DiscInfo) {
        debug!("SysutilManager::set_disc_info: type={}, game_id={}", 
               info.disc_type, info.game_id);
        self.disc = info;
    }

    /// Get the game ID from the current disc
    pub fn get_disc_game_id(&self) -> Option<&str> {
        if self.disc.status == DiscStatus::Ready && !self.disc.game_id.is_empty() {
            Some(&self.disc.game_id)
        } else {
            None
        }
    }

    /// Get the disc type
    pub fn get_disc_type(&self) -> u32 {
        self.disc.disc_type
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

    crate::context::get_hle_context_mut().sysutil.register_callback(slot, func, userdata)
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

    crate::context::get_hle_context_mut().sysutil.unregister_callback(slot)
}

/// cellSysutilCheckCallback - Check and process callbacks
///
/// Should be called regularly by the game (typically once per frame)
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_check_callback() -> i32 {
    trace!("cellSysutilCheckCallback()");

    crate::context::get_hle_context_mut().sysutil.check_callback()
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

    let ctx = crate::context::get_hle_context();
    if let Some(_value) = ctx.sysutil.get_system_param_int(param_id) {
        // TODO: Write value to memory at _value_addr
        0 // CELL_OK
    } else {
        0x80010002u32 as i32 // CELL_SYSUTIL_ERROR_VALUE
    }
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

    let ctx = crate::context::get_hle_context();
    if let Some(_value) = ctx.sysutil.get_system_param_string(param_id) {
        // TODO: Write string to memory at _buf_addr
        0 // CELL_OK
    } else {
        0x80010002u32 as i32 // CELL_SYSUTIL_ERROR_VALUE
    }
}

// ============================================================================
// Dialog Functions
// ============================================================================

/// cellMsgDialogOpen - Open a message dialog
///
/// # Arguments
/// * `type` - Dialog type
/// * `msg_addr` - Message address
/// * `callback` - Callback function
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_msg_dialog_open(_dialog_type: u32, _msg_addr: u32, _callback: u32, _userdata: u32) -> i32 {
    debug!("cellMsgDialogOpen()");
    
    // For now, open with a default message
    crate::context::get_hle_context_mut().sysutil.open_message_dialog("Message")
}

/// cellMsgDialogClose - Close the current message dialog
///
/// # Arguments
/// * `result` - Dialog result
///
/// # Returns
/// * 0 on success
pub fn cell_msg_dialog_close(result: u32) -> i32 {
    debug!("cellMsgDialogClose(result={})", result);
    
    let status = if result == 0 { DialogStatus::Ok } else { DialogStatus::Cancel };
    crate::context::get_hle_context_mut().sysutil.close_dialog(status)
}

/// cellMsgDialogProgressBarSetMsg - Set progress bar message
///
/// # Arguments
/// * `bar_index` - Progress bar index
/// * `msg_addr` - Message address
///
/// # Returns
/// * 0 on success
pub fn cell_msg_dialog_progress_bar_set_msg(_bar_index: u32, _msg_addr: u32) -> i32 {
    trace!("cellMsgDialogProgressBarSetMsg()");
    0 // CELL_OK
}

/// cellMsgDialogProgressBarInc - Increment progress bar
///
/// # Arguments
/// * `bar_index` - Progress bar index
/// * `delta` - Increment value
///
/// # Returns
/// * 0 on success
pub fn cell_msg_dialog_progress_bar_inc(_bar_index: u32, delta: u32) -> i32 {
    trace!("cellMsgDialogProgressBarInc(delta={})", delta);
    
    let ctx = crate::context::get_hle_context();
    let current = ctx.sysutil.dialog.progress;
    drop(ctx);
    
    crate::context::get_hle_context_mut().sysutil.update_progress(current + delta)
}

// ============================================================================
// PSID/Account Functions
// ============================================================================

/// cellSysutilGetPsId - Get the PlayStation ID
///
/// # Arguments
/// * `psid_addr` - Address to write PSID
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_get_ps_id(_psid_addr: u32) -> i32 {
    debug!("cellSysutilGetPsId()");
    
    let _psid = crate::context::get_hle_context().sysutil.get_psid();
    // TODO: Write PSID to memory at _psid_addr
    
    0 // CELL_OK
}

/// cellUserInfoGetStat - Get user information status
///
/// # Arguments
/// * `id` - User ID
/// * `stat_addr` - Address to write status
///
/// # Returns
/// * 0 on success
pub fn cell_user_info_get_stat(_id: u32, _stat_addr: u32) -> i32 {
    debug!("cellUserInfoGetStat()");
    
    // User is always signed in for HLE
    0 // CELL_OK
}

/// cellUserInfoGetList - Get user list
///
/// # Arguments
/// * `list_num_addr` - Address to write list count
/// * `list_addr` - Address to write user list
/// * `current_user_addr` - Address to write current user ID
///
/// # Returns
/// * 0 on success
pub fn cell_user_info_get_list(_list_num_addr: u32, _list_addr: u32, _current_user_addr: u32) -> i32 {
    debug!("cellUserInfoGetList()");
    
    // TODO: Write user list to memory
    // For now, return 1 user
    
    0 // CELL_OK
}

// ============================================================================
// Disc Detection Functions
// ============================================================================

/// cellDiscGameGetBootDiscInfo - Get boot disc information
///
/// # Arguments
/// * `info_addr` - Address to write disc info
///
/// # Returns
/// * 0 on success
pub fn cell_disc_game_get_boot_disc_info(_info_addr: u32) -> i32 {
    debug!("cellDiscGameGetBootDiscInfo()");
    
    let ctx = crate::context::get_hle_context();
    let disc = ctx.sysutil.get_disc_info();
    
    if disc.status != DiscStatus::Ready {
        return 0x80010002u32 as i32; // Disc not ready
    }
    
    // TODO: Write disc info to memory at _info_addr
    
    0 // CELL_OK
}

/// cellDiscGameRegisterDiscChangeCallback - Register disc change callback
///
/// # Arguments
/// * `callback` - Callback function address
/// * `userdata` - User data
///
/// # Returns
/// * 0 on success
pub fn cell_disc_game_register_disc_change_callback(_callback: u32, _userdata: u32) -> i32 {
    debug!("cellDiscGameRegisterDiscChangeCallback()");
    
    // For HLE, we just acknowledge the registration
    0 // CELL_OK
}

/// cellSysutilGetBgmPlaybackStatus - Get background music playback status
///
/// # Arguments
/// * `status_addr` - Address to write status
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_get_bgm_playback_status(_status_addr: u32) -> i32 {
    trace!("cellSysutilGetBgmPlaybackStatus()");
    
    // TODO: Write status to memory (0 = not playing)
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
        // First register a callback so we can unregister it
        crate::context::reset_hle_context();
        cell_sysutil_register_callback(0, 0x12345678, 0);
        
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

    // ========================================================================
    // Dialog Support Tests
    // ========================================================================

    #[test]
    fn test_sysutil_dialog_message() {
        let mut manager = SysutilManager::new();
        
        // No dialog initially
        assert!(!manager.is_dialog_open());
        assert_eq!(manager.get_dialog_status(), DialogStatus::None);
        
        // Open message dialog
        assert_eq!(manager.open_message_dialog("Test message"), 0);
        assert!(manager.is_dialog_open());
        assert_eq!(manager.get_dialog_status(), DialogStatus::Open);
        
        // Try to open another (should fail)
        assert!(manager.open_message_dialog("Another") != 0);
        
        // Close dialog
        assert_eq!(manager.close_dialog(DialogStatus::Ok), 0);
        assert!(!manager.is_dialog_open());
        assert_eq!(manager.get_dialog_status(), DialogStatus::Ok);
    }

    #[test]
    fn test_sysutil_dialog_error() {
        let mut manager = SysutilManager::new();
        
        assert_eq!(manager.open_error_dialog(0x80010001, "An error occurred"), 0);
        assert!(manager.is_dialog_open());
        
        assert_eq!(manager.close_dialog(DialogStatus::Ok), 0);
    }

    #[test]
    fn test_sysutil_dialog_progress() {
        let mut manager = SysutilManager::new();
        
        // Open progress dialog
        assert_eq!(manager.open_progress_dialog("Loading..."), 0);
        assert!(manager.is_dialog_open());
        
        // Update progress
        assert_eq!(manager.update_progress(50), 0);
        
        // Close
        assert_eq!(manager.close_dialog(DialogStatus::Ok), 0);
    }

    #[test]
    fn test_sysutil_dialog_progress_limits() {
        let mut manager = SysutilManager::new();
        manager.open_progress_dialog("Loading...");
        
        // Progress should be clamped to 100
        manager.update_progress(150);
        assert_eq!(manager.dialog.progress, 100);
    }

    #[test]
    fn test_sysutil_dialog_selection() {
        let mut manager = SysutilManager::new();
        
        manager.set_dialog_selection(2);
        assert_eq!(manager.get_dialog_selection(), 2);
        
        manager.set_dialog_selection(-1);
        assert_eq!(manager.get_dialog_selection(), -1);
    }

    // ========================================================================
    // PSID/Account Tests
    // ========================================================================

    #[test]
    fn test_sysutil_psid() {
        let mut manager = SysutilManager::new();
        
        // Get default PSID
        let psid = manager.get_psid();
        assert!(psid.high != 0 || psid.low != 0);
        
        // Set new PSID
        manager.set_psid(0x1234567890ABCDEF, 0xFEDCBA0987654321);
        let psid = manager.get_psid();
        assert_eq!(psid.high, 0x1234567890ABCDEF);
        assert_eq!(psid.low, 0xFEDCBA0987654321);
    }

    #[test]
    fn test_sysutil_account() {
        let manager = SysutilManager::new();
        
        // Check default account
        assert!(manager.get_account_id() != 0);
        assert!(manager.is_signed_in());
        
        let info = manager.get_account_info();
        assert!(!info.username.is_empty());
        assert!(!info.online_id.is_empty());
    }

    #[test]
    fn test_sysutil_account_modification() {
        let mut manager = SysutilManager::new();
        
        let new_info = AccountInfo {
            account_id: 0x0002000000000001,
            username: "TestUser".to_string(),
            online_id: "TestPlayer".to_string(),
            region: 2,
            language: 3,
        };
        
        manager.set_account_info(new_info);
        
        assert_eq!(manager.get_account_id(), 0x0002000000000001);
        assert_eq!(manager.get_online_id(), "TestPlayer");
    }

    // ========================================================================
    // Disc Detection Tests
    // ========================================================================

    #[test]
    fn test_sysutil_disc_status() {
        let mut manager = SysutilManager::new();
        
        // Default - no disc
        assert_eq!(manager.get_disc_status(), DiscStatus::NoDisc);
        assert!(!manager.is_disc_inserted());
        assert!(!manager.is_disc_ready());
        
        // Insert disc
        manager.set_disc_status(DiscStatus::Inserted);
        assert!(manager.is_disc_inserted());
        assert!(!manager.is_disc_ready());
        
        // Disc ready
        manager.set_disc_status(DiscStatus::Ready);
        assert!(manager.is_disc_inserted());
        assert!(manager.is_disc_ready());
        
        // Remove disc
        manager.set_disc_status(DiscStatus::NoDisc);
        assert!(!manager.is_disc_inserted());
    }

    #[test]
    fn test_sysutil_disc_info() {
        let mut manager = SysutilManager::new();
        
        let info = DiscInfo {
            status: DiscStatus::Ready,
            disc_type: 1, // PS3 disc
            game_id: "BLUS00001".to_string(),
            label: "Test Game".to_string(),
        };
        
        manager.set_disc_info(info);
        
        let disc = manager.get_disc_info();
        assert_eq!(disc.disc_type, 1);
        assert_eq!(disc.game_id, "BLUS00001");
    }

    #[test]
    fn test_sysutil_disc_game_id() {
        let mut manager = SysutilManager::new();
        
        // No game ID when no disc
        assert!(manager.get_disc_game_id().is_none());
        
        // Set disc info
        manager.set_disc_info(DiscInfo {
            status: DiscStatus::Ready,
            disc_type: 1,
            game_id: "BLES00001".to_string(),
            label: "EU Game".to_string(),
        });
        
        assert_eq!(manager.get_disc_game_id(), Some("BLES00001"));
    }

    #[test]
    fn test_sysutil_disc_events() {
        let mut manager = SysutilManager::new();
        
        // Setting disc status should queue events
        manager.set_disc_status(DiscStatus::Inserted);
        assert_eq!(manager.pending_event_count(), 1);
        
        manager.set_disc_status(DiscStatus::Ready);
        assert_eq!(manager.pending_event_count(), 2);
    }

    // ========================================================================
    // Public API Tests
    // ========================================================================

    #[test]
    fn test_msg_dialog_api() {
        crate::context::reset_hle_context();
        
        assert_eq!(cell_msg_dialog_open(0, 0, 0, 0), 0);
        assert_eq!(cell_msg_dialog_close(0), 0);
    }

    #[test]
    fn test_psid_api() {
        crate::context::reset_hle_context();
        
        assert_eq!(cell_sysutil_get_ps_id(0x10000000), 0);
    }

    #[test]
    fn test_user_info_api() {
        crate::context::reset_hle_context();
        
        assert_eq!(cell_user_info_get_stat(0, 0), 0);
        assert_eq!(cell_user_info_get_list(0, 0, 0), 0);
    }

    #[test]
    fn test_disc_api() {
        crate::context::reset_hle_context();
        
        // Set up a ready disc first
        crate::context::get_hle_context_mut().sysutil.set_disc_info(DiscInfo {
            status: DiscStatus::Ready,
            disc_type: 1,
            game_id: "TEST00001".to_string(),
            label: "Test".to_string(),
        });
        
        assert_eq!(cell_disc_game_get_boot_disc_info(0), 0);
        assert_eq!(cell_disc_game_register_disc_change_callback(0, 0), 0);
    }

    #[test]
    fn test_dialog_status_enum() {
        assert_eq!(DialogStatus::None as u32, 0);
        assert_eq!(DialogStatus::Open as u32, 1);
        assert_eq!(DialogStatus::Ok as u32, 2);
        assert_eq!(DialogStatus::Cancel as u32, 3);
    }

    #[test]
    fn test_disc_status_enum() {
        assert_eq!(DiscStatus::NoDisc as u32, 0);
        assert_eq!(DiscStatus::Inserted as u32, 1);
        assert_eq!(DiscStatus::Ready as u32, 3);
    }
}
