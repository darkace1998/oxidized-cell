//! Configuration system for oxidized-cell emulator

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub general: GeneralConfig,
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub audio: AudioConfig,
    pub input: InputConfig,
    pub paths: PathConfig,
    pub debug: DebugConfig,
}

/// General emulator settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub start_paused: bool,
    pub confirm_exit: bool,
    pub auto_save_state: bool,
}

/// CPU emulation settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CpuConfig {
    pub ppu_decoder: PpuDecoder,
    pub spu_decoder: SpuDecoder,
    pub ppu_threads: u32,
    pub spu_threads: u32,
    pub accurate_dfma: bool,
    pub accurate_rsx_reservation: bool,
    pub spu_loop_detection: bool,
    /// Enable cycle-accurate timing simulation
    pub cycle_accurate_timing: bool,
    /// Enable pipeline simulation
    pub pipeline_simulation: bool,
    /// Enable power management emulation
    pub power_management: bool,
    /// Enable cache simulation
    pub cache_simulation: bool,
    /// Enable memory access profiling
    pub memory_profiling: bool,
}

/// PPU decoder type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum PpuDecoder {
    Interpreter,
    #[default]
    Recompiler,
}

/// SPU decoder type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum SpuDecoder {
    Interpreter,
    #[default]
    Recompiler,
}

/// GPU settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GpuConfig {
    pub backend: GpuBackend,
    pub resolution_scale: u32,
    pub anisotropic_filter: u32,
    pub vsync: bool,
    pub frame_limit: u32,
    pub shader_cache: bool,
    pub write_color_buffers: bool,
    pub write_depth_buffer: bool,
}

/// GPU backend type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum GpuBackend {
    #[default]
    Vulkan,
    Null,
}

/// Audio settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub backend: AudioBackend,
    pub enable: bool,
    pub volume: f32,
    pub buffer_duration_ms: u32,
    pub time_stretching: bool,
}

/// Audio backend type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum AudioBackend {
    #[default]
    Auto,
    Null,
}

/// Input settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct InputConfig {
    pub controller: ControllerConfig,
    pub keyboard_mapping: KeyboardMapping,
}

/// Controller configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ControllerConfig {
    pub player1: Option<String>,
    pub player2: Option<String>,
    pub player3: Option<String>,
    pub player4: Option<String>,
}

/// Keyboard to PS3 button mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardMapping {
    pub cross: String,
    pub circle: String,
    pub square: String,
    pub triangle: String,
    pub l1: String,
    pub l2: String,
    pub l3: String,
    pub r1: String,
    pub r2: String,
    pub r3: String,
    pub start: String,
    pub select: String,
    pub dpad_up: String,
    pub dpad_down: String,
    pub dpad_left: String,
    pub dpad_right: String,
}

/// Path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathConfig {
    pub games: PathBuf,
    pub dev_hdd0: PathBuf,
    pub dev_hdd1: PathBuf,
    pub dev_flash: PathBuf,
    pub save_data: PathBuf,
    pub shader_cache: PathBuf,
    pub firmware: PathBuf,
}

/// Debug settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugConfig {
    pub log_level: LogLevel,
    pub log_to_file: bool,
    pub log_path: PathBuf,
    pub dump_shaders: bool,
    pub trace_ppu: bool,
    pub trace_spu: bool,
    pub trace_rsx: bool,
    pub breakpoints: Vec<u32>,
}

/// Logging level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

// Default implementations


impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_paused: false,
            confirm_exit: true,
            auto_save_state: false,
        }
    }
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            ppu_decoder: PpuDecoder::default(),
            spu_decoder: SpuDecoder::default(),
            ppu_threads: 2,
            spu_threads: 6,
            accurate_dfma: false,
            accurate_rsx_reservation: false,
            spu_loop_detection: true,
            cycle_accurate_timing: false,
            pipeline_simulation: false,
            power_management: false,
            cache_simulation: false,
            memory_profiling: false,
        }
    }
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            backend: GpuBackend::default(),
            resolution_scale: 100,
            anisotropic_filter: 8,
            vsync: true,
            frame_limit: 60,
            shader_cache: true,
            write_color_buffers: false,
            write_depth_buffer: false,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            backend: AudioBackend::default(),
            enable: true,
            volume: 1.0,
            buffer_duration_ms: 100,
            time_stretching: true,
        }
    }
}


impl Default for KeyboardMapping {
    fn default() -> Self {
        Self {
            cross: "X".to_string(),
            circle: "C".to_string(),
            square: "Z".to_string(),
            triangle: "V".to_string(),
            l1: "Q".to_string(),
            l2: "1".to_string(),
            l3: "F".to_string(),
            r1: "E".to_string(),
            r2: "3".to_string(),
            r3: "G".to_string(),
            start: "Return".to_string(),
            select: "Backspace".to_string(),
            dpad_up: "Up".to_string(),
            dpad_down: "Down".to_string(),
            dpad_left: "Left".to_string(),
            dpad_right: "Right".to_string(),
        }
    }
}

impl Default for PathConfig {
    fn default() -> Self {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oxidized-cell");

        Self {
            games: base.join("games"),
            dev_hdd0: base.join("dev_hdd0"),
            dev_hdd1: base.join("dev_hdd1"),
            dev_flash: base.join("dev_flash"),
            save_data: base.join("savedata"),
            shader_cache: base.join("cache/shaders"),
            firmware: base.join("firmware"),
        }
    }
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            log_level: LogLevel::default(),
            log_to_file: false,
            log_path: PathBuf::from("oxidized-cell.log"),
            dump_shaders: false,
            trace_ppu: false,
            trace_spu: false,
            trace_rsx: false,
            breakpoints: Vec::new(),
        }
    }
}

impl Config {
    /// Load configuration from file, or create default if it doesn't exist
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path();

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get the path to the configuration file
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oxidized-cell")
            .join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.general.start_paused);
        assert!(config.general.confirm_exit);
        assert_eq!(config.cpu.ppu_threads, 2);
        assert_eq!(config.cpu.spu_threads, 6);
        assert_eq!(config.gpu.resolution_scale, 100);
        assert!(config.audio.enable);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.cpu.ppu_threads, config.cpu.ppu_threads);
    }
}
