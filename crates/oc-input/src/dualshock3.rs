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

    #[test]
    fn test_usb_hid_input_report_parsing() {
        let mut report = UsbHidInputReport::default();
        
        // Simulate Cross button press
        // According to our parsing: buttons[1] bit 6 = Cross
        report.buttons[1] |= 0x40;
        
        let state = report.to_pad_state();
        assert!(state.is_button_pressed(PadButtons::CROSS));
    }

    #[test]
    fn test_calibration() {
        let mut calibration = SixaxisCalibration::default();
        
        // Simulate calibration at rest
        calibration.calibrate_at_rest(&SixaxisData::at_rest());
        
        assert!(calibration.is_calibrated);
    }

    #[test]
    fn test_bluetooth_pairing_info() {
        let host_addr = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let info = BluetoothPairingInfo::new(host_addr);
        
        assert_eq!(info.host_address, host_addr);
        assert!(!info.is_paired);
    }

    #[test]
    fn test_output_report() {
        let mut output = UsbHidOutputReport::default();
        output.set_leds(3);
        output.set_vibration(255, 128);
        
        assert_eq!(output.led_mask, 0x06); // LED 1 and 2 (player 3)
        assert_eq!(output.large_motor, 128);
        // small_motor is on/off (0 or 1), not the raw value
        assert_eq!(output.small_motor, 1);
    }
}

// =============================================================================
// USB HID Report Structures
// =============================================================================

/// DualShock 3 USB HID Input Report (49 bytes)
/// Contains all input data from the controller
#[derive(Debug, Clone)]
pub struct UsbHidInputReport {
    /// Report ID (always 0x01 for input)
    pub report_id: u8,
    /// Reserved byte
    pub reserved: u8,
    /// Button data (bytes 2-4)
    pub buttons: [u8; 3],
    /// PS button (byte 5)
    pub ps_button: u8,
    /// Reserved (byte 6)
    pub reserved2: u8,
    /// Left analog X (0-255, 128=center)
    pub left_x: u8,
    /// Left analog Y (0-255, 128=center, inverted)
    pub left_y: u8,
    /// Right analog X (0-255, 128=center)
    pub right_x: u8,
    /// Right analog Y (0-255, 128=center, inverted)
    pub right_y: u8,
    /// Reserved (bytes 11-14)
    pub reserved3: [u8; 4],
    /// Pressure-sensitive buttons (bytes 15-26)
    pub pressure: [u8; 12],
    /// Reserved (bytes 27-30)
    pub reserved4: [u8; 4],
    /// Battery status (byte 31)
    pub battery: u8,
    /// Reserved (bytes 32-37)
    pub reserved5: [u8; 6],
    /// Accelerometer X (bytes 38-39, big-endian)
    pub accel_x: [u8; 2],
    /// Accelerometer Y (bytes 40-41, big-endian)
    pub accel_y: [u8; 2],
    /// Accelerometer Z (bytes 42-43, big-endian)
    pub accel_z: [u8; 2],
    /// Gyroscope Z (bytes 44-45, big-endian)
    pub gyro_z: [u8; 2],
}

impl Default for UsbHidInputReport {
    fn default() -> Self {
        Self {
            report_id: 0x01,
            reserved: 0,
            buttons: [0; 3],
            ps_button: 0,
            reserved2: 0,
            left_x: 128,
            left_y: 128,
            right_x: 128,
            right_y: 128,
            reserved3: [0; 4],
            pressure: [0; 12],
            reserved4: [0; 4],
            battery: 0xEE, // Fully charged
            reserved5: [0; 6],
            accel_x: [0x02, 0x00], // ~512 (neutral)
            accel_y: [0x02, 0x00],
            accel_z: [0x02, 0x00],
            gyro_z: [0x02, 0x00],
        }
    }
}

impl UsbHidInputReport {
    /// Parse from raw USB HID report bytes (49 bytes)
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 49 {
            return None;
        }

