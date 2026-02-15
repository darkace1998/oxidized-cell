//! cellPad HLE - Controller Input
//!
//! This module provides HLE implementations for PS3 controller input.
//! It bridges to the oc-input subsystem.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, trace};
use oc_input::{DualShock3Manager, dualshock3::PadData as OcInputPadData, pad::PadButtons};
use crate::memory::{write_be32, write_be16, write_u8, read_u8, ToGuestMemory, FromGuestMemory};

/// OC-Input backend reference
/// Holds a shared reference to the oc-input DualShock3Manager for controller polling
pub type InputBackend = Option<Arc<RwLock<DualShock3Manager>>>;

/// Maximum number of controllers
pub const CELL_PAD_MAX_PORT_NUM: usize = 7;

/// Maximum number of codes (buttons/axes) per controller
pub const CELL_PAD_MAX_CODES: usize = 64;

/// Pad data length constants
pub const CELL_PAD_DATA_LEN_STANDARD: i32 = 24;
pub const CELL_PAD_DATA_LEN_WITH_SENSORS: i32 = 32;

/// Button codes (digital buttons in button[0])
pub mod button_codes {
    pub const CELL_PAD_CTRL_LEFT: u16 = 0x0080;
    pub const CELL_PAD_CTRL_DOWN: u16 = 0x0040;
    pub const CELL_PAD_CTRL_RIGHT: u16 = 0x0020;
    pub const CELL_PAD_CTRL_UP: u16 = 0x0010;
    pub const CELL_PAD_CTRL_START: u16 = 0x0008;
    pub const CELL_PAD_CTRL_R3: u16 = 0x0004;
    pub const CELL_PAD_CTRL_L3: u16 = 0x0002;
    pub const CELL_PAD_CTRL_SELECT: u16 = 0x0001;
}

/// Button codes (digital buttons in button[1])
pub mod button_codes_2 {
    pub const CELL_PAD_CTRL_SQUARE: u16 = 0x0080;
    pub const CELL_PAD_CTRL_CROSS: u16 = 0x0040;
    pub const CELL_PAD_CTRL_CIRCLE: u16 = 0x0020;
    pub const CELL_PAD_CTRL_TRIANGLE: u16 = 0x0010;
    pub const CELL_PAD_CTRL_R1: u16 = 0x0008;
    pub const CELL_PAD_CTRL_L1: u16 = 0x0004;
    pub const CELL_PAD_CTRL_R2: u16 = 0x0002;
    pub const CELL_PAD_CTRL_L2: u16 = 0x0001;
}

/// Device type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellPadDeviceType {
    /// Standard controller (DUALSHOCK 3)
    Standard = 0,
    /// Guitar controller
    Guitar = 4,
    /// Drum controller
    Drum = 6,
}

/// PS3 controller button identifiers for keyboard mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ps3Button {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Start,
    Select,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

/// Virtual key codes for keyboard-to-pad mapping
///
/// Uses u32 key codes matching winit's VirtualKeyCode values.
/// Common codes: W=87, A=65, S=83, D=68, I=73, J=74, K=75, L=76,
/// Z=90, X=88, C=67, V=86, Enter=13, Space=32, Up=38, Down=40,
/// Left=37, Right=39, Q=81, E=69
pub type KeyCode = u32;

/// Key code constants matching common keyboard layouts
pub mod key_codes {
    pub const KEY_W: u32 = 87;
    pub const KEY_A: u32 = 65;
    pub const KEY_S: u32 = 83;
    pub const KEY_D: u32 = 68;
    pub const KEY_I: u32 = 73;
    pub const KEY_J: u32 = 74;
    pub const KEY_K: u32 = 75;
    pub const KEY_L: u32 = 76;
    pub const KEY_Z: u32 = 90;
    pub const KEY_X: u32 = 88;
    pub const KEY_C: u32 = 67;
    pub const KEY_V: u32 = 86;
    pub const KEY_Q: u32 = 81;
    pub const KEY_E: u32 = 69;
    pub const KEY_ENTER: u32 = 13;
    pub const KEY_SPACE: u32 = 32;
    pub const KEY_UP: u32 = 38;
    pub const KEY_DOWN: u32 = 40;
    pub const KEY_LEFT: u32 = 37;
    pub const KEY_RIGHT: u32 = 39;
}

/// Maps keyboard keys to PS3 controller buttons
///
/// Provides a default WASD layout and allows customization via `set_key_binding()`.
#[derive(Debug, Clone)]
pub struct KeyboardMapping {
    /// Map from keyboard key code to PS3 button
    bindings: HashMap<KeyCode, Ps3Button>,
}

impl KeyboardMapping {
    /// Create a new keyboard mapping with default WASD layout:
    /// - WASD = D-pad / Left stick directions
    /// - Z = Cross, X = Circle, C = Square, V = Triangle
    /// - Q = L1, E = R1, 1 = L2, 3 = R2
    /// - Enter = Start, Space = Select
    /// - Arrow keys = D-pad (alternate)
    pub fn new() -> Self {
        let mut bindings = HashMap::new();

        // D-pad / left stick via WASD
        bindings.insert(key_codes::KEY_W, Ps3Button::DpadUp);
        bindings.insert(key_codes::KEY_A, Ps3Button::DpadLeft);
        bindings.insert(key_codes::KEY_S, Ps3Button::DpadDown);
        bindings.insert(key_codes::KEY_D, Ps3Button::DpadRight);

        // Arrow keys as alternate D-pad
        bindings.insert(key_codes::KEY_UP, Ps3Button::DpadUp);
        bindings.insert(key_codes::KEY_LEFT, Ps3Button::DpadLeft);
        bindings.insert(key_codes::KEY_DOWN, Ps3Button::DpadDown);
        bindings.insert(key_codes::KEY_RIGHT, Ps3Button::DpadRight);

        // Face buttons
        bindings.insert(key_codes::KEY_Z, Ps3Button::Cross);
        bindings.insert(key_codes::KEY_X, Ps3Button::Circle);
        bindings.insert(key_codes::KEY_C, Ps3Button::Square);
        bindings.insert(key_codes::KEY_V, Ps3Button::Triangle);

        // Shoulder buttons
        bindings.insert(key_codes::KEY_Q, Ps3Button::L1);
        bindings.insert(key_codes::KEY_E, Ps3Button::R1);
        bindings.insert(49, Ps3Button::L2); // '1' key
        bindings.insert(51, Ps3Button::R2); // '3' key

        // System buttons
        bindings.insert(key_codes::KEY_ENTER, Ps3Button::Start);
        bindings.insert(key_codes::KEY_SPACE, Ps3Button::Select);

        Self { bindings }
    }

    /// Set or override a key binding
    pub fn set_key_binding(&mut self, key: KeyCode, button: Ps3Button) {
        self.bindings.insert(key, button);
    }

    /// Remove a key binding
    pub fn remove_key_binding(&mut self, key: KeyCode) {
        self.bindings.remove(&key);
    }

    /// Get the PS3 button mapped to a key, if any
    pub fn get_button(&self, key: KeyCode) -> Option<&Ps3Button> {
        self.bindings.get(&key)
    }

    /// Get all bindings
    pub fn bindings(&self) -> &HashMap<KeyCode, Ps3Button> {
        &self.bindings
    }
}

impl Default for KeyboardMapping {
    fn default() -> Self {
        Self::new()
    }
}

/// Default dead zone threshold for analog sticks
pub const DEFAULT_DEAD_ZONE: f32 = 0.15;

