//! PlayStation Eye Camera Emulation
//!
//! Support for PlayStation Eye and generic USB camera devices.
//! The PS Eye was used for:
//! - PlayStation Move tracking
//! - Video chat
//! - Motion games (EyeCreate, EyePet)
//! - Eye of Judgment (AR card game)

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Camera resolution modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraResolution {
    /// 320x240 @ up to 120fps
    QVGA,
    /// 640x480 @ up to 60fps  
    VGA,
    /// 1280x720 (generic cameras only, not PS Eye)
    HD720,
    /// 1920x1080 (generic cameras only)
    HD1080,
}

impl CameraResolution {
    pub fn width(&self) -> u32 {
        match self {
            CameraResolution::QVGA => 320,
            CameraResolution::VGA => 640,
            CameraResolution::HD720 => 1280,
            CameraResolution::HD1080 => 1920,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            CameraResolution::QVGA => 240,
            CameraResolution::VGA => 480,
            CameraResolution::HD720 => 720,
            CameraResolution::HD1080 => 1080,
        }
    }

    pub fn max_fps(&self) -> u32 {
        match self {
            CameraResolution::QVGA => 120,
            CameraResolution::VGA => 60,
            CameraResolution::HD720 => 30,
            CameraResolution::HD1080 => 30,
        }
    }

    pub fn frame_size_rgb(&self) -> usize {
        (self.width() * self.height() * 3) as usize
    }

    pub fn frame_size_yuv(&self) -> usize {
        // YUV420 planar: Y plane + U plane (quarter) + V plane (quarter)
        let pixels = self.width() * self.height();
        (pixels + pixels / 2) as usize
    }
}

/// Pixel format for camera output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraPixelFormat {
    /// RGB 24-bit
    RGB24,
    /// BGR 24-bit (common in Windows)
    BGR24,
    /// YUV420 planar
    YUV420P,
    /// YUYV packed
    YUYV,
    /// Raw Bayer pattern (PS Eye native)
    BayerGB,
}

/// Camera exposure mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposureMode {
    Auto,
    Manual(u16),
}

/// Camera settings
#[derive(Debug, Clone)]
pub struct CameraSettings {
    /// Resolution
    pub resolution: CameraResolution,
    /// Target framerate
    pub framerate: u32,
    /// Output pixel format
    pub pixel_format: CameraPixelFormat,
    /// Brightness (0-255)
    pub brightness: u8,
    /// Contrast (0-255)
    pub contrast: u8,
    /// Saturation (0-255)
    pub saturation: u8,
    /// Hue (-180 to 180)
    pub hue: i16,
    /// Gain (0-255)
    pub gain: u8,
    /// Exposure mode
    pub exposure: ExposureMode,
    /// Auto white balance
    pub auto_white_balance: bool,
    /// Flip horizontally
    pub flip_h: bool,
    /// Flip vertically
    pub flip_v: bool,
    /// Enable LED (PS Eye has 4-LED array)
    pub led_enabled: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            resolution: CameraResolution::VGA,
            framerate: 30,
            pixel_format: CameraPixelFormat::RGB24,
            brightness: 128,
            contrast: 128,
            saturation: 128,
            hue: 0,
            gain: 32,
            exposure: ExposureMode::Auto,
            auto_white_balance: true,
            flip_h: false,
            flip_v: false,
            led_enabled: true,
        }
    }
}

/// Camera device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraType {
    /// PlayStation Eye (PS3)
    PlayStationEye,
    /// EyeToy (PS2, backward compat)
    EyeToy,
    /// Generic USB camera
    Generic,
    /// Virtual/test camera
    Virtual,
}

/// Camera frame data
#[derive(Debug, Clone)]
pub struct CameraFrame {
    /// Frame data
    pub data: Vec<u8>,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Pixel format
    pub format: CameraPixelFormat,
    /// Frame timestamp
    pub timestamp: Duration,
    /// Frame sequence number
    pub sequence: u64,
}

