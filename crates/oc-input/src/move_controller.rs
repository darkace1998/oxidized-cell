//! PlayStation Move Controller Emulation
//!
//! Full emulation of the PS Move motion controller including:
//! - Motion sensors (accelerometer, gyroscope, magnetometer)
//! - Tracking sphere (LED color, position tracking)
//! - Buttons and trigger
//! - Vibration feedback

use std::time::Instant;

/// Move button flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveButtons(pub u32);

impl MoveButtons {
    /// Select button
    pub const SELECT: Self = Self(0x0001);
    /// T (Trigger) button - analog
    pub const TRIGGER: Self = Self(0x0002);
    /// Move button (large center button)
    pub const MOVE: Self = Self(0x0004);
    /// Start button
    pub const START: Self = Self(0x0008);
    /// Triangle button
    pub const TRIANGLE: Self = Self(0x0010);
    /// Circle button
    pub const CIRCLE: Self = Self(0x0020);
    /// Cross button
    pub const CROSS: Self = Self(0x0040);
    /// Square button
    pub const SQUARE: Self = Self(0x0080);
    /// PS button
    pub const PS: Self = Self(0x0100);

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

/// RGB color for the tracking sphere
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SphereColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl SphereColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Red color
    pub fn red() -> Self {
        Self::new(255, 0, 0)
    }

    /// Green color
    pub fn green() -> Self {
        Self::new(0, 255, 0)
    }

    /// Blue color
    pub fn blue() -> Self {
        Self::new(0, 0, 255)
    }

    /// Magenta color
    pub fn magenta() -> Self {
        Self::new(255, 0, 255)
    }

    /// Cyan color
    pub fn cyan() -> Self {
        Self::new(0, 255, 255)
    }

    /// Yellow color
    pub fn yellow() -> Self {
        Self::new(255, 255, 0)
    }

    /// White color
    pub fn white() -> Self {
        Self::new(255, 255, 255)
    }

    /// Off (no light)
    pub fn off() -> Self {
        Self::new(0, 0, 0)
    }

    /// Default player colors
    pub fn player_color(player: u8) -> Self {
        match player {
            1 => Self::red(),
            2 => Self::blue(),
            3 => Self::green(),
            4 => Self::magenta(),
            _ => Self::white(),
        }
    }
}

/// 3D position in camera space (millimeters)
#[derive(Debug, Clone, Copy, Default)]
pub struct Position3D {
    /// X position (horizontal, left/right)
    pub x: f32,
    /// Y position (vertical, up/down)
    pub y: f32,
    /// Z position (depth, distance from camera)
    pub z: f32,
}

impl Position3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Distance from origin
    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// Quaternion for orientation
#[derive(Debug, Clone, Copy)]
pub struct Quaternion {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::identity()
    }
}

impl Quaternion {
    pub fn new(w: f32, x: f32, y: f32, z: f32) -> Self {
        Self { w, x, y, z }
    }

    /// Identity quaternion (no rotation)
    pub fn identity() -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0)
    }

    /// Create from Euler angles (radians)
    pub fn from_euler(roll: f32, pitch: f32, yaw: f32) -> Self {
        let (sr, cr) = (roll / 2.0).sin_cos();
        let (sp, cp) = (pitch / 2.0).sin_cos();
        let (sy, cy) = (yaw / 2.0).sin_cos();

        Self {
            w: cr * cp * cy + sr * sp * sy,
            x: sr * cp * cy - cr * sp * sy,
            y: cr * sp * cy + sr * cp * sy,
            z: cr * cp * sy - sr * sp * cy,
        }
    }

    /// Normalize the quaternion
    pub fn normalize(&mut self) {
        let mag = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if mag > 0.0 {
            self.w /= mag;
            self.x /= mag;
            self.y /= mag;
            self.z /= mag;
        }
    }
}

/// Motion sensor data (raw)
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveMotionData {
    /// Accelerometer X (m/s²)
    pub accel_x: f32,
    /// Accelerometer Y (m/s²)
    pub accel_y: f32,
    /// Accelerometer Z (m/s²)
    pub accel_z: f32,
    /// Gyroscope X (rad/s)
    pub gyro_x: f32,
    /// Gyroscope Y (rad/s)
    pub gyro_y: f32,
    /// Gyroscope Z (rad/s)
    pub gyro_z: f32,
    /// Magnetometer X (µT)
    pub mag_x: f32,
    /// Magnetometer Y (µT)
    pub mag_y: f32,
    /// Magnetometer Z (µT)
    pub mag_z: f32,
}

impl MoveMotionData {
    /// At rest, pointing up
    pub fn at_rest() -> Self {
        Self {
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: 9.81, // 1G pointing up
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
            mag_x: 0.0,
            mag_y: 0.0,
            mag_z: 0.0,
        }
    }
}

/// Tracking quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingQuality {
    /// No tracking (sphere not visible)
    NotTracked,
    /// Poor tracking (partial occlusion)
    Poor,
    /// Good tracking
    Good,
    /// Excellent tracking
    Excellent,
}

