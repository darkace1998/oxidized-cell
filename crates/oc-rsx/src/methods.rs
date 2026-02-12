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
pub const NV4097_SET_BLEND_COLOR: u32 = 0x0358;  // Blend constant color (RGBA packed)
pub const NV4097_SET_BLEND_COLOR2: u32 = 0x035C; // Extended blend color

// Depth/stencil methods
pub const NV4097_SET_DEPTH_TEST_ENABLE: u32 = 0x030C;
pub const NV4097_SET_DEPTH_FUNC: u32 = 0x0374;
pub const NV4097_SET_DEPTH_MASK: u32 = 0x0378;
pub const NV4097_SET_STENCIL_TEST_ENABLE: u32 = 0x0348;
pub const NV4097_SET_STENCIL_FUNC: u32 = 0x034C;
pub const NV4097_SET_STENCIL_OP_FAIL: u32 = 0x0460;
pub const NV4097_SET_STENCIL_OP_ZFAIL: u32 = 0x0464;
pub const NV4097_SET_STENCIL_OP_ZPASS: u32 = 0x0468;
pub const NV4097_SET_STENCIL_MASK: u32 = 0x046C;  // Write mask
pub const NV4097_SET_STENCIL_FUNC_REF: u32 = 0x0360;
pub const NV4097_SET_STENCIL_FUNC_MASK: u32 = 0x0364;

// Cull face methods
pub const NV4097_SET_CULL_FACE_ENABLE: u32 = 0x0410;
pub const NV4097_SET_CULL_FACE: u32 = 0x0414;
pub const NV4097_SET_FRONT_FACE: u32 = 0x0418;

// Alpha test methods
pub const NV4097_SET_ALPHA_TEST_ENABLE: u32 = 0x0300;
pub const NV4097_SET_ALPHA_FUNC: u32 = 0x037C;
pub const NV4097_SET_ALPHA_REF: u32 = 0x0380;

// Polygon offset methods
pub const NV4097_SET_POLYGON_OFFSET_FILL_ENABLE: u32 = 0x0370;
pub const NV4097_SET_POLYGON_OFFSET_LINE_ENABLE: u32 = 0x0368;
pub const NV4097_SET_POLYGON_OFFSET_POINT_ENABLE: u32 = 0x036C;
pub const NV4097_SET_POLYGON_OFFSET_SCALE_FACTOR: u32 = 0x0A18;
pub const NV4097_SET_POLYGON_OFFSET_BIAS: u32 = 0x0A1C;

// Line and point methods
pub const NV4097_SET_LINE_WIDTH: u32 = 0x1DB0;
pub const NV4097_SET_POINT_SIZE: u32 = 0x1DB4;
pub const NV4097_SET_POINT_SPRITE_CONTROL: u32 = 0x1DB8;

// Anti-aliasing methods
pub const NV4097_SET_ANTI_ALIASING_CONTROL: u32 = 0x017C;
pub const NV4097_SET_SAMPLE_COUNT_CONTROL: u32 = 0x0178;

// Primitive restart methods
pub const NV4097_SET_RESTART_INDEX_ENABLE: u32 = 0x0DEC;
pub const NV4097_SET_RESTART_INDEX: u32 = 0x0DF0;

// Occlusion query methods
pub const NV4097_SET_ZPASS_PIXEL_COUNT_ENABLE: u32 = 0x1DA0;
pub const NV4097_SET_REPORT_SEMAPHORE_OFFSET: u32 = 0x1D00;

// Scissor methods
pub const NV4097_SET_SCISSOR_HORIZONTAL: u32 = 0x08C0;
pub const NV4097_SET_SCISSOR_VERTICAL: u32 = 0x08C4;

// Logic operation methods
pub const NV4097_SET_LOGIC_OP_ENABLE: u32 = 0x09C0;
pub const NV4097_SET_LOGIC_OP: u32 = 0x09C4;

// Color mask methods
pub const NV4097_SET_COLOR_MASK: u32 = 0x0328;
pub const NV4097_SET_COLOR_MASK_MRT: u32 = 0x032C;

// Fog methods
pub const NV4097_SET_FOG_MODE: u32 = 0x0420;
pub const NV4097_SET_FOG_PARAMS: u32 = 0x0424;  // Fog parameter 0 (start)
pub const NV4097_SET_FOG_PARAMS_1: u32 = 0x0428;  // Fog parameter 1 (end)

// Dither methods
pub const NV4097_SET_DITHER_ENABLE: u32 = 0x0334;

// Two-sided stencil methods
pub const NV4097_SET_TWO_SIDED_STENCIL_TEST_ENABLE: u32 = 0x038C;
pub const NV4097_SET_BACK_STENCIL_FUNC: u32 = 0x0390;
pub const NV4097_SET_BACK_STENCIL_FUNC_REF: u32 = 0x0394;
pub const NV4097_SET_BACK_STENCIL_FUNC_MASK: u32 = 0x0398;
pub const NV4097_SET_BACK_STENCIL_OP_FAIL: u32 = 0x039C;
pub const NV4097_SET_BACK_STENCIL_OP_ZFAIL: u32 = 0x03A0;
pub const NV4097_SET_BACK_STENCIL_OP_ZPASS: u32 = 0x03A4;
pub const NV4097_SET_BACK_STENCIL_MASK: u32 = 0x03A8;

