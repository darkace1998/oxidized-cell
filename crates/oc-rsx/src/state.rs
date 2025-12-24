//! RSX graphics state

/// RSX graphics state
#[derive(Debug, Clone, Default)]
pub struct RsxState {
    // Surface state
    pub surface_color_target: u32,
    pub surface_format: u32,
    pub surface_pitch: [u32; 4],
    pub surface_offset_color: [u32; 4],
    pub surface_offset_depth: u32,
    pub context_dma_color: [u32; 4],
    pub context_dma_depth: u32,
    pub surface_clip_x: u16,
    pub surface_clip_y: u16,
    pub surface_clip_width: u16,
    pub surface_clip_height: u16,

    // Clear values
    pub clear_color: u32,
    pub clear_depth: f32,
    pub clear_stencil: u8,

    // Viewport
    pub viewport_x: f32,
    pub viewport_y: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub depth_min: f32,
    pub depth_max: f32,

    // Primitive state
    pub primitive_type: u32,

    // Blend state
    pub blend_enable: bool,
    pub blend_src_factor: u32,
    pub blend_dst_factor: u32,
    pub blend_equation: u32,

    // Depth state
    pub depth_test_enable: bool,
    pub depth_write_enable: bool,
    pub depth_func: u32,

    // Stencil state
    pub stencil_test_enable: bool,
    pub stencil_func: u32,
    pub stencil_ref: u8,
    pub stencil_mask: u8,

    // Cull state
    pub cull_face_enable: bool,
    pub cull_face_mode: u32,
    pub front_face: u32,

    // Shader state
    pub vertex_program_addr: u32,
    pub fragment_program_addr: u32,
    pub vertex_attrib_input_mask: u32,
    pub vertex_attrib_output_mask: u32,

    // Vertex attribute state (16 attributes max)
    pub vertex_attrib_format: [u32; 16],
    pub vertex_attrib_offset: [u32; 16],

    // Texture state (16 texture units)
    pub texture_offset: [u32; 16],
    pub texture_format: [u32; 16],
    pub texture_control: [u32; 16],
    pub texture_filter: [u32; 16],
}

impl RsxState {
    /// Create a new RSX state with defaults
    pub fn new() -> Self {
        Self {
            depth_max: 1.0,
            clear_depth: 1.0,
            ..Default::default()
        }
    }

    /// Reset state to defaults
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsx_state_creation() {
        let state = RsxState::new();
        assert_eq!(state.depth_max, 1.0);
        assert!(!state.blend_enable);
    }
}
