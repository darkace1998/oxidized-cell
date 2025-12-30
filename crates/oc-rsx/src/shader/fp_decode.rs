//! RSX Fragment Program decoder
//!
//! Decodes 128-bit fragment program instructions into a structured format.
//! 
//! Fragment programs use a byte-swapped encoding where each 16-bit half-word
//! is swapped within each 32-bit word.

use super::types::*;

/// Maximum fragment program instructions
pub const MAX_FP_INSTRUCTIONS: usize = 512;

/// Fragment program decoder
pub struct FpDecoder;

impl FpDecoder {
    /// Decode a fragment program from raw data
    /// 
    /// Each instruction is 128 bits (4 x u32 words).
    /// Note: FP instructions use half-word swapped encoding!
    pub fn decode(program: &mut FragmentProgram) -> Result<(), String> {
        let data = &program.instructions;
        if data.len() % 4 != 0 {
            return Err("Fragment program data must be multiple of 4 words".to_string());
        }

        let instr_count = data.len() / 4;
        if instr_count > MAX_FP_INSTRUCTIONS {
            return Err(format!(
                "Too many FP instructions: {} (max {})",
                instr_count, MAX_FP_INSTRUCTIONS
            ));
        }

        program.decoded.clear();
        program.decoded.reserve(instr_count);

        let mut i = 0;
        while i < instr_count {
            let base = i * 4;

            // Decode with half-word swap applied
            let word0 = Self::swap_halfwords(data[base]);
            let word1 = Self::swap_halfwords(data[base + 1]);
            let word2 = Self::swap_halfwords(data[base + 2]);
            let word3 = Self::swap_halfwords(data[base + 3]);

            let instr = Self::decode_instruction(word0, word1, word2, word3)?;

            // Track texture unit usage
            if Self::is_texture_op(instr.opcode) {
                program.texture_mask |= 1 << instr.tex_unit;
            }

            let is_end = instr.end;
            program.decoded.push(instr);

            i += 1;

            // Some instructions have embedded literal constants (next 128 bits)
            // Skip them here (they would be read by spirv_gen when needed)

            if is_end {
                break;
            }
        }

        Ok(())
    }

    /// Swap half-words in a 32-bit value (RSX FP encoding quirk)
    #[inline]
    fn swap_halfwords(val: u32) -> u32 {
        ((val & 0xFF00FF00) >> 8) | ((val & 0x00FF00FF) << 8)
    }

    /// Decode a single 128-bit FP instruction (after half-word swap)
    pub fn decode_instruction(
        word0: u32,
        word1: u32,
        word2: u32,
        word3: u32,
    ) -> Result<DecodedFpInstruction, String> {
        let dest = FpOpDest::decode(word0);

        // Check for extended opcode (bit 31 of word2 indicates branch/flow control)
        let opcode = if (word2 & (1 << 31)) != 0 {
            // Flow control - high bit of opcode comes from SRC1
            let base_opcode = (word0 >> 24) & 0x3F;
            FpOpcode::from((base_opcode | 0x40) as u8)
        } else {
            dest.opcode
        };

        // Decode source operands
        let src0 = FpSource::decode(word1, true);
        let src1 = FpSource::decode(word2, false);
        let src2 = FpSource::decode(word3, false);

        // Texture unit from src1 bits
        let tex_unit = ((word2 >> 19) & 0xF) as u8;

        Ok(DecodedFpInstruction {
            opcode,
            dest,
            sources: [src0, src1, src2],
            tex_unit,
            end: dest.end,
        })
    }

    /// Check if opcode is a texture sampling operation
    fn is_texture_op(opcode: FpOpcode) -> bool {
        matches!(
            opcode,
            FpOpcode::Tex | FpOpcode::Txp | FpOpcode::Txd | FpOpcode::Txl | FpOpcode::Txb
        )
    }
}

/// Fragment input attribute names
pub const FP_INPUT_NAMES: [&str; 15] = [
    "WPOS",  // 0 - Window position
    "COL0",  // 1 - Primary color
    "COL1",  // 2 - Secondary color
    "FOGC",  // 3 - Fog coordinate
    "TEX0",  // 4 - Texture coord 0
    "TEX1",  // 5 - Texture coord 1
    "TEX2",  // 6 - Texture coord 2
    "TEX3",  // 7 - Texture coord 3
    "TEX4",  // 8 - Texture coord 4
    "TEX5",  // 9 - Texture coord 5
    "TEX6",  // 10 - Texture coord 6
    "TEX7",  // 11 - Texture coord 7
    "TEX8",  // 12 - Texture coord 8
    "TEX9",  // 13 - Texture coord 9
    "SSA",   // 14 - ?
];

/// Opcode names for debugging
pub const FP_OPCODE_NAMES: [&str; 70] = [
    "NOP", "MOV", "MUL", "ADD", "MAD", "DP3", "DP4", "DST",
    "MIN", "MAX", "SLT", "SGE", "SLE", "SGT", "SNE", "SEQ",
    "FRC", "FLR", "KIL", "PK4", "UP4", "DDX", "DDY", "TEX",
    "TXP", "TXD", "RCP", "RSQ", "EX2", "LG2", "LIT", "LRP",
    "STR", "SFL", "COS", "SIN", "PK2", "UP2", "POW", "PKB",
    "UPB", "PK16", "UP16", "BEM", "PKG", "UPG", "DP2A", "TXL",
    "???", "TXB", "???", "???", "???", "???", "REFL", "???",
    "DP2", "NRM", "DIV", "DIVSQ", "LIF", "FENCT", "FENCB", "???",
    "BRK", "CAL", "IFE", "LOOP", "REP", "RET",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_halfwords() {
        assert_eq!(FpDecoder::swap_halfwords(0x12345678), 0x34127856);
        assert_eq!(FpDecoder::swap_halfwords(0xAABBCCDD), 0xBBAADDCC);
    }

    #[test]
    fn test_fp_decode_nop() {
        // After swap, all zeros should still be NOP
        let instr = FpDecoder::decode_instruction(0, 0, 0, 0).unwrap();
        assert_eq!(instr.opcode, FpOpcode::Nop);
    }
}
