//! USB Controller Support
//!
//! Generic USB HID controller support for various gamepads including:
//! - Generic USB HID gamepads
//! - XInput controllers (Xbox 360/One style)
//! - DirectInput controllers
//! - USB adapters for PS3 controllers

use crate::pad::{PadButtons, PadState};
use std::collections::HashMap;

/// USB device vendor/product IDs for known controllers
pub mod known_devices {
    /// Sony DualShock 3 (wired)
    pub const DUALSHOCK3: (u16, u16) = (0x054C, 0x0268);
    /// Sony DualShock 4
    pub const DUALSHOCK4: (u16, u16) = (0x054C, 0x05C4);
    /// Sony DualShock 4 v2
    pub const DUALSHOCK4_V2: (u16, u16) = (0x054C, 0x09CC);
    /// Sony DualSense
    pub const DUALSENSE: (u16, u16) = (0x054C, 0x0CE6);
    /// Xbox 360 Controller
    pub const XBOX360: (u16, u16) = (0x045E, 0x028E);
    /// Xbox One Controller
    pub const XBOX_ONE: (u16, u16) = (0x045E, 0x02D1);
    /// Nintendo Switch Pro Controller
    pub const SWITCH_PRO: (u16, u16) = (0x057E, 0x2009);
    /// 8BitDo Pro 2
    pub const BITDO_PRO2: (u16, u16) = (0x2DC8, 0x6101);
}

/// USB device info
#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device name
    pub name: String,
    /// Manufacturer name
    pub manufacturer: String,
    /// Serial number (if available)
    pub serial: Option<String>,
}

impl UsbDeviceInfo {
    /// Check if this is a known Sony controller
    pub fn is_sony_controller(&self) -> bool {
        self.vendor_id == 0x054C
    }

    /// Check if this is an Xbox controller
    pub fn is_xbox_controller(&self) -> bool {
        self.vendor_id == 0x045E
    }

    /// Get controller type
    pub fn controller_type(&self) -> UsbControllerType {
        match (self.vendor_id, self.product_id) {
            known_devices::DUALSHOCK3 => UsbControllerType::DualShock3,
            known_devices::DUALSHOCK4 | known_devices::DUALSHOCK4_V2 => UsbControllerType::DualShock4,
            known_devices::DUALSENSE => UsbControllerType::DualSense,
            known_devices::XBOX360 => UsbControllerType::Xbox360,
            known_devices::XBOX_ONE => UsbControllerType::XboxOne,
            known_devices::SWITCH_PRO => UsbControllerType::SwitchPro,
            _ => UsbControllerType::GenericHid,
        }
    }
}

/// USB controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbControllerType {
    /// Sony DualShock 3
    DualShock3,
    /// Sony DualShock 4
    DualShock4,
    /// Sony DualSense
    DualSense,
    /// Xbox 360 Controller
    Xbox360,
    /// Xbox One Controller
    XboxOne,
    /// Nintendo Switch Pro
    SwitchPro,
    /// Generic HID gamepad
    GenericHid,
    /// Unknown/unsupported
    Unknown,
}

/// USB HID report descriptor button mapping
#[derive(Debug, Clone, Default)]
pub struct UsbButtonMapping {
    /// Button index to PS3 button mapping
    pub buttons: HashMap<u8, PadButtons>,
    /// Axis index for left stick X
    pub left_x_axis: u8,
    /// Axis index for left stick Y
    pub left_y_axis: u8,
    /// Axis index for right stick X
    pub right_x_axis: u8,
    /// Axis index for right stick Y
    pub right_y_axis: u8,
    /// Axis index for L2 trigger
    pub l2_axis: Option<u8>,
    /// Axis index for R2 trigger
    pub r2_axis: Option<u8>,
    /// D-pad type (hat switch or buttons)
    pub dpad_type: DpadType,
    /// Hat switch axis index (if hat switch)
    pub hat_axis: Option<u8>,
}

/// D-pad input type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DpadType {
    /// Individual buttons for each direction
    #[default]
    Buttons,
    /// Single hat switch
    HatSwitch,
    /// Two axes (X/Y)
    Axes,
}