// Additional blend methods
pub const NV4097_SET_BLEND_ENABLE_MRT: u32 = 0x0338;
pub const NV4097_SET_BLEND_EQUATION_RGB: u32 = 0x0344;
// Note: Separate alpha equation uses a different register
pub const NV4097_SET_BLEND_EQUATION_ALPHA: u32 = 0x0346;

// Polygon mode methods  
pub const NV4097_SET_POLYGON_SMOOTH_ENABLE: u32 = 0x0440;

// Semaphore methods
pub const NV4097_SET_SEMAPHORE_OFFSET: u32 = 0x0D64;
pub const NV4097_BACK_END_WRITE_SEMAPHORE_RELEASE: u32 = 0x0D6C;
pub const NV4097_TEXTURE_READ_SEMAPHORE_RELEASE: u32 = 0x0D70;

// Transform feedback methods
pub const NV4097_SET_TRANSFORM_FEEDBACK_ENABLE: u32 = 0x1D88;
pub const NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_OFFSET: u32 = 0x1D8C;  // Base + index * 4
pub const NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_SIZE: u32 = 0x1D9C;
pub const NV4097_SET_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS: u32 = 0x1DAC;

// Conditional rendering methods
pub const NV4097_SET_CONDITIONAL_RENDER_ENABLE: u32 = 0x1DC0;
pub const NV4097_SET_CONDITIONAL_RENDER_MODE: u32 = 0x1DC4;

// Additional surface methods
pub const NV4097_SET_SURFACE_CLIP_ID: u32 = 0x0220;
pub const NV4097_SET_SURFACE_TYPE: u32 = 0x0224;

// Index array configuration
pub const NV4097_SET_INDEX_ARRAY_ADDRESS: u32 = 0x1688;
pub const NV4097_SET_INDEX_ARRAY_DMA: u32 = 0x168C;

// Vertex program methods
pub const NV4097_SET_VERTEX_PROGRAM_START_SLOT: u32 = 0x0480;
pub const NV4097_SET_VERTEX_PROGRAM_LOAD_SLOT: u32 = 0x0484;
pub const NV4097_SET_VERTEX_ATTRIB_INPUT_MASK: u32 = 0x1640;
pub const NV4097_SET_VERTEX_ATTRIB_OUTPUT_MASK: u32 = 0x1644;

// Vertex program constant methods (512 vec4 constants, 4 floats each)
pub const NV4097_SET_TRANSFORM_CONSTANT_LOAD: u32 = 0x1EFC;  // Set starting slot
pub const NV4097_SET_TRANSFORM_CONSTANT: u32 = 0x0B00;      // Write constant data (range 0x0B00-0x0EFC)
pub const NV4097_SET_TRANSFORM_CONSTANT_END: u32 = 0x0EFC;

// Fragment program methods
pub const NV4097_SET_SHADER_PROGRAM: u32 = 0x0848;
pub const NV4097_SET_SHADER_CONTROL: u32 = 0x084C;

// Draw methods
pub const NV4097_SET_BEGIN_END: u32 = 0x1808;
pub const NV4097_DRAW_ARRAYS: u32 = 0x1810;
pub const NV4097_DRAW_INDEX_ARRAY: u32 = 0x1814;
pub const NV4097_INLINE_ARRAY: u32 = 0x1818;
pub const NV4097_ARRAY_ELEMENT16: u32 = 0x181C;
pub const NV4097_ARRAY_ELEMENT32: u32 = 0x1820;
pub const NV4097_SET_PRIMITIVE_TYPE: u32 = 0x1824;  // Set primitive type for draw

// Vertex attribute methods
pub const NV4097_SET_VERTEX_DATA_ARRAY_FORMAT: u32 = 0x1900;
pub const NV4097_SET_VERTEX_DATA_ARRAY_OFFSET: u32 = 0x1980;

// Texture methods
pub const NV4097_SET_TEXTURE_OFFSET: u32 = 0x1A00;
pub const NV4097_SET_TEXTURE_FORMAT: u32 = 0x1A04;
pub const NV4097_SET_TEXTURE_CONTROL0: u32 = 0x1A08;
pub const NV4097_SET_TEXTURE_FILTER: u32 = 0x1A0C;
pub const NV4097_SET_TEXTURE_ADDRESS: u32 = 0x1A10;
pub const NV4097_SET_TEXTURE_IMAGE_RECT: u32 = 0x1A14;
pub const NV4097_SET_TEXTURE_BORDER_COLOR: u32 = 0x1A18;
pub const NV4097_SET_TEXTURE_CONTROL3: u32 = 0x1A1C;  // Anisotropic filtering, etc.

