//! cellSysutil HLE - System Utilities
//!
//! This module provides system utility functions including callback management,
//! system events, and game exit handling.

use std::collections::{HashMap, VecDeque};
use tracing::{debug, trace};
use crate::memory::{write_be32, write_bytes, write_string, read_be32};

/// Maximum number of callback slots
pub const CELL_SYSUTIL_MAX_CALLBACK_SLOTS: usize = 4;

/// Error: Invalid value/parameter
pub const CELL_SYSUTIL_ERROR_VALUE: i32 = 0x80010002u32 as i32;

/// Error: Dialog already open
pub const CELL_SYSUTIL_ERROR_DIALOG_ALREADY_OPEN: i32 = 0x80010003u32 as i32;

/// Get current UNIX timestamp
fn get_current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// System callback function type
pub type SysutilCallback = fn(status: u64, param: u64, userdata: u64);

/// System callback entry
#[derive(Debug, Clone, Copy)]
struct CallbackEntry {
    func: u32,      // Address of callback function
    userdata: u32,  // User data pointer
}

/// Pending callback - describes a callback that needs to be invoked on PPU
#[derive(Debug, Clone, Copy)]
pub struct PendingCallback {
    /// Function address to call
    pub func: u32,
    /// Event status/type (first argument)
    pub status: u64,
    /// Event parameter (second argument)
    pub param: u64,
    /// User data pointer (third argument)
    pub userdata: u32,
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

// ============================================================================
// Trophy System
// ============================================================================

/// Trophy grade
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrophyGrade {
    /// Unknown grade
    #[default]
    Unknown = 0,
    /// Platinum trophy
    Platinum = 1,
    /// Gold trophy
    Gold = 2,
    /// Silver trophy
    Silver = 3,
    /// Bronze trophy
    Bronze = 4,
}

/// Trophy state
#[derive(Debug, Clone)]
pub struct TrophyInfo {
    /// Trophy ID
    pub id: u32,
    /// Trophy name
    pub name: String,
    /// Trophy description
    pub description: String,
    /// Trophy grade
    pub grade: TrophyGrade,
    /// Whether trophy is unlocked
    pub unlocked: bool,
    /// Unlock timestamp (if unlocked)
    pub unlock_time: u64,
    /// Whether trophy is hidden until unlocked
    pub hidden: bool,
}

impl Default for TrophyInfo {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            description: String::new(),
            grade: TrophyGrade::Unknown,
            unlocked: false,
            unlock_time: 0,
            hidden: false,
        }
    }
}

/// Trophy context state
#[derive(Debug, Clone, Default)]
pub struct TrophyContext {
    /// Trophy set initialized
    pub initialized: bool,
    /// Communication ID (game ID)
    pub comm_id: String,
    /// Trophy count by grade
    pub trophy_counts: [u32; 5], // Unknown, Platinum, Gold, Silver, Bronze
    /// Unlocked counts by grade
    pub unlocked_counts: [u32; 5],
    /// All trophies
    pub trophies: Vec<TrophyInfo>,
}

// ============================================================================
// Screen Saver Control
// ============================================================================

/// Screen saver state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenSaverState {
    /// Screen saver enabled
    #[default]
    Enabled = 0,
    /// Screen saver disabled
    Disabled = 1,
}

// ============================================================================
// Video Settings
// ============================================================================

/// Video resolution
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoResolution {
    /// 480i
    Res480i = 1,
    /// 480p
    Res480p = 2,
    /// 576i
    Res576i = 3,
    /// 576p
    Res576p = 4,
    /// 720p
    Res720p = 5,
    /// 1080i
    Res1080i = 6,
    /// 1080p
    Res1080p = 7,
}

impl Default for VideoResolution {
    fn default() -> Self {
        Self::Res1080p
    }
}

/// Video aspect ratio
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoAspect {
    /// 4:3
    Aspect4_3 = 0,
    /// 16:9
    Aspect16_9 = 1,
}

impl Default for VideoAspect {
    fn default() -> Self {
        Self::Aspect16_9
    }
}

/// Video output settings
#[derive(Debug, Clone, Default)]
pub struct VideoSettings {
    /// Current resolution
    pub resolution: VideoResolution,
    /// Aspect ratio
    pub aspect: VideoAspect,
    /// Color space (0=RGB, 1=YCbCr)
    pub color_space: u32,
    /// Deep color mode (0=off, 1=10bit, 2=12bit)
    pub deep_color: u32,
    /// 3D enabled
    pub stereo_3d: bool,
}

// ============================================================================
// Audio Settings
// ============================================================================

/// Audio output mode
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioOutput {
    /// HDMI
    Hdmi = 0,
    /// Optical
    Optical = 1,
    /// AV Multi
    AvMulti = 2,
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::Hdmi
    }
}

/// Audio format
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// Linear PCM 2ch
    Lpcm2 = 0,
    /// Linear PCM 5.1ch
    Lpcm51 = 1,
    /// Linear PCM 7.1ch
    Lpcm71 = 2,
    /// Dolby Digital
    DolbyDigital = 3,
    /// DTS
    Dts = 4,
    /// AAC
    Aac = 5,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::Lpcm2
    }
}

/// Audio output settings
#[derive(Debug, Clone, Default)]
pub struct AudioSettings {
    /// Output device
    pub output: AudioOutput,
    /// Audio format
    pub format: AudioFormat,
    /// Volume level (0-100)
    pub volume: u32,
    /// Muted
    pub muted: bool,
    /// Downmix enabled (convert surround to stereo)
    pub downmix: bool,
}

/// System utility manager
pub struct SysutilManager {
    /// Registered callbacks by slot
    callbacks: [Option<CallbackEntry>; CELL_SYSUTIL_MAX_CALLBACK_SLOTS],
    /// Pending events queue
    pending_events: VecDeque<SystemEvent>,
    /// Pending callbacks ready to invoke on PPU
    pending_callbacks: VecDeque<PendingCallback>,
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
    /// Trophy context
    trophy: TrophyContext,
    /// Screen saver state
    screen_saver: ScreenSaverState,
    /// Video settings
    video: VideoSettings,
    /// Audio settings
    audio_settings: AudioSettings,
    /// Background music enabled
    bgm_enabled: bool,
    /// Background music volume (0-100)
    bgm_volume: u32,
}

