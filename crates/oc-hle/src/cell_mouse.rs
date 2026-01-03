//! cellMouse HLE - Mouse Input
//!
//! This module provides HLE implementations for PS3 mouse input.
//! It supports mouse position tracking and button state handling with full oc-input integration.

use std::sync::{Arc, RwLock};
use tracing::{debug, trace};
use oc_input::mouse::{Mouse, MouseState, MouseButtons};
use crate::memory::{write_be32, write_be64, write_u8, write_be16, ToGuestMemory};

/// OC-Input mouse backend reference
pub type MouseBackend = Option<Arc<RwLock<Vec<Mouse>>>>;

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

impl ToGuestMemory for CellMouseInfo {
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32> {
        let mut offset = 0u32;
        write_be32(addr + offset, self.max)?; offset += 4;
        write_be32(addr + offset, self.now_connect)?; offset += 4;
        write_be32(addr + offset, self.system_info)?; offset += 4;
        for i in 0..CELL_MOUSE_MAX_MICE {
            write_be32(addr + offset, self.tablet_mode[i])?; offset += 4;
        }
        for i in 0..CELL_MOUSE_MAX_MICE {
            write_be16(addr + offset, self.vendor_id[i])?; offset += 2;
        }
        for i in 0..CELL_MOUSE_MAX_MICE {
            write_be16(addr + offset, self.product_id[i])?; offset += 2;
        }
        for i in 0..CELL_MOUSE_MAX_MICE {
            write_u8(addr + offset, self.status[i])?; offset += 1;
        }
        Ok(())
    }
}

impl ToGuestMemory for CellMouseData {
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32> {
        write_be64(addr, self.update)?;
        write_be32(addr + 8, self.buttons)?;
        // Note: Casting i32 to u32 preserves the bit pattern (two's complement)
        // which is correct for PS3 big-endian memory representation
        write_be32(addr + 12, self.x_pos as u32)?;
        write_be32(addr + 16, self.y_pos as u32)?;
        write_be32(addr + 20, self.wheel as u32)?;
        write_be32(addr + 24, self.tilt_x as u32)?;
        write_be32(addr + 28, self.tilt_y as u32)?;
        Ok(())
    }
}

impl ToGuestMemory for CellMouseRawData {
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32> {
        write_u8(addr, self.buttons)?;
        // Note: Casting i8 to u8 preserves the bit pattern (two's complement)
        // which is correct for PS3 memory representation of signed deltas
        write_u8(addr + 1, self.x_axis as u8)?;
        write_u8(addr + 2, self.y_axis as u8)?;
        write_u8(addr + 3, self.wheel as u8)?;
        write_u8(addr + 4, self.tilt_x as u8)?;
        write_u8(addr + 5, self.tilt_y as u8)?;
        Ok(())
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
    /// Previous mouse positions (for delta calculation)
    prev_positions: [(i32, i32); CELL_MOUSE_MAX_MICE],
    /// Current button states
    button_states: [u32; CELL_MOUSE_MAX_MICE],
    /// Cached mouse data
    mouse_data: [CellMouseData; CELL_MOUSE_MAX_MICE],
    /// Movement delta since last read
    movement_delta: [(i32, i32); CELL_MOUSE_MAX_MICE],
    /// Wheel delta since last read
    wheel_delta: [i32; CELL_MOUSE_MAX_MICE],
    /// OC-Input mouse backend
    input_backend: MouseBackend,
}

impl MouseManager {
    /// Create a new mouse manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            connected_mice: 0,
            positions: [(0, 0); CELL_MOUSE_MAX_MICE],
            prev_positions: [(0, 0); CELL_MOUSE_MAX_MICE],
            button_states: [0; CELL_MOUSE_MAX_MICE],
            mouse_data: [CellMouseData::default(); CELL_MOUSE_MAX_MICE],
            movement_delta: [(0, 0); CELL_MOUSE_MAX_MICE],
            wheel_delta: [0; CELL_MOUSE_MAX_MICE],
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
        self.positions = [(0, 0); CELL_MOUSE_MAX_MICE];
        self.prev_positions = [(0, 0); CELL_MOUSE_MAX_MICE];
        self.movement_delta = [(0, 0); CELL_MOUSE_MAX_MICE];
        self.wheel_delta = [0; CELL_MOUSE_MAX_MICE];

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