// Primitive type constants
pub const NV4097_PRIMITIVE_POINTS: u32 = 0x0001;
pub const NV4097_PRIMITIVE_LINES: u32 = 0x0002;
pub const NV4097_PRIMITIVE_LINE_LOOP: u32 = 0x0003;
pub const NV4097_PRIMITIVE_LINE_STRIP: u32 = 0x0004;
pub const NV4097_PRIMITIVE_TRIANGLES: u32 = 0x0005;
pub const NV4097_PRIMITIVE_TRIANGLE_STRIP: u32 = 0x0006;
pub const NV4097_PRIMITIVE_TRIANGLE_FAN: u32 = 0x0007;
pub const NV4097_PRIMITIVE_QUADS: u32 = 0x0008;
pub const NV4097_PRIMITIVE_QUAD_STRIP: u32 = 0x0009;
pub const NV4097_PRIMITIVE_POLYGON: u32 = 0x000A;

// Surface color format constants
pub const NV4097_SURFACE_FORMAT_X1R5G5B5_Z1R5G5B5: u32 = 0x01;
pub const NV4097_SURFACE_FORMAT_X1R5G5B5_O1R5G5B5: u32 = 0x02;
pub const NV4097_SURFACE_FORMAT_R5G6B5: u32 = 0x03;
pub const NV4097_SURFACE_FORMAT_X8R8G8B8_Z8R8G8B8: u32 = 0x04;
pub const NV4097_SURFACE_FORMAT_X8R8G8B8_O8R8G8B8: u32 = 0x05;
pub const NV4097_SURFACE_FORMAT_A8R8G8B8: u32 = 0x08;
pub const NV4097_SURFACE_FORMAT_B8: u32 = 0x09;
pub const NV4097_SURFACE_FORMAT_G8B8: u32 = 0x0A;
pub const NV4097_SURFACE_FORMAT_F_W16Z16Y16X16: u32 = 0x0B;
pub const NV4097_SURFACE_FORMAT_F_W32Z32Y32X32: u32 = 0x0C;
pub const NV4097_SURFACE_FORMAT_F_X32: u32 = 0x0D;
pub const NV4097_SURFACE_FORMAT_X8B8G8R8_Z8B8G8R8: u32 = 0x0E;
pub const NV4097_SURFACE_FORMAT_X8B8G8R8_O8B8G8R8: u32 = 0x0F;
pub const NV4097_SURFACE_FORMAT_A8B8G8R8: u32 = 0x10;

// Surface depth format constants
pub const NV4097_SURFACE_DEPTH_FORMAT_Z16: u32 = 0x01;
pub const NV4097_SURFACE_DEPTH_FORMAT_Z24S8: u32 = 0x02;

// Surface type constants
pub const NV4097_SURFACE_TYPE_LINEAR: u32 = 0x01;
pub const NV4097_SURFACE_TYPE_SWIZZLE: u32 = 0x02;
pub const NV4097_SURFACE_TYPE_TILE: u32 = 0x03;

// Blend factor constants
pub const NV4097_BLEND_ZERO: u32 = 0x0000;
pub const NV4097_BLEND_ONE: u32 = 0x0001;
pub const NV4097_BLEND_SRC_COLOR: u32 = 0x0300;
pub const NV4097_BLEND_ONE_MINUS_SRC_COLOR: u32 = 0x0301;
pub const NV4097_BLEND_SRC_ALPHA: u32 = 0x0302;
pub const NV4097_BLEND_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const NV4097_BLEND_DST_ALPHA: u32 = 0x0304;
pub const NV4097_BLEND_ONE_MINUS_DST_ALPHA: u32 = 0x0305;
pub const NV4097_BLEND_DST_COLOR: u32 = 0x0306;
pub const NV4097_BLEND_ONE_MINUS_DST_COLOR: u32 = 0x0307;
pub const NV4097_BLEND_SRC_ALPHA_SATURATE: u32 = 0x0308;
pub const NV4097_BLEND_CONSTANT_COLOR: u32 = 0x8001;
pub const NV4097_BLEND_ONE_MINUS_CONSTANT_COLOR: u32 = 0x8002;
pub const NV4097_BLEND_CONSTANT_ALPHA: u32 = 0x8003;
pub const NV4097_BLEND_ONE_MINUS_CONSTANT_ALPHA: u32 = 0x8004;

// Blend equation constants
pub const NV4097_BLEND_EQUATION_ADD: u32 = 0x8006;
pub const NV4097_BLEND_EQUATION_MIN: u32 = 0x8007;
pub const NV4097_BLEND_EQUATION_MAX: u32 = 0x8008;
pub const NV4097_BLEND_EQUATION_SUBTRACT: u32 = 0x800A;
pub const NV4097_BLEND_EQUATION_REVERSE_SUBTRACT: u32 = 0x800B;

