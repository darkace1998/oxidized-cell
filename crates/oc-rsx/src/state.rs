//! RSX graphics state

/// Default maximum LOD for texture sampling
pub const DEFAULT_MAX_LOD: f32 = 1000.0;

/// RSX graphics state
#[derive(Debug, Clone)]
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
    
    // Alpha test state
    pub alpha_test_enable: bool,
    pub alpha_test_func: u32,
    pub alpha_test_ref: f32,
    
    // Polygon offset state
    pub polygon_offset_fill_enable: bool,
    pub polygon_offset_line_enable: bool,
    pub polygon_offset_point_enable: bool,
    pub polygon_offset_factor: f32,
    pub polygon_offset_units: f32,
    
    // Line and point state
    pub line_width: f32,
    pub point_size: f32,
    pub point_sprite_enable: bool,
    
    // Anti-aliasing state
    pub multisample_enable: bool,
    pub sample_alpha_to_coverage_enable: bool,
    pub sample_count: u8,
    
    // Primitive restart
    pub primitive_restart_enable: bool,
    pub primitive_restart_index: u32,
    
    // Occlusion query
    pub occlusion_query_enable: bool,
    pub occlusion_query_offset: u32,
    
    // Scissor state
    pub scissor_x: u16,
    pub scissor_y: u16,
    pub scissor_width: u16,
    pub scissor_height: u16,
    
    // Logic operation state
    pub logic_op_enable: bool,
    pub logic_op: u32,
    
    // Color mask state
    pub color_mask: u32,
    pub color_mask_mrt: u32,
    
    // Fog state
    pub fog_mode: u32,
    pub fog_params: [f32; 2],
    
    // Dither state
    pub dither_enable: bool,
    
    // Two-sided stencil state
    pub two_sided_stencil_enable: bool,
    pub back_stencil_func: u32,
    pub back_stencil_ref: u8,
    pub back_stencil_mask: u8,
    pub back_stencil_op_fail: u32,
    pub back_stencil_op_zfail: u32,
    pub back_stencil_op_zpass: u32,
    pub back_stencil_write_mask: u8,
    
    // Additional blend state
    pub blend_enable_mrt: u32,
    pub blend_equation_rgb: u32,
    pub blend_equation_alpha: u32,
    
    // Polygon smooth state
    pub polygon_smooth_enable: bool,
    
    // Semaphore state
    pub semaphore_offset: u32,
    
    // Transform feedback state
    pub transform_feedback_enable: bool,
    /// Transform feedback buffer offsets (4 buffers)
    pub transform_feedback_buffer_offset: [u32; 4],
    /// Transform feedback buffer sizes (4 buffers)
    pub transform_feedback_buffer_size: [u32; 4],
    /// Transform feedback output stride per buffer
    pub transform_feedback_stride: [u32; 4],
    
    // Shader control state
    pub shader_control: u32,
    
    // Blend color state (RGBA constant for blending)
    pub blend_color: [f32; 4],
    
    // Front stencil operations (back stencil already exists)
    pub stencil_op_fail: u32,
    pub stencil_op_zfail: u32,
    pub stencil_op_zpass: u32,
    pub stencil_write_mask: u8,
    
    // Extended texture sampling state
    pub texture_control3: [u32; 16],  // Anisotropic filtering, etc.
    pub texture_lod_bias: [f32; 16],  // LOD bias per texture unit
    pub texture_lod_min: [f32; 16],   // Min LOD clamp
    pub texture_lod_max: [f32; 16],   // Max LOD clamp
    pub texture_type: [u32; 16],      // 1D, 2D, 3D, Cube
    
    // Occlusion query extended state
    pub occlusion_query_result_offset: u32,
    pub conditional_render_enable: bool,
    pub conditional_render_mode: u32,
    
    // Draw call state (set by draw commands)
    pub draw_first: u32,
    pub draw_count: u32,
    pub draw_index_offset: u32,
    pub draw_index_type: u32,  // 0 = u16, 1 = u32
    
    // Surface extended state
    pub surface_type: u32,      // Linear, Swizzle, Tile
    pub surface_antialias: u32, // MSAA mode
    pub surface_depth_format: u32,
    pub surface_color_format: u32,
    pub surface_log2_width: u8,
    pub surface_log2_height: u8,
    
    // Vertex program constants (512 vec4 registers)
    pub vertex_constants: [[f32; 4]; 512],
    /// Currently loading VP constant index
    pub vertex_constant_load_slot: u32,
    
    // Fragment program constants (embedded in program memory)
    // These are indexed by address offset within the FP
    pub fragment_constants: Vec<(u32, [f32; 4])>,
    
    // Extended texture state (dimensions, swizzle, wrap modes)
    pub texture_width: [u32; 16],
    pub texture_height: [u32; 16],
    pub texture_depth: [u32; 16],
    pub texture_address: [u32; 16],  // wrap modes
    pub texture_border_color: [u32; 16],
}

