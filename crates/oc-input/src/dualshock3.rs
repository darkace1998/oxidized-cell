//! DualShock 3 (Sixaxis) Controller Emulation
//!
//! Full emulation of the PS3 DualShock 3 controller including:
//! - Sixaxis motion sensors (accelerometer + gyroscope)
//! - Vibration/rumble feedback (dual motor)
//! - Pressure-sensitive buttons
//! - USB and Bluetooth connection modes

use crate::pad::{PadButtons, PadState};
use std::time::{Duration, Instant};

/// Sixaxis motion sensor data
#[derive(Debug, Clone, Copy, Default)]
pub struct SixaxisData {
    /// Accelerometer X axis (-512 to 511, 0 = level)
    pub accel_x: i16,
    /// Accelerometer Y axis (-512 to 511, 0 = level)
    pub accel_y: i16,
    /// Accelerometer Z axis (-512 to 511, ~511 = 1G at rest pointing up)
    pub accel_z: i16,
    /// Gyroscope X axis (roll rate)
    pub gyro_x: i16,
    /// Gyroscope Y axis (pitch rate)  
    pub gyro_y: i16,
    /// Gyroscope Z axis (yaw rate)
    pub gyro_z: i16,
}

impl SixaxisData {
    /// Create new sixaxis data at rest (level, facing up)
    pub fn at_rest() -> Self {
        Self {
            accel_x: 0,
            accel_y: 0,
            accel_z: 511, // 1G pointing up
            gyro_x: 0,
            gyro_y: 0,
            gyro_z: 0,
        }
    }

    /// Set accelerometer from normalized values (-1.0 to 1.0)
    pub fn set_accel_normalized(&mut self, x: f32, y: f32, z: f32) {
        self.accel_x = (x * 511.0).clamp(-512.0, 511.0) as i16;
        self.accel_y = (y * 511.0).clamp(-512.0, 511.0) as i16;
        self.accel_z = (z * 511.0).clamp(-512.0, 511.0) as i16;
    }

    /// Set gyroscope from normalized angular velocity
    pub fn set_gyro_normalized(&mut self, x: f32, y: f32, z: f32) {
        self.gyro_x = (x * 32767.0).clamp(-32768.0, 32767.0) as i16;
        self.gyro_y = (y * 32767.0).clamp(-32768.0, 32767.0) as i16;
        self.gyro_z = (z * 32767.0).clamp(-32768.0, 32767.0) as i16;
    }

    /// Get pitch angle in degrees (estimated from accelerometer)
    pub fn get_pitch(&self) -> f32 {
        let x = self.accel_x as f32 / 511.0;
        let z = self.accel_z as f32 / 511.0;
        x.atan2(z).to_degrees()
    }

    /// Get roll angle in degrees (estimated from accelerometer)
    pub fn get_roll(&self) -> f32 {
        let y = self.accel_y as f32 / 511.0;
        let z = self.accel_z as f32 / 511.0;
        y.atan2(z).to_degrees()
    }
}

/// Vibration motor state
#[derive(Debug, Clone, Copy, Default)]
pub struct VibrationState {
    /// Small motor (high frequency, 0 or 1)
    pub small_motor: u8,
    /// Large motor (low frequency, 0-255 intensity)
    pub large_motor: u8,
}

impl VibrationState {
    /// Create new vibration state (both motors off)
    pub fn new() -> Self {
        Self::default()
    }

    /// Set small motor (on/off)
    pub fn set_small_motor(&mut self, on: bool) {
        self.small_motor = if on { 1 } else { 0 };
    }

    /// Set large motor intensity (0-255)
    pub fn set_large_motor(&mut self, intensity: u8) {
        self.large_motor = intensity;
    }

    /// Check if any motor is active
    pub fn is_active(&self) -> bool {
        self.small_motor > 0 || self.large_motor > 0
    }

    /// Stop all motors
    pub fn stop(&mut self) {
        self.small_motor = 0;
        self.large_motor = 0;
    }
}

