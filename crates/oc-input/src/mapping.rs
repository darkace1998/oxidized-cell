//! Input mapping
//!
//! Maps host input devices to PS3 controller/keyboard/mouse inputs.

use crate::pad::{PadButtons, PadState};
use crate::keyboard::KeyCode;
use crate::mouse::MouseButtons;
use std::collections::HashMap;

/// Host input source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostInput {
    /// Keyboard key
    Key(u16),
    /// Mouse button
    MouseButton(MouseButtons),
    /// Gamepad button
    GamepadButton(u8),
}

/// PS3 input target
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ps3Input {
    /// Controller button
    PadButton(PadButtons),
    /// Left analog X axis
    LeftAnalogX,
    /// Left analog Y axis
    LeftAnalogY,
    /// Right analog X axis
    RightAnalogX,
    /// Right analog Y axis
    RightAnalogY,
}

/// Input mapping configuration
pub struct InputMapping {
    /// Mappings from host input to PS3 input
    mappings: HashMap<HostInput, Ps3Input>,
}

impl InputMapping {
    /// Create a new input mapping
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Create default keyboard to controller mapping
    pub fn default_keyboard_mapping() -> Self {
        let mut mapping = Self::new();
        
        // D-pad
        mapping.map_key(KeyCode::Up as u16, Ps3Input::PadButton(PadButtons::DPAD_UP));
        mapping.map_key(KeyCode::Down as u16, Ps3Input::PadButton(PadButtons::DPAD_DOWN));
        mapping.map_key(KeyCode::Left as u16, Ps3Input::PadButton(PadButtons::DPAD_LEFT));
        mapping.map_key(KeyCode::Right as u16, Ps3Input::PadButton(PadButtons::DPAD_RIGHT));
        
        // Face buttons (using typical PC game layout)
        mapping.map_key(KeyCode::Z as u16, Ps3Input::PadButton(PadButtons::CROSS));
        mapping.map_key(KeyCode::X as u16, Ps3Input::PadButton(PadButtons::CIRCLE));
        mapping.map_key(KeyCode::C as u16, Ps3Input::PadButton(PadButtons::SQUARE));
        mapping.map_key(KeyCode::V as u16, Ps3Input::PadButton(PadButtons::TRIANGLE));
        
        // Shoulder buttons
        mapping.map_key(KeyCode::Q as u16, Ps3Input::PadButton(PadButtons::L1));
        mapping.map_key(KeyCode::E as u16, Ps3Input::PadButton(PadButtons::R1));
        mapping.map_key(KeyCode::A as u16, Ps3Input::PadButton(PadButtons::L2));
        mapping.map_key(KeyCode::D as u16, Ps3Input::PadButton(PadButtons::R2));
        
        // Special buttons
        mapping.map_key(KeyCode::Enter as u16, Ps3Input::PadButton(PadButtons::START));
        mapping.map_key(KeyCode::Backspace as u16, Ps3Input::PadButton(PadButtons::SELECT));
        
        mapping
    }

    /// Map a keyboard key to a PS3 input
    pub fn map_key(&mut self, key_code: u16, ps3_input: Ps3Input) {
        self.mappings.insert(HostInput::Key(key_code), ps3_input);
    }

    /// Map a mouse button to a PS3 input
    pub fn map_mouse_button(&mut self, button: MouseButtons, ps3_input: Ps3Input) {
        self.mappings.insert(HostInput::MouseButton(button), ps3_input);
    }

    /// Map a gamepad button to a PS3 input
    pub fn map_gamepad_button(&mut self, button: u8, ps3_input: Ps3Input) {
        self.mappings.insert(HostInput::GamepadButton(button), ps3_input);
    }

    /// Get PS3 input for a host input
    pub fn get_mapping(&self, host_input: HostInput) -> Option<Ps3Input> {
        self.mappings.get(&host_input).copied()
    }

    /// Remove a mapping
    pub fn remove_mapping(&mut self, host_input: HostInput) {
        self.mappings.remove(&host_input);
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.mappings.clear();
    }

    /// Apply keyboard state to pad state
    pub fn apply_keyboard_to_pad(&self, pressed_keys: &[u16], pad_state: &mut PadState) {
        for &key in pressed_keys {
            if let Some(Ps3Input::PadButton(button)) = self.get_mapping(HostInput::Key(key)) {
                pad_state.set_button(button, true);
            }
        }
    }

    /// Get all mappings
    pub fn get_all_mappings(&self) -> &HashMap<HostInput, Ps3Input> {
        &self.mappings
    }
}

impl Default for InputMapping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_mapping_creation() {
        let mapping = InputMapping::new();
        assert_eq!(mapping.mappings.len(), 0);
    }

    #[test]
    fn test_map_key() {
        let mut mapping = InputMapping::new();
        mapping.map_key(KeyCode::Space as u16, Ps3Input::PadButton(PadButtons::CROSS));
        
        let result = mapping.get_mapping(HostInput::Key(KeyCode::Space as u16));
        assert!(result.is_some());
    }

    #[test]
    fn test_default_keyboard_mapping() {
        let mapping = InputMapping::default_keyboard_mapping();
        assert!(mapping.mappings.len() > 0);
        
        // Test a few default mappings
        let up = mapping.get_mapping(HostInput::Key(KeyCode::Up as u16));
        assert_eq!(up, Some(Ps3Input::PadButton(PadButtons::DPAD_UP)));
    }

    #[test]
    fn test_apply_keyboard_to_pad() {
        let mapping = InputMapping::default_keyboard_mapping();
        let mut pad_state = PadState::new();
        
        let keys = vec![KeyCode::Z as u16, KeyCode::Up as u16];
        mapping.apply_keyboard_to_pad(&keys, &mut pad_state);
        
        assert!(pad_state.is_button_pressed(PadButtons::CROSS));
        assert!(pad_state.is_button_pressed(PadButtons::DPAD_UP));
    }

    #[test]
    fn test_remove_mapping() {
        let mut mapping = InputMapping::new();
        mapping.map_key(KeyCode::A as u16, Ps3Input::PadButton(PadButtons::CROSS));
        
        mapping.remove_mapping(HostInput::Key(KeyCode::A as u16));
        assert!(mapping.get_mapping(HostInput::Key(KeyCode::A as u16)).is_none());
    }
}