impl UsbButtonMapping {
    /// Create mapping for DualShock 3/4 style controllers
    pub fn dualshock_mapping() -> Self {
        let mut mapping = Self::default();
        
        // Face buttons
        mapping.buttons.insert(0, PadButtons::CROSS);
        mapping.buttons.insert(1, PadButtons::CIRCLE);
        mapping.buttons.insert(2, PadButtons::SQUARE);
        mapping.buttons.insert(3, PadButtons::TRIANGLE);
        
        // Shoulder buttons
        mapping.buttons.insert(4, PadButtons::L1);
        mapping.buttons.insert(5, PadButtons::R1);
        mapping.buttons.insert(6, PadButtons::L2);
        mapping.buttons.insert(7, PadButtons::R2);
        
        // Special buttons
        mapping.buttons.insert(8, PadButtons::SELECT);
        mapping.buttons.insert(9, PadButtons::START);
        mapping.buttons.insert(10, PadButtons::L3);
        mapping.buttons.insert(11, PadButtons::R3);
        
        // Sticks
        mapping.left_x_axis = 0;
        mapping.left_y_axis = 1;
        mapping.right_x_axis = 2;
        mapping.right_y_axis = 3;
        
        // Triggers as axes
        mapping.l2_axis = Some(4);
        mapping.r2_axis = Some(5);
        
        // D-pad as hat switch
        mapping.dpad_type = DpadType::HatSwitch;
        mapping.hat_axis = Some(0);
        
        mapping
    }

    /// Create mapping for Xbox style controllers
    pub fn xbox_mapping() -> Self {
        let mut mapping = Self::default();
        
        // Face buttons (Xbox layout)
        mapping.buttons.insert(0, PadButtons::CROSS);    // A
        mapping.buttons.insert(1, PadButtons::CIRCLE);   // B
        mapping.buttons.insert(2, PadButtons::SQUARE);   // X
        mapping.buttons.insert(3, PadButtons::TRIANGLE); // Y
        
        // Bumpers
        mapping.buttons.insert(4, PadButtons::L1);
        mapping.buttons.insert(5, PadButtons::R1);
        
        // Special buttons
        mapping.buttons.insert(6, PadButtons::SELECT); // Back
        mapping.buttons.insert(7, PadButtons::START);
        mapping.buttons.insert(8, PadButtons::L3);
        mapping.buttons.insert(9, PadButtons::R3);
        
        // Sticks
        mapping.left_x_axis = 0;
        mapping.left_y_axis = 1;
        mapping.right_x_axis = 2;
        mapping.right_y_axis = 3;
        
        // Triggers
        mapping.l2_axis = Some(4);
        mapping.r2_axis = Some(5);
        
        // D-pad
        mapping.dpad_type = DpadType::HatSwitch;
        mapping.hat_axis = Some(0);
        
        mapping
    }

    /// Create generic HID mapping
    pub fn generic_mapping() -> Self {
        Self::dualshock_mapping()
    }
}

/// USB controller state
#[derive(Debug, Clone)]
pub struct UsbControllerState {
    /// Raw button states (up to 32 buttons)
    pub raw_buttons: u32,
    /// Raw axis values (up to 8 axes)
    pub raw_axes: [i16; 8],
    /// Hat switch value (0-8, 8 = centered)
    pub hat_value: u8,
    /// Mapped PS3 pad state
    pub pad_state: PadState,
}

impl Default for UsbControllerState {
    fn default() -> Self {
        Self {
            raw_buttons: 0,
            raw_axes: [0; 8],
            hat_value: 8,
            pad_state: PadState::new(),
        }
    }
}

/// USB controller instance
#[derive(Debug)]
pub struct UsbController {
    /// Device info
    pub info: UsbDeviceInfo,
    /// Controller type
    pub controller_type: UsbControllerType,
    /// Button/axis mapping
    pub mapping: UsbButtonMapping,
    /// Current state
    pub state: UsbControllerState,
    /// Assigned PS3 port (-1 = not assigned)
    pub ps3_port: i8,
    /// Is connected
    pub connected: bool,
}

impl UsbController {
    /// Create a new USB controller
    pub fn new(info: UsbDeviceInfo) -> Self {
        let controller_type = info.controller_type();
        let mapping = match controller_type {
            UsbControllerType::DualShock3 |
            UsbControllerType::DualShock4 |
            UsbControllerType::DualSense => UsbButtonMapping::dualshock_mapping(),
            UsbControllerType::Xbox360 |
            UsbControllerType::XboxOne => UsbButtonMapping::xbox_mapping(),
            _ => UsbButtonMapping::generic_mapping(),
        };

        Self {
            info,
            controller_type,
            mapping,
            state: UsbControllerState::default(),
            ps3_port: -1,
            connected: true,
        }
    }