        Some(Self {
            report_id: data[0],
            reserved: data[1],
            buttons: [data[2], data[3], data[4]],
            ps_button: data[5],
            reserved2: data[6],
            left_x: data[7],
            left_y: data[8],
            right_x: data[9],
            right_y: data[10],
            reserved3: [data[11], data[12], data[13], data[14]],
            pressure: [
                data[15], data[16], data[17], data[18], data[19], data[20],
                data[21], data[22], data[23], data[24], data[25], data[26],
            ],
            reserved4: [data[27], data[28], data[29], data[30]],
            battery: data[31],
            reserved5: [data[32], data[33], data[34], data[35], data[36], data[37]],
            accel_x: [data[38], data[39]],
            accel_y: [data[40], data[41]],
            accel_z: [data[42], data[43]],
            gyro_z: [data[44], data[45]],
        })
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> [u8; 49] {
        let mut data = [0u8; 49];
        data[0] = self.report_id;
        data[1] = self.reserved;
        data[2..5].copy_from_slice(&self.buttons);
        data[5] = self.ps_button;
        data[6] = self.reserved2;
        data[7] = self.left_x;
        data[8] = self.left_y;
        data[9] = self.right_x;
        data[10] = self.right_y;
        data[11..15].copy_from_slice(&self.reserved3);
        data[15..27].copy_from_slice(&self.pressure);
        data[27..31].copy_from_slice(&self.reserved4);
        data[31] = self.battery;
        data[32..38].copy_from_slice(&self.reserved5);
        data[38..40].copy_from_slice(&self.accel_x);
        data[40..42].copy_from_slice(&self.accel_y);
        data[42..44].copy_from_slice(&self.accel_z);
        data[44..46].copy_from_slice(&self.gyro_z);
        data
    }

    /// Get accelerometer X value (-512 to 511)
    pub fn get_accel_x(&self) -> i16 {
        let raw = u16::from_be_bytes(self.accel_x);
        (raw as i16).wrapping_sub(512)
    }

    /// Get accelerometer Y value (-512 to 511)
    pub fn get_accel_y(&self) -> i16 {
        let raw = u16::from_be_bytes(self.accel_y);
        (raw as i16).wrapping_sub(512)
    }

    /// Get accelerometer Z value (-512 to 511)
    pub fn get_accel_z(&self) -> i16 {
        let raw = u16::from_be_bytes(self.accel_z);
        (raw as i16).wrapping_sub(512)
    }

    /// Get gyroscope Z value
    pub fn get_gyro_z(&self) -> i16 {
        let raw = u16::from_be_bytes(self.gyro_z);
        (raw as i16).wrapping_sub(512)
    }

    /// Convert to PadState
    pub fn to_pad_state(&self) -> PadState {
        let mut state = PadState::new();

        // Parse button byte 2
        // bit 0: Select, bit 1: L3, bit 2: R3, bit 3: Start
        // bit 4: D-pad up, bit 5: D-pad right, bit 6: D-pad down, bit 7: D-pad left
        if self.buttons[0] & 0x01 != 0 { state.set_button(PadButtons::SELECT, true); }
        if self.buttons[0] & 0x02 != 0 { state.set_button(PadButtons::L3, true); }
        if self.buttons[0] & 0x04 != 0 { state.set_button(PadButtons::R3, true); }
        if self.buttons[0] & 0x08 != 0 { state.set_button(PadButtons::START, true); }
        if self.buttons[0] & 0x10 != 0 { state.set_button(PadButtons::DPAD_UP, true); }
        if self.buttons[0] & 0x20 != 0 { state.set_button(PadButtons::DPAD_RIGHT, true); }
        if self.buttons[0] & 0x40 != 0 { state.set_button(PadButtons::DPAD_DOWN, true); }
        if self.buttons[0] & 0x80 != 0 { state.set_button(PadButtons::DPAD_LEFT, true); }

        // Parse button byte 3
        // bit 0: L2, bit 1: R2, bit 2: L1, bit 3: R1
        // bit 4: Triangle, bit 5: Circle, bit 6: Cross, bit 7: Square
        if self.buttons[1] & 0x01 != 0 { state.set_button(PadButtons::L2, true); }
        if self.buttons[1] & 0x02 != 0 { state.set_button(PadButtons::R2, true); }
        if self.buttons[1] & 0x04 != 0 { state.set_button(PadButtons::L1, true); }
        if self.buttons[1] & 0x08 != 0 { state.set_button(PadButtons::R1, true); }
        if self.buttons[1] & 0x10 != 0 { state.set_button(PadButtons::TRIANGLE, true); }
        if self.buttons[1] & 0x20 != 0 { state.set_button(PadButtons::CIRCLE, true); }
        if self.buttons[1] & 0x40 != 0 { state.set_button(PadButtons::CROSS, true); }
        if self.buttons[1] & 0x80 != 0 { state.set_button(PadButtons::SQUARE, true); }

        // PS button is stored in ps_button field but not in standard PadButtons
        // It's handled separately by the emulator as a system-level button

        // Analog sticks
        state.left_x = self.left_x;
        state.left_y = self.left_y;
        state.right_x = self.right_x;
        state.right_y = self.right_y;

        // Pressure-sensitive buttons (in order: dpad up/right/down/left, L2, R2, L1, R1, tri, circle, cross, square)
        state.pressure.copy_from_slice(&self.pressure);

        state
    }

