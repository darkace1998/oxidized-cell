//! cellKb HLE - Keyboard Input
//!
//! This module provides HLE implementations for PS3 keyboard input.
//! It supports multiple keyboard layouts and key mapping.

use tracing::{debug, trace};

/// Error codes
pub const CELL_KB_ERROR_NOT_INITIALIZED: i32 = 0x80121201u32 as i32;
pub const CELL_KB_ERROR_ALREADY_INITIALIZED: i32 = 0x80121202u32 as i32;
pub const CELL_KB_ERROR_NO_DEVICE: i32 = 0x80121203u32 as i32;
pub const CELL_KB_ERROR_INVALID_PARAMETER: i32 = 0x80121204u32 as i32;
pub const CELL_KB_ERROR_SYS_SETTING_FAILED: i32 = 0x80121205u32 as i32;

/// Maximum number of keyboards
pub const CELL_KB_MAX_KEYBOARDS: usize = 2;

/// Maximum number of keycodes per read
pub const CELL_KB_MAX_KEYCODES: usize = 8;

/// Keyboard layout types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellKbLayout {
    /// US layout
    #[default]
    Us = 0,
    /// UK layout
    Uk = 1,
    /// Japanese layout
    Japanese = 2,
    /// German layout
    German = 3,
    /// French layout
    French = 4,
    /// Spanish layout
    Spanish = 5,
    /// Italian layout
    Italian = 6,
    /// Portuguese layout
    Portuguese = 7,
    /// Russian layout
    Russian = 8,
    /// Chinese layout
    Chinese = 9,
}


/// Keyboard read mode
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CellKbReadMode {
    /// Input gets single-char strings
    #[default]
    InputCharacter = 0,
    /// Input gets raw keycodes
    RawKey = 1,
}


/// Keyboard code type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKbCodeType {
    /// Key was pressed
    Press = 0,
    /// Key was released
    Release = 1,
}

/// Keyboard modifier flags
pub const CELL_KB_MKEY_L_CTRL: u32 = 0x01;
pub const CELL_KB_MKEY_L_SHIFT: u32 = 0x02;
pub const CELL_KB_MKEY_L_ALT: u32 = 0x04;
pub const CELL_KB_MKEY_L_WIN: u32 = 0x08;
pub const CELL_KB_MKEY_R_CTRL: u32 = 0x10;
pub const CELL_KB_MKEY_R_SHIFT: u32 = 0x20;
pub const CELL_KB_MKEY_R_ALT: u32 = 0x40;
pub const CELL_KB_MKEY_R_WIN: u32 = 0x80;

/// Keyboard LED flags
pub const CELL_KB_LED_NUM_LOCK: u32 = 0x01;
pub const CELL_KB_LED_CAPS_LOCK: u32 = 0x02;
pub const CELL_KB_LED_SCROLL_LOCK: u32 = 0x04;
pub const CELL_KB_LED_COMPOSE: u32 = 0x08;
pub const CELL_KB_LED_KANA: u32 = 0x10;

/// Keyboard info structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellKbInfo {
    /// Maximum keyboards
    pub max: u32,
    /// Currently connected keyboards
    pub now_connect: u32,
    /// System info flags
    pub system_info: u32,
    /// Connection status per keyboard
    pub status: [u32; CELL_KB_MAX_KEYBOARDS],
}

impl Default for CellKbInfo {
    fn default() -> Self {
        Self {
            max: CELL_KB_MAX_KEYBOARDS as u32,
            now_connect: 0,
            system_info: 0,
            status: [0; CELL_KB_MAX_KEYBOARDS],
        }
    }
}

/// Keyboard data structure
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellKbData {
    /// LED flags
    pub led: u32,
    /// Modifier key flags
    pub mkey: u32,
    /// Number of keycodes
    pub len: i32,
    /// Keycodes
    pub keycodes: [u16; CELL_KB_MAX_KEYCODES],
}