impl CameraFrame {
    /// Create a blank frame
    pub fn blank(settings: &CameraSettings) -> Self {
        let size = match settings.pixel_format {
            CameraPixelFormat::RGB24 | CameraPixelFormat::BGR24 => {
                settings.resolution.frame_size_rgb()
            }
            CameraPixelFormat::YUV420P => settings.resolution.frame_size_yuv(),
            CameraPixelFormat::YUYV => {
                (settings.resolution.width() * settings.resolution.height() * 2) as usize
            }
            CameraPixelFormat::BayerGB => {
                (settings.resolution.width() * settings.resolution.height()) as usize
            }
        };

        Self {
            data: vec![0; size],
            width: settings.resolution.width(),
            height: settings.resolution.height(),
            format: settings.pixel_format,
            timestamp: Duration::ZERO,
            sequence: 0,
        }
    }

    /// Create a test pattern frame
    pub fn test_pattern(settings: &CameraSettings, sequence: u64) -> Self {
        let mut frame = Self::blank(settings);
        frame.sequence = sequence;
        
        // Generate color bars pattern
        let width = frame.width as usize;
        let height = frame.height as usize;
        
        if matches!(settings.pixel_format, CameraPixelFormat::RGB24) {
            let bar_width = width / 8;
            let colors: [(u8, u8, u8); 8] = [
                (255, 255, 255), // White
                (255, 255, 0),   // Yellow
                (0, 255, 255),   // Cyan
                (0, 255, 0),     // Green
                (255, 0, 255),   // Magenta
                (255, 0, 0),     // Red
                (0, 0, 255),     // Blue
                (0, 0, 0),       // Black
            ];
            
            for y in 0..height {
                for x in 0..width {
                    let bar = (x / bar_width).min(7);
                    let (r, g, b) = colors[bar];
                    let idx = (y * width + x) * 3;
                    if idx + 2 < frame.data.len() {
                        frame.data[idx] = r;
                        frame.data[idx + 1] = g;
                        frame.data[idx + 2] = b;
                    }
                }
            }
        }
        
        frame
    }
}

/// Camera capture state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraState {
    /// Not initialized
    Uninitialized,
    /// Ready but not capturing
    Ready,
    /// Actively capturing frames
    Capturing,
    /// Error state
    Error,
}

/// Frame callback type
pub type FrameCallback = Box<dyn Fn(&CameraFrame) + Send + Sync>;

/// Camera device
pub struct Camera {
    /// Camera index
    pub index: u8,
    /// Camera type
    pub camera_type: CameraType,
    /// Current settings
    pub settings: CameraSettings,
    /// Capture state
    state: CameraState,
    /// Frame counter
    frame_count: u64,
    /// Start time for timestamps
    start_time: Option<Instant>,
    /// Frame callback
    frame_callback: Option<Arc<Mutex<FrameCallback>>>,
    /// Last frame
    last_frame: Option<CameraFrame>,
}

impl std::fmt::Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("index", &self.index)
            .field("camera_type", &self.camera_type)
            .field("settings", &self.settings)
            .field("state", &self.state)
            .field("frame_count", &self.frame_count)
            .finish()
    }
}

impl Camera {
    pub fn new(index: u8, camera_type: CameraType) -> Self {
        Self {
            index,
            camera_type,
            settings: CameraSettings::default(),
            state: CameraState::Uninitialized,
            frame_count: 0,
            start_time: None,
            frame_callback: None,
            last_frame: None,
        }
    }

    /// Initialize the camera
    pub fn initialize(&mut self) -> Result<(), CameraError> {
        if self.state != CameraState::Uninitialized {
            return Err(CameraError::AlreadyInitialized);
        }

        // Validate settings for camera type
        if self.camera_type == CameraType::PlayStationEye {
            // PS Eye only supports QVGA and VGA
            if !matches!(
                self.settings.resolution,
                CameraResolution::QVGA | CameraResolution::VGA
            ) {
                self.settings.resolution = CameraResolution::VGA;
            }
        }

        self.state = CameraState::Ready;
        tracing::info!(
            "Camera {} ({:?}) initialized at {:?}",
            self.index,
            self.camera_type,
            self.settings.resolution
        );
        Ok(())
    }