// Stencil operation constants
pub const NV4097_STENCIL_OP_KEEP: u32 = 0x1E00;
pub const NV4097_STENCIL_OP_ZERO: u32 = 0x0000;
pub const NV4097_STENCIL_OP_REPLACE: u32 = 0x1E01;
pub const NV4097_STENCIL_OP_INCR: u32 = 0x1E02;
pub const NV4097_STENCIL_OP_DECR: u32 = 0x1E03;
pub const NV4097_STENCIL_OP_INVERT: u32 = 0x150A;
pub const NV4097_STENCIL_OP_INCR_WRAP: u32 = 0x8507;
pub const NV4097_STENCIL_OP_DECR_WRAP: u32 = 0x8508;

// Comparison function constants (for depth/stencil/alpha)
pub const NV4097_COMPARE_FUNC_NEVER: u32 = 0x0200;
pub const NV4097_COMPARE_FUNC_LESS: u32 = 0x0201;
pub const NV4097_COMPARE_FUNC_EQUAL: u32 = 0x0202;
pub const NV4097_COMPARE_FUNC_LEQUAL: u32 = 0x0203;
pub const NV4097_COMPARE_FUNC_GREATER: u32 = 0x0204;
pub const NV4097_COMPARE_FUNC_NOTEQUAL: u32 = 0x0205;
pub const NV4097_COMPARE_FUNC_GEQUAL: u32 = 0x0206;
pub const NV4097_COMPARE_FUNC_ALWAYS: u32 = 0x0207;

// Logic operation constants
pub const NV4097_LOGIC_OP_CLEAR: u32 = 0x1500;
pub const NV4097_LOGIC_OP_AND: u32 = 0x1501;
pub const NV4097_LOGIC_OP_AND_REVERSE: u32 = 0x1502;
pub const NV4097_LOGIC_OP_COPY: u32 = 0x1503;
pub const NV4097_LOGIC_OP_AND_INVERTED: u32 = 0x1504;
pub const NV4097_LOGIC_OP_NOOP: u32 = 0x1505;
pub const NV4097_LOGIC_OP_XOR: u32 = 0x1506;
pub const NV4097_LOGIC_OP_OR: u32 = 0x1507;
pub const NV4097_LOGIC_OP_NOR: u32 = 0x1508;
pub const NV4097_LOGIC_OP_EQUIV: u32 = 0x1509;
pub const NV4097_LOGIC_OP_INVERT: u32 = 0x150A;
pub const NV4097_LOGIC_OP_OR_REVERSE: u32 = 0x150B;
pub const NV4097_LOGIC_OP_COPY_INVERTED: u32 = 0x150C;
pub const NV4097_LOGIC_OP_OR_INVERTED: u32 = 0x150D;
pub const NV4097_LOGIC_OP_NAND: u32 = 0x150E;
pub const NV4097_LOGIC_OP_SET: u32 = 0x150F;

// Texture type constants
pub const NV4097_TEXTURE_TYPE_1D: u32 = 0x01;
pub const NV4097_TEXTURE_TYPE_2D: u32 = 0x02;
pub const NV4097_TEXTURE_TYPE_3D: u32 = 0x03;
pub const NV4097_TEXTURE_TYPE_CUBE: u32 = 0x04;

// Clear surface flags
pub const NV4097_CLEAR_Z: u32 = 0x01;
pub const NV4097_CLEAR_S: u32 = 0x02;
pub const NV4097_CLEAR_R: u32 = 0x10;
pub const NV4097_CLEAR_G: u32 = 0x20;
pub const NV4097_CLEAR_B: u32 = 0x40;
pub const NV4097_CLEAR_A: u32 = 0x80;
pub const NV4097_CLEAR_COLOR: u32 = NV4097_CLEAR_R | NV4097_CLEAR_G | NV4097_CLEAR_B | NV4097_CLEAR_A;
pub const NV4097_CLEAR_ALL: u32 = NV4097_CLEAR_Z | NV4097_CLEAR_S | NV4097_CLEAR_COLOR;

/// Handler for NV4097 methods
pub struct MethodHandler;