/// Move controller state
#[derive(Debug, Clone)]
pub struct MoveState {
    /// Buttons pressed
    pub buttons: MoveButtons,
    /// Trigger value (0-255)
    pub trigger: u8,
    /// Motion sensor data
    pub motion: MoveMotionData,
    /// Orientation (from sensor fusion)
    pub orientation: Quaternion,
    /// Position in camera space
    pub position: Position3D,
    /// Tracking quality
    pub tracking: TrackingQuality,
    /// Temperature (controller internal, °C)
    pub temperature: f32,
}

impl Default for MoveState {
    fn default() -> Self {
        Self {
            buttons: MoveButtons::empty(),
            trigger: 0,
            motion: MoveMotionData::at_rest(),
            orientation: Quaternion::identity(),
            position: Position3D::new(0.0, 0.0, 500.0), // 50cm from camera
            tracking: TrackingQuality::NotTracked,
            temperature: 25.0,
        }
    }
}

/// Move vibration pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveVibration {
    /// No vibration
    Off,
    /// Light vibration
    Light,
    /// Medium vibration
    Medium,
    /// Heavy vibration
    Heavy,
    /// Pulse pattern
    Pulse,
    /// Custom intensity (0-255)
    Custom(u8),
}

/// PlayStation Move controller
#[derive(Debug)]
pub struct MoveController {
    /// Controller index (0-3)
    pub index: u8,
    /// Current state
    pub state: MoveState,
    /// Sphere LED color
    pub sphere_color: SphereColor,
    /// Vibration state
    pub vibration: MoveVibration,
    /// Is connected
    pub connected: bool,
    /// Is calibrated
    pub calibrated: bool,
    /// Battery level (0-5)
    pub battery_level: u8,
    /// Is charging
    pub charging: bool,
    /// Last update time
    last_update: Instant,
}

impl MoveController {
    /// Create a new Move controller
    pub fn new(index: u8) -> Self {
        let sphere_color = SphereColor::player_color(index + 1);
        
        Self {
            index,
            state: MoveState::default(),
            sphere_color,
            vibration: MoveVibration::Off,
            connected: false,
            calibrated: false,
            battery_level: 5,
            charging: false,
            last_update: Instant::now(),
        }
    }

    /// Connect the controller
    pub fn connect(&mut self) {
        self.connected = true;
        self.last_update = Instant::now();
        tracing::info!("Move controller {} connected", self.index);
    }

    /// Disconnect the controller
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.state = MoveState::default();
        self.vibration = MoveVibration::Off;
        tracing::info!("Move controller {} disconnected", self.index);
    }

    /// Set button state
    pub fn set_button(&mut self, button: MoveButtons, pressed: bool) {
        if pressed {
            self.state.buttons.insert(button);
        } else {
            self.state.buttons.remove(button);
        }
    }

    /// Set trigger value
    pub fn set_trigger(&mut self, value: u8) {
        self.state.trigger = value;
        // Trigger button is pressed if above threshold
        if value > 128 {
            self.state.buttons.insert(MoveButtons::TRIGGER);
        } else {
            self.state.buttons.remove(MoveButtons::TRIGGER);
        }
    }

    /// Set motion sensor data
    pub fn set_motion(&mut self, motion: MoveMotionData) {
        self.state.motion = motion;
    }

    /// Set orientation
    pub fn set_orientation(&mut self, orientation: Quaternion) {
        self.state.orientation = orientation;
    }

    /// Set position (from camera tracking)
    pub fn set_position(&mut self, position: Position3D, quality: TrackingQuality) {
        self.state.position = position;
        self.state.tracking = quality;
    }

    /// Set sphere color
    pub fn set_sphere_color(&mut self, color: SphereColor) {
        self.sphere_color = color;
    }

    /// Set vibration
    pub fn set_vibration(&mut self, vibration: MoveVibration) {
        self.vibration = vibration;
    }

    /// Start calibration
    pub fn calibrate(&mut self) {
        // In a real implementation, this would:
        // 1. Sample gyro bias
        // 2. Sample magnetometer for heading
        // 3. Establish gravity direction from accelerometer
        self.calibrated = true;
        tracing::info!("Move controller {} calibrated", self.index);
    }

    /// Update controller state (call each frame)
    pub fn update(&mut self) {
        if !self.connected {
            return;
        }

        self.last_update = Instant::now();

        // Simple sensor fusion for orientation
        // In reality, this would use a proper fusion algorithm (Madgwick, Mahony, etc.)
        let dt = 1.0 / 60.0; // Assume 60fps
        
        // Integrate gyroscope (very simplified)
        let roll = self.state.motion.gyro_x * dt;
        let pitch = self.state.motion.gyro_y * dt;
        let yaw = self.state.motion.gyro_z * dt;
        
        if roll.abs() > 0.001 || pitch.abs() > 0.001 || yaw.abs() > 0.001 {
            let delta = Quaternion::from_euler(roll, pitch, yaw);
            // Apply rotation (simplified - should be quaternion multiplication)
            self.state.orientation.x += delta.x;
            self.state.orientation.y += delta.y;
            self.state.orientation.z += delta.z;
            self.state.orientation.normalize();
        }
    }

    /// Get raw data for cellGem
    pub fn get_gem_state(&self) -> GemState {
        GemState {
            position: self.state.position,
            orientation: self.state.orientation,
            tracking: self.state.tracking,
            pad: GemPadData {
                buttons: self.state.buttons.0 as u16,
                trigger: self.state.trigger,
            },
            ext: GemExtData {
                accel: [
                    (self.state.motion.accel_x * 1000.0) as i16,
                    (self.state.motion.accel_y * 1000.0) as i16,
                    (self.state.motion.accel_z * 1000.0) as i16,
                ],
                gyro: [
                    (self.state.motion.gyro_x * 1000.0) as i16,
                    (self.state.motion.gyro_y * 1000.0) as i16,
                    (self.state.motion.gyro_z * 1000.0) as i16,
                ],
            },
        }
    }
}