/// Pad info structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPadInfo {
    /// Maximum number of pads
    pub max: u32,
    /// Number of connected pads
    pub now_connect: u32,
    /// System information
    pub system_info: u32,
    /// Status of each port (connected=1, disconnected=0)
    pub port_status: [u32; CELL_PAD_MAX_PORT_NUM],
    /// Device capability info for each port
    pub device_capability: [u32; CELL_PAD_MAX_PORT_NUM],
    /// Device type for each port
    pub device_type: [u32; CELL_PAD_MAX_PORT_NUM],
}

impl Default for CellPadInfo {
    fn default() -> Self {
        Self {
            max: CELL_PAD_MAX_PORT_NUM as u32,
            now_connect: 0,
            system_info: 0,
            port_status: [0; CELL_PAD_MAX_PORT_NUM],
            device_capability: [0; CELL_PAD_MAX_PORT_NUM],
            device_type: [0; CELL_PAD_MAX_PORT_NUM],
        }
    }
}

impl ToGuestMemory for CellPadInfo {
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32> {
        let mut offset = 0u32;
        
        // Write max (4 bytes)
        write_be32(addr + offset, self.max)?;
        offset += 4;
        
        // Write now_connect (4 bytes)
        write_be32(addr + offset, self.now_connect)?;
        offset += 4;
        
        // Write system_info (4 bytes)
        write_be32(addr + offset, self.system_info)?;
        offset += 4;
        
        // Write port_status array (7 * 4 bytes)
        for status in &self.port_status {
            write_be32(addr + offset, *status)?;
            offset += 4;
        }
        
        // Write device_capability array (7 * 4 bytes)
        for cap in &self.device_capability {
            write_be32(addr + offset, *cap)?;
            offset += 4;
        }
        
        // Write device_type array (7 * 4 bytes)
        for dtype in &self.device_type {
            write_be32(addr + offset, *dtype)?;
            offset += 4;
        }
        
        Ok(())
    }
}

/// Pad data structure
/// 
/// This structure matches the PS3 cellPad data format with all button, analog, and sensor data.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPadData {
    /// Length of valid data (typically 24 for standard data, or larger with sensor data)
    pub len: i32,
    /// Digital button data (16 bits per word)
    /// button[0]: D-pad and system buttons (SELECT, L3, R3, START, UP, RIGHT, DOWN, LEFT)
    /// button[1]: Action buttons (SQUARE, CROSS, CIRCLE, TRIANGLE, R1, L1, R2, L2)
    pub button: [u16; 2],
    /// Right analog stick X axis (0-255, 128 = center)
    pub right_stick_x: u8,
    /// Right analog stick Y axis (0-255, 128 = center)
    pub right_stick_y: u8,
    /// Left analog stick X axis (0-255, 128 = center)
    pub left_stick_x: u8,
    /// Left analog stick Y axis (0-255, 128 = center)
    pub left_stick_y: u8,
    /// Pressure sensitivity for D-pad and action buttons (0-255)
    /// Order: RIGHT, LEFT, UP, DOWN, TRIANGLE, CIRCLE, CROSS, SQUARE, L1, R1, L2, R2
    pub pressure: [u8; 12],
    /// Sixaxis accelerometer X axis (10-bit value)
    pub sensor_x: u16,
    /// Sixaxis accelerometer Y axis (10-bit value)
    pub sensor_y: u16,
    /// Sixaxis accelerometer Z axis (10-bit value)
    pub sensor_z: u16,
    /// Sixaxis gyroscope Z axis (10-bit value)
    pub sensor_g: u16,
}

impl Default for CellPadData {
    fn default() -> Self {
        Self {
            len: 0,
            button: [0; 2],
            right_stick_x: 128, // Center
            right_stick_y: 128, // Center
            left_stick_x: 128,  // Center
            left_stick_y: 128,  // Center
            pressure: [0; 12],
            sensor_x: 512,      // Center (10-bit, 0-1023 range)
            sensor_y: 512,      // Center (10-bit, 0-1023 range)
            sensor_z: 512,      // Center (10-bit, actual rest value depends on orientation)
            sensor_g: 512,      // Center (10-bit, 0-1023 range)
        }
    }
}

impl ToGuestMemory for CellPadData {
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32> {
        let mut offset = 0u32;
        
        // Write len (4 bytes, signed but write as unsigned for memory)
        write_be32(addr + offset, self.len as u32)?;
        offset += 4;
        
        // Write button[0] and button[1] (2 * 2 bytes)
        write_be16(addr + offset, self.button[0])?;
        offset += 2;
        write_be16(addr + offset, self.button[1])?;
        offset += 2;
        
        // Write analog stick values (4 bytes)
        write_u8(addr + offset, self.right_stick_x)?;
        offset += 1;
        write_u8(addr + offset, self.right_stick_y)?;
        offset += 1;
        write_u8(addr + offset, self.left_stick_x)?;
        offset += 1;
        write_u8(addr + offset, self.left_stick_y)?;
        offset += 1;
        
        // Write pressure sensitivity values (12 bytes)
        for p in &self.pressure {
            write_u8(addr + offset, *p)?;
            offset += 1;
        }
        
        // Write sensor values (4 * 2 bytes)
        write_be16(addr + offset, self.sensor_x)?;
        offset += 2;
        write_be16(addr + offset, self.sensor_y)?;
        offset += 2;
        write_be16(addr + offset, self.sensor_z)?;
        offset += 2;
        write_be16(addr + offset, self.sensor_g)?;
        
        Ok(())
    }
}

impl FromGuestMemory for CellPadActParam {
    fn from_guest_memory(addr: u32) -> Result<Self, i32> {
        Ok(Self {
            motor_small: read_u8(addr)?,
            motor_large: read_u8(addr + 1)?,
            reserved: [0; 6],
        })
    }
}

/// Pressure sensitivity indices
pub mod pressure_index {
    pub const DPAD_RIGHT: usize = 0;
    pub const DPAD_LEFT: usize = 1;
    pub const DPAD_UP: usize = 2;
    pub const DPAD_DOWN: usize = 3;
    pub const TRIANGLE: usize = 4;
    pub const CIRCLE: usize = 5;
    pub const CROSS: usize = 6;
    pub const SQUARE: usize = 7;
    pub const L1: usize = 8;
    pub const R1: usize = 9;
    pub const L2: usize = 10;
    pub const R2: usize = 11;
}


/// Actuator (rumble/vibration) parameters
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub struct CellPadActParam {
    /// Small motor intensity (0-255)
    pub motor_small: u8,
    /// Large motor intensity (0-255)
    pub motor_large: u8,
    /// Reserved
    pub reserved: [u8; 6],
}


/// Rumble/vibration state for a controller
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub struct RumbleState {
    /// Small motor intensity (0-255)
    pub motor_small: u8,
    /// Large motor intensity (0-255)
    pub motor_large: u8,
    /// Whether rumble is currently active
    pub active: bool,
}


/// Pad capability info
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellPadCapabilityInfo {
    /// Capability bits
    pub info: [u32; CELL_PAD_MAX_CODES],
}

impl Default for CellPadCapabilityInfo {
    fn default() -> Self {
        Self {
            info: [0; CELL_PAD_MAX_CODES],
        }
    }
}

impl ToGuestMemory for CellPadCapabilityInfo {
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32> {
        for (i, val) in self.info.iter().enumerate() {
            write_be32(addr + (i as u32 * 4), *val)?;
        }
        Ok(())
    }
}

/// Pad manager state
pub struct PadManager {
    /// Initialization flag
    initialized: bool,
    /// Connected pad mask
    connected_pads: u8,
    /// Device types for each port
    device_types: [CellPadDeviceType; CELL_PAD_MAX_PORT_NUM],
    /// OC-Input backend
    input_backend: InputBackend,
    /// Cached pad data for each port
    pad_data: [CellPadData; CELL_PAD_MAX_PORT_NUM],
    /// Rumble/vibration state for each port
    rumble_states: [RumbleState; CELL_PAD_MAX_PORT_NUM],
    /// Keyboard-to-pad mapping for users without a gamepad
    keyboard_mapping: KeyboardMapping,
    /// Dead zone threshold for analog sticks (0.0-1.0)
    dead_zone: f32,
}