    /// Start capturing
    pub fn start_capture(&mut self) -> Result<(), CameraError> {
        match self.state {
            CameraState::Uninitialized => return Err(CameraError::NotInitialized),
            CameraState::Capturing => return Err(CameraError::AlreadyCapturing),
            CameraState::Error => return Err(CameraError::DeviceError),
            CameraState::Ready => {}
        }

        self.frame_count = 0;
        self.start_time = Some(Instant::now());
        self.state = CameraState::Capturing;
        
        tracing::info!("Camera {} started capture at {}fps", self.index, self.settings.framerate);
        Ok(())
    }

    /// Stop capturing
    pub fn stop_capture(&mut self) -> Result<(), CameraError> {
        if self.state != CameraState::Capturing {
            return Err(CameraError::NotCapturing);
        }

        self.state = CameraState::Ready;
        tracing::info!("Camera {} stopped capture", self.index);
        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> CameraState {
        self.state
    }

    /// Update camera settings
    pub fn set_settings(&mut self, settings: CameraSettings) -> Result<(), CameraError> {
        if self.state == CameraState::Capturing {
            return Err(CameraError::CantChangeWhileCapturing);
        }

        self.settings = settings;
        Ok(())
    }

    /// Set frame callback
    pub fn set_frame_callback(&mut self, callback: FrameCallback) {
        self.frame_callback = Some(Arc::new(Mutex::new(callback)));
    }

    /// Poll for next frame (generates test pattern if no real camera)
    pub fn poll_frame(&mut self) -> Option<CameraFrame> {
        if self.state != CameraState::Capturing {
            return None;
        }

        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();
        
        // Generate test frame
        self.frame_count += 1;
        let mut frame = CameraFrame::test_pattern(&self.settings, self.frame_count);
        frame.timestamp = elapsed;
        
        // Call frame callback
        if let Some(ref cb) = self.frame_callback {
            if let Ok(callback) = cb.lock() {
                callback(&frame);
            }
        }
        
        self.last_frame = Some(frame.clone());
        Some(frame)
    }

    /// Get last captured frame
    pub fn last_frame(&self) -> Option<&CameraFrame> {
        self.last_frame.as_ref()
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Shutdown camera
    pub fn shutdown(&mut self) {
        if self.state == CameraState::Capturing {
            let _ = self.stop_capture();
        }
        self.state = CameraState::Uninitialized;
        tracing::info!("Camera {} shutdown", self.index);
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Camera errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraError {
    /// Camera not initialized
    NotInitialized,
    /// Camera already initialized
    AlreadyInitialized,
    /// Already capturing
    AlreadyCapturing,
    /// Not capturing
    NotCapturing,
    /// Can't change settings while capturing
    CantChangeWhileCapturing,
    /// Device error
    DeviceError,
    /// Invalid resolution
    InvalidResolution,
    /// Camera not found
    NotFound,
}

impl std::fmt::Display for CameraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraError::NotInitialized => write!(f, "Camera not initialized"),
            CameraError::AlreadyInitialized => write!(f, "Camera already initialized"),
            CameraError::AlreadyCapturing => write!(f, "Already capturing"),
            CameraError::NotCapturing => write!(f, "Not capturing"),
            CameraError::CantChangeWhileCapturing => {
                write!(f, "Cannot change settings while capturing")
            }
            CameraError::DeviceError => write!(f, "Device error"),
            CameraError::InvalidResolution => write!(f, "Invalid resolution"),
            CameraError::NotFound => write!(f, "Camera not found"),
        }
    }
}

impl std::error::Error for CameraError {}

/// Camera manager
pub struct CameraManager {
    /// Connected cameras
    cameras: [Option<Camera>; 4],
}

impl CameraManager {
    pub fn new() -> Self {
        Self {
            cameras: Default::default(),
        }
    }