/// GemState for cellGem API
#[derive(Debug, Clone, Copy)]
pub struct GemState {
    pub position: Position3D,
    pub orientation: Quaternion,
    pub tracking: TrackingQuality,
    pub pad: GemPadData,
    pub ext: GemExtData,
}

/// Gem pad data
#[derive(Debug, Clone, Copy, Default)]
pub struct GemPadData {
    pub buttons: u16,
    pub trigger: u8,
}

/// Gem extended data (sensors)
#[derive(Debug, Clone, Copy, Default)]
pub struct GemExtData {
    pub accel: [i16; 3],
    pub gyro: [i16; 3],
}

/// Move controller manager
pub struct MoveManager {
    /// Controllers (up to 4)
    controllers: [Option<MoveController>; 4],
    /// Camera available
    pub camera_available: bool,
}

impl MoveManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            controllers: Default::default(),
            camera_available: false,
        }
    }

    /// Connect a controller
    pub fn connect(&mut self, index: u8) -> Result<(), &'static str> {
        if index >= 4 {
            return Err("Invalid controller index");
        }
        
        let mut controller = MoveController::new(index);
        controller.connect();
        self.controllers[index as usize] = Some(controller);
        Ok(())
    }

    /// Disconnect a controller
    pub fn disconnect(&mut self, index: u8) {
        if let Some(controller) = self.controllers.get_mut(index as usize) {
            *controller = None;
        }
    }

    /// Get a controller
    pub fn get(&self, index: u8) -> Option<&MoveController> {
        self.controllers.get(index as usize)?.as_ref()
    }

    /// Get a mutable controller
    pub fn get_mut(&mut self, index: u8) -> Option<&mut MoveController> {
        self.controllers.get_mut(index as usize)?.as_mut()
    }

    /// Get connected count
    pub fn connected_count(&self) -> u8 {
        self.controllers.iter().filter(|c| c.is_some()).count() as u8
    }

    /// Update all controllers
    pub fn update(&mut self) {
        for controller in self.controllers.iter_mut().flatten() {
            controller.update();
        }
    }

    /// Set camera availability (for tracking)
    pub fn set_camera_available(&mut self, available: bool) {
        self.camera_available = available;
        
        // Update tracking quality based on camera
        for controller in self.controllers.iter_mut().flatten() {
            if !available {
                controller.state.tracking = TrackingQuality::NotTracked;
            }
        }
    }
}

impl Default for MoveManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_buttons() {
        let mut buttons = MoveButtons::empty();
        assert!(!buttons.contains(MoveButtons::CROSS));
        
        buttons.insert(MoveButtons::CROSS);
        assert!(buttons.contains(MoveButtons::CROSS));
        
        buttons.remove(MoveButtons::CROSS);
        assert!(!buttons.contains(MoveButtons::CROSS));
    }

    #[test]
    fn test_sphere_color() {
        assert_eq!(SphereColor::player_color(1), SphereColor::red());
        assert_eq!(SphereColor::player_color(2), SphereColor::blue());
    }

    #[test]
    fn test_quaternion_identity() {
        let q = Quaternion::identity();
        assert_eq!(q.w, 1.0);
        assert_eq!(q.x, 0.0);
    }

    #[test]
    fn test_controller_connect() {
        let mut controller = MoveController::new(0);
        assert!(!controller.connected);
        
        controller.connect();
        assert!(controller.connected);
        
        controller.disconnect();
        assert!(!controller.connected);
    }

    #[test]
    fn test_trigger() {
        let mut controller = MoveController::new(0);
        controller.connect();
        
        controller.set_trigger(200);
        assert_eq!(controller.state.trigger, 200);
        assert!(controller.state.buttons.contains(MoveButtons::TRIGGER));
        
        controller.set_trigger(50);
        assert!(!controller.state.buttons.contains(MoveButtons::TRIGGER));
    }

    #[test]
    fn test_manager() {
        let mut manager = MoveManager::new();
        
        assert!(manager.connect(0).is_ok());
        assert_eq!(manager.connected_count(), 1);
        
        manager.disconnect(0);
        assert_eq!(manager.connected_count(), 0);
    }
}
