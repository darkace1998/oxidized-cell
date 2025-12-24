//! RSX NV4097 method handlers
//!
//! This module defines constants and handlers for RSX GPU commands.
//! The NV4097 is the command set for the RSX GPU based on NVIDIA G70/G71.

use crate::state::RsxState;

// Surface and render target methods
pub const NV4097_SET_SURFACE_FORMAT: u32 = 0x0180;
pub const NV4097_SET_CONTEXT_DMA_COLOR_A: u32 = 0x0184;
pub const NV4097_SET_CONTEXT_DMA_COLOR_B: u32 = 0x0188;
pub const NV4097_SET_CONTEXT_DMA_COLOR_C: u32 = 0x018C;
pub const NV4097_SET_CONTEXT_DMA_COLOR_D: u32 = 0x0190;
pub const NV4097_SET_SURFACE_COLOR_AOFFSET: u32 = 0x0194;
pub const NV4097_SET_SURFACE_COLOR_BOFFSET: u32 = 0x0198;
pub const NV4097_SET_SURFACE_COLOR_COFFSET: u32 = 0x019C;
pub const NV4097_SET_SURFACE_COLOR_DOFFSET: u32 = 0x01A0;
pub const NV4097_SET_SURFACE_PITCH_A: u32 = 0x01A4;
pub const NV4097_SET_SURFACE_PITCH_B: u32 = 0x01A8;
pub const NV4097_SET_SURFACE_PITCH_C: u32 = 0x01AC;
pub const NV4097_SET_SURFACE_PITCH_D: u32 = 0x01B0;
pub const NV4097_SET_CONTEXT_DMA_ZETA: u32 = 0x01B4;
pub const NV4097_SET_SURFACE_ZETA_OFFSET: u32 = 0x01B8;
pub const NV4097_SET_SURFACE_PITCH_Z: u32 = 0x01BC;
pub const NV4097_SET_SURFACE_COLOR_TARGET: u32 = 0x0200;

// Clip and viewport methods
pub const NV4097_SET_SURFACE_CLIP_HORIZONTAL: u32 = 0x02BC;
pub const NV4097_SET_SURFACE_CLIP_VERTICAL: u32 = 0x02C0;
pub const NV4097_SET_VIEWPORT_HORIZONTAL: u32 = 0x0A00;
pub const NV4097_SET_VIEWPORT_VERTICAL: u32 = 0x0A04;
pub const NV4097_SET_CLIP_MIN: u32 = 0x0A08;
pub const NV4097_SET_CLIP_MAX: u32 = 0x0A0C;
pub const NV4097_SET_VIEWPORT_OFFSET: u32 = 0x0A10;
pub const NV4097_SET_VIEWPORT_SCALE: u32 = 0x0A20;

// Clear methods
pub const NV4097_SET_COLOR_CLEAR_VALUE: u32 = 0x0304;
pub const NV4097_SET_ZSTENCIL_CLEAR_VALUE: u32 = 0x0308;
pub const NV4097_CLEAR_SURFACE: u32 = 0x1D94;

// Blend state methods
pub const NV4097_SET_BLEND_ENABLE: u32 = 0x0310;
pub const NV4097_SET_BLEND_FUNC_SFACTOR: u32 = 0x0314;
pub const NV4097_SET_BLEND_FUNC_DFACTOR: u32 = 0x0318;
pub const NV4097_SET_BLEND_EQUATION: u32 = 0x0340;
pub const NV4097_SET_BLEND_COLOR: u32 = 0x0350;

// Depth/stencil methods
pub const NV4097_SET_DEPTH_TEST_ENABLE: u32 = 0x030C;
pub const NV4097_SET_DEPTH_FUNC: u32 = 0x0374;
pub const NV4097_SET_DEPTH_MASK: u32 = 0x0378;
pub const NV4097_SET_STENCIL_TEST_ENABLE: u32 = 0x0348;
pub const NV4097_SET_STENCIL_FUNC: u32 = 0x034C;
pub const NV4097_SET_STENCIL_OP_FAIL: u32 = 0x0350;
pub const NV4097_SET_STENCIL_OP_ZFAIL: u32 = 0x0354;
pub const NV4097_SET_STENCIL_OP_ZPASS: u32 = 0x0358;
pub const NV4097_SET_STENCIL_MASK: u32 = 0x035C;
pub const NV4097_SET_STENCIL_FUNC_REF: u32 = 0x0360;
pub const NV4097_SET_STENCIL_FUNC_MASK: u32 = 0x0364;