/// Vibration effect preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VibrationEffect {
    /// Light rumble (small motor only)
    Light,
    /// Medium rumble (large motor at 50%)
    Medium,
    /// Heavy rumble (both motors full)
    Heavy,
    /// Impact effect (short burst)
    Impact,
    /// Continuous rumble
    Continuous,
    /// Custom effect
    Custom { small: u8, large: u8 },
}

/// DualShock 3 connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// USB wired connection
    Usb,
    /// Bluetooth wireless connection
    Bluetooth,
    /// Not connected
    Disconnected,
}

/// DualShock 3 LED state (player indicator)
#[derive(Debug, Clone, Copy, Default)]
pub struct LedState {
    /// LED 1 (Player 1)
    pub led1: bool,
    /// LED 2 (Player 2)
    pub led2: bool,
    /// LED 3 (Player 3)
    pub led3: bool,
    /// LED 4 (Player 4)
    pub led4: bool,
    /// LED blink rate (0 = solid, higher = faster blink)
    pub blink_rate: u8,
}

impl LedState {
    /// Set player number (1-4)
    pub fn set_player(&mut self, player: u8) {
        self.led1 = player >= 1;
        self.led2 = player >= 2;
        self.led3 = player >= 3;
        self.led4 = player >= 4;
    }

    /// Set charging animation
    pub fn set_charging(&mut self) {
        self.led1 = true;
        self.led2 = false;
        self.led3 = false;
        self.led4 = false;
        self.blink_rate = 2;
    }
}

/// Button pressure sensitivity indices
pub mod pressure_index {
    pub const DPAD_UP: usize = 0;
    pub const DPAD_RIGHT: usize = 1;
    pub const DPAD_DOWN: usize = 2;
    pub const DPAD_LEFT: usize = 3;
    pub const L2: usize = 4;
    pub const R2: usize = 5;
    pub const L1: usize = 6;
    pub const R1: usize = 7;
    pub const TRIANGLE: usize = 8;
    pub const CIRCLE: usize = 9;
    pub const CROSS: usize = 10;
    pub const SQUARE: usize = 11;
}

/// Full DualShock 3 controller state
#[derive(Debug, Clone)]
pub struct DualShock3 {
    /// Controller port (0-6)
    pub port: u8,
    /// Basic pad state (buttons, analogs)
    pub pad: PadState,
    /// Sixaxis motion data
    pub sixaxis: SixaxisData,
    /// Vibration state
    pub vibration: VibrationState,
    /// LED state
    pub leds: LedState,
    /// Connection mode
    pub connection: ConnectionMode,
    /// Battery level (0-100)
    pub battery_level: u8,
    /// Is charging
    pub is_charging: bool,
    /// Sixaxis enabled
    pub sixaxis_enabled: bool,
    /// Vibration enabled
    pub vibration_enabled: bool,
    /// Last update timestamp
    last_update: Instant,
    /// Vibration effect end time
    vibration_end: Option<Instant>,
}

impl DualShock3 {
    /// Create a new DualShock 3 controller
    pub fn new(port: u8) -> Self {
        let mut leds = LedState::default();
        leds.set_player(port + 1);

        Self {
            port,
            pad: PadState::new(),
            sixaxis: SixaxisData::at_rest(),
            vibration: VibrationState::new(),
            leds,
            connection: ConnectionMode::Disconnected,
            battery_level: 100,
            is_charging: false,
            sixaxis_enabled: true,
            vibration_enabled: true,
            last_update: Instant::now(),
            vibration_end: None,
        }
    }

    /// Connect the controller
    pub fn connect(&mut self, mode: ConnectionMode) {
        self.connection = mode;
        self.last_update = Instant::now();
        tracing::info!("DualShock 3 port {} connected via {:?}", self.port, mode);
    }

    /// Disconnect the controller
    pub fn disconnect(&mut self) {
        self.connection = ConnectionMode::Disconnected;
        self.pad = PadState::new();
        self.sixaxis = SixaxisData::at_rest();
        self.vibration.stop();
        tracing::info!("DualShock 3 port {} disconnected", self.port);
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connection != ConnectionMode::Disconnected
    }

