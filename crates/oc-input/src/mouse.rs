//! Mouse handling (cellMouse)
//!
//! Emulates the PS3's cellMouse library for mouse input.

use bitflags::bitflags;

bitflags! {
    /// Mouse button flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MouseButtons: u8 {
        const LEFT   = 0x01;
        const RIGHT  = 0x02;
        const MIDDLE = 0x04;
        const BUTTON4 = 0x08;
        const BUTTON5 = 0x10;
    }
}

/// Mouse event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventType {
    Move,
    ButtonDown,
    ButtonUp,
    Wheel,
}

/// Mouse event
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub event_type: MouseEventType,
    pub x: i32,
    pub y: i32,
    pub buttons: MouseButtons,
    pub wheel_delta: i8,
}

impl MouseEvent {
    pub fn new_move(x: i32, y: i32) -> Self {
        Self {
            event_type: MouseEventType::Move,
            x,
            y,
            buttons: MouseButtons::empty(),
            wheel_delta: 0,
        }
    }

    pub fn new_button(event_type: MouseEventType, x: i32, y: i32, buttons: MouseButtons) -> Self {
        Self {
            event_type,
            x,
            y,
            buttons,
            wheel_delta: 0,
        }
    }

    pub fn new_wheel(x: i32, y: i32, delta: i8) -> Self {
        Self {
            event_type: MouseEventType::Wheel,
            x,
            y,
            buttons: MouseButtons::empty(),
            wheel_delta: delta,
        }
    }
}

/// Mouse state
#[derive(Debug, Clone)]
pub struct MouseState {
    /// Current X position
    pub x: i32,
    /// Current Y position
    pub y: i32,
    /// Button state
    pub buttons: MouseButtons,
    /// Wheel position (accumulated)
    pub wheel: i32,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            buttons: MouseButtons::empty(),
            wheel: 0,
        }
    }

    pub fn is_button_pressed(&self, button: MouseButtons) -> bool {
        self.buttons.contains(button)
    }

    pub fn set_button(&mut self, button: MouseButtons, pressed: bool) {
        if pressed {
            self.buttons.insert(button);
        } else {
            self.buttons.remove(button);
        }
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn add_wheel_delta(&mut self, delta: i8) {
        self.wheel = self.wheel.saturating_add(delta as i32);
    }
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

/// Mouse manager for cellMouse
pub struct Mouse {
    /// Mouse state
    pub state: MouseState,
    /// Connected flag
    pub connected: bool,
}

impl Mouse {
    pub fn new() -> Self {
        Self {
            state: MouseState::new(),
            connected: false,
        }
    }

    pub fn connect(&mut self) {
        self.connected = true;
        tracing::debug!("Mouse connected");
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.state = MouseState::new();
        tracing::debug!("Mouse disconnected");
    }

    pub fn process_event(&mut self, event: MouseEvent) {
        if !self.connected {
            return;
        }

        match event.event_type {
            MouseEventType::Move => {
                self.state.set_position(event.x, event.y);
            }
            MouseEventType::ButtonDown => {
                self.state.set_position(event.x, event.y);
                self.state.buttons = event.buttons;
            }
            MouseEventType::ButtonUp => {
                self.state.set_position(event.x, event.y);
                self.state.buttons = event.buttons;
            }
            MouseEventType::Wheel => {
                self.state.add_wheel_delta(event.wheel_delta);
            }
        }
    }
}

impl Default for Mouse {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_creation() {
        let mouse = Mouse::new();
        assert!(!mouse.connected);
    }

    #[test]
    fn test_mouse_connection() {
        let mut mouse = Mouse::new();
        mouse.connect();
        assert!(mouse.connected);
        
        mouse.disconnect();
        assert!(!mouse.connected);
    }

    #[test]
    fn test_mouse_move() {
        let mut mouse = Mouse::new();
        mouse.connect();
        
        let event = MouseEvent::new_move(100, 200);
        mouse.process_event(event);
        
        assert_eq!(mouse.state.x, 100);
        assert_eq!(mouse.state.y, 200);
    }

    #[test]
    fn test_mouse_buttons() {
        let mut state = MouseState::new();
        
        state.set_button(MouseButtons::LEFT, true);
        assert!(state.is_button_pressed(MouseButtons::LEFT));
        assert!(!state.is_button_pressed(MouseButtons::RIGHT));
        
        state.set_button(MouseButtons::LEFT, false);
        assert!(!state.is_button_pressed(MouseButtons::LEFT));
    }

    #[test]
    fn test_mouse_wheel() {
        let mut mouse = Mouse::new();
        mouse.connect();
        
        let event = MouseEvent::new_wheel(0, 0, 5);
        mouse.process_event(event);
        assert_eq!(mouse.state.wheel, 5);
        
        let event = MouseEvent::new_wheel(0, 0, -3);
        mouse.process_event(event);
        assert_eq!(mouse.state.wheel, 2);
    }
}
