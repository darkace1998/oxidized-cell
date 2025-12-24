//! RSX rendering backends

pub mod null;
pub mod vulkan;

use crate::vertex::VertexAttribute;

/// Primitive topology types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Points = 1,
    Lines = 2,
    LineLoop = 3,
    LineStrip = 4,
    Triangles = 5,
    TriangleStrip = 6,
    TriangleFan = 7,
    Quads = 8,
    QuadStrip = 9,
    Polygon = 10,
}

/// Graphics backend trait
pub trait GraphicsBackend {
    /// Initialize the backend
    fn init(&mut self) -> Result<(), String>;
    
    /// Shutdown the backend
    fn shutdown(&mut self);
    
    /// Begin a frame
    fn begin_frame(&mut self);
    
    /// End a frame and present
    fn end_frame(&mut self);
    
    /// Clear the screen
    fn clear(&mut self, color: [f32; 4], depth: f32, stencil: u8);

    /// Draw arrays (non-indexed)
    fn draw_arrays(&mut self, primitive: PrimitiveType, first: u32, count: u32);

    /// Draw indexed arrays
    fn draw_indexed(&mut self, primitive: PrimitiveType, first: u32, count: u32);

    /// Set vertex attributes
    fn set_vertex_attributes(&mut self, attributes: &[VertexAttribute]);

    /// Bind texture to a slot
    fn bind_texture(&mut self, slot: u32, offset: u32);

    /// Set viewport
    fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32, min_depth: f32, max_depth: f32);

    /// Set scissor rectangle
    fn set_scissor(&mut self, x: u32, y: u32, width: u32, height: u32);
}
