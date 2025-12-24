//! Null backend for testing

use super::{GraphicsBackend, PrimitiveType};
use crate::vertex::VertexAttribute;

/// Null graphics backend (does nothing)
pub struct NullBackend;

impl NullBackend {
    pub fn new() -> Self {
        Self
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

    fn end_frame(&mut self) {}

    fn clear(&mut self, _color: [f32; 4], _depth: f32, _stencil: u8) {}

    fn draw_arrays(&mut self, _primitive: PrimitiveType, _first: u32, _count: u32) {}

    fn draw_indexed(&mut self, _primitive: PrimitiveType, _first: u32, _count: u32) {}

    fn set_vertex_attributes(&mut self, _attributes: &[VertexAttribute]) {}

    fn bind_texture(&mut self, _slot: u32, _offset: u32) {}

    fn set_viewport(&mut self, _x: f32, _y: f32, _width: f32, _height: f32, _min_depth: f32, _max_depth: f32) {}

    fn set_scissor(&mut self, _x: u32, _y: u32, _width: u32, _height: u32) {}
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
