//! cpal audio backend
//!
//! Audio backend implementation using the cpal (Cross-Platform Audio Library).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig, SupportedStreamConfig};
use std::sync::Arc;
use parking_lot::Mutex;

/// Sample type for audio output
pub type AudioSample = f32;

/// Audio callback function type
pub type AudioCallback = Arc<dyn Fn(&mut [AudioSample]) + Send + Sync>;

/// cpal audio backend
pub struct CpalAudioBackend {
    host: Host,
    device: Option<Device>,
    config: Option<SupportedStreamConfig>,
    stream: Option<Stream>,
    callback: Arc<Mutex<Option<AudioCallback>>>,
}

impl CpalAudioBackend {
    /// Create a new cpal audio backend
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        
        Ok(Self {
            host,
            device: None,
            config: None,
            stream: None,
            callback: Arc::new(Mutex::new(None)),
        })
    }

    /// Initialize the audio device
    pub fn init(&mut self) -> Result<(), String> {
        // Get default output device
        let device = self.host
            .default_output_device()
            .ok_or("No output device available")?;
        
        tracing::info!("Audio device: {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));
        
        // Get default output config
        let config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {}", e))?;
        
        tracing::info!("Audio config: {:?}", config);
        
        self.device = Some(device);
        self.config = Some(config);
        
        Ok(())
    }

    /// Set the audio callback
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut [AudioSample]) + Send + Sync + 'static,
    {
        *self.callback.lock() = Some(Arc::new(callback));
    }

    /// Start audio playback
    pub fn start(&mut self) -> Result<(), String> {
        let device = self.device.as_ref()
            .ok_or("Device not initialized")?;
        
        let config = self.config.as_ref()
            .ok_or("Config not initialized")?;
        
        let callback = Arc::clone(&self.callback);
        
        // Create stream config
        let stream_config: StreamConfig = config.clone().into();
        
        // Build output stream
        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [AudioSample], _: &cpal::OutputCallbackInfo| {
                    if let Some(ref cb) = *callback.lock() {
                        cb(data);
                    } else {
                        // Fill with silence if no callback
                        data.fill(0.0);
                    }
                },
                |err| {
                    tracing::error!("Audio stream error: {}", err);
                },
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;
        
        // Start the stream
        stream.play()
            .map_err(|e| format!("Failed to play stream: {}", e))?;
        
        self.stream = Some(stream);
        tracing::info!("Audio stream started");
        
        Ok(())
    }

    /// Stop audio playback
    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(stream) = self.stream.take() {
            stream.pause()
                .map_err(|e| format!("Failed to pause stream: {}", e))?;
            tracing::info!("Audio stream stopped");
        }
        Ok(())
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> Option<u32> {
        self.config.as_ref().map(|c| c.sample_rate().0)
    }

    /// Get number of channels
    pub fn channels(&self) -> Option<u16> {
        self.config.as_ref().map(|c| c.channels())
    }
}

impl Default for CpalAudioBackend {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            tracing::warn!("Failed to create cpal backend: {}", e);
            Self {
                host: cpal::default_host(),
                device: None,
                config: None,
                stream: None,
                callback: Arc::new(Mutex::new(None)),
            }
        })
    }
}

impl Drop for CpalAudioBackend {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        // This may fail in CI environments without audio
        let result = CpalAudioBackend::new();
        if result.is_ok() {
            let backend = result.unwrap();
            assert!(backend.device.is_none());
        }
    }

    #[test]
    fn test_backend_init() {
        // This may fail in CI environments without audio
        if let Ok(mut backend) = CpalAudioBackend::new() {
            let _ = backend.init(); // May fail in headless environments
        }
    }
}
