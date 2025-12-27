//! cellMouse HLE - Mouse Input
//!
//! This module provides HLE implementations for PS3 mouse input.
//! It supports mouse position tracking and button state handling.

use tracing::{debug, trace};

/// Error codes
pub const CELL_MOUSE_ERROR_NOT_INITIALIZED: i32 = 0x80121301u32 as i32;
pub const CELL_MOUSE_ERROR_ALREADY_INITIALIZED: i32 = 0x80121302u32 as i32;
pub const CELL_MOUSE_ERROR_NO_DEVICE: i32 = 0x80121303u32 as i32;
pub const CELL_MOUSE_ERROR_INVALID_PARAMETER: i32 = 0x80121304u32 as i32;
pub const CELL_MOUSE_ERROR_SYS_SETTING_FAILED: i32 = 0x80121305u32 as i32;

/// Maximum number of mice
/// Note: PS3 hardware supports a maximum of 2 USB mice simultaneously
pub const CELL_MOUSE_MAX_MICE: usize = 2;

/// Maximum number of mouse data entries
pub const CELL_MOUSE_MAX_DATA: usize = 64;

/// Mouse button flags
pub const CELL_MOUSE_BUTTON_LEFT: u32 = 0x01;
pub const CELL_MOUSE_BUTTON_RIGHT: u32 = 0x02;
pub const CELL_MOUSE_BUTTON_MIDDLE: u32 = 0x04;
pub const CELL_MOUSE_BUTTON_4: u32 = 0x08;
pub const CELL_MOUSE_BUTTON_5: u32 = 0x10;

/// Mouse info structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellMouseInfo {
    /// Maximum mice
    pub max: u32,
    /// Currently connected mice
    pub now_connect: u32,
    /// System info flags
    pub system_info: u32,
    /// Tablet mode flags
    pub tablet_mode: [u32; CELL_MOUSE_MAX_MICE],
    /// Vendor ID
    pub vendor_id: [u16; CELL_MOUSE_MAX_MICE],
    /// Product ID
    pub product_id: [u16; CELL_MOUSE_MAX_MICE],
    /// Connection status
    pub status: [u8; CELL_MOUSE_MAX_MICE],
}

impl Default for CellMouseInfo {
    fn default() -> Self {
        Self {
            max: CELL_MOUSE_MAX_MICE as u32,
            now_connect: 0,
            system_info: 0,
            tablet_mode: [0; CELL_MOUSE_MAX_MICE],
            vendor_id: [0; CELL_MOUSE_MAX_MICE],
            product_id: [0; CELL_MOUSE_MAX_MICE],
            status: [0; CELL_MOUSE_MAX_MICE],
        }
    }
}

/// Mouse raw data entry
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellMouseRawData {
    /// Buttons currently pressed
    pub buttons: u8,
    /// X axis delta
    pub x_axis: i8,
    /// Y axis delta
    pub y_axis: i8,
    /// Wheel delta
    pub wheel: i8,
    /// Tilt X (for tablets)
    pub tilt_x: i8,
    /// Tilt Y (for tablets)
    pub tilt_y: i8,
}

/// Mouse data structure
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub struct CellMouseData {
    /// Update timestamp
    pub update: u64,
    /// Buttons currently pressed
    pub buttons: u32,
    /// X position (absolute or delta based on mode)
    pub x_pos: i32,
    /// Y position (absolute or delta based on mode)
    pub y_pos: i32,
    /// Wheel delta
    pub wheel: i32,
    /// Tilt X (for tablets)
    pub tilt_x: i32,
    /// Tilt Y (for tablets)
    pub tilt_y: i32,
}


/// Mouse data buffer
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellMouseDataList {
    /// Number of data entries
    pub list_num: u32,
    /// Data entries
    pub list: [CellMouseData; CELL_MOUSE_MAX_DATA],
}

impl Default for CellMouseDataList {
    fn default() -> Self {
        Self {
            list_num: 0,
            list: [CellMouseData::default(); CELL_MOUSE_MAX_DATA],
        }
    }
}

/// Mouse manager
pub struct MouseManager {
    /// Initialization flag
    initialized: bool,
    /// Connected mouse mask
    connected_mice: u8,
    /// Current mouse positions
    positions: [(i32, i32); CELL_MOUSE_MAX_MICE],
    /// Current button states
    button_states: [u32; CELL_MOUSE_MAX_MICE],
    /// Cached mouse data
    mouse_data: [CellMouseData; CELL_MOUSE_MAX_MICE],
    /// OC-Input backend placeholder
    input_backend: Option<()>,
}