    /// Update controller state (call each frame)
    pub fn update(&mut self) {
        self.last_update = Instant::now();

        // Check if timed vibration effect has ended
        if let Some(end_time) = self.vibration_end {
            if Instant::now() >= end_time {
                self.vibration.stop();
                self.vibration_end = None;
            }
        }

        // Simulate battery drain for bluetooth
        if self.connection == ConnectionMode::Bluetooth && !self.is_charging {
            // Very slow drain for simulation
            if self.battery_level > 0 {
                // Drain 1% every ~10 minutes of play (simulated)
            }
        }
    }

    /// Set button with pressure
    pub fn set_button_pressure(&mut self, button: PadButtons, pressure: u8) {
        let pressed = pressure > 0;
        self.pad.set_button(button, pressed);

        // Map button to pressure index
        let pressure_idx = match button {
            PadButtons::DPAD_UP => Some(pressure_index::DPAD_UP),
            PadButtons::DPAD_RIGHT => Some(pressure_index::DPAD_RIGHT),
            PadButtons::DPAD_DOWN => Some(pressure_index::DPAD_DOWN),
            PadButtons::DPAD_LEFT => Some(pressure_index::DPAD_LEFT),
            PadButtons::L2 => Some(pressure_index::L2),
            PadButtons::R2 => Some(pressure_index::R2),
            PadButtons::L1 => Some(pressure_index::L1),
            PadButtons::R1 => Some(pressure_index::R1),
            PadButtons::TRIANGLE => Some(pressure_index::TRIANGLE),
            PadButtons::CIRCLE => Some(pressure_index::CIRCLE),
            PadButtons::CROSS => Some(pressure_index::CROSS),
            PadButtons::SQUARE => Some(pressure_index::SQUARE),
            _ => None,
        };

        if let Some(idx) = pressure_idx {
            self.pad.pressure[idx] = pressure;
        }
    }

    /// Apply vibration effect
    pub fn vibrate(&mut self, effect: VibrationEffect, duration: Option<Duration>) {
        if !self.vibration_enabled {
            return;
        }

        match effect {
            VibrationEffect::Light => {
                self.vibration.set_small_motor(true);
                self.vibration.set_large_motor(0);
            }
            VibrationEffect::Medium => {
                self.vibration.set_small_motor(false);
                self.vibration.set_large_motor(128);
            }
            VibrationEffect::Heavy => {
                self.vibration.set_small_motor(true);
                self.vibration.set_large_motor(255);
            }
            VibrationEffect::Impact => {
                self.vibration.set_small_motor(true);
                self.vibration.set_large_motor(255);
            }
            VibrationEffect::Continuous => {
                self.vibration.set_small_motor(false);
                self.vibration.set_large_motor(192);
            }
            VibrationEffect::Custom { small, large } => {
                self.vibration.small_motor = small;
                self.vibration.large_motor = large;
            }
        }

        // Set effect end time
        if let Some(dur) = duration {
            self.vibration_end = Some(Instant::now() + dur);
        } else if effect == VibrationEffect::Impact {
            // Impact is always short
            self.vibration_end = Some(Instant::now() + Duration::from_millis(100));
        } else {
            self.vibration_end = None;
        }
    }

    /// Stop vibration
    pub fn stop_vibration(&mut self) {
        self.vibration.stop();
        self.vibration_end = None;
    }

    /// Set sixaxis data from host motion sensor
    pub fn set_motion(&mut self, accel_x: f32, accel_y: f32, accel_z: f32, 
                      gyro_x: f32, gyro_y: f32, gyro_z: f32) {
        if !self.sixaxis_enabled {
            return;
        }
        self.sixaxis.set_accel_normalized(accel_x, accel_y, accel_z);
        self.sixaxis.set_gyro_normalized(gyro_x, gyro_y, gyro_z);
    }

    /// Get raw data for cellPad report
    pub fn get_pad_data(&self) -> PadData {
        PadData {
            buttons: self.pad.buttons,
            left_x: self.pad.left_x,
            left_y: self.pad.left_y,
            right_x: self.pad.right_x,
            right_y: self.pad.right_y,
            pressure: self.pad.pressure,
            accel_x: self.sixaxis.accel_x,
            accel_y: self.sixaxis.accel_y,
            accel_z: self.sixaxis.accel_z,
            gyro_z: self.sixaxis.gyro_z,
        }
    }
}