    /// Connect a camera
    pub fn connect(&mut self, index: u8, camera_type: CameraType) -> Result<(), CameraError> {
        if index >= 4 {
            return Err(CameraError::NotFound);
        }

        let mut camera = Camera::new(index, camera_type);
        camera.initialize()?;
        self.cameras[index as usize] = Some(camera);
        Ok(())
    }

    /// Disconnect a camera
    pub fn disconnect(&mut self, index: u8) {
        if let Some(camera) = self.cameras.get_mut(index as usize).and_then(|c| c.as_mut()) {
            camera.shutdown();
        }
        if index < 4 {
            self.cameras[index as usize] = None;
        }
    }

    /// Get camera
    pub fn get(&self, index: u8) -> Option<&Camera> {
        self.cameras.get(index as usize)?.as_ref()
    }

    /// Get mutable camera
    pub fn get_mut(&mut self, index: u8) -> Option<&mut Camera> {
        self.cameras.get_mut(index as usize)?.as_mut()
    }

    /// List connected cameras
    pub fn list_connected(&self) -> Vec<u8> {
        self.cameras
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.as_ref().map(|_| i as u8))
            .collect()
    }

    /// Poll all cameras
    pub fn poll_all(&mut self) {
        for camera in self.cameras.iter_mut().flatten() {
            if camera.state() == CameraState::Capturing {
                camera.poll_frame();
            }
        }
    }
}

impl Default for CameraManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution() {
        assert_eq!(CameraResolution::VGA.width(), 640);
        assert_eq!(CameraResolution::VGA.height(), 480);
        assert_eq!(CameraResolution::QVGA.max_fps(), 120);
    }

    #[test]
    fn test_camera_lifecycle() {
        let mut camera = Camera::new(0, CameraType::PlayStationEye);
        
        assert_eq!(camera.state(), CameraState::Uninitialized);
        
        camera.initialize().unwrap();
        assert_eq!(camera.state(), CameraState::Ready);
        
        camera.start_capture().unwrap();
        assert_eq!(camera.state(), CameraState::Capturing);
        
        // Get a frame
        let frame = camera.poll_frame();
        assert!(frame.is_some());
        assert_eq!(camera.frame_count(), 1);
        
        camera.stop_capture().unwrap();
        assert_eq!(camera.state(), CameraState::Ready);
    }

    #[test]
    fn test_test_pattern() {
        let settings = CameraSettings::default();
        let frame = CameraFrame::test_pattern(&settings, 0);
        
        assert_eq!(frame.width, 640);
        assert_eq!(frame.height, 480);
        assert!(!frame.data.is_empty());
    }

    #[test]
    fn test_camera_manager() {
        let mut manager = CameraManager::new();
        
        manager.connect(0, CameraType::Virtual).unwrap();
        
        let connected = manager.list_connected();
        assert_eq!(connected, vec![0]);
        
        manager.disconnect(0);
        assert!(manager.list_connected().is_empty());
    }

    #[test]
    fn test_bayer_demosaic() {
        let mut processor = ImageProcessor::new();
        processor.settings.resolution = CameraResolution::QVGA;
        
        // Create simple test Bayer pattern
        let width = 320;
        let height = 240;
        let bayer = vec![128u8; width * height];
        
        let rgb = processor.bayer_to_rgb(&bayer, width as u32, height as u32);
        assert_eq!(rgb.len(), width * height * 3);
    }

    #[test]
    fn test_brightness_contrast() {
        let mut processor = ImageProcessor::new();
        processor.settings.brightness = 150;
        processor.settings.contrast = 200;
        
        let input = vec![128u8; 100 * 3]; // 100 gray pixels
        let output = processor.apply_brightness_contrast(&input);
        
        // Brightness increased, so output should be brighter
        assert!(output[0] > 128);
    }

    #[test]
    fn test_white_balance() {
        let settings = WhiteBalanceSettings {
            mode: WhiteBalanceMode::Manual,
            red_gain: 1.2,
            blue_gain: 0.9,
            color_temperature: 5500,
        };
        
        let processor = ImageProcessor::with_white_balance(settings);
        assert!((processor.white_balance.red_gain - 1.2).abs() < 0.001);
    }
}