impl RsxState {
    /// Create a new RSX state with defaults
    pub fn new() -> Self {
        Self {
            surface_color_target: 0,
            surface_format: 0,
            surface_pitch: [0; 4],
            surface_offset_color: [0; 4],
            surface_offset_depth: 0,
            context_dma_color: [0; 4],
            context_dma_depth: 0,
            surface_clip_x: 0,
            surface_clip_y: 0,
            surface_clip_width: 0,
            surface_clip_height: 0,
            clear_color: 0,
            clear_depth: 1.0,
            clear_stencil: 0,
            viewport_x: 0.0,
            viewport_y: 0.0,
            viewport_width: 0.0,
            viewport_height: 0.0,
            depth_min: 0.0,
            depth_max: 1.0,
            primitive_type: 0,
            blend_enable: false,
            blend_src_factor: 0,
            blend_dst_factor: 0,
            blend_equation: 0,
            depth_test_enable: false,
            depth_write_enable: false,
            depth_func: 0,
            stencil_test_enable: false,
            stencil_func: 0,
            stencil_ref: 0,
            stencil_mask: 0,
            cull_face_enable: false,
            cull_face_mode: 0,
            front_face: 0,
            vertex_program_addr: 0,
            fragment_program_addr: 0,
            vertex_attrib_input_mask: 0,
            vertex_attrib_output_mask: 0,
            vertex_attrib_format: [0; 16],
            vertex_attrib_offset: [0; 16],
            texture_offset: [0; 16],
            texture_format: [0; 16],
            texture_control: [0; 16],
            texture_filter: [0; 16],
            alpha_test_enable: false,
            alpha_test_func: 0,
            alpha_test_ref: 0.0,
            polygon_offset_fill_enable: false,
            polygon_offset_line_enable: false,
            polygon_offset_point_enable: false,
            polygon_offset_factor: 0.0,
            polygon_offset_units: 0.0,
            line_width: 1.0,
            point_size: 1.0,
            point_sprite_enable: false,
            multisample_enable: false,
            sample_alpha_to_coverage_enable: false,
            sample_count: 1,
            primitive_restart_enable: false,
            primitive_restart_index: 0xFFFFFFFF,
            occlusion_query_enable: false,
            occlusion_query_offset: 0,
            // Scissor state
            scissor_x: 0,
            scissor_y: 0,
            scissor_width: 4096,
            scissor_height: 4096,
            // Logic operation state
            logic_op_enable: false,
            logic_op: 0,
            // Color mask state
            color_mask: 0xFFFFFFFF,
            color_mask_mrt: 0xFFFFFFFF,
            // Fog state
            fog_mode: 0,
            fog_params: [0.0, 1.0],
            // Dither state
            dither_enable: true,
            // Two-sided stencil state
            two_sided_stencil_enable: false,
            back_stencil_func: 0,
            back_stencil_ref: 0,
            back_stencil_mask: 0xFF,
            back_stencil_op_fail: 0,
            back_stencil_op_zfail: 0,
            back_stencil_op_zpass: 0,
            back_stencil_write_mask: 0xFF,
            // Additional blend state
            blend_enable_mrt: 0,
            blend_equation_rgb: 0,
            blend_equation_alpha: 0,
            // Polygon smooth state
            polygon_smooth_enable: false,
            // Semaphore state
            semaphore_offset: 0,
            // Transform feedback state
            transform_feedback_enable: false,
            transform_feedback_buffer_offset: [0; 4],
            transform_feedback_buffer_size: [0; 4],
            transform_feedback_stride: [0; 4],
            // Shader control state
            shader_control: 0,
            // Blend color state
            blend_color: [0.0; 4],
            // Front stencil operations
            stencil_op_fail: 0,
            stencil_op_zfail: 0,
            stencil_op_zpass: 0,
            stencil_write_mask: 0xFF,
            // Extended texture sampling state
            texture_control3: [0; 16],
            texture_lod_bias: [0.0; 16],
            texture_lod_min: [0.0; 16],
            texture_lod_max: [DEFAULT_MAX_LOD; 16],
            texture_type: [0; 16],
            // Occlusion query extended state
            occlusion_query_result_offset: 0,
            conditional_render_enable: false,
            conditional_render_mode: 0,
            // Draw call state
            draw_first: 0,
            draw_count: 0,
            draw_index_offset: 0,
            draw_index_type: 0,
            // Surface extended state
            surface_type: 0,
            surface_antialias: 0,
            surface_depth_format: 0,
            surface_color_format: 0,
            surface_log2_width: 0,
            surface_log2_height: 0,
            vertex_constants: [[0.0; 4]; 512],
            vertex_constant_load_slot: 0,
            fragment_constants: Vec::new(),
            texture_width: [1; 16],
            texture_height: [1; 16],
            texture_depth: [1; 16],
            texture_address: [0; 16],
            texture_border_color: [0; 16],
        }
    }

    /// Set a vertex program constant
    pub fn set_vertex_constant(&mut self, index: u32, x: f32, y: f32, z: f32, w: f32) {
        if (index as usize) < 512 {
            self.vertex_constants[index as usize] = [x, y, z, w];
        }
    }

    /// Get vertex program constants for shader compilation
    pub fn get_vertex_constants(&self) -> &[[f32; 4]; 512] {
        &self.vertex_constants
    }

    /// Set a fragment program constant (by offset in program)
    pub fn set_fragment_constant(&mut self, offset: u32, x: f32, y: f32, z: f32, w: f32) {
        // Update or insert
        if let Some(entry) = self.fragment_constants.iter_mut().find(|(o, _)| *o == offset) {
            entry.1 = [x, y, z, w];
        } else {
            self.fragment_constants.push((offset, [x, y, z, w]));
        }
    }

    /// Get fragment program constants
    pub fn get_fragment_constants(&self) -> &[(u32, [f32; 4])] {
        &self.fragment_constants
    }

    /// Reset state to defaults
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for RsxState {
    fn default() -> Self {
        Self::new()
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
