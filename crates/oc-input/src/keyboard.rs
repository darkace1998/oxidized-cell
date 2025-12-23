//! Keyboard handling (cellKb)
//!
//! Emulates the PS3's cellKb library for keyboard input.

use bitflags::bitflags;

bitflags! {
    /// Keyboard modifier keys
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KeyModifiers: u32 {
        const LEFT_CTRL   = 0x0001;
        const RIGHT_CTRL  = 0x0002;
        const LEFT_SHIFT  = 0x0004;
        const RIGHT_SHIFT = 0x0008;
        const LEFT_ALT    = 0x0010;
        const RIGHT_ALT   = 0x0020;
        const LEFT_WIN    = 0x0040;
        const RIGHT_WIN   = 0x0080;
    }
}

/// Keyboard key codes (USB HID usage codes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum KeyCode {
    // Letters
    A = 0x04, B = 0x05, C = 0x06, D = 0x07,
    E = 0x08, F = 0x09, G = 0x0A, H = 0x0B,
    I = 0x0C, J = 0x0D, K = 0x0E, L = 0x0F,
    M = 0x10, N = 0x11, O = 0x12, P = 0x13,
    Q = 0x14, R = 0x15, S = 0x16, T = 0x17,
    U = 0x18, V = 0x19, W = 0x1A, X = 0x1B,
    Y = 0x1C, Z = 0x1D,
    
    // Numbers
    Num1 = 0x1E, Num2 = 0x1F, Num3 = 0x20, Num4 = 0x21,
    Num5 = 0x22, Num6 = 0x23, Num7 = 0x24, Num8 = 0x25,
    Num9 = 0x26, Num0 = 0x27,
    
    // Special keys
    Enter = 0x28,
    Escape = 0x29,
    Backspace = 0x2A,
    Tab = 0x2B,
    Space = 0x2C,
    
    // Function keys
    F1 = 0x3A, F2 = 0x3B, F3 = 0x3C, F4 = 0x3D,
    F5 = 0x3E, F6 = 0x3F, F7 = 0x40, F8 = 0x41,
    F9 = 0x42, F10 = 0x43, F11 = 0x44, F12 = 0x45,
    
    // Arrow keys
    Right = 0x4F,
    Left = 0x50,
    Down = 0x51,
    Up = 0x52,
}

/// Keyboard event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    KeyDown,
    KeyUp,
}

/// Keyboard event
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub key_code: u16,
    pub modifiers: KeyModifiers,
    pub event_type: KeyEventType,
}

impl KeyEvent {
    pub fn new(key_code: u16, modifiers: KeyModifiers, event_type: KeyEventType) -> Self {
        Self {
            key_code,
            modifiers,
            event_type,
        }
    }
}

/// Keyboard state
#[derive(Debug, Clone)]
pub struct KeyboardState {
    /// Currently pressed keys (USB HID usage codes)
    pub pressed_keys: Vec<u16>,
    /// Current modifier state
    pub modifiers: KeyModifiers,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self {
            pressed_keys: Vec::new(),
            modifiers: KeyModifiers::empty(),
        }
    }

    pub fn is_key_pressed(&self, key_code: u16) -> bool {
        self.pressed_keys.contains(&key_code)
    }

    pub fn press_key(&mut self, key_code: u16) {
        if !self.is_key_pressed(key_code) {
            self.pressed_keys.push(key_code);
        }
    }

    pub fn release_key(&mut self, key_code: u16) {
        self.pressed_keys.retain(|&k| k != key_code);
    }

    pub fn set_modifiers(&mut self, modifiers: KeyModifiers) {
        self.modifiers = modifiers;
    }

    pub fn clear(&mut self) {
        self.pressed_keys.clear();
        self.modifiers = KeyModifiers::empty();
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

/// Keyboard manager for cellKb
pub struct Keyboard {
    /// Keyboard state
    pub state: KeyboardState,
    /// Connected flag
    pub connected: bool,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            state: KeyboardState::new(),
            connected: false,
        }
    }

    pub fn connect(&mut self) {
        self.connected = true;
        tracing::debug!("Keyboard connected");
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.state.clear();
        tracing::debug!("Keyboard disconnected");
    }

    pub fn process_event(&mut self, event: KeyEvent) {
        if !self.connected {
            return;
        }

        match event.event_type {
            KeyEventType::KeyDown => {
                self.state.press_key(event.key_code);
            }
            KeyEventType::KeyUp => {
                self.state.release_key(event.key_code);
            }
        }
        
        self.state.set_modifiers(event.modifiers);
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_creation() {
        let kb = Keyboard::new();
        assert!(!kb.connected);
    }

    #[test]
    fn test_keyboard_connection() {
        let mut kb = Keyboard::new();
        kb.connect();
        assert!(kb.connected);
        
        kb.disconnect();
        assert!(!kb.connected);
    }

    #[test]
    fn test_key_press() {
        let mut kb = Keyboard::new();
        kb.connect();
        
        let event = KeyEvent::new(
            KeyCode::A as u16,
            KeyModifiers::empty(),
            KeyEventType::KeyDown,
        );
        
        kb.process_event(event);
        assert!(kb.state.is_key_pressed(KeyCode::A as u16));
        
        let event = KeyEvent::new(
            KeyCode::A as u16,
            KeyModifiers::empty(),
            KeyEventType::KeyUp,
        );
        
        kb.process_event(event);
        assert!(!kb.state.is_key_pressed(KeyCode::A as u16));
    }

    #[test]
    fn test_modifiers() {
        let mut state = KeyboardState::new();
        state.set_modifiers(KeyModifiers::LEFT_CTRL | KeyModifiers::LEFT_SHIFT);
        
        assert!(state.modifiers.contains(KeyModifiers::LEFT_CTRL));
        assert!(state.modifiers.contains(KeyModifiers::LEFT_SHIFT));
        assert!(!state.modifiers.contains(KeyModifiers::LEFT_ALT));
    }
}