// =============================================================================
// Camera Image Processing
// =============================================================================

/// Image processor for camera feed
pub struct ImageProcessor {
    /// Processing settings
    pub settings: CameraSettings,
    /// White balance settings
    pub white_balance: WhiteBalanceSettings,
}

impl ImageProcessor {
    /// Create new image processor
    pub fn new() -> Self {
        Self {
            settings: CameraSettings::default(),
            white_balance: WhiteBalanceSettings::default(),
        }
    }

    /// Create with specific white balance settings
    pub fn with_white_balance(white_balance: WhiteBalanceSettings) -> Self {
        Self {
            settings: CameraSettings::default(),
            white_balance,
        }
    }

    /// Convert raw Bayer pattern to RGB using bilinear interpolation
    /// This implements a basic GBRG Bayer demosaicing algorithm
    pub fn bayer_to_rgb(&self, bayer: &[u8], width: u32, height: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let mut rgb = vec![0u8; w * h * 3];

        // GBRG Bayer pattern (PS Eye uses this):
        // Row 0: G B G B G B ...
        // Row 1: R G R G R G ...
        // Row 2: G B G B G B ...
        // Row 3: R G R G R G ...

        // Helper to safely get pixel value with boundary clamping
        let get_pixel = |x: i32, y: i32| -> u16 {
            let cx = x.clamp(0, w as i32 - 1) as usize;
            let cy = y.clamp(0, h as i32 - 1) as usize;
            bayer[cy * w + cx] as u16
        };

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let rgb_idx = idx * 3;
                let xi = x as i32;
                let yi = y as i32;

                let (r, g, b) = if y % 2 == 0 {
                    // Even row (G B G B...)
                    if x % 2 == 0 {
                        // Green pixel at (even x, even y)
                        let g = bayer[idx];
                        let r = (get_pixel(xi, yi - 1) + get_pixel(xi, yi + 1)) / 2;
                        let b = (get_pixel(xi - 1, yi) + get_pixel(xi + 1, yi)) / 2;
                        (r as u8, g, b as u8)
                    } else {
                        // Blue pixel at (odd x, even y)
                        let b = bayer[idx];
                        let g = (get_pixel(xi - 1, yi) + get_pixel(xi + 1, yi) +
                                 get_pixel(xi, yi - 1) + get_pixel(xi, yi + 1)) / 4;
                        let r = (get_pixel(xi - 1, yi - 1) + get_pixel(xi + 1, yi - 1) +
                                 get_pixel(xi - 1, yi + 1) + get_pixel(xi + 1, yi + 1)) / 4;
                        (r as u8, g as u8, b)
                    }
                } else {
                    // Odd row (R G R G...)
                    if x % 2 == 0 {
                        // Red pixel at (even x, odd y)
                        let r = bayer[idx];
                        let g = (get_pixel(xi - 1, yi) + get_pixel(xi + 1, yi) +
                                 get_pixel(xi, yi - 1) + get_pixel(xi, yi + 1)) / 4;
                        let b = (get_pixel(xi - 1, yi - 1) + get_pixel(xi + 1, yi - 1) +
                                 get_pixel(xi - 1, yi + 1) + get_pixel(xi + 1, yi + 1)) / 4;
                        (r, g as u8, b as u8)
                    } else {
                        // Green pixel at (odd x, odd y)
                        let g = bayer[idx];
                        let r = (get_pixel(xi - 1, yi) + get_pixel(xi + 1, yi)) / 2;
                        let b = (get_pixel(xi, yi - 1) + get_pixel(xi, yi + 1)) / 2;
                        (r as u8, g, b as u8)
                    }
                };

                rgb[rgb_idx] = r;
                rgb[rgb_idx + 1] = g;
                rgb[rgb_idx + 2] = b;
            }
        }

        rgb
    }

    /// Apply brightness and contrast adjustments
    pub fn apply_brightness_contrast(&self, rgb: &[u8]) -> Vec<u8> {
        let brightness = self.settings.brightness as f32 - 128.0;
        let contrast = self.settings.contrast as f32 / 128.0;

        rgb.iter()
            .map(|&pixel| {
                // Apply contrast around midpoint (128), then add brightness
                let adjusted = ((pixel as f32 - 128.0) * contrast + 128.0 + brightness)
                    .clamp(0.0, 255.0) as u8;
                adjusted
            })
            .collect()
    }

    /// Apply white balance adjustment
    pub fn apply_white_balance(&self, rgb: &[u8]) -> Vec<u8> {
        let red_gain = self.white_balance.red_gain;
        let blue_gain = self.white_balance.blue_gain;

        rgb.chunks(3)
            .flat_map(|pixel| {
                let r = (pixel[0] as f32 * red_gain).clamp(0.0, 255.0) as u8;
                let g = pixel[1]; // Green channel unchanged
                let b = (pixel[2] as f32 * blue_gain).clamp(0.0, 255.0) as u8;
                [r, g, b]
            })
            .collect()
    }

    /// Flip image horizontally
    pub fn flip_horizontal(&self, rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let mut result = vec![0u8; w * h * 3];

        for y in 0..h {
            for x in 0..w {
                let src_idx = (y * w + x) * 3;
                let dst_idx = (y * w + (w - 1 - x)) * 3;
                result[dst_idx..dst_idx + 3].copy_from_slice(&rgb[src_idx..src_idx + 3]);
            }
        }

        result
    }

    /// Flip image vertically
    pub fn flip_vertical(&self, rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let row_size = w * 3;
        let mut result = vec![0u8; w * h * 3];

        for y in 0..h {
            let src_row = y * row_size;
            let dst_row = (h - 1 - y) * row_size;
            result[dst_row..dst_row + row_size].copy_from_slice(&rgb[src_row..src_row + row_size]);
        }

        result
    }

    /// Process a raw frame (apply all enabled adjustments)
    pub fn process_frame(&self, frame: &CameraFrame) -> CameraFrame {
        let mut data = frame.data.clone();
        let width = frame.width;
        let height = frame.height;

        // Convert from Bayer if needed
        if frame.format == CameraPixelFormat::BayerGB {
            data = self.bayer_to_rgb(&data, width, height);
        }

        // Apply brightness/contrast
        if self.settings.brightness != 128 || self.settings.contrast != 128 {
            data = self.apply_brightness_contrast(&data);
        }

        // Apply white balance
        if !self.settings.auto_white_balance {
            data = self.apply_white_balance(&data);
        }

        // Apply flips
        if self.settings.flip_h {
            data = self.flip_horizontal(&data, width, height);
        }
        if self.settings.flip_v {
            data = self.flip_vertical(&data, width, height);
        }

        CameraFrame {
            data,
            width,
            height,
            format: CameraPixelFormat::RGB24,
            timestamp: frame.timestamp,
            sequence: frame.sequence,
        }
    }
}