impl MethodHandler {
    /// Parse draw command data into first vertex/index and count
    /// The format is: (count << 24) | first for small counts, or full data for larger counts
    fn parse_draw_command(data: u32) -> (u32, u32) {
        let first = data & 0x00FFFFFF;
        let count = (data >> 24) & 0xFF;
        // If count is 0 in upper byte, it means the entire word is the count
        if count == 0 {
            (0, data)
        } else {
            (first, count)
        }
    }
    
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
            NV4097_SET_STENCIL_OP_FAIL => {
                state.stencil_op_fail = data;
            }
            NV4097_SET_STENCIL_OP_ZFAIL => {
                state.stencil_op_zfail = data;
            }
            NV4097_SET_STENCIL_OP_ZPASS => {
                state.stencil_op_zpass = data;
            }
            NV4097_SET_STENCIL_MASK => {
                state.stencil_write_mask = data as u8;
            }
            NV4097_SET_BLEND_COLOR => {
                // Blend color is packed as RGBA (8 bits each)
                state.blend_color[0] = ((data >> 16) & 0xFF) as f32 / 255.0;  // R
                state.blend_color[1] = ((data >> 8) & 0xFF) as f32 / 255.0;   // G
                state.blend_color[2] = (data & 0xFF) as f32 / 255.0;          // B
                state.blend_color[3] = ((data >> 24) & 0xFF) as f32 / 255.0;  // A
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

            // Alpha test
            NV4097_SET_ALPHA_TEST_ENABLE => {
                state.alpha_test_enable = data != 0;
            }
            NV4097_SET_ALPHA_FUNC => {
                state.alpha_test_func = data;
            }
            NV4097_SET_ALPHA_REF => {
                state.alpha_test_ref = f32::from_bits(data);
            }

            // Polygon offset
            NV4097_SET_POLYGON_OFFSET_FILL_ENABLE => {
                state.polygon_offset_fill_enable = data != 0;
            }
            NV4097_SET_POLYGON_OFFSET_LINE_ENABLE => {
                state.polygon_offset_line_enable = data != 0;
            }
            NV4097_SET_POLYGON_OFFSET_POINT_ENABLE => {
                state.polygon_offset_point_enable = data != 0;
            }
            NV4097_SET_POLYGON_OFFSET_SCALE_FACTOR => {
                state.polygon_offset_factor = f32::from_bits(data);
            }
            NV4097_SET_POLYGON_OFFSET_BIAS => {
                state.polygon_offset_units = f32::from_bits(data);
            }

            // Line and point
            NV4097_SET_LINE_WIDTH => {
                state.line_width = f32::from_bits(data);
            }
            NV4097_SET_POINT_SIZE => {
                state.point_size = f32::from_bits(data);
            }
            NV4097_SET_POINT_SPRITE_CONTROL => {
                state.point_sprite_enable = (data & 0x1) != 0;
            }

            // Anti-aliasing
            NV4097_SET_ANTI_ALIASING_CONTROL => {
                state.multisample_enable = (data & 0x1) != 0;
                state.sample_alpha_to_coverage_enable = (data & 0x10) != 0;
            }
            NV4097_SET_SAMPLE_COUNT_CONTROL => {
                // Extract sample count from data
                // 0 = 1 sample, 1 = 2 samples, 2 = 4 samples, 3 = 8 samples
                state.sample_count = match data & 0x3 {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 1,
                };
            }

            // Primitive restart
            NV4097_SET_RESTART_INDEX_ENABLE => {
                state.primitive_restart_enable = data != 0;
            }
            NV4097_SET_RESTART_INDEX => {
                state.primitive_restart_index = data;
            }

            // Occlusion query
            NV4097_SET_ZPASS_PIXEL_COUNT_ENABLE => {
                state.occlusion_query_enable = data != 0;
            }
            NV4097_SET_REPORT_SEMAPHORE_OFFSET => {
                state.occlusion_query_offset = data;
            }

            // Scissor
            NV4097_SET_SCISSOR_HORIZONTAL => {
                state.scissor_x = (data & 0xFFFF) as u16;
                state.scissor_width = ((data >> 16) & 0xFFFF) as u16;
            }
            NV4097_SET_SCISSOR_VERTICAL => {
                state.scissor_y = (data & 0xFFFF) as u16;
                state.scissor_height = ((data >> 16) & 0xFFFF) as u16;
            }

            // Logic operations
            NV4097_SET_LOGIC_OP_ENABLE => {
                state.logic_op_enable = data != 0;
            }
            NV4097_SET_LOGIC_OP => {
                state.logic_op = data;
            }

            // Color mask
            NV4097_SET_COLOR_MASK => {
                state.color_mask = data;
            }
            NV4097_SET_COLOR_MASK_MRT => {
                state.color_mask_mrt = data;
            }

            // Fog
            NV4097_SET_FOG_MODE => {
                state.fog_mode = data;
            }
            NV4097_SET_FOG_PARAMS => {
                // Fog parameter 0 (typically fog start/scale)
                state.fog_params[0] = f32::from_bits(data);
            }
            NV4097_SET_FOG_PARAMS_1 => {
                // Fog parameter 1 (typically fog end/bias)
                state.fog_params[1] = f32::from_bits(data);
            }

            // Dither
            NV4097_SET_DITHER_ENABLE => {
                state.dither_enable = data != 0;
            }

            // Two-sided stencil
            NV4097_SET_TWO_SIDED_STENCIL_TEST_ENABLE => {
                state.two_sided_stencil_enable = data != 0;
            }
            NV4097_SET_BACK_STENCIL_FUNC => {
                state.back_stencil_func = data;
            }
            NV4097_SET_BACK_STENCIL_FUNC_REF => {
                state.back_stencil_ref = data as u8;
            }
            NV4097_SET_BACK_STENCIL_FUNC_MASK => {
                state.back_stencil_mask = data as u8;
            }
            NV4097_SET_BACK_STENCIL_OP_FAIL => {
                state.back_stencil_op_fail = data;
            }
            NV4097_SET_BACK_STENCIL_OP_ZFAIL => {
                state.back_stencil_op_zfail = data;
            }
            NV4097_SET_BACK_STENCIL_OP_ZPASS => {
                state.back_stencil_op_zpass = data;
            }
            NV4097_SET_BACK_STENCIL_MASK => {
                state.back_stencil_write_mask = data as u8;
            }

            // Additional blend methods
            NV4097_SET_BLEND_ENABLE_MRT => {
                state.blend_enable_mrt = data;
            }
            NV4097_SET_BLEND_EQUATION_RGB => {
                state.blend_equation_rgb = data;
            }
            NV4097_SET_BLEND_EQUATION_ALPHA => {
                state.blend_equation_alpha = data;
            }

            // Polygon smooth
            NV4097_SET_POLYGON_SMOOTH_ENABLE => {
                state.polygon_smooth_enable = data != 0;
            }

            // Semaphore
            NV4097_SET_SEMAPHORE_OFFSET => {
                state.semaphore_offset = data;
            }
            NV4097_BACK_END_WRITE_SEMAPHORE_RELEASE | NV4097_TEXTURE_READ_SEMAPHORE_RELEASE => {
                // Signal semaphore - handled by RSX thread
                tracing::trace!("Semaphore release: method=0x{:04X}, data=0x{:08X}", method, data);
            }

            // Transform feedback
            NV4097_SET_TRANSFORM_FEEDBACK_ENABLE => {
                state.transform_feedback_enable = data != 0;
            }
            
            // Conditional rendering
            NV4097_SET_CONDITIONAL_RENDER_ENABLE => {
                state.conditional_render_enable = data != 0;
            }
            NV4097_SET_CONDITIONAL_RENDER_MODE => {
                state.conditional_render_mode = data;
            }
            
            // Surface type
            NV4097_SET_SURFACE_TYPE => {
                state.surface_type = data;
            }

            // Shader control
            NV4097_SET_SHADER_CONTROL => {
                state.shader_control = data;
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

            // Draw commands - record state and log for RSX thread
            NV4097_DRAW_ARRAYS => {
                let (first, count) = Self::parse_draw_command(data);
                state.draw_first = first;
                state.draw_count = count;
                tracing::trace!("DRAW_ARRAYS: first={}, count={}", state.draw_first, state.draw_count);
            }
            NV4097_DRAW_INDEX_ARRAY => {
                let (first, count) = Self::parse_draw_command(data);
                state.draw_first = first;
                state.draw_count = count;
                tracing::trace!("DRAW_INDEX_ARRAY: first={}, count={}", state.draw_first, state.draw_count);
            }
            NV4097_CLEAR_SURFACE => {
                // Clear surface with specified flags
                // Bit 0: Clear Z, Bit 1: Clear Stencil
                // Bits 4-7: Clear Color (R, G, B, A)
                let clear_z = (data & NV4097_CLEAR_Z) != 0;
                let clear_s = (data & NV4097_CLEAR_S) != 0;
                let clear_r = (data & NV4097_CLEAR_R) != 0;
                let clear_g = (data & NV4097_CLEAR_G) != 0;
                let clear_b = (data & NV4097_CLEAR_B) != 0;
                let clear_a = (data & NV4097_CLEAR_A) != 0;
                tracing::trace!(
                    "CLEAR_SURFACE: Z={}, S={}, R={}, G={}, B={}, A={}",
                    clear_z, clear_s, clear_r, clear_g, clear_b, clear_a
                );
            }
            NV4097_SET_PRIMITIVE_TYPE => {
                state.primitive_type = data;
            }
            NV4097_INLINE_ARRAY | NV4097_ARRAY_ELEMENT16 | NV4097_ARRAY_ELEMENT32 => {
                // These are handled by the RSX thread, not just state updates
                tracing::trace!("Draw command: method=0x{:04X}, data=0x{:08X}", method, data);
            }

            // Draw commands
            NV4097_SET_BEGIN_END => {
                if data != 0 {
                    state.primitive_type = data;
                }
            }

            NV4097_SET_SURFACE_PITCH_Z => {
                state.surface_pitch_z = data;
            }
            NV4097_SET_SURFACE_CLIP_ID => {
                state.surface_clip_id = data;
            }
            NV4097_SET_INDEX_ARRAY_ADDRESS => {
                state.index_array_address = data;
            }
            NV4097_SET_INDEX_ARRAY_DMA => {
                state.index_array_dma = data;
            }
            NV4097_SET_VERTEX_PROGRAM_START_SLOT => {
                state.vertex_program_start_slot = data;
            }
            NV4097_SET_VERTEX_PROGRAM_LOAD_SLOT => {
                state.vertex_program_load_slot = data;
            }
            NV4097_SET_BLEND_COLOR2 => {
                state.blend_color2 = data;
            }

            _ => {
                // Check for vertex attribute array ranges
                if method >= NV4097_SET_VERTEX_DATA_ARRAY_FORMAT 
                    && method < NV4097_SET_VERTEX_DATA_ARRAY_FORMAT + 16 {
                    let index = (method - NV4097_SET_VERTEX_DATA_ARRAY_FORMAT) as usize;
                    if index < state.vertex_attrib_format.len() {
                        state.vertex_attrib_format[index] = data;
                    }
                } else if method >= NV4097_SET_VERTEX_DATA_ARRAY_OFFSET 
                    && method < NV4097_SET_VERTEX_DATA_ARRAY_OFFSET + 16 {
                    let index = (method - NV4097_SET_VERTEX_DATA_ARRAY_OFFSET) as usize;
                    if index < state.vertex_attrib_offset.len() {
                        state.vertex_attrib_offset[index] = data;
                    }
                }
                // Vertex program constants (0x0B00 - 0x0EFC, 4 floats per constant)
                else if method >= NV4097_SET_TRANSFORM_CONSTANT 
                    && method < NV4097_SET_TRANSFORM_CONSTANT_END {
                    // Each constant is 4 consecutive writes (x, y, z, w)
                    // method offset / 4 = which float component
                    let offset = method - NV4097_SET_TRANSFORM_CONSTANT;
                    let const_idx = state.vertex_constant_load_slot + (offset / 16);
                    let component = ((offset / 4) % 4) as usize;
                    
                    if (const_idx as usize) < 512 {
                        state.vertex_constants[const_idx as usize][component] = f32::from_bits(data);
                    }
                }
                // Vertex program constant load slot
                else if method == NV4097_SET_TRANSFORM_CONSTANT_LOAD {
                    state.vertex_constant_load_slot = data;
                }
                // Check for texture ranges (texture methods are spaced 0x20 apart)
                else if method >= NV4097_SET_TEXTURE_OFFSET 
                    && method < NV4097_SET_TEXTURE_OFFSET + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_OFFSET) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_OFFSET) / 0x20) as usize;
                    if index < state.texture_offset.len() {
                        state.texture_offset[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_FORMAT 
                    && method < NV4097_SET_TEXTURE_FORMAT + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_FORMAT) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_FORMAT) / 0x20) as usize;
                    if index < state.texture_format.len() {
                        state.texture_format[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_CONTROL0 
                    && method < NV4097_SET_TEXTURE_CONTROL0 + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_CONTROL0) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_CONTROL0) / 0x20) as usize;
                    if index < state.texture_control.len() {
                        state.texture_control[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_FILTER 
                    && method < NV4097_SET_TEXTURE_FILTER + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_FILTER) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_FILTER) / 0x20) as usize;
                    if index < state.texture_filter.len() {
                        state.texture_filter[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_ADDRESS 
                    && method < NV4097_SET_TEXTURE_ADDRESS + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_ADDRESS) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_ADDRESS) / 0x20) as usize;
                    if index < state.texture_address.len() {
                        state.texture_address[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_IMAGE_RECT 
                    && method < NV4097_SET_TEXTURE_IMAGE_RECT + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_IMAGE_RECT) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_IMAGE_RECT) / 0x20) as usize;
                    if index < 16 {
                        // Format: height in upper 16 bits, width in lower 16 bits
                        state.texture_width[index] = data & 0xFFFF;
                        state.texture_height[index] = (data >> 16) & 0xFFFF;
                    }
                } else if method >= NV4097_SET_TEXTURE_BORDER_COLOR 
                    && method < NV4097_SET_TEXTURE_BORDER_COLOR + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_BORDER_COLOR) % 0x20 == 0 {
                    let index = ((method - NV4097_SET_TEXTURE_BORDER_COLOR) / 0x20) as usize;
                    if index < state.texture_border_color.len() {
                        state.texture_border_color[index] = data;
                    }
                } else if method >= NV4097_SET_TEXTURE_CONTROL3 
                    && method < NV4097_SET_TEXTURE_CONTROL3 + (16 * 0x20) 
                    && (method - NV4097_SET_TEXTURE_CONTROL3) % 0x20 == 0 {
                    // Texture Control 3 - contains anisotropic filtering level, etc.
                    let index = ((method - NV4097_SET_TEXTURE_CONTROL3) / 0x20) as usize;
                    if index < state.texture_control3.len() {
                        state.texture_control3[index] = data;
                        // Extract texture depth/type from the format
                        state.texture_depth[index] = (data >> 20) & 0xFFF;
                        state.texture_type[index] = (data >> 4) & 0x03;
                    }
                }
                // Transform feedback buffer offsets (4 buffers, spaced 4 bytes apart)
                else if method >= NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_OFFSET 
                    && method < NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_OFFSET + 16 {
                    let index = ((method - NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_OFFSET) / 4) as usize;
                    if index < 4 {
                        state.transform_feedback_buffer_offset[index] = data;
                    }
                } else if method >= NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_SIZE 
                    && method < NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_SIZE + 16 {
                    let index = ((method - NV4097_SET_TRANSFORM_FEEDBACK_BUFFER_SIZE) / 4) as usize;
                    if index < 4 {
                        state.transform_feedback_buffer_size[index] = data;
                    }
                } else {
                    // Unknown or unimplemented method
                    tracing::trace!("Unimplemented NV4097 method: 0x{:04X}", method);
                }
            }
        }
    }
    
    /// Parse surface format into color and depth format components
    pub fn parse_surface_format(format: u32) -> (u32, u32, u32, u8, u8) {
        // Surface format is encoded as:
        // bits 0-3: color format
        // bits 4-7: depth format  
        // bits 8-11: type (linear, swizzle, tile)
        // bits 12-15: antialias mode
        // bits 16-23: log2 width
        // bits 24-31: log2 height
        let color_format = format & 0x1F;
        let depth_format = (format >> 5) & 0x07;
        let surface_type = (format >> 8) & 0x0F;
        let log2_width = ((format >> 16) & 0xFF) as u8;
        let log2_height = ((format >> 24) & 0xFF) as u8;
        
        (color_format, depth_format, surface_type, log2_width, log2_height)
    }
    
    /// Get the bytes per pixel for a given color format
    pub fn color_format_bpp(format: u32) -> u32 {
        match format {
            NV4097_SURFACE_FORMAT_B8 => 1,
            NV4097_SURFACE_FORMAT_G8B8 | NV4097_SURFACE_FORMAT_R5G6B5 |
            NV4097_SURFACE_FORMAT_X1R5G5B5_Z1R5G5B5 | NV4097_SURFACE_FORMAT_X1R5G5B5_O1R5G5B5 => 2,
            NV4097_SURFACE_FORMAT_A8R8G8B8 | NV4097_SURFACE_FORMAT_X8R8G8B8_Z8R8G8B8 |
            NV4097_SURFACE_FORMAT_X8R8G8B8_O8R8G8B8 | NV4097_SURFACE_FORMAT_A8B8G8R8 |
            NV4097_SURFACE_FORMAT_X8B8G8R8_Z8B8G8R8 | NV4097_SURFACE_FORMAT_X8B8G8R8_O8B8G8R8 |
            NV4097_SURFACE_FORMAT_F_X32 => 4,
            NV4097_SURFACE_FORMAT_F_W16Z16Y16X16 => 8,
            NV4097_SURFACE_FORMAT_F_W32Z32Y32X32 => 16,
            _ => 4, // Default to 32-bit
        }
    }
    
    /// Get the bytes per pixel for a given depth format
    pub fn depth_format_bpp(format: u32) -> u32 {
        match format {
            NV4097_SURFACE_DEPTH_FORMAT_Z16 => 2,
            NV4097_SURFACE_DEPTH_FORMAT_Z24S8 => 4,
            _ => 4, // Default to 32-bit
        }
    }
    
    /// Calculate surface pitch for swizzled surfaces
    /// Returns power-of-two aligned pitch, or None if calculation would overflow
    pub fn calculate_swizzle_pitch(width: u32, bpp: u32) -> Option<u32> {
        // Swizzled surfaces have power-of-two aligned pitch
        let min_pitch = width.checked_mul(bpp)?;
        Some(min_pitch.next_power_of_two())
    }
    
    /// Check if a primitive type supports primitive restart
    pub fn primitive_supports_restart(prim_type: u32) -> bool {
        matches!(prim_type, 
            NV4097_PRIMITIVE_TRIANGLE_STRIP | NV4097_PRIMITIVE_TRIANGLE_FAN |
            NV4097_PRIMITIVE_LINE_STRIP | NV4097_PRIMITIVE_LINE_LOOP |
            NV4097_PRIMITIVE_QUAD_STRIP)
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

    #[test]
    fn test_surface_pitch_z() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_SURFACE_PITCH_Z, 0x100, &mut state);
        assert_eq!(state.surface_pitch_z, 0x100);
    }

    #[test]
    fn test_surface_clip_id() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_SURFACE_CLIP_ID, 0x03, &mut state);
        assert_eq!(state.surface_clip_id, 0x03);
    }

    #[test]
    fn test_index_array_address() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_INDEX_ARRAY_ADDRESS, 0x80000, &mut state);
        assert_eq!(state.index_array_address, 0x80000);
    }

    #[test]
    fn test_index_array_dma() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_INDEX_ARRAY_DMA, 0xFEED0001, &mut state);
        assert_eq!(state.index_array_dma, 0xFEED0001);
    }

    #[test]
    fn test_vertex_program_slots() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_VERTEX_PROGRAM_START_SLOT, 10, &mut state);
        assert_eq!(state.vertex_program_start_slot, 10);
        MethodHandler::execute(NV4097_SET_VERTEX_PROGRAM_LOAD_SLOT, 20, &mut state);
        assert_eq!(state.vertex_program_load_slot, 20);
    }

    #[test]
    fn test_blend_color2() {
        let mut state = RsxState::new();
        MethodHandler::execute(NV4097_SET_BLEND_COLOR2, 0xFF00FF00, &mut state);
        assert_eq!(state.blend_color2, 0xFF00FF00);
    }
}