    /// Convert to SixaxisData
    pub fn to_sixaxis_data(&self) -> SixaxisData {
        SixaxisData {
            accel_x: self.get_accel_x(),
            accel_y: self.get_accel_y(),
            accel_z: self.get_accel_z(),
            gyro_x: 0, // DS3 only has Z-axis gyro
            gyro_y: 0,
            gyro_z: self.get_gyro_z(),
        }
    }

    /// Get battery level (0-100)
    pub fn get_battery_level(&self) -> u8 {
        match self.battery {
            0x00 => 0,
            0x01 => 20,
            0x02 => 40,
            0x03 => 60,
            0x04 => 80,
            0xEE | 0xEF => 100, // Charging or full
            _ => 50,
        }
    }
}

/// DualShock 3 USB HID Output Report (49 bytes)
/// Used to control LEDs and vibration
#[derive(Debug, Clone)]
pub struct UsbHidOutputReport {
    /// Report ID (0x01 for output)
    pub report_id: u8,
    /// Reserved (byte 1)
    pub reserved: u8,
    /// Duration for small motor (right side, high freq)
    pub small_motor_duration: u8,
    /// Small motor power (0 = off, 1 = on)
    pub small_motor: u8,
    /// Duration for large motor (left side, low freq)
    pub large_motor_duration: u8,
    /// Large motor power (0-255)
    pub large_motor: u8,
    /// Reserved (bytes 6-9)
    pub reserved2: [u8; 4],
    /// LED mask (bits 1-4 = LED 1-4)
    pub led_mask: u8,
    /// LED 4 blink parameters
    pub led4_params: [u8; 5],
    /// LED 3 blink parameters
    pub led3_params: [u8; 5],
    /// LED 2 blink parameters
    pub led2_params: [u8; 5],
    /// LED 1 blink parameters
    pub led1_params: [u8; 5],
    /// Reserved (remaining bytes)
    pub reserved3: [u8; 17],
}

impl Default for UsbHidOutputReport {
    fn default() -> Self {
        Self {
            report_id: 0x01,
            reserved: 0x00,
            small_motor_duration: 0xFF, // Continuous
            small_motor: 0,
            large_motor_duration: 0xFF, // Continuous
            large_motor: 0,
            reserved2: [0; 4],
            led_mask: 0x02, // LED 1 (player 1)
            led4_params: [0xFF, 0x27, 0x10, 0x00, 0x32],
            led3_params: [0xFF, 0x27, 0x10, 0x00, 0x32],
            led2_params: [0xFF, 0x27, 0x10, 0x00, 0x32],
            led1_params: [0xFF, 0x27, 0x10, 0x00, 0x32],
            reserved3: [0; 17],
        }
    }
}

impl UsbHidOutputReport {
    /// Set vibration motors
    pub fn set_vibration(&mut self, small: u8, large: u8) {
        self.small_motor = if small > 0 { 1 } else { 0 };
        self.large_motor = large;
    }

    /// Set LED for player number (1-4)
    pub fn set_leds(&mut self, player: u8) {
        self.led_mask = match player {
            1 => 0x02, // LED 1
            2 => 0x04, // LED 2
            3 => 0x06, // LED 1+2
            4 => 0x08, // LED 3
            5 => 0x0A, // LED 1+3
            6 => 0x0C, // LED 2+3
            7 => 0x0E, // LED 1+2+3
            _ => 0x02,
        };
    }