impl Default for ImageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// White balance mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteBalanceMode {
    /// Automatic white balance
    Auto,
    /// Manual white balance with gains
    Manual,
    /// Preset for daylight
    Daylight,
    /// Preset for cloudy conditions
    Cloudy,
    /// Preset for incandescent lighting
    Incandescent,
    /// Preset for fluorescent lighting
    Fluorescent,
}

impl Default for WhiteBalanceMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// White balance settings
#[derive(Debug, Clone, Copy)]
pub struct WhiteBalanceSettings {
    /// White balance mode
    pub mode: WhiteBalanceMode,
    /// Red channel gain (1.0 = no change)
    pub red_gain: f32,
    /// Blue channel gain (1.0 = no change)
    pub blue_gain: f32,
    /// Color temperature (Kelvin, for Auto/Manual modes)
    pub color_temperature: u16,
}

impl Default for WhiteBalanceSettings {
    fn default() -> Self {
        Self {
            mode: WhiteBalanceMode::Auto,
            red_gain: 1.0,
            blue_gain: 1.0,
            color_temperature: 5500, // Daylight
        }
    }
}

impl WhiteBalanceSettings {
    /// Create preset for specific lighting conditions
    pub fn preset(mode: WhiteBalanceMode) -> Self {
        let (red_gain, blue_gain, temp) = match mode {
            WhiteBalanceMode::Daylight => (1.0, 1.0, 5500),
            WhiteBalanceMode::Cloudy => (1.05, 0.95, 6500),
            WhiteBalanceMode::Incandescent => (0.9, 1.2, 3200),
            WhiteBalanceMode::Fluorescent => (0.95, 1.1, 4200),
            _ => (1.0, 1.0, 5500),
        };

        Self {
            mode,
            red_gain,
            blue_gain,
            color_temperature: temp,
        }
    }