impl PadManager {
    /// Create a new pad manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            connected_pads: 0,
            device_types: [CellPadDeviceType::Standard; CELL_PAD_MAX_PORT_NUM],
            input_backend: None,
            pad_data: [CellPadData::default(); CELL_PAD_MAX_PORT_NUM],
            rumble_states: [RumbleState::default(); CELL_PAD_MAX_PORT_NUM],
            keyboard_mapping: KeyboardMapping::new(),
            dead_zone: DEFAULT_DEAD_ZONE,
        }
    }

    /// Initialize pad system
    pub fn init(&mut self, max_connect: u32) -> i32 {
        if self.initialized {
            debug!("cellPadInit: already initialized");
            return 0x80121101u32 as i32; // CELL_PAD_ERROR_ALREADY_INITIALIZED
        }

        debug!("cellPadInit: initializing pad system with max_connect={}", max_connect);
        self.initialized = true;

        // Simulate one controller connected on port 0
        self.connected_pads = 0x01;
        self.device_types[0] = CellPadDeviceType::Standard;
        
        // Initialize pad data for the connected controller
        self.pad_data[0].len = CELL_PAD_DATA_LEN_STANDARD;

        // Note: oc-input backend is connected via set_input_backend() method
        // which is called by the integration layer when the input system is ready.
        // The poll_input() method reads from the backend when called.

        0 // CELL_OK
    }

    /// Shutdown pad system
    pub fn end(&mut self) -> i32 {
        if !self.initialized {
            debug!("cellPadEnd: not initialized");
            return 0x80121103u32 as i32; // CELL_PAD_ERROR_UNINITIALIZED
        }

        debug!("cellPadEnd: shutting down pad system");
        self.initialized = false;
        self.connected_pads = 0;

        0 // CELL_OK
    }

    /// Get pad info
    pub fn get_info(&self) -> CellPadInfo {
        let mut info = CellPadInfo::default();

        if self.initialized {
            // Report connected pads
            for port in 0..CELL_PAD_MAX_PORT_NUM {
                if (self.connected_pads & (1 << port)) != 0 {
                    info.now_connect += 1;
                    info.port_status[port] = 1;
                    info.device_capability[port] = 0; // Standard controller
                    info.device_type[port] = self.device_types[port] as u32;
                }
            }
        }

        info
    }

    /// Get pad data
    pub fn get_data(&self, port: u32) -> Result<CellPadData, i32> {
        if !self.initialized {
            return Err(0x80121103u32 as i32); // CELL_PAD_ERROR_UNINITIALIZED
        }

        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return Err(0x80121104u32 as i32); // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return Err(0x80121102u32 as i32); // CELL_PAD_ERROR_NO_DEVICE
        }

        // Return cached pad data for the specified port
        // This supports multiple controllers by using the port index
        Ok(self.pad_data[port as usize])
    }

    /// Get capability info for a pad
    pub fn get_capability_info(&self, port: u32) -> Result<CellPadCapabilityInfo, i32> {
        if !self.initialized {
            return Err(0x80121103u32 as i32); // CELL_PAD_ERROR_UNINITIALIZED
        }

        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return Err(0x80121104u32 as i32); // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return Err(0x80121102u32 as i32); // CELL_PAD_ERROR_NO_DEVICE
        }

        // Return DUALSHOCK 3 capabilities
        let mut cap = CellPadCapabilityInfo::default();
        
        // Standard DUALSHOCK 3 has:
        // - D-Pad (4 buttons)
        // - Triangle, Circle, Cross, Square
        // - L1, L2, L3, R1, R2, R3
        // - Start, Select
        // - Analog sticks (2x2 axes)
        // - Pressure sensitive buttons
        
        // Mark all digital buttons as available
        cap.info[0] = 0xFFFF; // All standard buttons
        
        Ok(cap)
    }

    /// Connect a pad on a specific port
    pub fn connect_pad(&mut self, port: u32, device_type: CellPadDeviceType) -> i32 {
        if !self.initialized {
            return 0x80121103u32 as i32; // CELL_PAD_ERROR_UNINITIALIZED
        }

        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        self.connected_pads |= 1 << port;
        self.device_types[port as usize] = device_type;
        
        // Initialize pad data for the connected controller
        self.pad_data[port as usize].len = CELL_PAD_DATA_LEN_STANDARD;
        
        debug!("Connected pad on port {} with type {:?}", port, device_type);
        
        0 // CELL_OK
    }

    /// Disconnect a pad from a specific port
    pub fn disconnect_pad(&mut self, port: u32) -> i32 {
        if !self.initialized {
            return 0x80121103u32 as i32; // CELL_PAD_ERROR_UNINITIALIZED
        }

        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        self.connected_pads &= !(1 << port);
        self.pad_data[port as usize] = CellPadData::default();
        
        debug!("Disconnected pad on port {}", port);
        
        0 // CELL_OK
    }

    /// Update pad data from input backend
    pub fn update_pad_data(&mut self, port: u32, buttons: [u16; 2]) -> i32 {
        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return 0x80121102u32 as i32; // CELL_PAD_ERROR_NO_DEVICE
        }

        self.pad_data[port as usize].button = buttons;
        self.pad_data[port as usize].len = CELL_PAD_DATA_LEN_STANDARD;

        trace!("Updated pad data for port {}: buttons=[0x{:04X}, 0x{:04X}]", 
            port, buttons[0], buttons[1]);

        0 // CELL_OK
    }

    /// Update analog stick data for a controller
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// * `left_x` - Left stick X axis (0-255, 128 = center)
    /// * `left_y` - Left stick Y axis (0-255, 128 = center)
    /// * `right_x` - Right stick X axis (0-255, 128 = center)
    /// * `right_y` - Right stick Y axis (0-255, 128 = center)
    pub fn update_analog_sticks(&mut self, port: u32, left_x: u8, left_y: u8, right_x: u8, right_y: u8) -> i32 {
        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return 0x80121102u32 as i32; // CELL_PAD_ERROR_NO_DEVICE
        }

        let data = &mut self.pad_data[port as usize];
        data.left_stick_x = left_x;
        data.left_stick_y = left_y;
        data.right_stick_x = right_x;
        data.right_stick_y = right_y;

        trace!(
            "Updated analog sticks for port {}: L=({}, {}), R=({}, {})",
            port, left_x, left_y, right_x, right_y
        );

        0 // CELL_OK
    }

    /// Update pressure sensitivity data for buttons
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// * `pressure` - Array of 12 pressure values (0-255)
    pub fn update_pressure_data(&mut self, port: u32, pressure: [u8; 12]) -> i32 {
        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return 0x80121102u32 as i32; // CELL_PAD_ERROR_NO_DEVICE
        }

        self.pad_data[port as usize].pressure = pressure;

        trace!("Updated pressure data for port {}", port);

        0 // CELL_OK
    }

    /// Update sixaxis sensor data
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// * `accel_x` - Accelerometer X axis (10-bit value, 0-1023)
    /// * `accel_y` - Accelerometer Y axis (10-bit value, 0-1023)
    /// * `accel_z` - Accelerometer Z axis (10-bit value, 0-1023)
    /// * `gyro_z` - Gyroscope Z axis (10-bit value, 0-1023)
    pub fn update_sensor_data(&mut self, port: u32, accel_x: u16, accel_y: u16, accel_z: u16, gyro_z: u16) -> i32 {
        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return 0x80121102u32 as i32; // CELL_PAD_ERROR_NO_DEVICE
        }

        let data = &mut self.pad_data[port as usize];
        data.sensor_x = accel_x;
        data.sensor_y = accel_y;
        data.sensor_z = accel_z;
        data.sensor_g = gyro_z;
        
        // Sensor data extends the data length
        data.len = CELL_PAD_DATA_LEN_WITH_SENSORS;

        trace!(
            "Updated sensor data for port {}: accel=({}, {}, {}), gyro={}",
            port, accel_x, accel_y, accel_z, gyro_z
        );

        0 // CELL_OK
    }

    /// Update complete pad state (buttons, analogs, pressure, sensors)
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// * `data` - Complete pad data structure
    pub fn update_complete_pad_data(&mut self, port: u32, data: CellPadData) -> i32 {
        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return 0x80121102u32 as i32; // CELL_PAD_ERROR_NO_DEVICE
        }

        self.pad_data[port as usize] = data;

        trace!("Updated complete pad data for port {}", port);

        0 // CELL_OK
    }

    // ========================================================================
    // OC-Input Backend Integration
    // ========================================================================

    /// Set the oc-input backend for controller polling
    /// 
    /// This connects the PadManager to the oc-input DualShock3Manager,
    /// enabling actual controller input polling.
    /// 
    /// # Arguments
    /// * `backend` - Shared reference to DualShock3Manager
    pub fn set_input_backend(&mut self, backend: Arc<RwLock<DualShock3Manager>>) {
        debug!("PadManager::set_input_backend - connecting to oc-input");
        self.input_backend = Some(backend);
    }

    /// Check if the input backend is connected
    pub fn has_input_backend(&self) -> bool {
        self.input_backend.is_some()
    }

    /// Poll input from backend
    /// 
    /// Reads current input state from oc-input and updates pad data.
    pub fn poll_input(&mut self) -> i32 {
        if !self.initialized {
            return 0x80121103u32 as i32; // CELL_PAD_ERROR_UNINITIALIZED
        }

        trace!("PadManager::poll_input");

        // Get backend or fall back to manual updates
        let backend = match &self.input_backend {
            Some(b) => b.clone(),
            None => {
                // No backend connected, pad data is manually updated
                return 0;
            }
        };

        // Lock backend for reading
        let manager = match backend.read() {
            Ok(m) => m,
            Err(e) => {
                debug!("PadManager::poll_input - failed to lock backend: {}", e);
                return 0;
            }
        };

        // Update connected pads mask and poll each controller
        let mut connected_mask = 0u8;
        
        for port in 0..CELL_PAD_MAX_PORT_NUM {
            if let Some(controller) = manager.get(port as u8) {
                if controller.is_connected() {
                    connected_mask |= 1 << port;
                    
                    // Get raw pad data from oc-input
                    let input_data = controller.get_pad_data();
                    
                    // Convert to PS3 format
                    self.convert_input_to_pad_data(port, &input_data);
                }
            }
        }
        
        self.connected_pads = connected_mask;

        0 // CELL_OK
    }

    /// Convert oc-input PadData to PS3 CellPadData format
    fn convert_input_to_pad_data(&mut self, port: usize, input: &OcInputPadData) {
        let pad = &mut self.pad_data[port];
        
        // Set data length (with sensors since oc-input includes sixaxis)
        pad.len = CELL_PAD_DATA_LEN_WITH_SENSORS;
        
        // Convert button bitmask to PS3 button[0] and button[1] format
        let (btn0, btn1) = Self::convert_buttons_to_ps3(input.buttons);
        pad.button[0] = btn0 as u16;
        pad.button[1] = btn1 as u16;
        
        // Analog sticks with dead zone applied
        // oc-input already uses 0-255 format with 128 center
        pad.right_stick_x = Self::apply_dead_zone(input.right_x, self.dead_zone);
        pad.right_stick_y = Self::apply_dead_zone(input.right_y, self.dead_zone);
        pad.left_stick_x = Self::apply_dead_zone(input.left_x, self.dead_zone);
        pad.left_stick_y = Self::apply_dead_zone(input.left_y, self.dead_zone);
        
        // Pressure-sensitive button values
        // PS3 pressure order: RIGHT, LEFT, UP, DOWN, TRIANGLE, CIRCLE, CROSS, SQUARE, L1, R1, L2, R2
        for i in 0..12 {
            pad.pressure[i] = input.pressure[i];
        }
        
        // Sixaxis sensor data (convert from i16 to u16 with offset)
        // PS3 expects values in range 0-1023 (10-bit)
        pad.sensor_x = (input.accel_x + 512).clamp(0, 1023) as u16;
        pad.sensor_y = (input.accel_y + 512).clamp(0, 1023) as u16;
        pad.sensor_z = (input.accel_z + 512).clamp(0, 1023) as u16;
        pad.sensor_g = (input.gyro_z + 512).clamp(0, 1023) as u16;
    }

    /// Convert oc-input button bitmask to PS3 button[0]/button[1] format
    /// 
    /// oc-input uses single u32 bitmask, PS3 uses two u8 values
    fn convert_buttons_to_ps3(buttons: u32) -> (u8, u8) {
        let mut btn0: u8 = 0;
        let mut btn1: u8 = 0;
        
        // button[0] - D-pad and system buttons
        if buttons & PadButtons::DPAD_LEFT.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_LEFT as u8;
        }
        if buttons & PadButtons::DPAD_DOWN.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_DOWN as u8;
        }
        if buttons & PadButtons::DPAD_RIGHT.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_RIGHT as u8;
        }
        if buttons & PadButtons::DPAD_UP.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_UP as u8;
        }
        if buttons & PadButtons::START.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_START as u8;
        }
        if buttons & PadButtons::R3.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_R3 as u8;
        }
        if buttons & PadButtons::L3.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_L3 as u8;
        }
        if buttons & PadButtons::SELECT.bits() != 0 {
            btn0 |= button_codes::CELL_PAD_CTRL_SELECT as u8;
        }
        
        // button[1] - Face and shoulder buttons
        if buttons & PadButtons::SQUARE.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_SQUARE as u8;
        }
        if buttons & PadButtons::CROSS.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_CROSS as u8;
        }
        if buttons & PadButtons::CIRCLE.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_CIRCLE as u8;
        }
        if buttons & PadButtons::TRIANGLE.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_TRIANGLE as u8;
        }
        if buttons & PadButtons::R1.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_R1 as u8;
        }
        if buttons & PadButtons::L1.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_L1 as u8;
        }
        if buttons & PadButtons::R2.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_R2 as u8;
        }
        if buttons & PadButtons::L2.bits() != 0 {
            btn1 |= button_codes_2::CELL_PAD_CTRL_L2 as u8;
        }
        
        (btn0, btn1)
    }

    /// Map oc-input button to PS3 button
    /// 
    /// Converts button codes between oc-input and PS3 formats.
    #[allow(dead_code)]
    pub fn map_button(oc_input_button: u32) -> u16 {
        // Map individual button constants
        if oc_input_button & PadButtons::CROSS.bits() != 0 {
            return button_codes_2::CELL_PAD_CTRL_CROSS;
        }
        if oc_input_button & PadButtons::CIRCLE.bits() != 0 {
            return button_codes_2::CELL_PAD_CTRL_CIRCLE;
        }
        if oc_input_button & PadButtons::SQUARE.bits() != 0 {
            return button_codes_2::CELL_PAD_CTRL_SQUARE;
        }
        if oc_input_button & PadButtons::TRIANGLE.bits() != 0 {
            return button_codes_2::CELL_PAD_CTRL_TRIANGLE;
        }
        
        trace!("Mapping button: 0x{:08X}", oc_input_button);
        0
    }

    /// Convert analog axis value
    /// 
    /// Converts axis values from oc-input format to PS3 format (0-255).
    pub fn convert_axis(oc_input_value: f32) -> u8 {
        // oc-input typically uses -1.0 to 1.0 range
        // PS3 uses 0-255 with 128 as center
        
        let normalized = (oc_input_value + 1.0) / 2.0; // Convert to 0.0-1.0
        
        
        (normalized * 255.0) as u8
    }

    /// Apply dead zone to an analog stick axis value
    ///
    /// Values within the dead zone around center (128) are snapped to center.
    /// Values outside the dead zone are rescaled to use the full 0-255 range.
    ///
    /// # Arguments
    /// * `value` - Raw axis value (0-255, 128 = center)
    /// * `threshold` - Dead zone threshold (0.0-1.0, e.g. 0.15 = 15%)
    pub fn apply_dead_zone(value: u8, threshold: f32) -> u8 {
        let center = 128.0f32;
        let offset = (value as f32) - center;
        let magnitude = offset.abs() / center; // 0.0-1.0

        if magnitude < threshold {
            128 // Snap to center
        } else {
            // Rescale: map [threshold..1.0] → [0.0..1.0]
            let rescaled = (magnitude - threshold) / (1.0 - threshold);
            let sign = offset.signum();
            (center + sign * rescaled * center).clamp(0.0, 255.0) as u8
        }
    }

    /// Set the dead zone threshold for analog sticks
    ///
    /// # Arguments
    /// * `threshold` - Dead zone threshold (0.0-1.0, default 0.15)
    pub fn set_dead_zone(&mut self, threshold: f32) {
        self.dead_zone = threshold.clamp(0.0, 1.0);
    }

    /// Get the current dead zone threshold
    pub fn dead_zone(&self) -> f32 {
        self.dead_zone
    }

    /// Get a reference to the keyboard mapping
    pub fn keyboard_mapping(&self) -> &KeyboardMapping {
        &self.keyboard_mapping
    }

    /// Get a mutable reference to the keyboard mapping for customization
    pub fn keyboard_mapping_mut(&mut self) -> &mut KeyboardMapping {
        &mut self.keyboard_mapping
    }

    /// Update pad data from keyboard state (for users without a gamepad)
    ///
    /// This maps keyboard keys to PS3 controller buttons using the configured
    /// `KeyboardMapping`. Updated data is written to pad port 0.
    ///
    /// # Arguments
    /// * `pressed_keys` - Set of currently pressed key codes
    pub fn update_from_keyboard(&mut self, pressed_keys: &[KeyCode]) {
        if !self.initialized {
            return;
        }

        let pad = &mut self.pad_data[0];
        pad.len = CELL_PAD_DATA_LEN_WITH_SENSORS;

        let mut btn0: u16 = 0;
        let mut btn1: u16 = 0;

        for &key in pressed_keys {
            if let Some(&button) = self.keyboard_mapping.get_button(key) {
                match button {
                    // button[0] - D-pad and system buttons
                    Ps3Button::DpadLeft => btn0 |= button_codes::CELL_PAD_CTRL_LEFT,
                    Ps3Button::DpadDown => btn0 |= button_codes::CELL_PAD_CTRL_DOWN,
                    Ps3Button::DpadRight => btn0 |= button_codes::CELL_PAD_CTRL_RIGHT,
                    Ps3Button::DpadUp => btn0 |= button_codes::CELL_PAD_CTRL_UP,
                    Ps3Button::Start => btn0 |= button_codes::CELL_PAD_CTRL_START,
                    Ps3Button::Select => btn0 |= button_codes::CELL_PAD_CTRL_SELECT,
                    Ps3Button::L3 => btn0 |= button_codes::CELL_PAD_CTRL_L3,
                    Ps3Button::R3 => btn0 |= button_codes::CELL_PAD_CTRL_R3,
                    // button[1] - Face and shoulder buttons
                    Ps3Button::Cross => btn1 |= button_codes_2::CELL_PAD_CTRL_CROSS,
                    Ps3Button::Circle => btn1 |= button_codes_2::CELL_PAD_CTRL_CIRCLE,
                    Ps3Button::Square => btn1 |= button_codes_2::CELL_PAD_CTRL_SQUARE,
                    Ps3Button::Triangle => btn1 |= button_codes_2::CELL_PAD_CTRL_TRIANGLE,
                    Ps3Button::L1 => btn1 |= button_codes_2::CELL_PAD_CTRL_L1,
                    Ps3Button::R1 => btn1 |= button_codes_2::CELL_PAD_CTRL_R1,
                    Ps3Button::L2 => btn1 |= button_codes_2::CELL_PAD_CTRL_L2,
                    Ps3Button::R2 => btn1 |= button_codes_2::CELL_PAD_CTRL_R2,
                }
            }
        }

        pad.button[0] = btn0;
        pad.button[1] = btn1;

        // Set pressure-sensitive values for pressed face buttons (255 = fully pressed)
        pad.pressure[6] = if btn1 & button_codes_2::CELL_PAD_CTRL_CROSS != 0 { 255 } else { 0 };
        pad.pressure[5] = if btn1 & button_codes_2::CELL_PAD_CTRL_CIRCLE != 0 { 255 } else { 0 };
        pad.pressure[4] = if btn1 & button_codes_2::CELL_PAD_CTRL_TRIANGLE != 0 { 255 } else { 0 };
        pad.pressure[7] = if btn1 & button_codes_2::CELL_PAD_CTRL_SQUARE != 0 { 255 } else { 0 };
        pad.pressure[8] = if btn1 & button_codes_2::CELL_PAD_CTRL_L1 != 0 { 255 } else { 0 };
        pad.pressure[9] = if btn1 & button_codes_2::CELL_PAD_CTRL_R1 != 0 { 255 } else { 0 };
        pad.pressure[10] = if btn1 & button_codes_2::CELL_PAD_CTRL_L2 != 0 { 255 } else { 0 };
        pad.pressure[11] = if btn1 & button_codes_2::CELL_PAD_CTRL_R2 != 0 { 255 } else { 0 };

        // Analog sticks at center (keyboard doesn't have analog input)
        pad.left_stick_x = 128;
        pad.left_stick_y = 128;
        pad.right_stick_x = 128;
        pad.right_stick_y = 128;

        // Ensure sixaxis stays at neutral
        pad.sensor_x = 512;
        pad.sensor_y = 512;
        pad.sensor_z = 512;
        pad.sensor_g = 512;

        // Mark pad 0 as connected when keyboard is in use
        self.connected_pads |= 1;
    }

    /// Check if backend is connected
    pub fn is_backend_connected(&self) -> bool {
        self.input_backend.is_some()
    }

    // ========================================================================
    // Rumble/Vibration Support
    // ========================================================================

    /// Set actuator (rumble/vibration) parameters
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// * `param` - Actuator parameters (motor intensities)
    /// 
    /// # Returns
    /// * 0 on success, error code otherwise
    pub fn set_actuator(&mut self, port: u32, param: &CellPadActParam) -> i32 {
        if !self.initialized {
            return 0x80121103u32 as i32; // CELL_PAD_ERROR_UNINITIALIZED
        }

        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return 0x80121104u32 as i32; // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return 0x80121102u32 as i32; // CELL_PAD_ERROR_NO_DEVICE
        }

        let port_idx = port as usize;
        self.rumble_states[port_idx].motor_small = param.motor_small;
        self.rumble_states[port_idx].motor_large = param.motor_large;
        self.rumble_states[port_idx].active = param.motor_small > 0 || param.motor_large > 0;

        debug!(
            "Set actuator for port {}: small={}, large={}",
            port, param.motor_small, param.motor_large
        );

        // Forward rumble command to oc-input backend
        if let Some(backend) = &self.input_backend {
            if let Ok(mut manager) = backend.write() {
                if let Some(controller) = manager.get_mut(port as u8) {
                    use oc_input::dualshock3::VibrationEffect;
                    
                    if param.motor_small > 0 || param.motor_large > 0 {
                        // Apply custom vibration with exact motor values
                        controller.vibrate(
                            VibrationEffect::Custom {
                                small: param.motor_small,
                                large: param.motor_large,
                            },
                            None, // No automatic timeout
                        );
                    } else {
                        // Stop vibration
                        controller.stop_vibration();
                    }
                }
            }
        }

        0 // CELL_OK
    }

    /// Get actuator status
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// 
    /// # Returns
    /// * Current actuator parameters
    pub fn get_actuator(&self, port: u32) -> Result<CellPadActParam, i32> {
        if !self.initialized {
            return Err(0x80121103u32 as i32); // CELL_PAD_ERROR_UNINITIALIZED
        }

        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return Err(0x80121104u32 as i32); // CELL_PAD_ERROR_INVALID_PARAMETER
        }

        if (self.connected_pads & (1 << port)) == 0 {
            return Err(0x80121102u32 as i32); // CELL_PAD_ERROR_NO_DEVICE
        }

        let port_idx = port as usize;
        let rumble = &self.rumble_states[port_idx];

        Ok(CellPadActParam {
            motor_small: rumble.motor_small,
            motor_large: rumble.motor_large,
            reserved: [0; 6],
        })
    }

    /// Stop rumble/vibration on a controller
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// 
    /// # Returns
    /// * 0 on success
    pub fn stop_actuator(&mut self, port: u32) -> i32 {
        let param = CellPadActParam::default();
        self.set_actuator(port, &param)
    }

    /// Check if rumble is active on a controller
    /// 
    /// # Arguments
    /// * `port` - Controller port number
    /// 
    /// # Returns
    /// * true if rumble is active
    pub fn is_rumble_active(&self, port: u32) -> bool {
        if port >= CELL_PAD_MAX_PORT_NUM as u32 {
            return false;
        }

        self.rumble_states[port as usize].active
    }
}

