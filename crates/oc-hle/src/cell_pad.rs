//! cellPad HLE - Controller Input
//!
//! This module provides HLE implementations for PS3 controller input.
//! It bridges to the oc-input subsystem.

use tracing::{debug, trace};

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
pub struct CellPadData {
    /// Length of valid data
    pub len: i32,
    /// Digital button data (16 bits)
    pub button: [u16; 2],
}

impl Default for CellPadData {
    fn default() -> Self {
        Self {
            len: 0,
            button: [0; 2],
        }
    }
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
}

impl PadManager {
    /// Create a new pad manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            connected_pads: 0,
            device_types: [CellPadDeviceType::Standard; CELL_PAD_MAX_PORT_NUM],
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

        // TODO: Get actual pad data from oc-input subsystem
        // For now, return empty data
        let mut data = CellPadData::default();
        data.len = 24; // Standard data length

        Ok(data)
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
        
        debug!("Disconnected pad on port {}", port);
        
        0 // CELL_OK
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
}