    /// Estimate gains from color temperature
    pub fn from_color_temperature(kelvin: u16) -> Self {
        // Approximate RGB gains from color temperature
        // Based on Planckian locus approximation
        let temp = kelvin.clamp(1000, 15000) as f32;
        
        let red_gain = if temp <= 6600.0 {
            1.0
        } else {
            1.2929 * ((temp - 6000.0) / 100.0).powf(-0.1332)
        };

        // Blue gain calculation with minimum value to preserve blue channel
        let blue_gain = if temp >= 6600.0 {
            1.0
        } else {
            // Use a minimum of 0.15 to maintain some blue presence even at very low temperatures
            let calculated = 0.5668 * ((temp.max(2001.0) - 2000.0) / 100.0).powf(0.3109);
            calculated.max(0.15)
        };

        Self {
            mode: WhiteBalanceMode::Manual,
            red_gain: red_gain.clamp(0.5, 2.0),
            blue_gain: blue_gain.clamp(0.15, 2.0),
            color_temperature: kelvin,
        }
    }
}

/// Color correction matrix (3x3)
#[derive(Debug, Clone, Copy)]
pub struct ColorCorrectionMatrix {
    /// Matrix coefficients [row][col]
    pub matrix: [[f32; 3]; 3],
}

impl Default for ColorCorrectionMatrix {
    fn default() -> Self {
        // Identity matrix (no correction)
        Self {
            matrix: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }
}

impl ColorCorrectionMatrix {
    /// Apply color correction to RGB values
    pub fn apply(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;

        let new_r = (self.matrix[0][0] * rf + self.matrix[0][1] * gf + self.matrix[0][2] * bf)
            .clamp(0.0, 255.0) as u8;
        let new_g = (self.matrix[1][0] * rf + self.matrix[1][1] * gf + self.matrix[1][2] * bf)
            .clamp(0.0, 255.0) as u8;
        let new_b = (self.matrix[2][0] * rf + self.matrix[2][1] * gf + self.matrix[2][2] * bf)
            .clamp(0.0, 255.0) as u8;

        (new_r, new_g, new_b)
    }

    /// Create saturation adjustment matrix
    pub fn saturation(amount: f32) -> Self {
        // Grayscale coefficients
        let lr = 0.2126;
        let lg = 0.7152;
        let lb = 0.0722;
        let s = amount;
        let sr = (1.0 - s) * lr;
        let sg = (1.0 - s) * lg;
        let sb = (1.0 - s) * lb;

        Self {
            matrix: [
                [sr + s, sg, sb],
                [sr, sg + s, sb],
                [sr, sg, sb + s],
            ],
        }
    }
}