    /// Set LED blink pattern (on_time/off_time in 10ms units)
    pub fn set_led_blink(&mut self, led: u8, on_time: u8, off_time: u8) {
        let params = [0xFF, 0x27, on_time, off_time, 0x32];
        match led {
            1 => self.led1_params = params,
            2 => self.led2_params = params,
            3 => self.led3_params = params,
            4 => self.led4_params = params,
            _ => {}
        }
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> [u8; 49] {
        let mut data = [0u8; 49];
        data[0] = self.report_id;
        data[1] = self.reserved;
        data[2] = self.small_motor_duration;
        data[3] = self.small_motor;
        data[4] = self.large_motor_duration;
        data[5] = self.large_motor;
        data[6..10].copy_from_slice(&self.reserved2);
        data[10] = self.led_mask;
        data[11..16].copy_from_slice(&self.led4_params);
        data[16..21].copy_from_slice(&self.led3_params);
        data[21..26].copy_from_slice(&self.led2_params);
        data[26..31].copy_from_slice(&self.led1_params);
        data[31..48].copy_from_slice(&self.reserved3);
        data
    }
}

/// Feature report type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureReportType {
    /// Get/Set BD_ADDR (report 0xF2)
    BdAddr = 0xF2,
    /// Get/Set link key (report 0xF5)
    LinkKey = 0xF5,
    /// Unknown report type
    Unknown = 0x00,
}

/// DualShock 3 Feature Report for BD_ADDR (report 0xF2)
#[derive(Debug, Clone)]
pub struct BdAddrFeatureReport {
    /// Report ID (0xF2)
    pub report_id: u8,
    /// Reserved (byte 1)
    pub reserved: u8,
    /// Controller BD_ADDR (bytes 2-7)
    pub controller_addr: [u8; 6],
    /// Host BD_ADDR to pair with (bytes 8-13)
    pub host_addr: [u8; 6],
    /// Reserved (remaining bytes)
    pub reserved2: [u8; 3],
}

impl Default for BdAddrFeatureReport {
    fn default() -> Self {
        Self {
            report_id: 0xF2,
            reserved: 0,
            controller_addr: [0; 6],
            host_addr: [0; 6],
            reserved2: [0; 3],
        }
    }
}

impl BdAddrFeatureReport {
    /// Parse from raw bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 17 || data[0] != 0xF2 {
            return None;
        }

        let mut report = Self::default();
        report.reserved = data[1];
        report.controller_addr.copy_from_slice(&data[2..8]);
        report.host_addr.copy_from_slice(&data[8..14]);
        report.reserved2.copy_from_slice(&data[14..17]);
        Some(report)
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; 17] {
        let mut data = [0u8; 17];
        data[0] = self.report_id;
        data[1] = self.reserved;
        data[2..8].copy_from_slice(&self.controller_addr);
        data[8..14].copy_from_slice(&self.host_addr);
        data[14..17].copy_from_slice(&self.reserved2);
        data
    }
}

// =============================================================================
// Sixaxis Calibration
// =============================================================================

/// Sixaxis sensor calibration data
#[derive(Debug, Clone, Copy, Default)]
pub struct SixaxisCalibration {
    /// Accelerometer X offset (raw value at rest)
    pub accel_x_offset: i16,
    /// Accelerometer Y offset (raw value at rest)
    pub accel_y_offset: i16,
    /// Accelerometer Z offset (raw value at 1G rest)
    pub accel_z_offset: i16,
    /// Gyroscope X offset (raw value at rest)
    pub gyro_x_offset: i16,
    /// Gyroscope Y offset (raw value at rest)
    pub gyro_y_offset: i16,
    /// Gyroscope Z offset (raw value at rest)
    pub gyro_z_offset: i16,
    /// Accelerometer scale (sensitivity adjustment)
    pub accel_scale: f32,
    /// Gyroscope scale (sensitivity adjustment)
    pub gyro_scale: f32,
    /// Is calibration complete
    pub is_calibrated: bool,
    /// Number of samples collected for calibration
    pub sample_count: u32,
    /// Accumulated samples for averaging
    accel_x_sum: i64,
    accel_y_sum: i64,
    accel_z_sum: i64,
    gyro_x_sum: i64,
    gyro_y_sum: i64,
    gyro_z_sum: i64,
}

/// Expected accelerometer Z value at rest (1G pointing up)
const ACCEL_Z_REST_VALUE: i16 = 511;