    /// Update raw input data
    pub fn update_raw(&mut self, buttons: u32, axes: &[i16], hat: u8) {
        self.state.raw_buttons = buttons;
        for (i, &axis) in axes.iter().enumerate().take(8) {
            self.state.raw_axes[i] = axis;
        }
        self.state.hat_value = hat;
        
        // Apply mapping to get PS3 state
        self.apply_mapping();
    }

    /// Apply button/axis mapping to produce PS3 pad state
    fn apply_mapping(&mut self) {
        let mut pad = PadState::new();

        // Map buttons
        for (&usb_btn, &ps3_btn) in &self.mapping.buttons {
            let pressed = (self.state.raw_buttons & (1 << usb_btn)) != 0;
            pad.set_button(ps3_btn, pressed);
        }

        // Map D-pad from hat switch
        if self.mapping.dpad_type == DpadType::HatSwitch {
            match self.state.hat_value {
                0 => pad.set_button(PadButtons::DPAD_UP, true),
                1 => {
                    pad.set_button(PadButtons::DPAD_UP, true);
                    pad.set_button(PadButtons::DPAD_RIGHT, true);
                }
                2 => pad.set_button(PadButtons::DPAD_RIGHT, true),
                3 => {
                    pad.set_button(PadButtons::DPAD_DOWN, true);
                    pad.set_button(PadButtons::DPAD_RIGHT, true);
                }
                4 => pad.set_button(PadButtons::DPAD_DOWN, true),
                5 => {
                    pad.set_button(PadButtons::DPAD_DOWN, true);
                    pad.set_button(PadButtons::DPAD_LEFT, true);
                }
                6 => pad.set_button(PadButtons::DPAD_LEFT, true),
                7 => {
                    pad.set_button(PadButtons::DPAD_UP, true);
                    pad.set_button(PadButtons::DPAD_LEFT, true);
                }
                _ => {} // Centered
            }
        }

        // Map analog sticks (convert from -32768..32767 to 0..255)
        pad.left_x = Self::axis_to_u8(self.state.raw_axes[self.mapping.left_x_axis as usize]);
        pad.left_y = Self::axis_to_u8(self.state.raw_axes[self.mapping.left_y_axis as usize]);
        pad.right_x = Self::axis_to_u8(self.state.raw_axes[self.mapping.right_x_axis as usize]);
        pad.right_y = Self::axis_to_u8(self.state.raw_axes[self.mapping.right_y_axis as usize]);

        // Map triggers if available
        if let Some(l2_axis) = self.mapping.l2_axis {
            let value = Self::axis_to_u8(self.state.raw_axes[l2_axis as usize]);
            if value > 128 {
                pad.set_button(PadButtons::L2, true);
                pad.pressure[4] = value;
            }
        }
        if let Some(r2_axis) = self.mapping.r2_axis {
            let value = Self::axis_to_u8(self.state.raw_axes[r2_axis as usize]);
            if value > 128 {
                pad.set_button(PadButtons::R2, true);
                pad.pressure[5] = value;
            }
        }

        self.state.pad_state = pad;
    }

    /// Convert axis value (-32768..32767) to u8 (0..255)
    fn axis_to_u8(axis: i16) -> u8 {
        ((axis as i32 + 32768) >> 8) as u8
    }

    /// Get the mapped PS3 pad state
    pub fn get_pad_state(&self) -> &PadState {
        &self.state.pad_state
    }
}

/// USB controller manager
pub struct UsbControllerManager {
    /// Connected controllers
    controllers: Vec<UsbController>,
    /// Port assignments (PS3 port -> controller index)
    port_assignments: [Option<usize>; 7],
}