        // Return the cached mouse data which is updated by poll_input()
        // when the oc-input backend is connected, or manually via update methods
        let port_idx = port as usize;
        
        // Build current mouse data from cached state
        let data = CellMouseData {
            update: self.mouse_data[port_idx].update,
            x_pos: self.positions[port_idx].0,
            y_pos: self.positions[port_idx].1,
            buttons: self.button_states[port_idx],
            wheel: self.mouse_data[port_idx].wheel,
            tilt_x: self.mouse_data[port_idx].tilt_x,
            tilt_y: self.mouse_data[port_idx].tilt_y,
        };

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

        // Return buffered mouse data from oc-input subsystem
        // The mouse_data array is updated by poll_input() when the backend is connected
        let port_idx = port as usize;
        let mut list = CellMouseDataList::default();
        
        // Get current state as a single entry in the list
        // Note: For full buffering, the oc-input backend would maintain a ring buffer
        // of mouse events. Currently we return the most recent state.
        let data = CellMouseData {
            update: self.mouse_data[port_idx].update,
            x_pos: self.positions[port_idx].0,
            y_pos: self.positions[port_idx].1,
            buttons: self.button_states[port_idx],
            wheel: self.mouse_data[port_idx].wheel,
            tilt_x: self.mouse_data[port_idx].tilt_x,
            tilt_y: self.mouse_data[port_idx].tilt_y,
        };
        
        list.list[0] = data;
        list.list_num = 1;

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

        let port_idx = port as usize;
        let mut raw = CellMouseRawData::default();
        raw.buttons = self.button_states[port_idx] as u8;
        
        // Clamp delta to i8 range for raw data
        raw.x_axis = self.movement_delta[port_idx].0.clamp(-128, 127) as i8;
        raw.y_axis = self.movement_delta[port_idx].1.clamp(-128, 127) as i8;
        raw.wheel = self.wheel_delta[port_idx].clamp(-128, 127) as i8;

        Ok(raw)
    }

