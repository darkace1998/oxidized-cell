//! RSX buffer management for color and depth buffers
//!
//! This module handles render target buffers including color and depth/stencil.

use std::sync::Arc;
use oc_memory::MemoryManager;

/// Surface format constants
pub mod format {
    pub const ARGB8: u32 = 0x05;
    pub const FLOAT_R32: u32 = 0x0A;
    pub const DEPTH24_STENCIL8: u32 = 0x0E;
}

/// Depth value conversion constants
const DEPTH_24BIT_MAX: f32 = 16777215.0; // 2^24 - 1

/// Convert normalized depth (0.0-1.0) to 24-bit depth value
fn depth_to_24bit(depth: f32) -> u32 {
    (depth.clamp(0.0, 1.0) * DEPTH_24BIT_MAX) as u32
}

/// Color buffer descriptor
#[derive(Debug, Clone)]
pub struct ColorBuffer {
    /// GPU memory offset
    pub offset: u32,
    /// Buffer pitch (bytes per row)
    pub pitch: u32,
    /// Buffer width
    pub width: u16,
    /// Buffer height
    pub height: u16,
    /// Surface format
    pub format: u32,
    /// DMA context
    pub dma_context: u32,
}

impl ColorBuffer {
    /// Create a new color buffer
    pub fn new(offset: u32, pitch: u32, width: u16, height: u16, format: u32) -> Self {
        Self {
            offset,
            pitch,
            width,
            height,
            format,
            dma_context: 0,
        }
    }

    /// Get buffer size in bytes
    pub fn size(&self) -> u32 {
        self.pitch * (self.height as u32)
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> u32 {
        match self.format {
            format::ARGB8 => 4,
            format::FLOAT_R32 => 4,
            _ => 4,
        }
    }

    /// Write pixel data to buffer
    pub fn write_pixel(
        &self,
        memory: &Arc<MemoryManager>,
        x: u16,
        y: u16,
        color: [u8; 4],
    ) -> Result<(), String> {
        if x >= self.width || y >= self.height {
            return Err(format!("Pixel ({}, {}) out of bounds", x, y));
        }

        let pixel_offset = self.offset + (y as u32 * self.pitch) + (x as u32 * self.bytes_per_pixel());

        match self.format {
            format::ARGB8 => {
                // Write ARGB8 pixel
                memory.write_be32(pixel_offset, u32::from_be_bytes(color))
                    .map_err(|e| format!("Failed to write color pixel: {:?}", e))?;
            }
            _ => {
                return Err(format!("Unsupported color format: 0x{:02X}", self.format));
            }
        }

        Ok(())
    }

    /// Clear buffer with color
    pub fn clear(
        &self,
        memory: &Arc<MemoryManager>,
        color: [u8; 4],
    ) -> Result<(), String> {
        let color_u32 = u32::from_be_bytes(color);
        let pixel_count = (self.width as u32) * (self.height as u32);

        for i in 0..pixel_count {
            let offset = self.offset + (i * self.bytes_per_pixel());
            memory.write_be32(offset, color_u32)
                .map_err(|e| format!("Failed to clear color buffer: {:?}", e))?;
        }

        Ok(())
    }
}

/// Depth/stencil buffer descriptor
#[derive(Debug, Clone)]
pub struct DepthBuffer {
    /// GPU memory offset
    pub offset: u32,
    /// Buffer pitch (bytes per row)
    pub pitch: u32,
    /// Buffer width
    pub width: u16,
    /// Buffer height
    pub height: u16,
    /// Surface format
    pub format: u32,
    /// DMA context
    pub dma_context: u32,
}

impl DepthBuffer {
    /// Create a new depth buffer
    pub fn new(offset: u32, pitch: u32, width: u16, height: u16, format: u32) -> Self {
        Self {
            offset,
            pitch,
            width,
            height,
            format,
            dma_context: 0,
        }
    }

