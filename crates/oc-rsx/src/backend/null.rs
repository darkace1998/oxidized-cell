//! Null backend for testing

use super::{GraphicsBackend, FramebufferData, PrimitiveType};
use crate::vertex::VertexAttribute;

/// Null graphics backend (does nothing but provides test pattern)
pub struct NullBackend {
    width: u32,
    height: u32,
    frame_count: u64,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            width: 1280,
            height: 720,
            frame_count: 0,
        }
    }
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphicsBackend for NullBackend {
    fn init(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn shutdown(&mut self) {}

    fn begin_frame(&mut self) {}

    fn end_frame(&mut self) {
        self.frame_count += 1;
    }

    fn clear(&mut self, _color: [f32; 4], _depth: f32, _stencil: u8) {}

    fn draw_arrays(&mut self, _primitive: PrimitiveType, _first: u32, _count: u32) {}

    fn draw_indexed(&mut self, _primitive: PrimitiveType, _first: u32, _count: u32) {}

    fn set_vertex_attributes(&mut self, _attributes: &[VertexAttribute]) {}

    fn bind_texture(&mut self, _slot: u32, _offset: u32) {}

    fn set_viewport(&mut self, _x: f32, _y: f32, _width: f32, _height: f32, _min_depth: f32, _max_depth: f32) {}

    fn set_scissor(&mut self, _x: u32, _y: u32, _width: u32, _height: u32) {}
    
    fn submit_vertex_buffer(&mut self, _binding: u32, _data: &[u8], _stride: u32) {}
    
    fn submit_index_buffer(&mut self, _data: &[u8], _index_type: u32) {}
    
    fn get_framebuffer(&self) -> Option<FramebufferData> {
        // Return an animated test pattern for the null backend
        let mut fb = FramebufferData::new(self.width, self.height);
        
        let time = (self.frame_count as f32 * 0.02).sin() * 0.5 + 0.5;
        
        for y in 0..self.height {
            for x in 0..self.width {
                let i = ((y * self.width + x) * 4) as usize;
                
                // Animated gradient pattern
                let r = ((x as f32 / self.width as f32) * 255.0 * time) as u8;
                let g = ((y as f32 / self.height as f32) * 255.0) as u8;
                let b = (((x + y) as f32 / (self.width + self.height) as f32) * 255.0 * (1.0 - time)) as u8;
                
                fb.pixels[i] = r;
                fb.pixels[i + 1] = g;
                fb.pixels[i + 2] = b;
                fb.pixels[i + 3] = 255;
            }
        }
        
        Some(fb)
    }
    
    fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_backend() {
        let mut backend = NullBackend::new();
        assert!(backend.init().is_ok());
        backend.begin_frame();
        backend.clear([0.0, 0.0, 0.0, 1.0], 1.0, 0);
        backend.end_frame();
        backend.shutdown();
    }
}
