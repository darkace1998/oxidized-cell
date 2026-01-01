//! RSX rendering backends

pub mod null;
pub mod vulkan;

use crate::vertex::VertexAttribute;

/// Framebuffer data for display
#[derive(Debug, Clone)]
pub struct FramebufferData {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// RGBA pixel data (4 bytes per pixel)
    pub pixels: Vec<u8>,
}

impl FramebufferData {
    /// Create a new framebuffer with the given dimensions
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    /// Create a test pattern for debugging
    pub fn test_pattern(width: u32, height: u32) -> Self {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        
        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;
                // Create a gradient pattern
                pixels[i] = (x * 255 / width) as u8;     // R
                pixels[i + 1] = (y * 255 / height) as u8; // G
                pixels[i + 2] = 128;                       // B
                pixels[i + 3] = 255;                       // A
            }
        }
        
        Self { width, height, pixels }
    }
}

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
    
    /// Submit vertex buffer data to the GPU
    /// 
    /// # Arguments
    /// * `binding` - The vertex buffer binding index
    /// * `data` - The raw vertex data bytes
    /// * `stride` - The stride between vertices in bytes
    fn submit_vertex_buffer(&mut self, binding: u32, data: &[u8], stride: u32);
    
    /// Submit index buffer data to the GPU
    /// 
    /// # Arguments
    /// * `data` - The raw index data bytes
    /// * `index_type` - The index type (2 for u16, 4 for u32)
    fn submit_index_buffer(&mut self, data: &[u8], index_type: u32);
    
    /// Get the current framebuffer contents as RGBA pixels
    /// Returns None if the framebuffer is not available
    fn get_framebuffer(&self) -> Option<FramebufferData>;
    
    /// Get the framebuffer dimensions
    fn get_dimensions(&self) -> (u32, u32);
}
