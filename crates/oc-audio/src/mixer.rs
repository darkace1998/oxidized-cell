//! Audio mixer
//!
//! Provides audio mixing capabilities for multiple audio sources.

use std::collections::HashMap;

/// Audio sample format
pub type Sample = f32;

/// Audio channel configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround51,
    Surround71,
}

impl ChannelLayout {
    pub fn num_channels(&self) -> usize {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
            ChannelLayout::Surround51 => 6,
            ChannelLayout::Surround71 => 8,
        }
    }
}

/// Audio source identifier
pub type SourceId = u32;

/// Audio source
pub struct AudioSource {
    /// Source ID
    pub id: SourceId,
    /// Channel layout
    pub layout: ChannelLayout,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
    /// Audio buffer
    pub buffer: Vec<Sample>,
}

impl AudioSource {
    pub fn new(id: SourceId, layout: ChannelLayout) -> Self {
        Self {
            id,
            layout,
            volume: 1.0,
            buffer: Vec::new(),
        }
    }

    pub fn write_samples(&mut self, samples: &[Sample]) {
        self.buffer.extend_from_slice(samples);
    }

    pub fn read_samples(&mut self, count: usize) -> Vec<Sample> {
        let available = self.buffer.len().min(count);
        let samples = self.buffer.drain(..available).collect();
        samples
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Audio mixer
pub struct AudioMixer {
    /// Audio sources
    sources: HashMap<SourceId, AudioSource>,
    /// Master volume
    master_volume: f32,
    /// Output channel layout
    output_layout: ChannelLayout,
    /// Next source ID
    next_id: SourceId,
}

impl AudioMixer {
    /// Create a new audio mixer
    pub fn new(output_layout: ChannelLayout) -> Self {
        Self {
            sources: HashMap::new(),
            master_volume: 1.0,
            output_layout,
            next_id: 0,
        }
    }

    /// Add a new audio source
    pub fn add_source(&mut self, layout: ChannelLayout) -> SourceId {
        let id = self.next_id;
        self.next_id += 1;
        
        let source = AudioSource::new(id, layout);
        self.sources.insert(id, source);
        
        tracing::debug!("Audio source {} added with {:?} layout", id, layout);
        id
    }

    /// Remove an audio source
    pub fn remove_source(&mut self, id: SourceId) -> bool {
        if self.sources.remove(&id).is_some() {
            tracing::debug!("Audio source {} removed", id);
            true
        } else {
            false
        }
    }

    /// Write samples to a source
    pub fn write_to_source(&mut self, id: SourceId, samples: &[Sample]) -> Result<(), String> {
        if let Some(source) = self.sources.get_mut(&id) {
            source.write_samples(samples);
            Ok(())
        } else {
            Err(format!("Source {} not found", id))
        }
    }

    /// Set source volume
    pub fn set_source_volume(&mut self, id: SourceId, volume: f32) -> Result<(), String> {
        if let Some(source) = self.sources.get_mut(&id) {
            source.volume = volume.clamp(0.0, 1.0);
            Ok(())
        } else {
            Err(format!("Source {} not found", id))
        }
    }

    /// Set master volume
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    /// Get master volume
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Mix audio sources into output buffer
    pub fn mix(&mut self, output: &mut [Sample], frames: usize) {
        let channels = self.output_layout.num_channels();
        let samples_needed = frames * channels;
        
        // Clear output buffer
        for sample in output[..samples_needed].iter_mut() {
            *sample = 0.0;
        }

        // Mix all sources
        for source in self.sources.values_mut() {
            let source_samples = source.read_samples(samples_needed);
            
            // Apply volume and mix into output
            for (i, &sample) in source_samples.iter().enumerate() {
                if i < samples_needed {
                    output[i] += sample * source.volume * self.master_volume;
                }
            }
        }

        // Clamp output to prevent clipping
        for sample in output[..samples_needed].iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }

    /// Clear all sources
    pub fn clear_all(&mut self) {
        for source in self.sources.values_mut() {
            source.clear();
        }
    }
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new(ChannelLayout::Stereo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_creation() {
        let mixer = AudioMixer::new(ChannelLayout::Stereo);
        assert_eq!(mixer.master_volume(), 1.0);
    }

    #[test]
    fn test_add_remove_source() {
        let mut mixer = AudioMixer::new(ChannelLayout::Stereo);
        
        let id = mixer.add_source(ChannelLayout::Stereo);
        assert!(mixer.remove_source(id));
        assert!(!mixer.remove_source(id));
    }

    #[test]
    fn test_write_and_mix() {
        let mut mixer = AudioMixer::new(ChannelLayout::Stereo);
        let id = mixer.add_source(ChannelLayout::Stereo);
        
        let samples = vec![0.5, 0.5, 0.5, 0.5];
        mixer.write_to_source(id, &samples).unwrap();
        
        let mut output = vec![0.0; 4];
        mixer.mix(&mut output, 2);
        
        // Check that some mixing occurred
        assert!(output.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_volume_control() {
        let mut mixer = AudioMixer::new(ChannelLayout::Stereo);
        let id = mixer.add_source(ChannelLayout::Stereo);
        
        mixer.set_source_volume(id, 0.5).unwrap();
        mixer.set_master_volume(0.75);
        
        assert_eq!(mixer.master_volume(), 0.75);
    }

    #[test]
    fn test_channel_layouts() {
        assert_eq!(ChannelLayout::Mono.num_channels(), 1);
        assert_eq!(ChannelLayout::Stereo.num_channels(), 2);
        assert_eq!(ChannelLayout::Surround51.num_channels(), 6);
        assert_eq!(ChannelLayout::Surround71.num_channels(), 8);
    }
}