    /// Get buffer size in bytes
    pub fn size(&self) -> u32 {
        self.pitch * (self.height as u32)
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> u32 {
        match self.format {
            format::DEPTH24_STENCIL8 => 4,
            _ => 4,
        }
    }

    /// Write depth/stencil value
    pub fn write_depth_stencil(
        &self,
        memory: &Arc<MemoryManager>,
        x: u16,
        y: u16,
        depth: f32,
        stencil: u8,
    ) -> Result<(), String> {
        if x >= self.width || y >= self.height {
            return Err(format!("Pixel ({}, {}) out of bounds", x, y));
        }

        let pixel_offset = self.offset + (y as u32 * self.pitch) + (x as u32 * self.bytes_per_pixel());

        match self.format {
            format::DEPTH24_STENCIL8 => {
                // Convert depth (0.0-1.0) to 24-bit value
                let depth_24 = depth_to_24bit(depth);
                let value = (depth_24 << 8) | (stencil as u32);

                memory.write_be32(pixel_offset, value)
                    .map_err(|e| format!("Failed to write depth/stencil: {:?}", e))?;
            }
            _ => {
                return Err(format!("Unsupported depth format: 0x{:02X}", self.format));
            }
        }

        Ok(())
    }

    /// Clear depth buffer
    pub fn clear_depth(
        &self,
        memory: &Arc<MemoryManager>,
        depth: f32,
    ) -> Result<(), String> {
        let depth_24 = depth_to_24bit(depth);
        let value = depth_24 << 8;
        let pixel_count = (self.width as u32) * (self.height as u32);

        for i in 0..pixel_count {
            let offset = self.offset + (i * self.bytes_per_pixel());
            memory.write_be32(offset, value)
                .map_err(|e| format!("Failed to clear depth buffer: {:?}", e))?;
        }

        Ok(())
    }

    /// Clear stencil buffer
    pub fn clear_stencil(
        &self,
        memory: &Arc<MemoryManager>,
        stencil: u8,
    ) -> Result<(), String> {
        let pixel_count = (self.width as u32) * (self.height as u32);

        for i in 0..pixel_count {
            let offset = self.offset + (i * self.bytes_per_pixel());
            // Read existing depth value, update only stencil
            let existing = memory.read_be32(offset)
                .map_err(|e| format!("Failed to read for stencil clear: {:?}", e))?;
            let new_value = (existing & 0xFFFFFF00) | (stencil as u32);
            memory.write_be32(offset, new_value)
                .map_err(|e| format!("Failed to clear stencil buffer: {:?}", e))?;
        }

        Ok(())
    }
}

/// Render target configuration
#[derive(Debug, Clone)]
pub struct RenderTarget {
    /// Color buffers (up to 4 MRTs)
    pub color_buffers: Vec<ColorBuffer>,
    /// Depth/stencil buffer
    pub depth_buffer: Option<DepthBuffer>,
}

impl RenderTarget {
    /// Create a new render target
    pub fn new() -> Self {
        Self {
            color_buffers: Vec::new(),
            depth_buffer: None,
        }
    }

    /// Set color buffer
    pub fn set_color_buffer(&mut self, index: usize, buffer: ColorBuffer) {
        if index >= self.color_buffers.len() {
            self.color_buffers.resize(index + 1, ColorBuffer::new(0, 0, 0, 0, 0));
        }
        self.color_buffers[index] = buffer;
    }

    /// Set depth buffer
    pub fn set_depth_buffer(&mut self, buffer: DepthBuffer) {
        self.depth_buffer = Some(buffer);
    }

    /// Clear all buffers
    pub fn clear_all(
        &self,
        memory: &Arc<MemoryManager>,
        color: [u8; 4],
        depth: f32,
        stencil: u8,
    ) -> Result<(), String> {
        // Clear all color buffers
        for buffer in &self.color_buffers {
            if buffer.width > 0 && buffer.height > 0 {
                buffer.clear(memory, color)?;
            }
        }

        // Clear depth/stencil buffer
        if let Some(depth_buffer) = &self.depth_buffer {
            depth_buffer.clear_depth(memory, depth)?;
            depth_buffer.clear_stencil(memory, stencil)?;
        }

        Ok(())
    }
}

impl Default for RenderTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_buffer_creation() {
        let buffer = ColorBuffer::new(0x1000, 1920 * 4, 1920, 1080, format::ARGB8);
        assert_eq!(buffer.offset, 0x1000);
        assert_eq!(buffer.width, 1920);
        assert_eq!(buffer.height, 1080);
        assert_eq!(buffer.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_color_buffer_size() {
        let buffer = ColorBuffer::new(0, 1920 * 4, 1920, 1080, format::ARGB8);
        assert_eq!(buffer.size(), 1920 * 4 * 1080);
    }

    #[test]
    fn test_depth_buffer_creation() {
        let buffer = DepthBuffer::new(0x2000, 1920 * 4, 1920, 1080, format::DEPTH24_STENCIL8);
        assert_eq!(buffer.offset, 0x2000);
        assert_eq!(buffer.width, 1920);
        assert_eq!(buffer.height, 1080);
        assert_eq!(buffer.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_render_target() {
        let mut rt = RenderTarget::new();
        assert_eq!(rt.color_buffers.len(), 0);
        assert!(rt.depth_buffer.is_none());

        let color = ColorBuffer::new(0x1000, 1920 * 4, 1920, 1080, format::ARGB8);
        rt.set_color_buffer(0, color);
        assert_eq!(rt.color_buffers.len(), 1);

        let depth = DepthBuffer::new(0x2000, 1920 * 4, 1920, 1080, format::DEPTH24_STENCIL8);
        rt.set_depth_buffer(depth);
        assert!(rt.depth_buffer.is_some());
    }

    #[test]
    fn test_color_buffer_write_pixel_out_of_bounds() {
        let memory = MemoryManager::new().unwrap();
        let buffer = ColorBuffer::new(0x1000, 1920 * 4, 1920, 1080, format::ARGB8);
        
        let result = buffer.write_pixel(&memory, 2000, 100, [255, 0, 0, 255]);
        assert!(result.is_err());
    }
}