impl Default for PadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellPadInit - Initialize pad system
///
/// # Arguments
/// * `max_connect` - Maximum number of controllers to support
///
/// # Returns
/// * 0 on success
pub fn cell_pad_init(max_connect: u32) -> i32 {
    debug!("cellPadInit(max_connect={})", max_connect);

    crate::context::get_hle_context_mut().pad.init(max_connect)
}

/// cellPadEnd - Shutdown pad system
///
/// # Returns
/// * 0 on success
pub fn cell_pad_end() -> i32 {
    debug!("cellPadEnd()");

    crate::context::get_hle_context_mut().pad.end()
}

/// cellPadGetInfo - Get pad info
///
/// # Arguments
/// * `info_addr` - Address to write pad info to
///
/// # Returns
/// * 0 on success
pub fn cell_pad_get_info(info_addr: u32) -> i32 {
    trace!("cellPadGetInfo(info_addr=0x{:08X})", info_addr);

    let info = crate::context::get_hle_context().pad.get_info();
    
    // Write info to memory
    if let Err(e) = info.to_guest_memory(info_addr) {
        return e;
    }

    0 // CELL_OK
}

/// cellPadGetInfo2 - Get extended pad info
///
/// # Arguments
/// * `info_addr` - Address to write pad info to
///
/// # Returns
/// * 0 on success
pub fn cell_pad_get_info2(info_addr: u32) -> i32 {
    trace!("cellPadGetInfo2(info_addr=0x{:08X})", info_addr);

    let info = crate::context::get_hle_context().pad.get_info();
    
    // Write info to memory (same structure as GetInfo)
    if let Err(e) = info.to_guest_memory(info_addr) {
        return e;
    }

    0 // CELL_OK
}