impl SysutilManager {
    /// Create a new system utility manager
    pub fn new() -> Self {
        let mut manager = Self {
            callbacks: [None; CELL_SYSUTIL_MAX_CALLBACK_SLOTS],
            pending_events: VecDeque::new(),
            pending_callbacks: VecDeque::new(),
            int_params: HashMap::new(),
            string_params: HashMap::new(),
            dialog: DialogState::default(),
            psid: CellSysutilPsid {
                high: 0x0123456789ABCDEF,
                low: 0xFEDCBA9876543210,
            },
            account: AccountInfo::default(),
            disc: DiscInfo::default(),
            trophy: TrophyContext::default(),
            screen_saver: ScreenSaverState::default(),
            video: VideoSettings::default(),
            audio_settings: AudioSettings::default(),
            bgm_enabled: true,
            bgm_volume: 100,
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
    /// 
    /// This processes pending events and generates callbacks that need to be
    /// invoked on the PPU. The runner should call `pop_pending_callback()` after
    /// this returns to get callbacks to invoke.
    pub fn check_callback(&mut self) -> i32 {
        trace!("SysutilManager::check_callback()");

        // Process pending events and generate callbacks for registered handlers
        while let Some(event) = self.pending_events.pop_front() {
            debug!(
                "Processing system event: type=0x{:X}, param=0x{:X}",
                event.event_type, event.param
            );
            
            // Send the event to all registered callback slots
            for slot in 0..CELL_SYSUTIL_MAX_CALLBACK_SLOTS {
                if let Some(entry) = &self.callbacks[slot] {
                    // Queue a pending callback for this slot
                    self.pending_callbacks.push_back(PendingCallback {
                        func: entry.func,
                        status: event.event_type,
                        param: event.param,
                        userdata: entry.userdata,
                    });
                    
                    debug!(
                        "Queued callback: slot={}, func=0x{:08X}, status=0x{:X}, param=0x{:X}",
                        slot, entry.func, event.event_type, event.param
                    );
                }
            }
        }

        0 // CELL_OK
    }

    /// Pop a pending callback that needs to be invoked on PPU
    /// 
    /// Returns None if there are no pending callbacks
    pub fn pop_pending_callback(&mut self) -> Option<PendingCallback> {
        self.pending_callbacks.pop_front()
    }

    /// Check if there are pending callbacks to invoke
    pub fn has_pending_callbacks(&self) -> bool {
        !self.pending_callbacks.is_empty()
    }

    /// Get the number of pending callbacks
    pub fn pending_callback_count(&self) -> usize {
        self.pending_callbacks.len()
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
            return CELL_SYSUTIL_ERROR_DIALOG_ALREADY_OPEN;
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
            return CELL_SYSUTIL_ERROR_DIALOG_ALREADY_OPEN;
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
            return CELL_SYSUTIL_ERROR_DIALOG_ALREADY_OPEN;
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
            return CELL_SYSUTIL_ERROR_VALUE; // No progress dialog open
        }

        self.dialog.progress = progress.min(100);
        trace!("SysutilManager::update_progress: {}%", self.dialog.progress);
        
        0 // CELL_OK
    }

    /// Close the current dialog
    pub fn close_dialog(&mut self, result: DialogStatus) -> i32 {
        if self.dialog.dialog_type.is_none() {
            return CELL_SYSUTIL_ERROR_VALUE; // No dialog open
        }

        debug!("SysutilManager::close_dialog: result={:?}", result);
        
        // Determine the event status code based on dialog result
        let event_status = match result {
            DialogStatus::Ok => 0x0000, // CELL_MSGDIALOG_BUTTON_YES / OK
            DialogStatus::Cancel => 0x0001, // CELL_MSGDIALOG_BUTTON_NO / Cancel
            DialogStatus::Error => 0x0002, // Error
            _ => 0x0000,
        };
        
        // Queue a dialog finished event so registered callbacks get notified
        self.queue_event(
            CellSysutilEvent::MenuClose as u64,
            event_status,
        );
        
        self.dialog.status = result;
        self.dialog.dialog_type = None;
        self.dialog.message.clear();
        self.dialog.progress = 0;
        
        0 // CELL_OK
    }

    /// Close dialog with OK result (simulates user pressing OK/Yes)
    pub fn close_dialog_ok(&mut self) -> i32 {
        self.close_dialog(DialogStatus::Ok)
    }

    /// Close dialog with Cancel result (simulates user pressing Cancel/No)
    pub fn close_dialog_cancel(&mut self) -> i32 {
        self.close_dialog(DialogStatus::Cancel)
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

    // ========================================================================
    // Trophy System
    // ========================================================================

    /// Initialize trophy context
    pub fn trophy_init(&mut self, comm_id: &str) -> i32 {
        if self.trophy.initialized {
            return CELL_SYSUTIL_ERROR_VALUE; // Already initialized
        }

        debug!("SysutilManager::trophy_init: comm_id={}", comm_id);

        self.trophy.initialized = true;
        self.trophy.comm_id = comm_id.to_string();
        self.trophy.trophy_counts = [0; 5];
        self.trophy.unlocked_counts = [0; 5];
        self.trophy.trophies.clear();

        0 // CELL_OK
    }

    /// Terminate trophy context
    pub fn trophy_term(&mut self) -> i32 {
        if !self.trophy.initialized {
            return CELL_SYSUTIL_ERROR_VALUE; // Not initialized
        }

        debug!("SysutilManager::trophy_term");

        self.trophy = TrophyContext::default();

        0 // CELL_OK
    }

    /// Register a trophy
    pub fn trophy_register(&mut self, id: u32, name: &str, description: &str, grade: TrophyGrade, hidden: bool) -> i32 {
        if !self.trophy.initialized {
            return CELL_SYSUTIL_ERROR_VALUE;
        }

        let trophy = TrophyInfo {
            id,
            name: name.to_string(),
            description: description.to_string(),
            grade,
            unlocked: false,
            unlock_time: 0,
            hidden,
        };

        // Update counts
        self.trophy.trophy_counts[grade as usize] += 1;

        self.trophy.trophies.push(trophy);

        debug!("SysutilManager::trophy_register: id={}, name={}, grade={:?}", id, name, grade);

        0 // CELL_OK
    }

    /// Unlock a trophy
    pub fn trophy_unlock(&mut self, trophy_id: u32) -> i32 {
        if !self.trophy.initialized {
            return CELL_SYSUTIL_ERROR_VALUE;
        }

        if let Some(trophy) = self.trophy.trophies.iter_mut().find(|t| t.id == trophy_id) {
            if trophy.unlocked {
                return CELL_SYSUTIL_ERROR_VALUE; // Already unlocked
            }

            trophy.unlocked = true;
            trophy.unlock_time = get_current_unix_timestamp();

            // Update unlocked count
            self.trophy.unlocked_counts[trophy.grade as usize] += 1;

            debug!("SysutilManager::trophy_unlock: id={}, name={}", trophy_id, trophy.name);

            // Queue trophy notification event
            self.queue_event(0x0301, trophy_id as u64); // Trophy unlocked event

            0 // CELL_OK
        } else {
            CELL_SYSUTIL_ERROR_VALUE // Trophy not found
        }
    }

    /// Get trophy info
    pub fn trophy_get_info(&self, trophy_id: u32) -> Option<&TrophyInfo> {
        if !self.trophy.initialized {
            return None;
        }

        self.trophy.trophies.iter().find(|t| t.id == trophy_id)
    }

    /// Check if trophy is unlocked
    pub fn trophy_is_unlocked(&self, trophy_id: u32) -> bool {
        self.trophy_get_info(trophy_id)
            .map(|t| t.unlocked)
            .unwrap_or(false)
    }

    /// Get trophy progress (unlocked / total)
    pub fn trophy_get_progress(&self) -> (u32, u32) {
        let total: u32 = self.trophy.trophy_counts.iter().sum();
        let unlocked: u32 = self.trophy.unlocked_counts.iter().sum();
        (unlocked, total)
    }

    /// Check if trophy context is initialized
    pub fn trophy_is_initialized(&self) -> bool {
        self.trophy.initialized
    }

    // ========================================================================
    // Screen Saver Control
    // ========================================================================

    /// Get screen saver state
    pub fn get_screen_saver_state(&self) -> ScreenSaverState {
        self.screen_saver
    }

    /// Enable screen saver
    pub fn enable_screen_saver(&mut self) -> i32 {
        debug!("SysutilManager::enable_screen_saver");
        self.screen_saver = ScreenSaverState::Enabled;
        0 // CELL_OK
    }

    /// Disable screen saver
    pub fn disable_screen_saver(&mut self) -> i32 {
        debug!("SysutilManager::disable_screen_saver");
        self.screen_saver = ScreenSaverState::Disabled;
        0 // CELL_OK
    }

    // ========================================================================
    // Video Settings
    // ========================================================================

    /// Get video settings
    pub fn get_video_settings(&self) -> &VideoSettings {
        &self.video
    }

    /// Get current resolution
    pub fn get_resolution(&self) -> VideoResolution {
        self.video.resolution
    }

    /// Set resolution
    pub fn set_resolution(&mut self, resolution: VideoResolution) -> i32 {
        debug!("SysutilManager::set_resolution: {:?}", resolution);
        self.video.resolution = resolution;
        0 // CELL_OK
    }

    /// Get aspect ratio
    pub fn get_aspect_ratio(&self) -> VideoAspect {
        self.video.aspect
    }

    /// Set aspect ratio
    pub fn set_aspect_ratio(&mut self, aspect: VideoAspect) -> i32 {
        debug!("SysutilManager::set_aspect_ratio: {:?}", aspect);
        self.video.aspect = aspect;
        0 // CELL_OK
    }

    /// Check if 3D output is enabled
    pub fn is_3d_enabled(&self) -> bool {
        self.video.stereo_3d
    }

    /// Enable/disable 3D output
    pub fn set_3d_enabled(&mut self, enabled: bool) -> i32 {
        debug!("SysutilManager::set_3d_enabled: {}", enabled);
        self.video.stereo_3d = enabled;
        0 // CELL_OK
    }

    // ========================================================================
    // Audio Settings
    // ========================================================================

    /// Get audio settings
    pub fn get_audio_settings(&self) -> &AudioSettings {
        &self.audio_settings
    }

    /// Get audio output device
    pub fn get_audio_output(&self) -> AudioOutput {
        self.audio_settings.output
    }

    /// Set audio output device
    pub fn set_audio_output(&mut self, output: AudioOutput) -> i32 {
        debug!("SysutilManager::set_audio_output: {:?}", output);
        self.audio_settings.output = output;
        0 // CELL_OK
    }

    /// Get audio format
    pub fn get_audio_format(&self) -> AudioFormat {
        self.audio_settings.format
    }

    /// Set audio format
    pub fn set_audio_format(&mut self, format: AudioFormat) -> i32 {
        debug!("SysutilManager::set_audio_format: {:?}", format);
        self.audio_settings.format = format;
        0 // CELL_OK
    }

    /// Get audio volume
    pub fn get_audio_volume(&self) -> u32 {
        self.audio_settings.volume
    }

    /// Set audio volume (0-100)
    pub fn set_audio_volume(&mut self, volume: u32) -> i32 {
        self.audio_settings.volume = volume.min(100);
        trace!("SysutilManager::set_audio_volume: {}", self.audio_settings.volume);
        0 // CELL_OK
    }

    /// Check if audio is muted
    pub fn is_audio_muted(&self) -> bool {
        self.audio_settings.muted
    }

    /// Mute/unmute audio
    pub fn set_audio_muted(&mut self, muted: bool) -> i32 {
        debug!("SysutilManager::set_audio_muted: {}", muted);
        self.audio_settings.muted = muted;
        0 // CELL_OK
    }

    // ========================================================================
    // Background Music Control
    // ========================================================================

    /// Check if background music playback is enabled
    pub fn is_bgm_playback_enabled(&self) -> bool {
        self.bgm_enabled
    }

    /// Enable/disable background music playback
    pub fn set_bgm_playback_enabled(&mut self, enabled: bool) -> i32 {
        debug!("SysutilManager::set_bgm_playback_enabled: {}", enabled);
        self.bgm_enabled = enabled;
        0 // CELL_OK
    }

    /// Get background music volume
    pub fn get_bgm_volume(&self) -> u32 {
        self.bgm_volume
    }

    /// Set background music volume (0-100)
    pub fn set_bgm_volume(&mut self, volume: u32) -> i32 {
        self.bgm_volume = volume.min(100);
        debug!("SysutilManager::set_bgm_volume: {}", self.bgm_volume);
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
/// Retrieves an integer system parameter and writes it to the provided
/// memory address.
///
/// # Arguments
/// * `param_id` - Parameter ID
/// * `value_addr` - Address to write value to
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_get_system_param_int(param_id: u32, value_addr: u32) -> i32 {
    debug!("cellSysutilGetSystemParamInt(param_id=0x{:X}, value_addr=0x{:08X})", param_id, value_addr);

    // Validate output address
    if value_addr == 0 {
        return CELL_SYSUTIL_ERROR_VALUE;
    }

    let ctx = crate::context::get_hle_context();
    if let Some(value) = ctx.sysutil.get_system_param_int(param_id) {
        // Write value to memory
        if let Err(_) = crate::memory::write_be32(value_addr, value as u32) {
            return CELL_SYSUTIL_ERROR_VALUE;
        }
        0 // CELL_OK
    } else {
        CELL_SYSUTIL_ERROR_VALUE
    }
}

/// cellSysutilGetSystemParamString - Get system parameter (string)
///
/// Retrieves a string system parameter and writes it to the provided
/// memory buffer.
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
    buf_addr: u32,
    buf_size: u32,
) -> i32 {
    debug!(
        "cellSysutilGetSystemParamString(param_id=0x{:X}, buf_addr=0x{:08X}, buf_size={})",
        param_id, buf_addr, buf_size
    );

    // Validate output address and size
    if buf_addr == 0 || buf_size == 0 {
        return CELL_SYSUTIL_ERROR_VALUE;
    }

    let ctx = crate::context::get_hle_context();
    if let Some(value) = ctx.sysutil.get_system_param_string(param_id) {
        // Write string to memory
        if let Err(_) = crate::memory::write_string(buf_addr, value, buf_size) {
            return CELL_SYSUTIL_ERROR_VALUE;
        }
        0 // CELL_OK
    } else {
        CELL_SYSUTIL_ERROR_VALUE
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
    
    // Use a single mutable context access to avoid race conditions
    let mut ctx = crate::context::get_hle_context_mut();
    let current = ctx.sysutil.dialog.progress;
    ctx.sysutil.update_progress(current + delta)
}

// ============================================================================
// PSID/Account Functions
// ============================================================================

/// cellSysutilGetPsId - Get the PlayStation ID
///
/// Retrieves the PlayStation ID (PSID) and writes it to the provided
/// memory address. The PSID is a 16-byte unique identifier.
///
/// # Arguments
/// * `psid_addr` - Address to write PSID (16 bytes)
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_get_ps_id(psid_addr: u32) -> i32 {
    debug!("cellSysutilGetPsId(psid_addr=0x{:08X})", psid_addr);
    
    // Validate output address
    if psid_addr == 0 {
        return CELL_SYSUTIL_ERROR_VALUE;
    }
    
    let psid = crate::context::get_hle_context().sysutil.get_psid();
    
    // Write PSID to memory (16 bytes: high u64 + low u64)
    if let Err(_) = crate::memory::write_be64(psid_addr, psid.high) {
        return CELL_SYSUTIL_ERROR_VALUE;
    }
    if let Err(_) = crate::memory::write_be64(psid_addr + 8, psid.low) {
        return CELL_SYSUTIL_ERROR_VALUE;
    }
    
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
pub fn cell_user_info_get_list(list_num_addr: u32, _list_addr: u32, current_user_addr: u32) -> i32 {
    debug!("cellUserInfoGetList(list_num={:08X}, current_user={:08X})", list_num_addr, current_user_addr);
    
    // Write user count = 1
    if list_num_addr != 0 {
        if let Err(e) = write_be32(list_num_addr, 1) {
            return e;
        }
    }
    
    // Write current user ID = 0
    if current_user_addr != 0 {
        if let Err(e) = write_be32(current_user_addr, 0) {
            return e;
        }
    }
    
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
pub fn cell_disc_game_get_boot_disc_info(info_addr: u32) -> i32 {
    debug!("cellDiscGameGetBootDiscInfo(info_addr=0x{:08X})", info_addr);
    
    let ctx = crate::context::get_hle_context();
    let disc = ctx.sysutil.get_disc_info();
    
    if disc.status != DiscStatus::Ready {
        return 0x80010002u32 as i32; // Disc not ready
    }
    
    // Write disc info to memory at info_addr
    // CellDiscGameDiscInfo structure:
    //   uint8_t type;        // offset 0: disc type (1=PS3 game disc)
    //   uint8_t reserved[3]; // offset 1-3: padding
    //   char titleId[10];    // offset 4: title ID (e.g., "BLUS00001")
    //   char reserved2[6];   // offset 14: padding
    if info_addr != 0 {
        // Write disc type (1 = PS3 game disc)
        if let Err(e) = write_be32(info_addr, disc.disc_type << 24) {
            return e;
        }
        
        // Write title ID (game_id) as null-terminated string
        let game_id = &disc.game_id;
        let game_id_bytes: Vec<u8> = game_id.bytes().take(9).collect();
        let mut title_id_buf = [0u8; 10];
        for (i, &b) in game_id_bytes.iter().enumerate() {
            title_id_buf[i] = b;
        }
        if let Err(e) = write_bytes(info_addr + 4, &title_id_buf) {
            return e;
        }
    }
    
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
pub fn cell_sysutil_get_bgm_playback_status(status_addr: u32) -> i32 {
    trace!("cellSysutilGetBgmPlaybackStatus(status_addr=0x{:08X})", status_addr);
    
    // Write status = 0 (not playing)
    if status_addr != 0 {
        if let Err(e) = write_be32(status_addr, 0) {
            return e;
        }
    }
    0 // CELL_OK
}

// ============================================================================
// Screen Saver Control Functions
// ============================================================================

/// cellSysutilEnableBgmPlayback - Enable background music playback
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_enable_bgm_playback() -> i32 {
    debug!("cellSysutilEnableBgmPlayback()");
    crate::context::get_hle_context_mut().sysutil.set_bgm_playback_enabled(true)
}

/// cellSysutilDisableBgmPlayback - Disable background music playback
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_disable_bgm_playback() -> i32 {
    debug!("cellSysutilDisableBgmPlayback()");
    crate::context::get_hle_context_mut().sysutil.set_bgm_playback_enabled(false)
}

/// cellSysutilSetBgmPlaybackVolume - Set background music volume
///
/// # Arguments
/// * `volume` - Volume level (0-100)
///
/// # Returns
/// * 0 on success
pub fn cell_sysutil_set_bgm_playback_volume(volume: u32) -> i32 {
    debug!("cellSysutilSetBgmPlaybackVolume(volume={})", volume);
    crate::context::get_hle_context_mut().sysutil.set_bgm_volume(volume)
}

/// cellScreenSaverEnable - Enable screen saver
///
/// # Returns
/// * 0 on success
pub fn cell_screen_saver_enable() -> i32 {
    debug!("cellScreenSaverEnable()");
    crate::context::get_hle_context_mut().sysutil.enable_screen_saver()
}

/// cellScreenSaverDisable - Disable screen saver
///
/// # Returns
/// * 0 on success
pub fn cell_screen_saver_disable() -> i32 {
    debug!("cellScreenSaverDisable()");
    crate::context::get_hle_context_mut().sysutil.disable_screen_saver()
}

// ============================================================================
// Video Settings Functions
// ============================================================================

/// cellVideoOutGetResolutionAvailability - Get resolution availability
///
/// # Arguments
/// * `video_out` - Video output type
/// * `resolution_id` - Resolution ID
/// * `aspect` - Aspect ratio
/// * `option` - Option flags
///
/// # Returns
/// * 1 if available, 0 if not
pub fn cell_video_out_get_resolution_availability(
    _video_out: u32,
    resolution_id: u32,
    _aspect: u32,
    _option: u32,
) -> i32 {
    trace!("cellVideoOutGetResolutionAvailability(resolution_id={})", resolution_id);
    
    // For HLE, report common resolutions as available
    match resolution_id {
        1 | 2 | 4 | 5 | 6 | 7 => 1, // 480i, 480p, 576p, 720p, 1080i, 1080p
        _ => 0,
    }
}

/// cellVideoOutGetState - Get video output state
///
/// # Arguments
/// * `video_out` - Video output type
/// * `device_index` - Device index
/// * `state_addr` - Address to write state
///
/// # Returns
/// * 0 on success
pub fn cell_video_out_get_state(_video_out: u32, _device_index: u32, state_addr: u32) -> i32 {
    trace!("cellVideoOutGetState(state_addr=0x{:08X})", state_addr);
    
    // Write video output state structure
    // struct CellVideoOutState { state: u8, colorSpace: u8, reserved[6], displayMode: CellVideoOutDisplayMode }
    if state_addr != 0 {
        // state = 2 (CELL_VIDEO_OUT_OUTPUT_STATE_ENABLED)
        if let Err(e) = write_be32(state_addr, 0x02000000) {
            return e;
        }
        // displayMode: resolutionId=2 (720p), scanMode=0, conversion=0, aspect=0, reserved, refreshRate
        if let Err(e) = write_be32(state_addr + 8, 0x02000000) {
            return e;
        }
    }
    0 // CELL_OK
}

/// cellVideoOutConfigure - Configure video output
///
/// # Arguments
/// * `video_out` - Video output type
/// * `config_addr` - Configuration address
/// * `option_addr` - Options address
/// * `wait` - Wait flag
///
/// # Returns
/// * 0 on success
pub fn cell_video_out_configure(
    _video_out: u32,
    config_addr: u32,
    _option_addr: u32,
    _wait: u32,
) -> i32 {
    debug!("cellVideoOutConfigure(config_addr=0x{:08X})", config_addr);
    
    // Read configuration from memory and apply
    // CellVideoOutConfiguration structure:
    //   uint8_t  resolutionId;  // offset 0
    //   uint8_t  format;        // offset 1
    //   uint8_t  aspect;        // offset 2
    //   uint8_t  reserved[9];   // offset 3-11
    //   uint32_t pitch;         // offset 12
    if config_addr != 0 {
        // Read resolution and format from config
        let config_word = read_be32(config_addr).unwrap_or(0);
        let resolution_id = (config_word >> 24) as u8;
        let _format = ((config_word >> 16) & 0xFF) as u8;
        
        // Convert resolution ID to VideoResolution enum
        let resolution = match resolution_id {
            1 => VideoResolution::Res480i,
            2 => VideoResolution::Res480p,
            3 => VideoResolution::Res576i,
            4 => VideoResolution::Res576p,
            5 => VideoResolution::Res720p,
            6 => VideoResolution::Res1080i,
            7 => VideoResolution::Res1080p,
            _ => VideoResolution::Res720p, // Default
        };
        
        // Apply configuration (store in sysutil manager)
        let mut ctx = crate::context::get_hle_context_mut();
        ctx.sysutil.set_resolution(resolution);
    }
    
    0 // CELL_OK
}

/// cellVideoOutGetConfiguration - Get video output configuration
///
/// # Arguments
/// * `video_out` - Video output type
/// * `config_addr` - Address to write configuration
/// * `option_addr` - Address to write options
///
/// # Returns
/// * 0 on success
pub fn cell_video_out_get_configuration(
    _video_out: u32,
    config_addr: u32,
    _option_addr: u32,
) -> i32 {
    debug!("cellVideoOutGetConfiguration(config_addr=0x{:08X})", config_addr);
    
    // Write configuration to memory
    // CellVideoOutConfiguration structure:
    //   uint8_t  resolutionId;  // offset 0
    //   uint8_t  format;        // offset 1 (0=XRGB, 1=XBGR)
    //   uint8_t  aspect;        // offset 2 (0=4:3, 1=16:9)
    //   uint8_t  reserved[9];   // offset 3-11
    //   uint32_t pitch;         // offset 12
    if config_addr != 0 {
        let ctx = crate::context::get_hle_context();
        let resolution = ctx.sysutil.get_resolution() as u32;
        
        // Write resolution, format (XRGB=0), and aspect (16:9=1)
        let config_word: u32 = (resolution << 24) | (0 << 16) | (1 << 8);
        if let Err(e) = write_be32(config_addr, config_word) {
            return e;
        }
        
        // Write reserved bytes and pitch (1280 * 4 = 5120 for 720p)
        if let Err(e) = write_be32(config_addr + 4, 0) {
            return e;
        }
        if let Err(e) = write_be32(config_addr + 8, 0) {
            return e;
        }
        if let Err(e) = write_be32(config_addr + 12, 5120) {
            return e;
        }
    }
    
    0 // CELL_OK
}

// ============================================================================
// Audio Settings Functions
// ============================================================================

/// cellAudioOutGetState - Get audio output state
///
/// # Arguments
/// * `audio_out` - Audio output type
/// * `device_index` - Device index
/// * `state_addr` - Address to write state
///
/// # Returns
/// * 0 on success
pub fn cell_audio_out_get_state(_audio_out: u32, _device_index: u32, state_addr: u32) -> i32 {
    trace!("cellAudioOutGetState(state_addr=0x{:08X})", state_addr);
    
    // Write audio output state structure
    // struct CellAudioOutState { state: u8, encoder: u8, reserved[6], downMixer: u32, soundMode: CellAudioOutSoundMode }
    if state_addr != 0 {
        // state = 2 (enabled), encoder = 0 (LPCM)
        if let Err(e) = write_be32(state_addr, 0x02000000) {
            return e;
        }
        // downMixer = 0 (none)
        if let Err(e) = write_be32(state_addr + 8, 0) {
            return e;
        }
        // soundMode: type=0, channel=2, fs=48000, bitDepth=16
        if let Err(e) = write_be32(state_addr + 12, 0x0002BB80) {
            return e;
        }
        if let Err(e) = write_be32(state_addr + 16, 0x00100000) {
            return e;
        }
    }
    0 // CELL_OK
}

/// cellAudioOutConfigure - Configure audio output
///
/// # Arguments
/// * `audio_out` - Audio output type
/// * `config_addr` - Configuration address
/// * `option_addr` - Options address
/// * `wait` - Wait flag
///
/// # Returns
/// * 0 on success
pub fn cell_audio_out_configure(
    _audio_out: u32,
    config_addr: u32,
    _option_addr: u32,
    _wait: u32,
) -> i32 {
    debug!("cellAudioOutConfigure(config_addr=0x{:08X})", config_addr);
    
    // Read configuration from memory and apply
    // CellAudioOutConfiguration structure:
    //   uint8_t  channel;     // offset 0: channel count
    //   uint8_t  encoder;     // offset 1: encoder type
    //   uint8_t  reserved[2]; // offset 2-3: padding
    //   uint32_t downMixer;   // offset 4: down mixer mode
    if config_addr != 0 {
        // Read channel and encoder from config
        let config_word = read_be32(config_addr).unwrap_or(0);
        let _channel = (config_word >> 24) as u8;
        let _encoder = ((config_word >> 16) & 0xFF) as u8;
        
        // Audio configuration is acknowledged but doesn't need to be stored
        // since audio output is handled by the audio subsystem
    }
    
    0 // CELL_OK
}

/// cellAudioOutGetConfiguration - Get audio output configuration
///
/// # Arguments
/// * `audio_out` - Audio output type
/// * `config_addr` - Address to write configuration
/// * `option_addr` - Address to write options
///
/// # Returns
/// * 0 on success
pub fn cell_audio_out_get_configuration(
    _audio_out: u32,
    config_addr: u32,
    _option_addr: u32,
) -> i32 {
    debug!("cellAudioOutGetConfiguration(config_addr=0x{:08X})", config_addr);
    
    // Write configuration to memory
    // CellAudioOutConfiguration structure:
    //   uint8_t  channel;     // offset 0: channel count (2 for stereo)
    //   uint8_t  encoder;     // offset 1: encoder type (0 = LPCM)
    //   uint8_t  reserved[2]; // offset 2-3: padding
    //   uint32_t downMixer;   // offset 4: down mixer mode (0 = none)
    if config_addr != 0 {
        // Default: stereo (2 channels), LPCM encoder, no padding
        let config_word: u32 = (2 << 24) | (0 << 16);
        if let Err(e) = write_be32(config_addr, config_word) {
            return e;
        }
        
        // downMixer = 0 (none)
        if let Err(e) = write_be32(config_addr + 4, 0) {
            return e;
        }
    }
    
    0 // CELL_OK
}

// ============================================================================
// Trophy Functions
// ============================================================================

/// cellNpTrophyInit - Initialize trophy system
///
/// # Arguments
/// * `mem_container` - Memory container handle
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_init(_mem_container: u32) -> i32 {
    debug!("cellNpTrophyInit()");
    
    // Trophy initialization is handled per-context
    0 // CELL_OK
}

/// cellNpTrophyTerm - Terminate trophy system
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_term() -> i32 {
    debug!("cellNpTrophyTerm()");
    crate::context::get_hle_context_mut().sysutil.trophy_term()
}

/// cellNpTrophyCreateContext - Create trophy context
///
/// # Arguments
/// * `context_addr` - Address to write context handle
/// * `comm_id` - Communication ID (game ID)
/// * `comm_sign` - Communication signature
/// * `options` - Options
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_create_context(
    _context_addr: u32,
    _comm_id_addr: u32,
    _comm_sign_addr: u32,
    _options: u64,
) -> i32 {
    debug!("cellNpTrophyCreateContext()");
    
    // Initialize with a default comm_id for HLE
    crate::context::get_hle_context_mut().sysutil.trophy_init("NPWR00000")
}

/// cellNpTrophyDestroyContext - Destroy trophy context
///
/// # Arguments
/// * `context` - Context handle
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_destroy_context(_context: u32) -> i32 {
    debug!("cellNpTrophyDestroyContext()");
    crate::context::get_hle_context_mut().sysutil.trophy_term()
}

/// cellNpTrophyRegisterContext - Register trophy context
///
/// # Arguments
/// * `context` - Context handle
/// * `handle` - Handle
/// * `callback` - Status callback
/// * `callback_arg` - Callback argument
/// * `options` - Options
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_register_context(
    _context: u32,
    _handle: u32,
    _callback: u32,
    _callback_arg: u32,
    _options: u64,
) -> i32 {
    debug!("cellNpTrophyRegisterContext()");
    
    // Context registration is successful for HLE
    0 // CELL_OK
}

/// cellNpTrophyUnlockTrophy - Unlock a trophy
///
/// # Arguments
/// * `context` - Context handle
/// * `handle` - Handle
/// * `trophy_id` - Trophy ID
/// * `platinum_id_addr` - Address to write platinum trophy ID (if earned)
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_unlock_trophy(
    _context: u32,
    _handle: u32,
    trophy_id: u32,
    _platinum_id_addr: u32,
) -> i32 {
    debug!("cellNpTrophyUnlockTrophy(trophy_id={})", trophy_id);
    crate::context::get_hle_context_mut().sysutil.trophy_unlock(trophy_id)
}

/// cellNpTrophyGetTrophyInfo - Get trophy information
///
/// # Arguments
/// * `context` - Context handle
/// * `handle` - Handle
/// * `trophy_id` - Trophy ID
/// * `details_addr` - Address to write trophy details
/// * `data_addr` - Address to write trophy data
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_get_trophy_info(
    _context: u32,
    _handle: u32,
    trophy_id: u32,
    details_addr: u32,
    data_addr: u32,
) -> i32 {
    trace!("cellNpTrophyGetTrophyInfo(trophy_id={}, details_addr=0x{:08X}, data_addr=0x{:08X})", 
        trophy_id, details_addr, data_addr);
    
    let ctx = crate::context::get_hle_context();
    if let Some(trophy_info) = ctx.sysutil.trophy_get_info(trophy_id) {
        // Write trophy details to memory
        // SceNpTrophyDetails structure:
        //   uint32_t trophyId;         // offset 0
        //   uint32_t trophyGrade;      // offset 4: platinum=1, gold=2, silver=3, bronze=4
        //   char     name[128];        // offset 8
        //   char     description[1024];// offset 136
        //   uint8_t  hidden;           // offset 1160
        //   uint8_t  reserved[3];      // offset 1161-1163
        if details_addr != 0 {
            // Write trophy ID
            if let Err(e) = write_be32(details_addr, trophy_id) {
                return e;
            }
            // Write trophy grade
            if let Err(e) = write_be32(details_addr + 4, trophy_info.grade as u32) {
                return e;
            }
            // Write name (max 127 chars + null)
            if let Err(e) = write_string(details_addr + 8, &trophy_info.name, 128) {
                return e;
            }
            // Write description (max 1023 chars + null)
            if let Err(e) = write_string(details_addr + 136, &trophy_info.description, 1024) {
                return e;
            }
            // Write hidden flag
            let hidden_byte = if trophy_info.hidden { 1u8 } else { 0u8 };
            if let Err(e) = write_bytes(details_addr + 1160, &[hidden_byte, 0, 0, 0]) {
                return e;
            }
        }
        
        // Write trophy data to memory
        // SceNpTrophyData structure:
        //   uint64_t timestamp;        // offset 0: unlock timestamp
        //   uint32_t trophyId;         // offset 8
        //   uint8_t  unlocked;         // offset 12
        //   uint8_t  reserved[3];      // offset 13-15
        if data_addr != 0 {
            // Write timestamp
            if let Err(e) = write_be32(data_addr, (trophy_info.unlock_time >> 32) as u32) {
                return e;
            }
            if let Err(e) = write_be32(data_addr + 4, trophy_info.unlock_time as u32) {
                return e;
            }
            // Write trophy ID
            if let Err(e) = write_be32(data_addr + 8, trophy_id) {
                return e;
            }
            // Write unlocked flag
            let unlocked_byte = if trophy_info.unlocked { 1u8 } else { 0u8 };
            if let Err(e) = write_bytes(data_addr + 12, &[unlocked_byte, 0, 0, 0]) {
                return e;
            }
        }
        
        0 // CELL_OK
    } else {
        CELL_SYSUTIL_ERROR_VALUE
    }
}

/// cellNpTrophyGetGameProgress - Get overall game trophy progress
///
/// # Arguments
/// * `context` - Context handle
/// * `handle` - Handle
/// * `percentage_addr` - Address to write percentage
///
/// # Returns
/// * 0 on success
pub fn cell_np_trophy_get_game_progress(
    _context: u32,
    _handle: u32,
    percentage_addr: u32,
) -> i32 {
    trace!("cellNpTrophyGetGameProgress(percentage_addr=0x{:08X})", percentage_addr);
    
    let ctx = crate::context::get_hle_context();
    let (unlocked, total) = ctx.sysutil.trophy_get_progress();
    let percentage = if total > 0 { (unlocked * 100) / total } else { 0 };
    
    // Write percentage to memory as i32
    if percentage_addr != 0 {
        if let Err(e) = write_be32(percentage_addr, percentage as u32) {
            return e;
        }
    }
    
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
        
        // Process events (no callbacks registered, so no pending callbacks)
        assert_eq!(manager.check_callback(), 0);
        
        // Events should be processed
        assert_eq!(manager.pending_event_count(), 0);
        assert!(!manager.has_pending_callbacks());
    }

    #[test]
    fn test_sysutil_callback_invocation() {
        let mut manager = SysutilManager::new();
        
        // Register callbacks in two slots
        assert_eq!(manager.register_callback(0, 0x10000100, 0xDEAD0000), 0);
        assert_eq!(manager.register_callback(1, 0x10000200, 0xBEEF0000), 0);
        
        // Queue an event
        manager.queue_event(CellSysutilEvent::MenuOpen as u64, 0x42);
        
        // Process - this should generate 2 pending callbacks (one per slot)
        assert_eq!(manager.check_callback(), 0);
        
        // We should have 2 pending callbacks
        assert!(manager.has_pending_callbacks());
        assert_eq!(manager.pending_callback_count(), 2);
        
        // Pop first callback
        let cb1 = manager.pop_pending_callback().unwrap();
        assert_eq!(cb1.func, 0x10000100);
        assert_eq!(cb1.status, CellSysutilEvent::MenuOpen as u64);
        assert_eq!(cb1.param, 0x42);
        assert_eq!(cb1.userdata, 0xDEAD0000);
        
        // Pop second callback
        let cb2 = manager.pop_pending_callback().unwrap();
        assert_eq!(cb2.func, 0x10000200);
        assert_eq!(cb2.status, CellSysutilEvent::MenuOpen as u64);
        assert_eq!(cb2.param, 0x42);
        assert_eq!(cb2.userdata, 0xBEEF0000);
        
        // No more callbacks
        assert!(!manager.has_pending_callbacks());
        assert!(manager.pop_pending_callback().is_none());
    }

    #[test]
    fn test_dialog_close_queues_event() {
        let mut manager = SysutilManager::new();
        
        // Register a callback
        assert_eq!(manager.register_callback(0, 0x10000100, 0), 0);
        
        // Open a dialog
        assert_eq!(manager.open_message_dialog("Test message"), 0);
        assert!(manager.is_dialog_open());
        
        // Close with OK
        assert_eq!(manager.close_dialog_ok(), 0);
        assert!(!manager.is_dialog_open());
        
        // Should have queued a MenuClose event
        assert_eq!(manager.pending_event_count(), 1);
        
        // Process the event
        assert_eq!(manager.check_callback(), 0);
        
        // Should have generated a pending callback
        assert!(manager.has_pending_callbacks());
        let cb = manager.pop_pending_callback().unwrap();
        assert_eq!(cb.func, 0x10000100);
        assert_eq!(cb.status, CellSysutilEvent::MenuClose as u64);
        assert_eq!(cb.param, 0x0000); // OK result
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
        
        // Test that null address is rejected
        assert!(cell_sysutil_get_ps_id(0) != 0);
        
        // When memory subsystem is initialized, the function would succeed
        // with a valid address. Without memory, we can only test validation.
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

    // ========================================================================
    // Trophy System Tests
    // ========================================================================

    #[test]
    fn test_sysutil_trophy_init() {
        let mut manager = SysutilManager::new();
        
        assert!(!manager.trophy_is_initialized());
        
        assert_eq!(manager.trophy_init("NPWR12345"), 0);
        assert!(manager.trophy_is_initialized());
        
        // Double init should fail
        assert!(manager.trophy_init("NPWR12345") != 0);
        
        assert_eq!(manager.trophy_term(), 0);
        assert!(!manager.trophy_is_initialized());
    }

    #[test]
    fn test_sysutil_trophy_register_unlock() {
        let mut manager = SysutilManager::new();
        manager.trophy_init("NPWR12345");
        
        // Register trophies
        assert_eq!(manager.trophy_register(1, "Bronze Trophy", "Do something", TrophyGrade::Bronze, false), 0);
        assert_eq!(manager.trophy_register(2, "Silver Trophy", "Do more", TrophyGrade::Silver, false), 0);
        assert_eq!(manager.trophy_register(3, "Hidden Trophy", "Secret", TrophyGrade::Gold, true), 0);
        
        // Check progress
        let (unlocked, total) = manager.trophy_get_progress();
        assert_eq!(unlocked, 0);
        assert_eq!(total, 3);
        
        // Unlock trophy
        assert_eq!(manager.trophy_unlock(1), 0);
        assert!(manager.trophy_is_unlocked(1));
        
        // Double unlock should fail
        assert!(manager.trophy_unlock(1) != 0);
        
        // Check progress after unlock
        let (unlocked, total) = manager.trophy_get_progress();
        assert_eq!(unlocked, 1);
        assert_eq!(total, 3);
        
        manager.trophy_term();
    }

    #[test]
    fn test_sysutil_trophy_get_info() {
        let mut manager = SysutilManager::new();
        manager.trophy_init("NPWR12345");
        manager.trophy_register(1, "Test Trophy", "Description", TrophyGrade::Bronze, false);
        
        let info = manager.trophy_get_info(1);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.id, 1);
        assert_eq!(info.name, "Test Trophy");
        assert_eq!(info.grade, TrophyGrade::Bronze);
        
        // Invalid ID
        assert!(manager.trophy_get_info(999).is_none());
        
        manager.trophy_term();
    }

    // ========================================================================
    // Screen Saver Tests
    // ========================================================================

    #[test]
    fn test_sysutil_screen_saver() {
        let mut manager = SysutilManager::new();
        
        // Default state
        assert_eq!(manager.get_screen_saver_state(), ScreenSaverState::Enabled);
        
        // Disable
        assert_eq!(manager.disable_screen_saver(), 0);
        assert_eq!(manager.get_screen_saver_state(), ScreenSaverState::Disabled);
        
        // Enable
        assert_eq!(manager.enable_screen_saver(), 0);
        assert_eq!(manager.get_screen_saver_state(), ScreenSaverState::Enabled);
    }

    // ========================================================================
    // Video Settings Tests
    // ========================================================================

    #[test]
    fn test_sysutil_video_settings() {
        let mut manager = SysutilManager::new();
        
        // Default resolution
        assert_eq!(manager.get_resolution(), VideoResolution::Res1080p);
        
        // Change resolution
        assert_eq!(manager.set_resolution(VideoResolution::Res720p), 0);
        assert_eq!(manager.get_resolution(), VideoResolution::Res720p);
        
        // Aspect ratio
        assert_eq!(manager.get_aspect_ratio(), VideoAspect::Aspect16_9);
        assert_eq!(manager.set_aspect_ratio(VideoAspect::Aspect4_3), 0);
        assert_eq!(manager.get_aspect_ratio(), VideoAspect::Aspect4_3);
        
        // 3D
        assert!(!manager.is_3d_enabled());
        assert_eq!(manager.set_3d_enabled(true), 0);
        assert!(manager.is_3d_enabled());
    }

    // ========================================================================
    // Audio Settings Tests
    // ========================================================================

    #[test]
    fn test_sysutil_audio_settings() {
        let mut manager = SysutilManager::new();
        
        // Default output
        assert_eq!(manager.get_audio_output(), AudioOutput::Hdmi);
        
        // Change output
        assert_eq!(manager.set_audio_output(AudioOutput::Optical), 0);
        assert_eq!(manager.get_audio_output(), AudioOutput::Optical);
        
        // Audio format
        assert_eq!(manager.get_audio_format(), AudioFormat::Lpcm2);
        assert_eq!(manager.set_audio_format(AudioFormat::DolbyDigital), 0);
        assert_eq!(manager.get_audio_format(), AudioFormat::DolbyDigital);
        
        // Volume
        assert_eq!(manager.set_audio_volume(75), 0);
        assert_eq!(manager.get_audio_volume(), 75);
        
        // Volume clamping
        assert_eq!(manager.set_audio_volume(150), 0);
        assert_eq!(manager.get_audio_volume(), 100);
        
        // Mute
        assert!(!manager.is_audio_muted());
        assert_eq!(manager.set_audio_muted(true), 0);
        assert!(manager.is_audio_muted());
    }

    // ========================================================================
    // Background Music Tests
    // ========================================================================

    #[test]
    fn test_sysutil_bgm() {
        let mut manager = SysutilManager::new();
        
        // Default - BGM enabled
        assert!(manager.is_bgm_playback_enabled());
        assert_eq!(manager.get_bgm_volume(), 100);
        
        // Disable
        assert_eq!(manager.set_bgm_playback_enabled(false), 0);
        assert!(!manager.is_bgm_playback_enabled());
        
        // Volume
        assert_eq!(manager.set_bgm_volume(50), 0);
        assert_eq!(manager.get_bgm_volume(), 50);
        
        // Volume clamping
        assert_eq!(manager.set_bgm_volume(200), 0);
        assert_eq!(manager.get_bgm_volume(), 100);
    }

    // ========================================================================
    // Trophy Grade Enum Tests
    // ========================================================================

    #[test]
    fn test_trophy_grade_enum() {
        assert_eq!(TrophyGrade::Unknown as u32, 0);
        assert_eq!(TrophyGrade::Platinum as u32, 1);
        assert_eq!(TrophyGrade::Gold as u32, 2);
        assert_eq!(TrophyGrade::Silver as u32, 3);
        assert_eq!(TrophyGrade::Bronze as u32, 4);
    }

    #[test]
    fn test_video_resolution_enum() {
        assert_eq!(VideoResolution::Res480i as u32, 1);
        assert_eq!(VideoResolution::Res720p as u32, 5);
        assert_eq!(VideoResolution::Res1080p as u32, 7);
    }

    #[test]
    fn test_audio_output_enum() {
        assert_eq!(AudioOutput::Hdmi as u32, 0);
        assert_eq!(AudioOutput::Optical as u32, 1);
        assert_eq!(AudioOutput::AvMulti as u32, 2);
    }

    // ========================================================================
    // System Utilities Memory Write Tests
    // ========================================================================

    #[test]
    fn test_disc_info_api_no_disc() {
        crate::context::reset_hle_context();
        
        // When no disc is ready, should return error
        let result = cell_disc_game_get_boot_disc_info(0);
        assert_ne!(result, 0);
    }

    #[test]
    fn test_video_configure_api() {
        crate::context::reset_hle_context();
        
        // Test with null address (should not crash)
        assert_eq!(cell_video_out_configure(0, 0, 0, 0), 0);
        assert_eq!(cell_video_out_get_configuration(0, 0, 0), 0);
    }

    #[test]
    fn test_audio_configure_api() {
        crate::context::reset_hle_context();
        
        // Test with null address (should not crash)
        assert_eq!(cell_audio_out_configure(0, 0, 0, 0), 0);
        assert_eq!(cell_audio_out_get_configuration(0, 0, 0), 0);
    }

    #[test]
    fn test_trophy_info_api() {
        crate::context::reset_hle_context();
        
        // Initialize trophy system and register a trophy
        crate::context::get_hle_context_mut().sysutil.trophy_init("NPWR12345");
        crate::context::get_hle_context_mut().sysutil.trophy_register(
            1, "Test Trophy", "Test Description", TrophyGrade::Bronze, false
        );
        
        // Get trophy info with null addresses (should not crash)
        assert_eq!(cell_np_trophy_get_trophy_info(0, 0, 1, 0, 0), 0);
        
        // Get info for non-existent trophy
        assert_ne!(cell_np_trophy_get_trophy_info(0, 0, 999, 0, 0), 0);
        
        crate::context::get_hle_context_mut().sysutil.trophy_term();
    }

    #[test]
    fn test_trophy_progress_api() {
        crate::context::reset_hle_context();
        
        // Initialize trophy system
        crate::context::get_hle_context_mut().sysutil.trophy_init("NPWR12345");
        crate::context::get_hle_context_mut().sysutil.trophy_register(
            1, "Trophy 1", "Description", TrophyGrade::Bronze, false
        );
        crate::context::get_hle_context_mut().sysutil.trophy_register(
            2, "Trophy 2", "Description", TrophyGrade::Silver, false
        );
        
        // Get progress with null address (should not crash)
        assert_eq!(cell_np_trophy_get_game_progress(0, 0, 0), 0);
        
        crate::context::get_hle_context_mut().sysutil.trophy_term();
    }
}