impl Default for CellKbData {
    fn default() -> Self {
        Self {
            led: 0,
            mkey: 0,
            len: 0,
            keycodes: [0; CELL_KB_MAX_KEYCODES],
        }
    }
}

/// Keyboard configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellKbConfig {
    /// Read mode
    pub read_mode: u32,
    /// Code type
    pub code_type: u32,
    /// Arrange value
    pub arrange: u32,
}

impl Default for CellKbConfig {
    fn default() -> Self {
        Self {
            read_mode: CellKbReadMode::InputCharacter as u32,
            code_type: 0,
            arrange: 0,
        }
    }
}

/// Keyboard manager
pub struct KbManager {
    /// Initialization flag
    initialized: bool,
    /// Connected keyboard mask
    connected_keyboards: u8,
    /// Keyboard layouts
    layouts: [CellKbLayout; CELL_KB_MAX_KEYBOARDS],
    /// Keyboard configurations
    configs: [CellKbConfig; CELL_KB_MAX_KEYBOARDS],
    /// Cached keyboard data for each keyboard
    keyboard_data: [CellKbData; CELL_KB_MAX_KEYBOARDS],
    /// OC-Input backend placeholder
    input_backend: Option<()>,
}

impl KbManager {
    /// Create a new keyboard manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            connected_keyboards: 0,
            layouts: [CellKbLayout::default(); CELL_KB_MAX_KEYBOARDS],
            configs: [CellKbConfig::default(); CELL_KB_MAX_KEYBOARDS],
            keyboard_data: [CellKbData::default(); CELL_KB_MAX_KEYBOARDS],
            input_backend: None,
        }
    }

    /// Initialize keyboard system
    pub fn init(&mut self, max_connect: u32) -> i32 {
        if self.initialized {
            return CELL_KB_ERROR_ALREADY_INITIALIZED;
        }

        debug!("KbManager::init: max_connect={}", max_connect);

        self.initialized = true;
        
        // Simulate one keyboard connected
        self.connected_keyboards = 0x01;

        0 // CELL_OK
    }

    /// Shutdown keyboard system
    pub fn end(&mut self) -> i32 {
        if !self.initialized {
            return CELL_KB_ERROR_NOT_INITIALIZED;
        }

        debug!("KbManager::end");

        self.initialized = false;
        self.connected_keyboards = 0;

        0 // CELL_OK
    }

    /// Get keyboard info
    pub fn get_info(&self) -> Result<CellKbInfo, i32> {
        if !self.initialized {
            return Err(CELL_KB_ERROR_NOT_INITIALIZED);
        }

        let mut info = CellKbInfo::default();

        for kb in 0..CELL_KB_MAX_KEYBOARDS {
            if (self.connected_keyboards & (1 << kb)) != 0 {
                info.now_connect += 1;
                info.status[kb] = 1;
            }
        }

        Ok(info)
    }

    /// Read keyboard data
    pub fn read(&self, port: u32) -> Result<CellKbData, i32> {
        if !self.initialized {
            return Err(CELL_KB_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return Err(CELL_KB_ERROR_INVALID_PARAMETER);
        }

        if (self.connected_keyboards & (1 << port)) == 0 {
            return Err(CELL_KB_ERROR_NO_DEVICE);
        }

        trace!("KbManager::read: port={}", port);

        // Return cached keyboard data from backend
        Ok(self.keyboard_data[port as usize])
    }

    /// Set read mode
    pub fn set_read_mode(&mut self, port: u32, read_mode: CellKbReadMode) -> i32 {
        if !self.initialized {
            return CELL_KB_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return CELL_KB_ERROR_INVALID_PARAMETER;
        }

        trace!("KbManager::set_read_mode: port={}, mode={:?}", port, read_mode);

        self.configs[port as usize].read_mode = read_mode as u32;

        0 // CELL_OK
    }

    /// Set code type
    pub fn set_code_type(&mut self, port: u32, code_type: u32) -> i32 {
        if !self.initialized {
            return CELL_KB_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return CELL_KB_ERROR_INVALID_PARAMETER;
        }

        trace!("KbManager::set_code_type: port={}, code_type={}", port, code_type);

        self.configs[port as usize].code_type = code_type;

        0 // CELL_OK
    }

    /// Set keyboard layout
    pub fn set_layout(&mut self, port: u32, layout: CellKbLayout) -> i32 {
        if !self.initialized {
            return CELL_KB_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return CELL_KB_ERROR_INVALID_PARAMETER;
        }

        debug!("KbManager::set_layout: port={}, layout={:?}", port, layout);

        self.layouts[port as usize] = layout;

        0 // CELL_OK
    }

    /// Get configuration
    pub fn get_config(&self, port: u32) -> Result<CellKbConfig, i32> {
        if !self.initialized {
            return Err(CELL_KB_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return Err(CELL_KB_ERROR_INVALID_PARAMETER);
        }

        Ok(self.configs[port as usize])
    }

    /// Clear input buffer
    pub fn clear_buf(&mut self, port: u32) -> i32 {
        if !self.initialized {
            return CELL_KB_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return CELL_KB_ERROR_INVALID_PARAMETER;
        }

        trace!("KbManager::clear_buf: port={}", port);

        // TODO: Clear actual input buffer

        0 // CELL_OK
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    // ========================================================================
    // OC-Input Backend Integration
    // ========================================================================

    /// Connect to oc-input backend
    /// 
    /// Integrates with oc-input for actual keyboard input.
    pub fn connect_input_backend(&mut self, _backend: Option<()>) -> i32 {
        debug!("KbManager::connect_input_backend");
        
        // In a real implementation:
        // 1. Store the oc-input backend reference
        // 2. Register keyboard input callbacks
        // 3. Query connected keyboards
        // 4. Set up key mappings
        
        self.input_backend = None; // Would store actual backend
        
        0 // CELL_OK
    }

    /// Poll input from backend
    /// 
    /// Reads current keyboard state from oc-input and updates keyboard data.
    pub fn poll_input(&mut self) -> i32 {
        if !self.initialized {
            return CELL_KB_ERROR_NOT_INITIALIZED;
        }

        trace!("KbManager::poll_input");

        // In a real implementation, this would:
        // 1. Query oc-input for current keyboard states
        // 2. Convert oc-input key events to PS3 format
        // 3. Update keyboard_data for each connected keyboard
        // 4. Handle modifier keys (Ctrl, Shift, Alt, etc.)
        // 5. Apply keyboard layout conversion

        0 // CELL_OK
    }

    /// Update keyboard data from input backend
    /// 
    /// # Arguments
    /// * `port` - Keyboard port
    /// * `keycodes` - Array of pressed keycodes
    /// * `modifiers` - Modifier key flags
    pub fn update_keyboard_data(&mut self, port: u32, keycodes: &[u16], modifiers: u32) -> i32 {
        if port >= CELL_KB_MAX_KEYBOARDS as u32 {
            return CELL_KB_ERROR_INVALID_PARAMETER;
        }

        if (self.connected_keyboards & (1 << port)) == 0 {
            return CELL_KB_ERROR_NO_DEVICE;
        }

        let port_idx = port as usize;
        let len = keycodes.len().min(CELL_KB_MAX_KEYCODES);
        
        self.keyboard_data[port_idx].len = len as i32;
        self.keyboard_data[port_idx].mkey = modifiers;
        
        // Copy keycodes
        for (i, &keycode) in keycodes.iter().take(len).enumerate() {
            self.keyboard_data[port_idx].keycodes[i] = keycode;
        }
        
        // Clear remaining slots
        for i in len..CELL_KB_MAX_KEYCODES {
            self.keyboard_data[port_idx].keycodes[i] = 0;
        }

        trace!(
            "Updated keyboard data for port {}: {} keys, modifiers=0x{:08X}",
            port, len, modifiers
        );

        0 // CELL_OK
    }

    /// Map oc-input key to PS3 keycode
    /// 
    /// Converts key codes between oc-input and PS3 formats.
    pub fn map_keycode(oc_input_key: u16) -> u16 {
        // In a real implementation, this would map:
        // oc-input key codes -> PS3 USB HID key codes
        // 
        // The mapping is mostly 1:1 for USB HID codes
        // but may need translation for special keys

        trace!("Mapping keycode: 0x{:04X}", oc_input_key);

        // Return as-is for now (assuming USB HID codes)
        oc_input_key
    }

    /// Map oc-input modifiers to PS3 modifiers
    /// 
    /// Converts modifier flags between oc-input and PS3 formats.
    pub fn map_modifiers(oc_input_modifiers: u32) -> u32 {
        // In a real implementation, this would map:
        // oc-input modifier flags -> PS3 modifier flags
        //
        // For example:
        // oc_input::KeyModifiers::LEFT_CTRL -> CELL_KB_MKEY_L_CTRL
        // oc_input::KeyModifiers::LEFT_SHIFT -> CELL_KB_MKEY_L_SHIFT
        // etc.

        trace!("Mapping modifiers: 0x{:08X}", oc_input_modifiers);

        // Return as-is for now (assuming compatible format)
        oc_input_modifiers
    }

    /// Check if backend is connected
    pub fn is_backend_connected(&self) -> bool {
        self.input_backend.is_some()
    }
}

impl Default for KbManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellKbInit - Initialize keyboard system
///
/// # Arguments
/// * `max_connect` - Maximum keyboards to support
///
/// # Returns
/// * 0 on success
pub fn cell_kb_init(max_connect: u32) -> i32 {
    debug!("cellKbInit(max_connect={})", max_connect);

    crate::context::get_hle_context_mut().kb.init(max_connect)
}

/// cellKbEnd - Shutdown keyboard system
///
/// # Returns
/// * 0 on success
pub fn cell_kb_end() -> i32 {
    debug!("cellKbEnd()");

    crate::context::get_hle_context_mut().kb.end()
}

/// cellKbGetInfo - Get keyboard info
///
/// # Arguments
/// * `info_addr` - Address to write info
///
/// # Returns
/// * 0 on success
pub fn cell_kb_get_info(_info_addr: u32) -> i32 {
    trace!("cellKbGetInfo()");

    match crate::context::get_hle_context().kb.get_info() {
        Ok(_info) => {
            // TODO: Write info to memory at _info_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellKbRead - Read keyboard data
///
/// # Arguments
/// * `port` - Keyboard port number
/// * `data_addr` - Address to write data
///
/// # Returns
/// * 0 on success
pub fn cell_kb_read(port: u32, _data_addr: u32) -> i32 {
    trace!("cellKbRead(port={})", port);

    match crate::context::get_hle_context().kb.read(port) {
        Ok(_data) => {
            // TODO: Write data to memory at _data_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellKbSetReadMode - Set keyboard read mode
///
/// # Arguments
/// * `port` - Keyboard port number
/// * `read_mode` - Read mode (0 = character, 1 = raw key)
///
/// # Returns
/// * 0 on success
pub fn cell_kb_set_read_mode(port: u32, read_mode: u32) -> i32 {
    trace!("cellKbSetReadMode(port={}, mode={})", port, read_mode);

    let mode = if read_mode == CellKbReadMode::RawKey as u32 {
        CellKbReadMode::RawKey
    } else {
        CellKbReadMode::InputCharacter
    };

    crate::context::get_hle_context_mut().kb.set_read_mode(port, mode)
}

/// cellKbSetCodeType - Set keyboard code type
///
/// # Arguments
/// * `port` - Keyboard port number
/// * `code_type` - Code type
///
/// # Returns
/// * 0 on success
pub fn cell_kb_set_code_type(port: u32, code_type: u32) -> i32 {
    trace!("cellKbSetCodeType(port={}, code_type={})", port, code_type);

    crate::context::get_hle_context_mut().kb.set_code_type(port, code_type)
}

/// cellKbSetLEDStatus - Set keyboard LED status
///
/// # Arguments
/// * `port` - Keyboard port number
/// * `led` - LED flags
///
/// # Returns
/// * 0 on success
pub fn cell_kb_set_led_status(port: u32, led: u32) -> i32 {
    trace!("cellKbSetLEDStatus(port={}, led=0x{:X})", port, led);

    // Check if initialized
    if !crate::context::get_hle_context().kb.is_initialized() {
        return CELL_KB_ERROR_NOT_INITIALIZED;
    }

    if port >= CELL_KB_MAX_KEYBOARDS as u32 {
        return CELL_KB_ERROR_INVALID_PARAMETER;
    }

    // TODO: Set actual LED status

    0 // CELL_OK
}

/// cellKbClearBuf - Clear keyboard input buffer
///
/// # Arguments
/// * `port` - Keyboard port number
///
/// # Returns
/// * 0 on success
pub fn cell_kb_clear_buf(port: u32) -> i32 {
    trace!("cellKbClearBuf(port={})", port);

    crate::context::get_hle_context_mut().kb.clear_buf(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kb_manager_lifecycle() {
        let mut manager = KbManager::new();
        
        assert_eq!(manager.init(2), 0);
        assert!(manager.is_initialized());
        
        // Double init should fail
        assert_eq!(manager.init(2), CELL_KB_ERROR_ALREADY_INITIALIZED);
        
        assert_eq!(manager.end(), 0);
        assert!(!manager.is_initialized());
        
        // Double end should fail
        assert_eq!(manager.end(), CELL_KB_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_kb_manager_info() {
        let mut manager = KbManager::new();
        manager.init(2);
        
        let info = manager.get_info().unwrap();
        assert_eq!(info.max, CELL_KB_MAX_KEYBOARDS as u32);
        assert_eq!(info.now_connect, 1); // Simulated keyboard
        
        manager.end();
    }

    #[test]
    fn test_kb_manager_read() {
        let mut manager = KbManager::new();
        manager.init(2);
        
        // Read from connected port
        let data = manager.read(0);
        assert!(data.is_ok());
        
        // Read from disconnected port
        let data = manager.read(1);
        assert_eq!(data, Err(CELL_KB_ERROR_NO_DEVICE));
        
        manager.end();
    }

    #[test]
    fn test_kb_manager_config() {
        let mut manager = KbManager::new();
        manager.init(2);
        
        // Set read mode
        assert_eq!(manager.set_read_mode(0, CellKbReadMode::RawKey), 0);
        
        // Set layout
        assert_eq!(manager.set_layout(0, CellKbLayout::Japanese), 0);
        
        // Get config
        let config = manager.get_config(0).unwrap();
        assert_eq!(config.read_mode, CellKbReadMode::RawKey as u32);
        
        manager.end();
    }

    #[test]
    fn test_kb_manager_validation() {
        let mut manager = KbManager::new();
        manager.init(2);
        
        // Invalid port
        assert_eq!(manager.set_read_mode(99, CellKbReadMode::RawKey), CELL_KB_ERROR_INVALID_PARAMETER);
        assert!(manager.read(99).is_err());
        
        manager.end();
    }

    #[test]
    fn test_kb_layout_values() {
        assert_eq!(CellKbLayout::Us as u32, 0);
        assert_eq!(CellKbLayout::Japanese as u32, 2);
    }

    #[test]
    fn test_kb_modifier_flags() {
        assert_eq!(CELL_KB_MKEY_L_CTRL, 0x01);
        assert_eq!(CELL_KB_MKEY_L_SHIFT, 0x02);
        assert_eq!(CELL_KB_MKEY_R_CTRL, 0x10);
    }

    #[test]
    fn test_kb_led_flags() {
        assert_eq!(CELL_KB_LED_NUM_LOCK, 0x01);
        assert_eq!(CELL_KB_LED_CAPS_LOCK, 0x02);
        assert_eq!(CELL_KB_LED_SCROLL_LOCK, 0x04);
    }
}