impl SixaxisCalibration {
    /// Create new calibration with default values
    pub fn new() -> Self {
        Self {
            accel_scale: 1.0,
            gyro_scale: 1.0,
            ..Default::default()
        }
    }

    /// Add a calibration sample (controller must be at rest)
    pub fn add_sample(&mut self, data: &SixaxisData) {
        self.accel_x_sum += data.accel_x as i64;
        self.accel_y_sum += data.accel_y as i64;
        self.accel_z_sum += data.accel_z as i64;
        self.gyro_x_sum += data.gyro_x as i64;
        self.gyro_y_sum += data.gyro_y as i64;
        self.gyro_z_sum += data.gyro_z as i64;
        self.sample_count += 1;
    }

    /// Complete calibration by averaging samples
    pub fn complete(&mut self) {
        if self.sample_count == 0 {
            return;
        }

        let n = self.sample_count as i64;
        self.accel_x_offset = (self.accel_x_sum / n) as i16;
        self.accel_y_offset = (self.accel_y_sum / n) as i16;
        // Z-axis should be ~ACCEL_Z_REST_VALUE at rest (1G)
        self.accel_z_offset = ((self.accel_z_sum / n) - ACCEL_Z_REST_VALUE as i64) as i16;
        self.gyro_x_offset = (self.gyro_x_sum / n) as i16;
        self.gyro_y_offset = (self.gyro_y_sum / n) as i16;
        self.gyro_z_offset = (self.gyro_z_sum / n) as i16;

        self.is_calibrated = true;
        tracing::info!(
            "Sixaxis calibration complete: accel offsets ({}, {}, {}), gyro offsets ({}, {}, {})",
            self.accel_x_offset, self.accel_y_offset, self.accel_z_offset,
            self.gyro_x_offset, self.gyro_y_offset, self.gyro_z_offset
        );
    }

    /// Quick calibration from single at-rest reading
    pub fn calibrate_at_rest(&mut self, data: &SixaxisData) {
        self.accel_x_offset = data.accel_x;
        self.accel_y_offset = data.accel_y;
        self.accel_z_offset = data.accel_z - ACCEL_Z_REST_VALUE; // Z should be ~ACCEL_Z_REST_VALUE at rest
        self.gyro_x_offset = data.gyro_x;
        self.gyro_y_offset = data.gyro_y;
        self.gyro_z_offset = data.gyro_z;
        self.is_calibrated = true;
    }

    /// Apply calibration to raw sensor data
    pub fn apply(&self, data: &SixaxisData) -> SixaxisData {
        if !self.is_calibrated {
            return *data;
        }

        SixaxisData {
            accel_x: ((data.accel_x - self.accel_x_offset) as f32 * self.accel_scale) as i16,
            accel_y: ((data.accel_y - self.accel_y_offset) as f32 * self.accel_scale) as i16,
            accel_z: ((data.accel_z - self.accel_z_offset) as f32 * self.accel_scale) as i16,
            gyro_x: ((data.gyro_x - self.gyro_x_offset) as f32 * self.gyro_scale) as i16,
            gyro_y: ((data.gyro_y - self.gyro_y_offset) as f32 * self.gyro_scale) as i16,
            gyro_z: ((data.gyro_z - self.gyro_z_offset) as f32 * self.gyro_scale) as i16,
        }
    }

    /// Reset calibration
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// =============================================================================
// Bluetooth Pairing Support
// =============================================================================

/// Bluetooth pairing state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothPairingState {
    /// Not paired
    Unpaired,
    /// Pairing in progress (via USB)
    Pairing,
    /// Paired and ready to connect
    Paired,
    /// Connected via Bluetooth
    Connected,
    /// Reconnecting
    Reconnecting,
}

/// Bluetooth pairing information
#[derive(Debug, Clone)]
pub struct BluetoothPairingInfo {
    /// Controller's Bluetooth address
    pub controller_address: [u8; 6],
    /// Host's Bluetooth address (PS3 or PC)
    pub host_address: [u8; 6],
    /// Link key for encrypted connection
    pub link_key: [u8; 16],
    /// Pairing state
    pub state: BluetoothPairingState,
    /// Is the controller paired to a host
    pub is_paired: bool,
    /// Last connection time
    pub last_connected: Option<Instant>,
    /// Connection attempt count
    pub reconnect_attempts: u32,
}