    /// Get raw data and clear deltas
    /// 
    /// Returns raw mouse data and resets the accumulated deltas.
    pub fn get_raw_data_and_clear(&mut self, port: u32) -> Result<CellMouseRawData, i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return Err(CELL_MOUSE_ERROR_INVALID_PARAMETER);
        }

        if (self.connected_mice & (1 << port)) == 0 {
            return Err(CELL_MOUSE_ERROR_NO_DEVICE);
        }

        trace!("MouseManager::get_raw_data_and_clear: port={}", port);

        let port_idx = port as usize;
        let mut raw = CellMouseRawData::default();
        raw.buttons = self.button_states[port_idx] as u8;
        
        // Clamp delta to i8 range for raw data
        raw.x_axis = self.movement_delta[port_idx].0.clamp(-128, 127) as i8;
        raw.y_axis = self.movement_delta[port_idx].1.clamp(-128, 127) as i8;
        raw.wheel = self.wheel_delta[port_idx].clamp(-128, 127) as i8;
        
        // Clear deltas after reading
        self.movement_delta[port_idx] = (0, 0);
        self.wheel_delta[port_idx] = 0;

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

        let port_idx = port as usize;
        
        // Calculate delta from previous position before updating
        let dx = x - self.positions[port_idx].0;
        let dy = y - self.positions[port_idx].1;
        
        // Store previous position
        self.prev_positions[port_idx] = self.positions[port_idx];
        
        // Update current position
        self.positions[port_idx] = (x, y);
        
        // Accumulate movement delta
        self.movement_delta[port_idx].0 = self.movement_delta[port_idx].0.saturating_add(dx);
        self.movement_delta[port_idx].1 = self.movement_delta[port_idx].1.saturating_add(dy);

        0 // CELL_OK
    }

    /// Update movement with delta values directly
    /// 
    /// Use this for relative mouse motion (e.g., in FPS games)
    /// 
    /// # Arguments
    /// * `port` - Mouse port number
    /// * `dx` - X axis movement delta
    /// * `dy` - Y axis movement delta
    pub fn update_delta(&mut self, port: u32, dx: i32, dy: i32) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return CELL_MOUSE_ERROR_INVALID_PARAMETER;
        }

        let port_idx = port as usize;
        
        // Accumulate delta
        self.movement_delta[port_idx].0 = self.movement_delta[port_idx].0.saturating_add(dx);
        self.movement_delta[port_idx].1 = self.movement_delta[port_idx].1.saturating_add(dy);
        
        // Update absolute position as well
        self.positions[port_idx].0 = self.positions[port_idx].0.saturating_add(dx);
        self.positions[port_idx].1 = self.positions[port_idx].1.saturating_add(dy);

        trace!("MouseManager::update_delta: port={}, dx={}, dy={}", port, dx, dy);

        0 // CELL_OK
    }

    /// Update wheel delta
    /// 
    /// # Arguments
    /// * `port` - Mouse port number
    /// * `delta` - Wheel scroll delta
    pub fn update_wheel(&mut self, port: u32, delta: i32) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return CELL_MOUSE_ERROR_INVALID_PARAMETER;
        }

        let port_idx = port as usize;
        self.wheel_delta[port_idx] = self.wheel_delta[port_idx].saturating_add(delta);

        trace!("MouseManager::update_wheel: port={}, delta={}", port, delta);

        0 // CELL_OK
    }

    /// Get and clear movement delta
    /// 
    /// Returns the accumulated movement delta since the last call and resets it.
    /// 
    /// # Arguments
    /// * `port` - Mouse port number
    /// 
    /// # Returns
    /// * (dx, dy) movement delta
    pub fn get_and_clear_delta(&mut self, port: u32) -> Result<(i32, i32), i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return Err(CELL_MOUSE_ERROR_INVALID_PARAMETER);
        }

        let port_idx = port as usize;
        let delta = self.movement_delta[port_idx];
        self.movement_delta[port_idx] = (0, 0);

        Ok(delta)
    }

    /// Get and clear wheel delta
    /// 
    /// Returns the accumulated wheel delta since the last call and resets it.
    /// 
    /// # Arguments
    /// * `port` - Mouse port number
    /// 
    /// # Returns
    /// * Wheel scroll delta
    pub fn get_and_clear_wheel(&mut self, port: u32) -> Result<i32, i32> {
        if !self.initialized {
            return Err(CELL_MOUSE_ERROR_NOT_INITIALIZED);
        }

        if port >= CELL_MOUSE_MAX_MICE as u32 {
            return Err(CELL_MOUSE_ERROR_INVALID_PARAMETER);
        }

        let port_idx = port as usize;
        let delta = self.wheel_delta[port_idx];
        self.wheel_delta[port_idx] = 0;

        Ok(delta)
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

        // Clear delta and wheel buffers
        let port_idx = port as usize;
        self.movement_delta[port_idx] = (0, 0);
        self.wheel_delta[port_idx] = 0;

        0 // CELL_OK
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    // ========================================================================
    // OC-Input Backend Integration
    // ========================================================================

    /// Set the oc-input mouse backend
    /// 
    /// Connects the MouseManager to the oc-input mouse system,
    /// enabling actual mouse input polling.
    /// 
    /// # Arguments
    /// * `backend` - Shared reference to mouse devices
    pub fn set_input_backend(&mut self, backend: Arc<RwLock<Vec<Mouse>>>) {
        debug!("MouseManager::set_input_backend - connecting to oc-input");
        self.input_backend = Some(backend);
    }

    /// Check if the input backend is connected
    pub fn has_input_backend(&self) -> bool {
        self.input_backend.is_some()
    }

    /// Poll input from backend
    /// 
    /// Reads current mouse state from oc-input and updates mouse data.
    pub fn poll_input(&mut self) -> i32 {
        if !self.initialized {
            return CELL_MOUSE_ERROR_NOT_INITIALIZED;
        }

        trace!("MouseManager::poll_input");

        // Get backend or fall back to manual updates
        let backend = match &self.input_backend {
            Some(b) => b.clone(),
            None => {
                // No backend connected, mouse data is manually updated
                return 0;
            }
        };

        // Lock backend for reading
        let mice = match backend.read() {
            Ok(m) => m,
            Err(e) => {
                debug!("MouseManager::poll_input - failed to lock backend: {}", e);
                return 0;
            }
        };

        // Update connected mice and poll each one
        let mut connected_mask = 0u8;

        for (port, mouse) in mice.iter().enumerate() {
            if port >= CELL_MOUSE_MAX_MICE {
                break;
            }

            if mouse.connected {
                connected_mask |= 1 << port;

                // Convert oc-input mouse state to PS3 format
                self.convert_mouse_state(port, &mouse.state);
            }
        }

        self.connected_mice = connected_mask;

        0 // CELL_OK
    }

    /// Convert oc-input mouse state to PS3 CellMouseData format
    fn convert_mouse_state(&mut self, port: usize, state: &MouseState) {
        // Calculate delta from previous position
        let dx = state.x - self.prev_positions[port].0;
        let dy = state.y - self.prev_positions[port].1;

        // Update previous position for next delta calculation
        self.prev_positions[port] = (state.x, state.y);

        // Update current position
        self.positions[port] = (state.x, state.y);

        // Accumulate movement delta
        self.movement_delta[port].0 = self.movement_delta[port].0.saturating_add(dx);
        self.movement_delta[port].1 = self.movement_delta[port].1.saturating_add(dy);

        // Accumulate wheel delta
        self.wheel_delta[port] = self.wheel_delta[port].saturating_add(state.wheel);

        // Convert button flags
        self.button_states[port] = Self::convert_buttons(state.buttons);

        // Update cached mouse data structure
        self.mouse_data[port].x_pos = state.x;
        self.mouse_data[port].y_pos = state.y;
        self.mouse_data[port].buttons = self.button_states[port];
        self.mouse_data[port].wheel = state.wheel;
        self.mouse_data[port].update += 1;
    }

    /// Convert oc-input button flags to PS3 button flags
    fn convert_buttons(buttons: MouseButtons) -> u32 {
        let mut result = 0u32;

        if buttons.contains(MouseButtons::LEFT) {
            result |= CELL_MOUSE_BUTTON_LEFT;
        }
        if buttons.contains(MouseButtons::RIGHT) {
            result |= CELL_MOUSE_BUTTON_RIGHT;
        }
        if buttons.contains(MouseButtons::MIDDLE) {
            result |= CELL_MOUSE_BUTTON_MIDDLE;
        }
        if buttons.contains(MouseButtons::BUTTON4) {
            result |= CELL_MOUSE_BUTTON_4;
        }
        if buttons.contains(MouseButtons::BUTTON5) {
            result |= CELL_MOUSE_BUTTON_5;
        }

        result
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
    pub fn map_button(oc_input_button: MouseButtons) -> u32 {
        Self::convert_buttons(oc_input_button)
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
pub fn cell_mouse_get_info(info_addr: u32) -> i32 {
    trace!("cellMouseGetInfo(info_addr=0x{:08X})", info_addr);

    match crate::context::get_hle_context().mouse.get_info() {
        Ok(info) => {
            if let Err(e) = info.to_guest_memory(info_addr) {
                return e;
            }
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
pub fn cell_mouse_get_data(port: u32, data_addr: u32) -> i32 {
    trace!("cellMouseGetData(port={}, data_addr=0x{:08X})", port, data_addr);

    match crate::context::get_hle_context().mouse.get_data(port) {
        Ok(data) => {
            if let Err(e) = data.to_guest_memory(data_addr) {
                return e;
            }
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
pub fn cell_mouse_get_data_list(port: u32, data_addr: u32) -> i32 {
    trace!("cellMouseGetDataList(port={}, data_addr=0x{:08X})", port, data_addr);

    match crate::context::get_hle_context().mouse.get_data_list(port) {
        Ok(list) => {
            // Write list_num first
            if let Err(e) = write_be32(data_addr, list.list_num) {
                return e;
            }
            // Write each data entry
            for i in 0..list.list_num.min(CELL_MOUSE_MAX_DATA as u32) as usize {
                let entry_addr = data_addr + 4 + (i as u32 * 32); // Each CellMouseData is 32 bytes
                if let Err(e) = list.list[i].to_guest_memory(entry_addr) {
                    return e;
                }
            }
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
pub fn cell_mouse_get_raw_data(port: u32, data_addr: u32) -> i32 {
    trace!("cellMouseGetRawData(port={}, data_addr=0x{:08X})", port, data_addr);

    match crate::context::get_hle_context().mouse.get_raw_data(port) {
        Ok(data) => {
            if let Err(e) = data.to_guest_memory(data_addr) {
                return e;
            }
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

    #[test]
    fn test_mouse_movement_delta() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        // Update delta directly
        assert_eq!(manager.update_delta(0, 10, -5), 0);
        assert_eq!(manager.update_delta(0, 5, 3), 0);
        
        // Delta should be accumulated
        let delta = manager.get_and_clear_delta(0).unwrap();
        assert_eq!(delta, (15, -2));
        
        // After clearing, delta should be zero
        let delta = manager.get_and_clear_delta(0).unwrap();
        assert_eq!(delta, (0, 0));
        
        manager.end();
    }

    #[test]
    fn test_mouse_wheel_delta() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        // Update wheel
        assert_eq!(manager.update_wheel(0, 3), 0);
        assert_eq!(manager.update_wheel(0, -1), 0);
        
        // Wheel delta should be accumulated
        let wheel = manager.get_and_clear_wheel(0).unwrap();
        assert_eq!(wheel, 2);
        
        // After clearing, wheel delta should be zero
        let wheel = manager.get_and_clear_wheel(0).unwrap();
        assert_eq!(wheel, 0);
        
        manager.end();
    }

    #[test]
    fn test_mouse_raw_data_with_delta() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        // Set some delta and buttons
        manager.update_delta(0, 50, -25);
        manager.update_wheel(0, 2);
        manager.set_buttons(0, CELL_MOUSE_BUTTON_LEFT);
        
        // Get raw data
        let raw = manager.get_raw_data(0).unwrap();
        assert_eq!(raw.x_axis, 50);
        assert_eq!(raw.y_axis, -25);
        assert_eq!(raw.wheel, 2);
        assert_eq!(raw.buttons, CELL_MOUSE_BUTTON_LEFT as u8);
        
        // Get raw data and clear should reset deltas
        let raw = manager.get_raw_data_and_clear(0).unwrap();
        assert_eq!(raw.x_axis, 50);
        
        // After clearing, deltas should be zero
        let raw = manager.get_raw_data(0).unwrap();
        assert_eq!(raw.x_axis, 0);
        assert_eq!(raw.y_axis, 0);
        assert_eq!(raw.wheel, 0);
        
        manager.end();
    }

    #[test]
    fn test_mouse_position_tracks_delta() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        // Set initial position (this creates delta from 0,0 to 100,100)
        manager.set_position(0, 100, 100);
        
        // Clear the initial delta
        let _ = manager.get_and_clear_delta(0);
        
        // Move to new position
        manager.set_position(0, 150, 90);
        
        // Delta should reflect only the second movement
        let delta = manager.get_and_clear_delta(0).unwrap();
        assert_eq!(delta, (50, -10));
        
        manager.end();
    }

    #[test]
    fn test_mouse_clear_buf_resets_delta() {
        let mut manager = MouseManager::new();
        manager.init(2);
        
        manager.update_delta(0, 100, 100);
        manager.update_wheel(0, 10);
        
        // Clear buffer
        manager.clear_buf(0);
        
        // Deltas should be reset
        let delta = manager.get_and_clear_delta(0).unwrap();
        assert_eq!(delta, (0, 0));
        
        let wheel = manager.get_and_clear_wheel(0).unwrap();
        assert_eq!(wheel, 0);
        
        manager.end();
    }
}
