//! Null backend for testing

use super::{GraphicsBackend, FramebufferData, PrimitiveType};
use crate::vertex::VertexAttribute;

/// Null graphics backend that provides a visible framebuffer during development.
///
/// When no Vulkan device is available, this backend returns a recognizable
/// solid-color framebuffer with an animated stripe so the egui emulation view
/// still shows something and the developer can tell the emulator is running.
pub struct NullBackend {
    width: u32,
    height: u32,
    frame_count: u64,
    /// Last clear color set by the game (if any)
    clear_color: [f32; 4],
    /// Whether clear() was called at least once
    has_cleared: bool,
    /// Draw call count this frame (for activity indicator)
    draw_calls_this_frame: u32,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            width: 1280,
            height: 720,
            frame_count: 0,
            clear_color: [0.0, 0.0, 0.3, 1.0], // Dark blue default
            has_cleared: false,
            draw_calls_this_frame: 0,
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

    fn begin_frame(&mut self) {
        self.draw_calls_this_frame = 0;
    }

    fn end_frame(&mut self) {
        self.frame_count += 1;
    }

    fn clear(&mut self, color: [f32; 4], _depth: f32, _stencil: u8) {
        self.clear_color = color;
        self.has_cleared = true;
    }

    fn draw_arrays(&mut self, _primitive: PrimitiveType, _first: u32, _count: u32) {
        self.draw_calls_this_frame += 1;
    }

    fn draw_indexed(&mut self, _primitive: PrimitiveType, _first: u32, _count: u32) {
        self.draw_calls_this_frame += 1;
    }

    fn set_vertex_attributes(&mut self, _attributes: &[VertexAttribute]) {}

    fn bind_texture(&mut self, _slot: u32, _offset: u32) {}

    fn set_viewport(&mut self, _x: f32, _y: f32, _width: f32, _height: f32, _min_depth: f32, _max_depth: f32) {}

    fn set_scissor(&mut self, _x: u32, _y: u32, _width: u32, _height: u32) {}
    
    fn submit_vertex_buffer(&mut self, _binding: u32, _data: &[u8], _stride: u32) {}
    
    fn submit_index_buffer(&mut self, _data: &[u8], _index_type: u32) {}
    
    fn get_framebuffer(&self) -> Option<FramebufferData> {
        let mut fb = FramebufferData::new(self.width, self.height);
        
        // Use the game's clear color if it has been set, otherwise use dark blue
        let base_r = (self.clear_color[0] * 255.0) as u8;
        let base_g = (self.clear_color[1] * 255.0) as u8;
        let base_b = (self.clear_color[2] * 255.0) as u8;
        
        // Fill with the clear color as the base
        for y in 0..self.height {
            for x in 0..self.width {
                let i = ((y * self.width + x) * 4) as usize;
                fb.pixels[i] = base_r;
                fb.pixels[i + 1] = base_g;
                fb.pixels[i + 2] = base_b;
                fb.pixels[i + 3] = 255;
            }
        }
        
        // Draw an animated horizontal stripe to show the emulator is alive
        // The stripe moves vertically based on frame_count
        let stripe_y = (self.frame_count as u32 * 2) % self.height;
        let stripe_height = 4u32;
        for dy in 0..stripe_height {
            let y = (stripe_y + dy) % self.height;
            for x in 0..self.width {
                let i = ((y * self.width + x) * 4) as usize;
                // Bright white stripe
                fb.pixels[i] = 255;
                fb.pixels[i + 1] = 255;
                fb.pixels[i + 2] = 255;
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
    
    #[test]
    fn test_null_backend_framebuffer_returns_data() {
        let backend = NullBackend::new();
        let fb = backend.get_framebuffer();
        assert!(fb.is_some());
        let fb = fb.unwrap();
        assert_eq!(fb.width, 1280);
        assert_eq!(fb.height, 720);
        assert_eq!(fb.pixels.len(), (1280 * 720 * 4) as usize);
        // All pixels should have full alpha
        for y in 0..fb.height {
            for x in 0..fb.width {
                let i = ((y * fb.width + x) * 4 + 3) as usize;
                assert_eq!(fb.pixels[i], 255, "Alpha should be 255 at ({}, {})", x, y);
            }
        }
    }
    
    #[test]
    fn test_null_backend_clear_color_applied() {
        let mut backend = NullBackend::new();
        backend.clear([1.0, 0.0, 0.0, 1.0], 1.0, 0); // Red
        let fb = backend.get_framebuffer().unwrap();
        // Most pixels should be red (except the animated stripe)
        let i = 0usize; // Top-left pixel (may or may not be in stripe)
        // Check a pixel that's definitely not in the first stripe
        let safe_y = 100u32; // Well below stripe start at frame 0
        let safe_i = ((safe_y * fb.width) * 4) as usize;
        assert_eq!(fb.pixels[safe_i], 255, "Red channel should be 255");
        assert_eq!(fb.pixels[safe_i + 1], 0, "Green channel should be 0");
        assert_eq!(fb.pixels[safe_i + 2], 0, "Blue channel should be 0");
    }
    
    #[test]
    fn test_null_backend_draw_call_tracking() {
        let mut backend = NullBackend::new();
        backend.begin_frame();
        assert_eq!(backend.draw_calls_this_frame, 0);
        backend.draw_arrays(PrimitiveType::Triangles, 0, 3);
        assert_eq!(backend.draw_calls_this_frame, 1);
        backend.draw_indexed(PrimitiveType::Triangles, 0, 6);
        assert_eq!(backend.draw_calls_this_frame, 2);
        backend.end_frame();
    }
    
    #[test]
    fn test_null_backend_animated_stripe() {
        let mut backend = NullBackend::new();
        // Get framebuffer at frame 0
        let fb0 = backend.get_framebuffer().unwrap();
        // Advance a few frames
        for _ in 0..10 {
            backend.begin_frame();
            backend.end_frame();
        }
        let fb10 = backend.get_framebuffer().unwrap();
        // The framebuffers should differ (stripe moved)
        assert_ne!(fb0.pixels, fb10.pixels, "Framebuffer should change over time (animated stripe)");
    }
}