/// Raw pad data for cellPad
#[derive(Debug, Clone, Copy)]
pub struct PadData {
    pub buttons: u32,
    pub left_x: u8,
    pub left_y: u8,
    pub right_x: u8,
    pub right_y: u8,
    pub pressure: [u8; 12],
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_z: i16,
}

/// DualShock 3 manager for multiple controllers
pub struct DualShock3Manager {
    /// Controllers (up to 7)
    controllers: [Option<DualShock3>; 7],
    /// Maximum controllers allowed
    _max_controllers: u8,
}

impl DualShock3Manager {
    /// Create a new manager
    pub fn new(max_controllers: u8) -> Self {
        Self {
            controllers: Default::default(),
            _max_controllers: max_controllers.min(7),
        }
    }

    /// Connect a controller to a port
    pub fn connect(&mut self, port: u8, mode: ConnectionMode) -> Result<(), &'static str> {
        if port as usize >= self.controllers.len() {
            return Err("Invalid port number");
        }
        if self.controllers[port as usize].is_some() {
            return Err("Port already in use");
        }

        let mut controller = DualShock3::new(port);
        controller.connect(mode);
        self.controllers[port as usize] = Some(controller);
        Ok(())
    }

    /// Disconnect a controller
    pub fn disconnect(&mut self, port: u8) {
        if let Some(controller) = self.controllers.get_mut(port as usize) {
            *controller = None;
        }
    }

    /// Get a controller
    pub fn get(&self, port: u8) -> Option<&DualShock3> {
        self.controllers.get(port as usize)?.as_ref()
    }

    /// Get a mutable controller
    pub fn get_mut(&mut self, port: u8) -> Option<&mut DualShock3> {
        self.controllers.get_mut(port as usize)?.as_mut()
    }

    /// Get number of connected controllers
    pub fn connected_count(&self) -> u8 {
        self.controllers.iter().filter(|c| c.is_some()).count() as u8
    }

    /// Update all controllers
    pub fn update(&mut self) {
        for controller in self.controllers.iter_mut().flatten() {
            controller.update();
        }
    }

    /// Iterate over connected controllers
    pub fn iter(&self) -> impl Iterator<Item = &DualShock3> {
        self.controllers.iter().filter_map(|c| c.as_ref())
    }

    /// Iterate over connected controllers mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut DualShock3> {
        self.controllers.iter_mut().filter_map(|c| c.as_mut())
    }
}

impl Default for DualShock3Manager {
    fn default() -> Self {
        Self::new(7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sixaxis_at_rest() {
        let sixaxis = SixaxisData::at_rest();
        assert_eq!(sixaxis.accel_x, 0);
        assert_eq!(sixaxis.accel_y, 0);
        assert_eq!(sixaxis.accel_z, 511);
    }

    #[test]
    fn test_vibration_effects() {
        let mut ds3 = DualShock3::new(0);
        ds3.connect(ConnectionMode::Usb);

        ds3.vibrate(VibrationEffect::Heavy, None);
        assert!(ds3.vibration.is_active());

        ds3.stop_vibration();
        assert!(!ds3.vibration.is_active());
    }

    #[test]
    fn test_button_pressure() {
        let mut ds3 = DualShock3::new(0);
        ds3.set_button_pressure(PadButtons::CROSS, 200);
        
        assert!(ds3.pad.is_button_pressed(PadButtons::CROSS));
        assert_eq!(ds3.pad.pressure[pressure_index::CROSS], 200);
    }

    #[test]
    fn test_manager() {
        let mut manager = DualShock3Manager::new(4);
        
        assert!(manager.connect(0, ConnectionMode::Usb).is_ok());
        assert_eq!(manager.connected_count(), 1);
        
        manager.disconnect(0);
        assert_eq!(manager.connected_count(), 0);
    }

    #[test]
    fn test_led_state() {
        let mut leds = LedState::default();
        leds.set_player(2);
        
        assert!(leds.led1);
        assert!(leds.led2);
        assert!(!leds.led3);
        assert!(!leds.led4);
    }
}