/// cellPadGetData - Get pad data
///
/// # Arguments
/// * `port` - Controller port number
/// * `data_addr` - Address to write pad data to
///
/// # Returns
/// * 0 on success
pub fn cell_pad_get_data(port: u32, data_addr: u32) -> i32 {
    trace!("cellPadGetData(port={}, data_addr=0x{:08X})", port, data_addr);

    match crate::context::get_hle_context().pad.get_data(port) {
        Ok(data) => {
            // Write data to memory
            if let Err(e) = data.to_guest_memory(data_addr) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellPadGetCapabilityInfo - Get controller capabilities
///
/// # Arguments
/// * `port` - Controller port number
/// * `info_addr` - Address to write capability info to
///
/// # Returns
/// * 0 on success
pub fn cell_pad_get_capability_info(port: u32, info_addr: u32) -> i32 {
    trace!("cellPadGetCapabilityInfo(port={}, info_addr=0x{:08X})", port, info_addr);

    match crate::context::get_hle_context().pad.get_capability_info(port) {
        Ok(info) => {
            // Write capability info to memory
            if let Err(e) = info.to_guest_memory(info_addr) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellPadSetActParam - Set actuator (rumble/vibration) parameters
///
/// # Arguments
/// * `port` - Controller port number
/// * `param_addr` - Address of CellPadActParam structure
///
/// # Returns
/// * 0 on success
pub fn cell_pad_set_act_param(port: u32, param_addr: u32) -> i32 {
    debug!("cellPadSetActParam(port={}, param_addr=0x{:08X})", port, param_addr);

    // Read param from memory
    let param = match CellPadActParam::from_guest_memory(param_addr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    
    crate::context::get_hle_context_mut().pad.set_actuator(port, &param)
}

/// cellPadLddSetActParam - Set actuator parameters (legacy)
///
/// # Arguments
/// * `port` - Controller port number
/// * `motor_small` - Small motor intensity (0-255)
/// * `motor_large` - Large motor intensity (0-255)
///
/// # Returns
/// * 0 on success
pub fn cell_pad_ldd_set_act_param(port: u32, motor_small: u8, motor_large: u8) -> i32 {
    debug!(
        "cellPadLddSetActParam(port={}, small={}, large={})",
        port, motor_small, motor_large
    );

    let param = CellPadActParam {
        motor_small,
        motor_large,
        reserved: [0; 6],
    };
    
    crate::context::get_hle_context_mut().pad.set_actuator(port, &param)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad_manager() {
        let mut manager = PadManager::new();
        assert_eq!(manager.init(7), 0);
        
        let info = manager.get_info();
        assert_eq!(info.max, CELL_PAD_MAX_PORT_NUM as u32);
        assert_eq!(info.now_connect, 1); // One simulated controller
        
        assert_eq!(manager.end(), 0);
    }

    #[test]
    fn test_pad_manager_data() {
        let mut manager = PadManager::new();
        manager.init(7);
        
        // Get data from connected port
        let data = manager.get_data(0);
        assert!(data.is_ok());
        assert_eq!(data.unwrap().len, CELL_PAD_DATA_LEN_STANDARD);
        
        // Try to get data from disconnected port
        let data = manager.get_data(1);
        assert!(data.is_err());
        
        manager.end();
    }

    #[test]
    fn test_pad_manager_capability() {
        let mut manager = PadManager::new();
        manager.init(7);
        
        // Get capability info from connected port
        let cap = manager.get_capability_info(0);
        assert!(cap.is_ok());
        assert_eq!(cap.unwrap().info[0], 0xFFFF);
        
        // Try from disconnected port
        let cap = manager.get_capability_info(1);
        assert!(cap.is_err());
        
        manager.end();
    }

    #[test]
    fn test_pad_manager_connect_disconnect() {
        let mut manager = PadManager::new();
        manager.init(7);
        
        // Connect a pad on port 1
        assert_eq!(manager.connect_pad(1, CellPadDeviceType::Standard), 0);
        
        let info = manager.get_info();
        assert_eq!(info.now_connect, 2); // Two pads now
        
        // Disconnect pad from port 1
        assert_eq!(manager.disconnect_pad(1), 0);
        
        let info = manager.get_info();
        assert_eq!(info.now_connect, 1); // Back to one pad
        
        manager.end();
    }

    #[test]
    fn test_pad_init() {
        let result = cell_pad_init(7);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_pad_info_default() {
        let info = CellPadInfo::default();
        assert_eq!(info.max, CELL_PAD_MAX_PORT_NUM as u32);
        assert_eq!(info.now_connect, 0);
    }

    #[test]
    fn test_pad_button_codes() {
        use button_codes::*;
        assert_eq!(CELL_PAD_CTRL_UP, 0x0010);
        assert_eq!(CELL_PAD_CTRL_DOWN, 0x0040);
        assert_eq!(CELL_PAD_CTRL_LEFT, 0x0080);
        assert_eq!(CELL_PAD_CTRL_RIGHT, 0x0020);
    }

    #[test]
    fn test_pad_button_codes_2() {
        use button_codes_2::*;
        assert_eq!(CELL_PAD_CTRL_TRIANGLE, 0x0010);
        assert_eq!(CELL_PAD_CTRL_CIRCLE, 0x0020);
        assert_eq!(CELL_PAD_CTRL_CROSS, 0x0040);
        assert_eq!(CELL_PAD_CTRL_SQUARE, 0x0080);
    }

    #[test]
    fn test_pad_device_types() {
        assert_eq!(CellPadDeviceType::Standard as u32, 0);
        assert_eq!(CellPadDeviceType::Guitar as u32, 4);
        assert_eq!(CellPadDeviceType::Drum as u32, 6);
    }

    #[test]
    fn test_rumble_support() {
        let mut manager = PadManager::new();
        manager.init(7);

        // Test setting actuator on connected port
        let param = CellPadActParam {
            motor_small: 100,
            motor_large: 200,
            reserved: [0; 6],
        };
        assert_eq!(manager.set_actuator(0, &param), 0);

        // Verify rumble state
        assert!(manager.is_rumble_active(0));
        let result = manager.get_actuator(0);
        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.motor_small, 100);
        assert_eq!(retrieved.motor_large, 200);

        // Stop rumble
        assert_eq!(manager.stop_actuator(0), 0);
        assert!(!manager.is_rumble_active(0));

        // Test on disconnected port should fail
        let result = manager.set_actuator(1, &param);
        assert!(result != 0);

        manager.end();
    }

    #[test]
    fn test_multiple_controllers() {
        let mut manager = PadManager::new();
        manager.init(7);

        // Connect multiple controllers
        assert_eq!(manager.connect_pad(1, CellPadDeviceType::Standard), 0);
        assert_eq!(manager.connect_pad(2, CellPadDeviceType::Guitar), 0);
        assert_eq!(manager.connect_pad(3, CellPadDeviceType::Drum), 0);

        let info = manager.get_info();
        assert_eq!(info.now_connect, 4); // 4 controllers connected

        // Update data for each controller
        assert_eq!(manager.update_pad_data(0, [0x0001, 0x0002]), 0);
        assert_eq!(manager.update_pad_data(1, [0x0004, 0x0008]), 0);
        assert_eq!(manager.update_pad_data(2, [0x0010, 0x0020]), 0);

        // Verify each controller has its own data
        let data0 = manager.get_data(0).unwrap();
        assert_eq!(data0.button[0], 0x0001);
        assert_eq!(data0.button[1], 0x0002);

        let data1 = manager.get_data(1).unwrap();
        assert_eq!(data1.button[0], 0x0004);
        assert_eq!(data1.button[1], 0x0008);

        let data2 = manager.get_data(2).unwrap();
        assert_eq!(data2.button[0], 0x0010);
        assert_eq!(data2.button[1], 0x0020);

        // Test rumble on different controllers
        let param = CellPadActParam {
            motor_small: 50,
            motor_large: 150,
            reserved: [0; 6],
        };
        assert_eq!(manager.set_actuator(1, &param), 0);
        assert!(manager.is_rumble_active(1));
        assert!(!manager.is_rumble_active(2));

        manager.end();
    }

    #[test]
    fn test_axis_conversion() {
        // Test axis conversion
        assert_eq!(PadManager::convert_axis(-1.0), 0);
        assert_eq!(PadManager::convert_axis(0.0), 127);
        assert_eq!(PadManager::convert_axis(1.0), 255);
    }

    #[test]
    fn test_analog_stick_reading() {
        let mut manager = PadManager::new();
        manager.init(7);

        // Update analog sticks
        assert_eq!(manager.update_analog_sticks(0, 100, 150, 200, 50), 0);

        let data = manager.get_data(0).unwrap();
        assert_eq!(data.left_stick_x, 100);
        assert_eq!(data.left_stick_y, 150);
        assert_eq!(data.right_stick_x, 200);
        assert_eq!(data.right_stick_y, 50);

        // Test on disconnected port
        assert_eq!(manager.update_analog_sticks(1, 128, 128, 128, 128), 0x80121102u32 as i32);

        manager.end();
    }

    #[test]
    fn test_pressure_sensitivity() {
        let mut manager = PadManager::new();
        manager.init(7);

        // Create pressure data with some buttons pressed
        let mut pressure = [0u8; 12];
        pressure[pressure_index::CROSS] = 255;    // Full press
        pressure[pressure_index::CIRCLE] = 128;   // Half press
        pressure[pressure_index::L2] = 200;       // Strong L2
        pressure[pressure_index::R2] = 100;       // Light R2

        assert_eq!(manager.update_pressure_data(0, pressure), 0);

        let data = manager.get_data(0).unwrap();
        assert_eq!(data.pressure[pressure_index::CROSS], 255);
        assert_eq!(data.pressure[pressure_index::CIRCLE], 128);
        assert_eq!(data.pressure[pressure_index::L2], 200);
        assert_eq!(data.pressure[pressure_index::R2], 100);
        assert_eq!(data.pressure[pressure_index::TRIANGLE], 0);

        manager.end();
    }

    #[test]
    fn test_sixaxis_sensor_data() {
        let mut manager = PadManager::new();
        manager.init(7);

        // Update sensor data (accelerometer + gyroscope)
        // Values are 10-bit (0-1023), 512 is center
        assert_eq!(manager.update_sensor_data(0, 512, 512, 600, 512), 0);

        let data = manager.get_data(0).unwrap();
        assert_eq!(data.sensor_x, 512);
        assert_eq!(data.sensor_y, 512);
        assert_eq!(data.sensor_z, 600); // Tilted forward
        assert_eq!(data.sensor_g, 512);
        assert_eq!(data.len, CELL_PAD_DATA_LEN_WITH_SENSORS); // Sensor data extends length

        manager.end();
    }

    #[test]
    fn test_complete_pad_data_update() {
        let mut manager = PadManager::new();
        manager.init(7);

        // Create complete pad data
        let complete_data = CellPadData {
            len: 32,
            button: [0x00FF, 0xFF00],
            left_stick_x: 64,
            left_stick_y: 192,
            right_stick_x: 200,
            right_stick_y: 56,
            pressure: [255, 200, 150, 100, 50, 25, 10, 5, 2, 1, 0, 255],
            sensor_x: 400,
            sensor_y: 500,
            sensor_z: 600,
            sensor_g: 700,
        };

        assert_eq!(manager.update_complete_pad_data(0, complete_data), 0);

        let data = manager.get_data(0).unwrap();
        assert_eq!(data.button[0], 0x00FF);
        assert_eq!(data.button[1], 0xFF00);
        assert_eq!(data.left_stick_x, 64);
        assert_eq!(data.left_stick_y, 192);
        assert_eq!(data.sensor_x, 400);

        manager.end();
    }

    #[test]
    fn test_pad_data_default_values() {
        let data = CellPadData::default();
        
        // Analog sticks should be centered
        assert_eq!(data.left_stick_x, 128);
        assert_eq!(data.left_stick_y, 128);
        assert_eq!(data.right_stick_x, 128);
        assert_eq!(data.right_stick_y, 128);
        
        // Sensors should be centered
        assert_eq!(data.sensor_x, 512);
        assert_eq!(data.sensor_y, 512);
        assert_eq!(data.sensor_z, 512);
        assert_eq!(data.sensor_g, 512);
        
        // Pressure should be zero
        assert_eq!(data.pressure, [0; 12]);
    }

    #[test]
    fn test_pressure_index_values() {
        // Verify pressure indices are correct
        assert_eq!(pressure_index::DPAD_RIGHT, 0);
        assert_eq!(pressure_index::DPAD_LEFT, 1);
        assert_eq!(pressure_index::DPAD_UP, 2);
        assert_eq!(pressure_index::DPAD_DOWN, 3);
        assert_eq!(pressure_index::TRIANGLE, 4);
        assert_eq!(pressure_index::CIRCLE, 5);
        assert_eq!(pressure_index::CROSS, 6);
        assert_eq!(pressure_index::SQUARE, 7);
        assert_eq!(pressure_index::L1, 8);
        assert_eq!(pressure_index::R1, 9);
        assert_eq!(pressure_index::L2, 10);
        assert_eq!(pressure_index::R2, 11);
    }

    #[test]
    fn test_keyboard_mapping_default() {
        let mapping = KeyboardMapping::new();

        // WASD maps to D-pad
        assert_eq!(mapping.get_button(key_codes::KEY_W), Some(&Ps3Button::DpadUp));
        assert_eq!(mapping.get_button(key_codes::KEY_A), Some(&Ps3Button::DpadLeft));
        assert_eq!(mapping.get_button(key_codes::KEY_S), Some(&Ps3Button::DpadDown));
        assert_eq!(mapping.get_button(key_codes::KEY_D), Some(&Ps3Button::DpadRight));

        // Face buttons
        assert_eq!(mapping.get_button(key_codes::KEY_Z), Some(&Ps3Button::Cross));
        assert_eq!(mapping.get_button(key_codes::KEY_X), Some(&Ps3Button::Circle));
        assert_eq!(mapping.get_button(key_codes::KEY_C), Some(&Ps3Button::Square));
        assert_eq!(mapping.get_button(key_codes::KEY_V), Some(&Ps3Button::Triangle));

        // System
        assert_eq!(mapping.get_button(key_codes::KEY_ENTER), Some(&Ps3Button::Start));
        assert_eq!(mapping.get_button(key_codes::KEY_SPACE), Some(&Ps3Button::Select));

        // Shoulders
        assert_eq!(mapping.get_button(key_codes::KEY_Q), Some(&Ps3Button::L1));
        assert_eq!(mapping.get_button(key_codes::KEY_E), Some(&Ps3Button::R1));
    }

    #[test]
    fn test_keyboard_mapping_custom() {
        let mut mapping = KeyboardMapping::new();

        // Override Z from Cross to Circle
        mapping.set_key_binding(key_codes::KEY_Z, Ps3Button::Circle);
        assert_eq!(mapping.get_button(key_codes::KEY_Z), Some(&Ps3Button::Circle));

        // Remove a binding
        mapping.remove_key_binding(key_codes::KEY_W);
        assert_eq!(mapping.get_button(key_codes::KEY_W), None);

        // Unmapped key returns None
        assert_eq!(mapping.get_button(999), None);
    }

    #[test]
    fn test_update_from_keyboard() {
        let mut pad = PadManager::new();
        pad.init(1);

        // Press Cross (Z key) + DpadUp (W key)
        pad.update_from_keyboard(&[key_codes::KEY_Z, key_codes::KEY_W]);
        let data = pad.get_data(0).unwrap();

        // Cross should be in button[1]
        assert_ne!(data.button[1] & button_codes_2::CELL_PAD_CTRL_CROSS, 0);
        // DpadUp should be in button[0]
        assert_ne!(data.button[0] & button_codes::CELL_PAD_CTRL_UP, 0);
        // Pressure for Cross should be 255
        assert_eq!(data.pressure[6], 255);
        // Sticks should be centered
        assert_eq!(data.left_stick_x, 128);
        assert_eq!(data.right_stick_y, 128);
        // Sixaxis should be neutral
        assert_eq!(data.sensor_x, 512);
        // Pad should be connected
        assert!(pad.connected_pads & 1 != 0);
    }

    #[test]
    fn test_dead_zone_center() {
        // Values near center should snap to 128
        assert_eq!(PadManager::apply_dead_zone(128, 0.15), 128);
        assert_eq!(PadManager::apply_dead_zone(130, 0.15), 128); // ~1.6% offset, below 15%
        assert_eq!(PadManager::apply_dead_zone(126, 0.15), 128);
        assert_eq!(PadManager::apply_dead_zone(148, 0.15), 128); // ~15.6%, at threshold edge
    }

    #[test]
    fn test_dead_zone_extremes() {
        // Full stick deflection should still reach near-max values
        assert_eq!(PadManager::apply_dead_zone(0, 0.15), 0);
        // At max deflection with dead zone, rescaling may not hit exactly 255
        assert!(PadManager::apply_dead_zone(255, 0.15) >= 253);

        // Zero dead zone should pass values through unchanged
        assert_eq!(PadManager::apply_dead_zone(100, 0.0), 100);
        assert_eq!(PadManager::apply_dead_zone(200, 0.0), 200);
    }

    #[test]
    fn test_set_dead_zone() {
        let mut pad = PadManager::new();
        assert!((pad.dead_zone() - DEFAULT_DEAD_ZONE).abs() < f32::EPSILON);

        pad.set_dead_zone(0.25);
        assert!((pad.dead_zone() - 0.25).abs() < f32::EPSILON);

        // Clamped to valid range
        pad.set_dead_zone(-0.5);
        assert!((pad.dead_zone() - 0.0).abs() < f32::EPSILON);
        pad.set_dead_zone(1.5);
        assert!((pad.dead_zone() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sixaxis_neutral_default() {
        let pad = CellPadData::default();
        // Sixaxis should default to neutral (512 = flat on table)
        assert_eq!(pad.sensor_x, 512);
        assert_eq!(pad.sensor_y, 512);
        assert_eq!(pad.sensor_z, 512);
        assert_eq!(pad.sensor_g, 512);
    }

    #[test]
    fn test_keyboard_no_keys_pressed() {
        let mut pad = PadManager::new();
        pad.init(1);

        pad.update_from_keyboard(&[]);
        let data = pad.get_data(0).unwrap();

        // No buttons pressed
        assert_eq!(data.button[0], 0);
        assert_eq!(data.button[1], 0);
        // All pressure values zero
        for p in &data.pressure {
            assert_eq!(*p, 0);
        }
    }

    #[test]
    fn test_ps3_button_enum_coverage() {
        // Ensure all 16 PS3 buttons are represented
        let buttons = [
            Ps3Button::Cross, Ps3Button::Circle, Ps3Button::Square, Ps3Button::Triangle,
            Ps3Button::L1, Ps3Button::R1, Ps3Button::L2, Ps3Button::R2,
            Ps3Button::L3, Ps3Button::R3, Ps3Button::Start, Ps3Button::Select,
            Ps3Button::DpadUp, Ps3Button::DpadDown, Ps3Button::DpadLeft, Ps3Button::DpadRight,
        ];
        assert_eq!(buttons.len(), 16);
    }
}
