//! RSX Vertex Program decoder
//!
//! Decodes 128-bit vertex program instructions into a structured format.

use super::types::*;

/// Maximum vertex program instructions
pub const MAX_VP_INSTRUCTIONS: usize = 512;

/// Vertex program decoder
pub struct VpDecoder;

impl VpDecoder {
    /// Decode a vertex program from raw data
    /// 
    /// Each instruction is 128 bits (4 x u32 words).
    pub fn decode(program: &mut VertexProgram) -> Result<(), String> {
        let data = &program.instructions;
        if data.len() % 4 != 0 {
            return Err("Vertex program data must be multiple of 4 words".to_string());
        }

        let instr_count = data.len() / 4;
        if instr_count > MAX_VP_INSTRUCTIONS {
            return Err(format!(
                "Too many VP instructions: {} (max {})",
                instr_count, MAX_VP_INSTRUCTIONS
            ));
        }

        program.decoded.clear();
        program.decoded.reserve(instr_count);

        for i in 0..instr_count {
            let base = i * 4;
            let instr = Self::decode_instruction(
                data[base],
                data[base + 1],
                data[base + 2],
                data[base + 3],
            )?;

            // Track input/output masks
            program.input_mask |= Self::get_input_mask(&instr);
            program.output_mask |= Self::get_output_mask(&instr);

            let is_end = instr.end;
            program.decoded.push(instr);

            if is_end {
                break;
            }
        }

        Ok(())
    }

    /// Decode a single 128-bit VP instruction
    pub fn decode_instruction(
        word0: u32,
        word1: u32,
        word2: u32,
        word3: u32,
    ) -> Result<DecodedVpInstruction, String> {
        let d0 = VpD0::decode(word0);
        let d1 = VpD1::decode(word1);
        let d2 = VpD2::decode(word2);
        let d3 = VpD3::decode(word3);

        // Decode source operands
        let src0 = VpSource::decode_src0(&d1, &d2);
        let src1 = VpSource::decode_src1(&d2);
        let src2 = VpSource::decode_src2(&d2, &d3);

        // Build writemasks (each bit represents x,y,z,w)
        let vec_writemask = ((d3.vec_writemask_x as u8) << 3)
            | ((d3.vec_writemask_y as u8) << 2)
            | ((d3.vec_writemask_z as u8) << 1)
            | (d3.vec_writemask_w as u8);

        let sca_writemask = ((d3.sca_writemask_x as u8) << 3)
            | ((d3.sca_writemask_y as u8) << 2)
            | ((d3.sca_writemask_z as u8) << 1)
            | (d3.sca_writemask_w as u8);

        Ok(DecodedVpInstruction {
            vec_opcode: d1.vec_opcode,
            sca_opcode: d1.sca_opcode,
            sources: [src0, src1, src2],
            vec_dst: d3.dst,
            sca_dst: d3.sca_dst_tmp,
            vec_writemask,
            sca_writemask,
            saturate: d0.saturate,
            end: d3.end,
            d0,
            d1,
            d2,
            d3,
        })
    }

    /// Get input attribute mask from instruction
    fn get_input_mask(instr: &DecodedVpInstruction) -> u32 {
        let mut mask = 0u32;

        // Check each source for input type
        for src in &instr.sources {
            if src.reg_type == VpRegType::Input {
                mask |= 1 << instr.d1.input_src;
            }
        }

        mask
    }

    /// Get output attribute mask from instruction
    fn get_output_mask(instr: &DecodedVpInstruction) -> u32 {
        // Output registers 0-15 correspond to various vertex outputs
        if instr.d3.dst != 0x1F {
            // Not writing to temp-only
            1 << instr.d3.dst
        } else {
            0
        }
    }
}

/// Input attribute names (based on RSX/NV40)
pub const VP_INPUT_NAMES: [&str; 16] = [
    "in_pos",        // 0 - Position
    "in_weight",     // 1 - Blend weight
    "in_normal",     // 2 - Normal
    "in_diff_color", // 3 - Primary color
    "in_spec_color", // 4 - Secondary color
    "in_fog",        // 5 - Fog coordinate
    "in_point_size", // 6 - Point size
    "in_7",          // 7 - (unused)
    "in_tc0",        // 8 - Texture coord 0
    "in_tc1",        // 9 - Texture coord 1
    "in_tc2",        // 10 - Texture coord 2
    "in_tc3",        // 11 - Texture coord 3
    "in_tc4",        // 12 - Texture coord 4
    "in_tc5",        // 13 - Texture coord 5
    "in_tc6",        // 14 - Texture coord 6
    "in_tc7",        // 15 - Texture coord 7
];

/// Output register names
pub const VP_OUTPUT_NAMES: [&str; 16] = [
    "dst_pos",       // 0 - Position (o[HPOS])
    "dst_diff",      // 1 - Diffuse color (o[COL0])
    "dst_spec",      // 2 - Specular color (o[COL1])
    "dst_back_diff", // 3 - Back diffuse
    "dst_back_spec", // 4 - Back specular
    "dst_fog",       // 5 - Fog coordinate
    "dst_point",     // 6 - Point size
    "dst_clip0",     // 7 - Clip plane 0
    "dst_tc0",       // 8 - Texture coord 0
    "dst_tc1",       // 9 - Texture coord 1
    "dst_tc2",       // 10 - Texture coord 2
    "dst_tc3",       // 11 - Texture coord 3
    "dst_tc4",       // 12 - Texture coord 4
    "dst_tc5",       // 13 - Texture coord 5
    "dst_tc6",       // 14 - Texture coord 6
    "dst_tc7",       // 15 - Texture coord 7
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vp_decode_nop() {
        let instr = VpDecoder::decode_instruction(0, 0, 0, 0).unwrap();
        assert_eq!(instr.vec_opcode, VpVecOpcode::Nop);
        assert_eq!(instr.sca_opcode, VpScaOpcode::Nop);
    }

    #[test]
    fn test_vp_decode_end_bit() {
        let instr = VpDecoder::decode_instruction(0, 0, 0, 1).unwrap();
        assert!(instr.end);
    }
}