// Cull face methods
pub const NV4097_SET_CULL_FACE_ENABLE: u32 = 0x0410;
pub const NV4097_SET_CULL_FACE: u32 = 0x0414;
pub const NV4097_SET_FRONT_FACE: u32 = 0x0418;

// Vertex program methods
pub const NV4097_SET_VERTEX_PROGRAM_START_SLOT: u32 = 0x0480;
pub const NV4097_SET_VERTEX_PROGRAM_LOAD_SLOT: u32 = 0x0484;
pub const NV4097_SET_VERTEX_ATTRIB_INPUT_MASK: u32 = 0x1640;
pub const NV4097_SET_VERTEX_ATTRIB_OUTPUT_MASK: u32 = 0x1644;

// Fragment program methods
pub const NV4097_SET_SHADER_PROGRAM: u32 = 0x0848;

// Draw methods
pub const NV4097_SET_BEGIN_END: u32 = 0x1808;
pub const NV4097_DRAW_ARRAYS: u32 = 0x1810;
pub const NV4097_DRAW_INDEX_ARRAY: u32 = 0x1814;
pub const NV4097_INLINE_ARRAY: u32 = 0x1818;

// Vertex attribute methods
pub const NV4097_SET_VERTEX_DATA_ARRAY_FORMAT: u32 = 0x1900;
pub const NV4097_SET_VERTEX_DATA_ARRAY_OFFSET: u32 = 0x1980;

// Texture methods
pub const NV4097_SET_TEXTURE_OFFSET: u32 = 0x1A00;
pub const NV4097_SET_TEXTURE_FORMAT: u32 = 0x1A04;
pub const NV4097_SET_TEXTURE_CONTROL0: u32 = 0x1A08;
pub const NV4097_SET_TEXTURE_FILTER: u32 = 0x1A0C;

/// Handler for NV4097 methods
pub struct MethodHandler;