impl MouseManager {
    /// Create a new mouse manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            connected_mice: 0,
            positions: [(0, 0); CELL_MOUSE_MAX_MICE],
            button_states: [0; CELL_MOUSE_MAX_MICE],
            mouse_data: [CellMouseData::default(); CELL_MOUSE_MAX_MICE],
            input_backend: None,
        }
    }

    /// Initialize mouse system
    pub fn init(&mut self, max_connect: u32) -> i32 {
        if self.initialized {
            return CELL_MOUSE_ERROR_ALREADY_INITIALIZED;
        }

        debug!("MouseManager::init: max_connect={}", max_connect);

        self.initialized = true;
        
        // Simulate one mouse connected
        self.connected_mice = 0x01;

        0 // CELL_OK
    }

    /// Shutdown mouse system
    pub fn end(&mut self) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        debug!("MouseManager::end");

        self.initialized = false;
        self.connected_mice = 0;

        0 // CELL_OK
    }

    /// Get mouse info
    pub fn get_info(&self) -> Result<CellMouseInfo, i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        let mut info = CellMouseInfo::default();

        for mouse in 0..CELL_MOUSE_MAX_MICE {
            if (self.connected_mice & (1 << mouse)) != 0 {
                info.now_connect += 1;
                info.status[mouse] = 1;
            }
        }

        Ok(info)
    }

    /// Get mouse data
    pub fn get_data(&self, port: u32) -> Result<CellMouseData, i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return Err(CELL_MOUSE_ERROR_INVALID_PARAMETER);
        }

        if (self.connected_mice & (1 << port)) == 0 {
            return Err(CELL_MOUSE_ERROR_NO_DEVICE);
        }

        trace!("MouseManager::get_data: port={}", port);

        // TODO: Get actual mouse data from oc-input subsystem
        let mut data = CellMouseData::default();
        data.x_pos = self.positions[port as usize].0;
        data.y_pos = self.positions[port as usize].1;
        data.buttons = self.button_states[port as usize];

        Ok(data)
    }

    /// Get mouse data list (buffered)
    pub fn get_data_list(&self, port: u32) -> Result<CellMouseDataList, i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return Err(CELL_MOUSE_ERROR_INVALID_PARAMETER);
        }

        if (self.connected_mice & (1 << port)) == 0 {
            return Err(CELL_MOUSE_ERROR_NO_DEVICE);
        }

        trace!("MouseManager::get_data_list: port={}", port);

        // TODO: Get buffered mouse data from oc-input subsystem
        // For now, return a single entry with current state
        let mut list = CellMouseDataList::default();
        
        if let Ok(data) = self.get_data(port) {
            list.list[0] = data;
            list.list_num = 1;
        }

        Ok(list)
    }

    /// Get raw data
    pub fn get_raw_data(&self, port: u32) -> Result<CellMouseRawData, i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return Err(CELL_MOUSE_ERROR_INVALID_PARAMETER);
        }

        if (self.connected_mice & (1 << port)) == 0 {
            return Err(CELL_MOUSE_ERROR_NO_DEVICE);
        }

        trace!("MouseManager::get_raw_data: port={}", port);

        // TODO: Get raw mouse data from oc-input subsystem
        let mut raw = CellMouseRawData::default();
        raw.buttons = self.button_states[port as usize] as u8;

        Ok(raw)
    }

    /// Set position (for testing/simulation)
    pub fn set_position(&mut self, port: u32, x: i32, y: i32) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return CELL_MOUSE_ERROR_INVALID_PARAMETER;
        }

        self.positions[port as usize] = (x, y);

        0 // CELL_OK
    }

    /// Set button state (for testing/simulation)
    pub fn set_buttons(&mut self, port: u32, buttons: u32) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return CELL_MOUSE_ERROR_INVALID_PARAMETER;
        }

        self.button_states[port as usize] = buttons;

        0 // CELL_OK
    }

    /// Clear buffer
    pub fn clear_buf(&mut self, port: u32) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return CELL_MOUSE_ERROR_INVALID_PARAMETER;
        }

        trace!("MouseManager::clear_buf: port={}", port);

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
    /// Integrates with oc-input for actual mouse input.
    pub fn connect_input_backend(&mut self, _backend: Option<()>) -> i32 {
        debug!("MouseManager::connect_input_backend");
        
        // In a real implementation:
        // 1. Store the oc-input backend reference
        // 2. Register mouse input callbacks
        // 3. Query connected mice
        // 4. Set up button/axis mappings
        
        self.input_backend = None; // Would store actual backend
        
        0 // CELL_OK
    }

    /// Poll input from backend
    /// 
    /// Reads current mouse state from oc-input and updates mouse data.
    pub fn poll_input(&mut self) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        trace!("MouseManager::poll_input");

        // In a real implementation, this would:
        // 1. Query oc-input for current mouse states
        // 2. Convert oc-input mouse events to PS3 format
        // 3. Update mouse_data for each connected mouse
        // 4. Handle button presses
        // 5. Handle position/delta updates
        // 6. Handle wheel scrolling

        0 // CELL_OK
    }

    /// Update mouse data from input backend
    /// 
    /// # Arguments
    /// * `port` - Mouse port
    /// * `x` - X position or delta
    /// * `y` - Y position or delta
    /// * `buttons` - Button state flags
    /// * `wheel` - Wheel delta
    pub fn update_mouse_data(&mut self, port: u32, x: i32, y: i32, buttons: u32, wheel: i32) -> i32 {
        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return CELL_MOUSE_ERROR_INVALID_PARAMETER;
        }

        if (self.connected_mice & (1 << port)) == 0 {
            return CELL_MOUSE_ERROR_NO_DEVICE;
        }

        let port_idx = port as usize;
        
        // Update cached state
        self.positions[port_idx] = (x, y);
        self.button_states[port_idx] = buttons;
        
        // Update mouse data structure
        self.mouse_data[port_idx].x_pos = x;
        self.mouse_data[port_idx].y_pos = y;
        self.mouse_data[port_idx].buttons = buttons;
        self.mouse_data[port_idx].wheel = wheel;
        self.mouse_data[port_idx].update += 1;

        trace!(
            "Updated mouse data for port {}: pos=({}, {}), buttons=0x{:08X}, wheel={}",
            port, x, y, buttons, wheel
        );

        0 // CELL_OK
    }

    /// Map oc-input button to PS3 mouse button
    /// 
    /// Converts button codes between oc-input and PS3 formats.
    pub fn map_button(oc_input_button: u32) -> u32 {
        // In a real implementation, this would map:
        // oc-input button codes -> PS3 button codes
        // 
        // For example:
        // oc_input::MouseButton::LEFT -> CELL_MOUSE_BUTTON_LEFT
        // oc_input::MouseButton::RIGHT -> CELL_MOUSE_BUTTON_RIGHT
        // etc.

        trace!("Mapping mouse button: 0x{:08X}", oc_input_button);

        // Return as-is for now (assuming compatible format)
        oc_input_button
    }

    /// Check if backend is connected
    pub fn is_backend_connected(&self) -> bool {
        self.input_backend.is_some()
    }
}

