//! cellPad HLE - Controller Input
//!
//! This module provides HLE implementations for PS3 controller input.
//! It bridges to the oc-input subsystem.

use tracing::{debug, trace};

/// OC-Input backend reference placeholder
/// In a real implementation, this would hold a reference to oc-input Pad system
type InputBackend = Option<()>;

/// Maximum number of controllers
pub const CELL_PAD_MAX_PORT_NUM: usize = 7;

/// Maximum number of codes (buttons/axes) per controller
pub const CELL_PAD_MAX_CODES: usize = 64;

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

/// Pad data structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub struct CellPadData {
    /// Length of valid data
    pub len: i32,
    /// Digital button data (16 bits)
    pub button: [u16; 2],
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
        self.pad_data[0].len = 24; // Standard data length

        // TODO: Connect to oc-input subsystem

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
        self.pad_data[port as usize].len = 24; // Standard data length
        
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
        self.pad_data[port as usize].len = 24;

        trace!("Updated pad data for port {}: buttons=[0x{:04X}, 0x{:04X}]", 
            port, buttons[0], buttons[1]);

        0 // CELL_OK
    }

    // ========================================================================
    // OC-Input Backend Integration
    // ========================================================================

    /// Connect to oc-input backend
    /// 
    /// This would integrate with oc-input for actual controller input.
    /// For now, this is a stub implementation.
    pub fn connect_input_backend(&mut self, _backend: InputBackend) -> i32 {
        debug!("PadManager::connect_input_backend");
        
        // In a real implementation:
        // 1. Store the oc-input backend reference
        // 2. Register pad input callbacks
        // 3. Query connected controllers
        // 4. Set up button/axis mappings
        
        self.input_backend = None; // Would store actual backend
        
        0 // CELL_OK
    }

    /// Poll input from backend
    /// 
    /// Reads current input state from oc-input and updates pad data.
    pub fn poll_input(&mut self) -> i32 {
        if !self.initialized {
            return 0x80121103u32 as i32; // CELL_PAD_ERROR_UNINITIALIZED
        }

        trace!("PadManager::poll_input");

        // In a real implementation, this would:
        // 1. Query oc-input for current controller states
        // 2. Convert oc-input button/axis data to PS3 format
        // 3. Update pad_data for each connected pad
        // 4. Handle button pressure sensitivity
        // 5. Handle analog stick values

        // For HLE, pad data is manually updated via update_pad_data()

        0 // CELL_OK
    }

    /// Map oc-input button to PS3 button
    /// 
    /// Converts button codes between oc-input and PS3 formats.
    pub fn map_button(oc_input_button: u32) -> u16 {
        // In a real implementation, this would map:
        // oc-input button codes -> PS3 button codes
        // 
        // For example:
        // oc_input::PadButtons::CROSS -> CELL_PAD_CTRL_CROSS
        // oc_input::PadButtons::CIRCLE -> CELL_PAD_CTRL_CIRCLE
        // etc.

        trace!("Mapping button: 0x{:08X}", oc_input_button);

        // Return unmapped for now
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

        // In a real implementation, this would:
        // 1. Send rumble commands to oc-input backend
        // 2. Control actual controller motors
        // 3. Handle timing and duration

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
pub fn cell_pad_get_info(_info_addr: u32) -> i32 {
    trace!("cellPadGetInfo()");

    let _info = crate::context::get_hle_context().pad.get_info();
    // TODO: Write info to memory at _info_addr

    0 // CELL_OK
}

/// cellPadGetInfo2 - Get extended pad info
///
/// # Arguments
/// * `info_addr` - Address to write pad info to
///
/// # Returns
/// * 0 on success
pub fn cell_pad_get_info2(_info_addr: u32) -> i32 {
    trace!("cellPadGetInfo2()");

    let _info = crate::context::get_hle_context().pad.get_info();
    // TODO: Write info to memory at _info_addr

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
pub fn cell_pad_get_data(port: u32, _data_addr: u32) -> i32 {
    trace!("cellPadGetData(port={})", port);

    match crate::context::get_hle_context().pad.get_data(port) {
        Ok(_data) => {
            // TODO: Write data to memory at _data_addr
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
pub fn cell_pad_get_capability_info(port: u32, _info_addr: u32) -> i32 {
    trace!("cellPadGetCapabilityInfo(port={})", port);

    match crate::context::get_hle_context().pad.get_capability_info(port) {
        Ok(_info) => {
            // TODO: Write capability info to memory at _info_addr
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
pub fn cell_pad_set_act_param(port: u32, _param_addr: u32) -> i32 {
    debug!("cellPadSetActParam(port={})", port);

    // TODO: Read param from memory at _param_addr
    let param = CellPadActParam::default();
    
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
        assert_eq!(data.unwrap().len, 24);
        
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
}