impl BluetoothPairingInfo {
    /// Create new pairing info
    pub fn new(host_address: [u8; 6]) -> Self {
        Self {
            controller_address: [0; 6],
            host_address,
            link_key: [0; 16],
            state: BluetoothPairingState::Unpaired,
            is_paired: false,
            last_connected: None,
            reconnect_attempts: 0,
        }
    }

    /// Set controller address (read from USB feature report)
    pub fn set_controller_address(&mut self, addr: [u8; 6]) {
        self.controller_address = addr;
    }

    /// Complete pairing process
    pub fn complete_pairing(&mut self, link_key: [u8; 16]) {
        self.link_key = link_key;
        self.state = BluetoothPairingState::Paired;
        self.is_paired = true;
        tracing::info!(
            "Bluetooth pairing complete: controller {} -> host {}",
            Self::addr_to_string(&self.controller_address),
            Self::addr_to_string(&self.host_address)
        );
    }

    /// Mark as connected
    pub fn mark_connected(&mut self) {
        self.state = BluetoothPairingState::Connected;
        self.last_connected = Some(Instant::now());
        self.reconnect_attempts = 0;
    }

    /// Mark as disconnected
    pub fn mark_disconnected(&mut self) {
        if self.is_paired {
            self.state = BluetoothPairingState::Paired;
        } else {
            self.state = BluetoothPairingState::Unpaired;
        }
    }

    /// Start reconnection attempt
    pub fn start_reconnect(&mut self) {
        self.state = BluetoothPairingState::Reconnecting;
        self.reconnect_attempts += 1;
    }

    /// Format Bluetooth address as string
    pub fn addr_to_string(addr: &[u8; 6]) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]
        )
    }
}

impl Default for BluetoothPairingInfo {
    fn default() -> Self {
        Self::new([0; 6])
    }
}

// =============================================================================
// USB Hot-Plug Detection
// =============================================================================

/// USB connection event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbConnectionEvent {
    /// Controller connected via USB
    Connected,
    /// Controller disconnected from USB
    Disconnected,
    /// USB suspend (power saving)
    Suspended,
    /// USB resume from suspend
    Resumed,
}

/// USB HID report rate configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbReportRate {
    /// 125 Hz (default USB HID)
    Rate125Hz = 125,
    /// 250 Hz
    Rate250Hz = 250,
    /// 500 Hz
    Rate500Hz = 500,
    /// 1000 Hz (full speed, requires high-speed USB)
    Rate1000Hz = 1000,
}

impl Default for UsbReportRate {
    fn default() -> Self {
        Self::Rate1000Hz
    }
}

/// USB controller state
#[derive(Debug, Clone)]
pub struct UsbControllerState {
    /// Is USB connected
    pub connected: bool,
    /// USB device path/identifier
    pub device_path: String,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Current report rate
    pub report_rate: UsbReportRate,
    /// Is suspended
    pub suspended: bool,
    /// Last report time
    pub last_report_time: Option<Instant>,
    /// Reports per second (measured)
    pub measured_rate: f32,
    /// Total reports received
    pub report_count: u64,
}

impl Default for UsbControllerState {
    fn default() -> Self {
        Self {
            connected: false,
            device_path: String::new(),
            vendor_id: 0x054C, // Sony
            product_id: 0x0268, // DualShock 3
            report_rate: UsbReportRate::default(),
            suspended: false,
            last_report_time: None,
            measured_rate: 0.0,
            report_count: 0,
        }
    }
}

impl UsbControllerState {
    /// Record a received report
    pub fn record_report(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_report_time {
            let elapsed = now.duration_since(last).as_secs_f32();
            if elapsed > 0.0 {
                // Exponential moving average for rate calculation
                let instant_rate = 1.0 / elapsed;
                self.measured_rate = self.measured_rate * 0.9 + instant_rate * 0.1;
            }
        }
        self.last_report_time = Some(now);
        self.report_count += 1;
    }

    /// Check if controller is responding (has sent reports recently)
    pub fn is_responsive(&self) -> bool {
        if let Some(last) = self.last_report_time {
            last.elapsed() < Duration::from_millis(100)
        } else {
            false
        }
    }
}