impl UsbControllerManager {
    /// Create a new USB controller manager
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
            port_assignments: [None; 7],
        }
    }

    /// Add a controller
    pub fn add_controller(&mut self, info: UsbDeviceInfo) -> usize {
        let index = self.controllers.len();
        self.controllers.push(UsbController::new(info));
        
        // Auto-assign to first free port
        for (port, assignment) in self.port_assignments.iter_mut().enumerate() {
            if assignment.is_none() {
                *assignment = Some(index);
                self.controllers[index].ps3_port = port as i8;
                break;
            }
        }
        
        index
    }

    /// Remove a controller by index
    pub fn remove_controller(&mut self, index: usize) {
        if index < self.controllers.len() {
            let port = self.controllers[index].ps3_port;
            if port >= 0 && (port as usize) < 7 {
                self.port_assignments[port as usize] = None;
            }
            self.controllers.remove(index);
            
            // Update indices in port assignments
            for idx in self.port_assignments.iter_mut().flatten() {
                if *idx > index {
                    *idx -= 1;
                }
            }
        }
    }

    /// Get controller by USB index
    pub fn get(&self, index: usize) -> Option<&UsbController> {
        self.controllers.get(index)
    }

    /// Get mutable controller by USB index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut UsbController> {
        self.controllers.get_mut(index)
    }

    /// Get controller assigned to PS3 port
    pub fn get_by_port(&self, port: u8) -> Option<&UsbController> {
        let index = self.port_assignments.get(port as usize)?.as_ref()?;
        self.controllers.get(*index)
    }

    /// Get number of connected controllers
    pub fn count(&self) -> usize {
        self.controllers.len()
    }

    /// Assign controller to PS3 port
    pub fn assign_port(&mut self, controller_index: usize, ps3_port: u8) -> bool {
        if controller_index >= self.controllers.len() || ps3_port >= 7 {
            return false;
        }

        // Unassign from old port
        let old_port = self.controllers[controller_index].ps3_port;
        if old_port >= 0 {
            self.port_assignments[old_port as usize] = None;
        }

        // Unassign old controller from new port
        if let Some(old_index) = self.port_assignments[ps3_port as usize] {
            self.controllers[old_index].ps3_port = -1;
        }

        // Assign new
        self.port_assignments[ps3_port as usize] = Some(controller_index);
        self.controllers[controller_index].ps3_port = ps3_port as i8;
        
        true
    }

    /// Enumerate connected USB devices (stub - would use system USB API)
    pub fn enumerate_devices(&mut self) -> Vec<UsbDeviceInfo> {
        // In a real implementation, this would:
        // 1. Use libusb or platform-specific API to enumerate USB devices
        // 2. Filter for HID game controllers
        // 3. Return device info for each
        
        Vec::new()
    }

    /// Poll for device changes
    pub fn poll_devices(&mut self) {
        // Would check for connect/disconnect events
    }
}

impl Default for UsbControllerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_info() {
        let info = UsbDeviceInfo {
            vendor_id: 0x054C,
            product_id: 0x0268,
            name: "PLAYSTATION(R)3 Controller".to_string(),
            manufacturer: "Sony".to_string(),
            serial: None,
        };

        assert!(info.is_sony_controller());
        assert_eq!(info.controller_type(), UsbControllerType::DualShock3);
    }

    #[test]
    fn test_controller_mapping() {
        let info = UsbDeviceInfo {
            vendor_id: 0x054C,
            product_id: 0x0268,
            name: "Test Controller".to_string(),
            manufacturer: "Test".to_string(),
            serial: None,
        };

        let mut controller = UsbController::new(info);
        
        // Simulate pressing X button (button 0 in DualShock mapping)
        controller.update_raw(0x01, &[0; 8], 8);
        
        assert!(controller.get_pad_state().is_button_pressed(PadButtons::CROSS));
    }

    #[test]
    fn test_manager() {
        let mut manager = UsbControllerManager::new();
        
        let info = UsbDeviceInfo {
            vendor_id: 0x054C,
            product_id: 0x0268,
            name: "Test".to_string(),
            manufacturer: "Test".to_string(),
            serial: None,
        };

        let index = manager.add_controller(info);
        assert_eq!(manager.count(), 1);
        
        // Should be auto-assigned to port 0
        assert!(manager.get_by_port(0).is_some());
        
        manager.remove_controller(index);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_axis_conversion() {
        assert_eq!(UsbController::axis_to_u8(-32768), 0);
        assert_eq!(UsbController::axis_to_u8(0), 128);
        assert_eq!(UsbController::axis_to_u8(32767), 255);
    }
}