impl MethodHandler {
    /// Execute a method
    pub fn execute(method: u32, data: u32, state: &mut RsxState) {
        match method {
            // Surface format and targets
            NV4097_SET_SURFACE_FORMAT => {
                state.surface_format = data;
            }
            NV4097_SET_SURFACE_COLOR_TARGET => {
                state.surface_color_target = data;
            }
            NV4097_SET_CONTEXT_DMA_COLOR_A => {
                state.context_dma_color[0] = data;
            }
            NV4097_SET_SURFACE_COLOR_AOFFSET => {
                state.surface_offset_color[0] = data;
            }
            NV4097_SET_SURFACE_PITCH_A => {
                state.surface_pitch[0] = data;
            }
            NV4097_SET_SURFACE_PITCH_B => {
                state.surface_pitch[1] = data;
            }
            NV4097_SET_SURFACE_PITCH_C => {
                state.surface_pitch[2] = data;
            }
            NV4097_SET_SURFACE_PITCH_D => {
                state.surface_pitch[3] = data;
            }
            NV4097_SET_CONTEXT_DMA_ZETA => {
                state.context_dma_depth = data;
            }
            NV4097_SET_SURFACE_ZETA_OFFSET => {
                state.surface_offset_depth = data;
            }

            // Clip and viewport
            NV4097_SET_SURFACE_CLIP_HORIZONTAL => {
                state.surface_clip_x = (data & 0xFFFF) as u16;
                state.surface_clip_width = ((data >> 16) & 0xFFFF) as u16;
            }
            NV4097_SET_SURFACE_CLIP_VERTICAL => {
                state.surface_clip_y = (data & 0xFFFF) as u16;
                state.surface_clip_height = ((data >> 16) & 0xFFFF) as u16;
            }
            NV4097_SET_VIEWPORT_HORIZONTAL => {
                let x = (data & 0xFFFF) as i16 as f32;
                let width = ((data >> 16) & 0xFFFF) as f32;
                state.viewport_x = x;
                state.viewport_width = width;
            }
            NV4097_SET_VIEWPORT_VERTICAL => {
                let y = (data & 0xFFFF) as i16 as f32;
                let height = ((data >> 16) & 0xFFFF) as f32;
                state.viewport_y = y;
                state.viewport_height = height;
            }
            NV4097_SET_CLIP_MIN => {
                state.depth_min = f32::from_bits(data);
            }
            NV4097_SET_CLIP_MAX => {
                state.depth_max = f32::from_bits(data);
            }

            // Clear values
            NV4097_SET_COLOR_CLEAR_VALUE => {
                state.clear_color = data;
            }
            NV4097_SET_ZSTENCIL_CLEAR_VALUE => {
                state.clear_depth = ((data >> 8) & 0xFFFFFF) as f32 / 16777215.0;
                state.clear_stencil = (data & 0xFF) as u8;
            }

            // Blend state
            NV4097_SET_BLEND_ENABLE => {
                state.blend_enable = data != 0;
            }
            NV4097_SET_BLEND_FUNC_SFACTOR => {
                state.blend_src_factor = data;
            }
            NV4097_SET_BLEND_FUNC_DFACTOR => {
                state.blend_dst_factor = data;
            }
            NV4097_SET_BLEND_EQUATION => {
                state.blend_equation = data;
            }

            // Depth/stencil state
            NV4097_SET_DEPTH_TEST_ENABLE => {
                state.depth_test_enable = data != 0;
            }
            NV4097_SET_DEPTH_FUNC => {
                state.depth_func = data;
            }
            NV4097_SET_DEPTH_MASK => {
                state.depth_write_enable = data != 0;
            }
            NV4097_SET_STENCIL_TEST_ENABLE => {
                state.stencil_test_enable = data != 0;
            }
            NV4097_SET_STENCIL_FUNC => {
                state.stencil_func = data;
            }
            NV4097_SET_STENCIL_FUNC_REF => {
                state.stencil_ref = data as u8;
            }
            NV4097_SET_STENCIL_FUNC_MASK => {
                state.stencil_mask = data as u8;
            }

            // Cull face
            NV4097_SET_CULL_FACE_ENABLE => {
                state.cull_face_enable = data != 0;
            }
            NV4097_SET_CULL_FACE => {
                state.cull_face_mode = data;
            }
            NV4097_SET_FRONT_FACE => {
                state.front_face = data;
            }

            // Shader programs
            NV4097_SET_SHADER_PROGRAM => {
                state.fragment_program_addr = data;
            }
            NV4097_SET_VERTEX_ATTRIB_INPUT_MASK => {
                state.vertex_attrib_input_mask = data;
            }
            NV4097_SET_VERTEX_ATTRIB_OUTPUT_MASK => {
                state.vertex_attrib_output_mask = data;
            }

            // Draw commands - These need special handling
            NV4097_DRAW_ARRAYS | NV4097_DRAW_INDEX_ARRAY | NV4097_INLINE_ARRAY => {
                // These are handled by the RSX thread, not just state updates
                tracing::trace!("Draw command: method=0x{:04X}, data=0x{:08X}", method, data);
            }

            // Draw commands
            NV4097_SET_BEGIN_END => {
                if data != 0 {
                    state.primitive_type = data;
                }
            }

            _ => {
                // Check for vertex attribute array ranges
                if method >= NV4097_SET_VERTEX_DATA_ARRAY_FORMAT 
                    && method < NV4097_SET_VERTEX_DATA_ARRAY_FORMAT + 16 {
                    let index = (method - NV4097_SET_VERTEX_DATA_ARRAY_FORMAT) as usize;
                    state.vertex_attrib_format[index] = data;
                } else if method >= NV4097_SET_VERTEX_DATA_ARRAY_OFFSET 
                    && method < NV4097_SET_VERTEX_DATA_ARRAY_OFFSET + 16 {
                    let index = (method - NV4097_SET_VERTEX_DATA_ARRAY_OFFSET) as usize;
                    state.vertex_attrib_offset[index] = data;
                }
                // Check for texture ranges (texture methods are spaced 0x20 apart)
                else if method >= NV4097_SET_TEXTURE_OFFSET 
                    && method < NV4097_SET_TEXTURE_OFFSET + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_OFFSET) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_OFFSET) / 0x20) as usize;
                    if index < 16 {
                        state.texture_offset[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_FORMAT 
                    && method < NV4097_SET_TEXTURE_FORMAT + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_FORMAT) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_FORMAT) / 0x20) as usize;
                    if index < 16 {
                        state.texture_format[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_CONTROL0 
                    && method < NV4097_SET_TEXTURE_CONTROL0 + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_CONTROL0) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_CONTROL0) / 0x20) as usize;
                    if index < 16 {
                        state.texture_control[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_FILTER 
                    && method < NV4097_SET_TEXTURE_FILTER + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_FILTER) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_FILTER) / 0x20) as usize;
                    if index < 16 {
                        state.texture_filter[index] = data;
                    }
                } else {
                    // Unknown or unimplemented method
                    tracing::trace!("Unimplemented NV4097 method: 0x{:04X}", method);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_format() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_SURFACE_FORMAT, 0x05, &mut state);
        assert_eq!(state.surface_format, 0x05);
    }

    #[test]
    fn test_blend_enable() {
        let mut state = RsxState::new();
        assert!(!state.blend_enable);
        MethodHandler::execute(NV4097_SET_BLEND_ENABLE, 1, &mut state);
        assert!(state.blend_enable);
    }

    #[test]
    fn test_viewport_horizontal() {
        let mut state = RsxState::new();
        // x=100, width=800
        let data = (800 << 16) | 100;
        MethodHandler::execute(NV4097_SET_VIEWPORT_HORIZONTAL, data, &mut state);
        assert_eq!(state.viewport_x, 100.0);
        assert_eq!(state.viewport_width, 800.0);
    }

    #[test]
    fn test_depth_test_enable() {
        let mut state = RsxState::new();
        assert!(!state.depth_test_enable);
        MethodHandler::execute(NV4097_SET_DEPTH_TEST_ENABLE, 1, &mut state);
        assert!(state.depth_test_enable);
    }

    #[test]
    fn test_cull_face() {
        let mut state = RsxState::new();
        assert!(!state.cull_face_enable);
        MethodHandler::execute(NV4097_SET_CULL_FACE_ENABLE, 1, &mut state);
        assert!(state.cull_face_enable);
        MethodHandler::execute(NV4097_SET_CULL_FACE, 0x0405, &mut state);
        assert_eq!(state.cull_face_mode, 0x0405);
    }

    #[test]
    fn test_vertex_attrib_format() {
        let mut state = RsxState::new();
        // Test first vertex attribute format
        MethodHandler::execute(NV4097_SET_VERTEX_DATA_ARRAY_FORMAT, 0x12345678, &mut state);
        assert_eq!(state.vertex_attrib_format[0], 0x12345678);
        
        // Test second vertex attribute format
        MethodHandler::execute(NV4097_SET_VERTEX_DATA_ARRAY_FORMAT + 1, 0xABCDEF00, &mut state);
        assert_eq!(state.vertex_attrib_format[1], 0xABCDEF00);
    }

    #[test]
    fn test_vertex_attrib_offset() {
        let mut state = RsxState::new();
        // Test first vertex attribute offset
        MethodHandler::execute(NV4097_SET_VERTEX_DATA_ARRAY_OFFSET, 0x1000, &mut state);
        assert_eq!(state.vertex_attrib_offset[0], 0x1000);
        
        // Test second vertex attribute offset
        MethodHandler::execute(NV4097_SET_VERTEX_DATA_ARRAY_OFFSET + 1, 0x2000, &mut state);
        assert_eq!(state.vertex_attrib_offset[1], 0x2000);
    }

    #[test]
    fn test_texture_offset() {
        let mut state = RsxState::new();
        // Test first texture offset (texture methods are spaced 0x20 apart)
        MethodHandler::execute(NV4097_SET_TEXTURE_OFFSET, 0x10000, &mut state);
        assert_eq!(state.texture_offset[0], 0x10000);
        
        // Test second texture offset
        MethodHandler::execute(NV4097_SET_TEXTURE_OFFSET + 0x20, 0x20000, &mut state);
        assert_eq!(state.texture_offset[1], 0x20000);
    }

    #[test]
    fn test_texture_format() {
        let mut state = RsxState::new();
        // Test first texture format
        MethodHandler::execute(NV4097_SET_TEXTURE_FORMAT, 0x8A, &mut state);
        assert_eq!(state.texture_format[0], 0x8A);
    }

    #[test]
    fn test_vertex_attrib_masks() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_VERTEX_ATTRIB_INPUT_MASK, 0xFFFF, &mut state);
        assert_eq!(state.vertex_attrib_input_mask, 0xFFFF);
        
        MethodHandler::execute(NV4097_SET_VERTEX_ATTRIB_OUTPUT_MASK, 0x00FF, &mut state);
        assert_eq!(state.vertex_attrib_output_mask, 0x00FF);
    }
}