impl Default for MouseManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellMouseInit - Initialize mouse system
///
/// # Arguments
/// * `max_connect` - Maximum mice to support
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_init(max_connect: u32) -> i32 {
    debug!("cellMouseInit(max_connect={})", max_connect);

    crate::context::get_hle_context_mut().mouse.init(max_connect)
}

/// cellMouseEnd - Shutdown mouse system
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_end() -> i32 {
    debug!("cellMouseEnd()");

    crate::context::get_hle_context_mut().mouse.end()
}

/// cellMouseGetInfo - Get mouse info
///
/// # Arguments
/// * `info_addr` - Address to write info
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_get_info(_info_addr: u32) -> i32 {
    trace!("cellMouseGetInfo()");

    match crate::context::get_hle_context().mouse.get_info() {
        Ok(_info) => {
            // TODO: Write info to memory at _info_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellMouseGetData - Get mouse data
///
/// # Arguments
/// * `port` - Mouse port number
/// * `data_addr` - Address to write data
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_get_data(port: u32, _data_addr: u32) -> i32 {
    trace!("cellMouseGetData(port={})", port);

    match crate::context::get_hle_context().mouse.get_data(port) {
        Ok(_data) => {
            // TODO: Write data to memory at _data_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellMouseGetDataList - Get buffered mouse data
///
/// # Arguments
/// * `port` - Mouse port number
/// * `data_addr` - Address to write data list
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_get_data_list(port: u32, _data_addr: u32) -> i32 {
    trace!("cellMouseGetDataList(port={})", port);

    match crate::context::get_hle_context().mouse.get_data_list(port) {
        Ok(_list) => {
            // TODO: Write data list to memory at _data_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellMouseGetRawData - Get raw mouse data
///
/// # Arguments
/// * `port` - Mouse port number
/// * `data_addr` - Address to write raw data
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_get_raw_data(port: u32, _data_addr: u32) -> i32 {
    trace!("cellMouseGetRawData(port={})", port);

    match crate::context::get_hle_context().mouse.get_raw_data(port) {
        Ok(_data) => {
            // TODO: Write raw data to memory at _data_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellMouseClearBuf - Clear mouse input buffer
///
/// # Arguments
/// * `port` - Mouse port number
///
/// # Returns
/// * 0 on success
pub fn cell_mouse_clear_buf(port: u32) -> i32 {
    trace!("cellMouseClearBuf(port={})", port);

    crate::context::get_hle_context_mut().mouse.clear_buf(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_manager_lifecycle() {
        let mut manager = MouseManager::new();
        
        assert_eq!(manager.init(2), 0);
        assert!(manager.is_initialized());
        
        // Double init should fail
        assert_eq!(manager.init(2), CELL_MOUSE_ERROR_ALREADY_INITIALIZED);
        
        assert_eq!(manager.end(), 0);
        assert!(!manager.is_initialized());
        
        // Double end should fail
        assert_eq!(manager.end(), CELL_MOUSE_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_mouse_manager_info() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        let info = manager.get_info().unwrap();
        assert_eq!(info.max, CELL_MOUSE_MAX_MICE as u32);
        assert_eq!(info.now_connect, 1); // Simulated mouse
        
        manager.end();
    }

    #[test]
    fn test_mouse_manager_data() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        // Set position
        manager.set_position(0, 100, 200);
        manager.set_buttons(0, CELL_MOUSE_BUTTON_LEFT);
        
        // Get data
        let data = manager.get_data(0).unwrap();
        assert_eq!(data.x_pos, 100);
        assert_eq!(data.y_pos, 200);
        assert_eq!(data.buttons, CELL_MOUSE_BUTTON_LEFT);
        
        // Disconnected port
        let data = manager.get_data(1);
        assert_eq!(data, Err(CELL_MOUSE_ERROR_NO_DEVICE));
        
        manager.end();
    }

    #[test]
    fn test_mouse_manager_data_list() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        manager.set_position(0, 50, 75);
        
        let list = manager.get_data_list(0).unwrap();
        assert_eq!(list.list_num, 1);
        assert_eq!(list.list[0].x_pos, 50);
        assert_eq!(list.list[0].y_pos, 75);
        
        manager.end();
    }

    #[test]
    fn test_mouse_manager_raw_data() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        manager.set_buttons(0, CELL_MOUSE_BUTTON_RIGHT);
        
        let raw = manager.get_raw_data(0).unwrap();
        assert_eq!(raw.buttons, CELL_MOUSE_BUTTON_RIGHT as u8);
        
        manager.end();
    }

    #[test]
    fn test_mouse_manager_validation() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        // Invalid port
        assert!(manager.get_data(99).is_err());
        assert_eq!(manager.set_position(99, 0, 0), CELL_MOUSE_ERROR_INVALID_PARAMETER);
        
        manager.end();
    }

    #[test]
    fn test_mouse_button_flags() {
        assert_eq!(CELL_MOUSE_BUTTON_LEFT, 0x01);
        assert_eq!(CELL_MOUSE_BUTTON_RIGHT, 0x02);
        assert_eq!(CELL_MOUSE_BUTTON_MIDDLE, 0x04);
    }

    #[test]
    fn test_mouse_info_default() {
        let info = CellMouseInfo::default();
        assert_eq!(info.max, CELL_MOUSE_MAX_MICE as u32);
        assert_eq!(info.now_connect, 0);
    }

    #[test]
    fn test_mouse_data_default() {
        let data = CellMouseData::default();
        assert_eq!(data.buttons, 0);
        assert_eq!(data.x_pos, 0);
        assert_eq!(data.y_pos, 0);
    }
}
