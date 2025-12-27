//! SPU instruction decoder

/// Decoded SPU instruction
#[derive(Debug, Clone, Copy)]
pub struct DecodedSpuInstruction {
    /// Raw opcode
    pub opcode: u32,
    /// Instruction type
    pub itype: SpuInstructionType,
}

/// SPU instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuInstructionType {
    /// RRR-type (three register operands)
    RRR,
    /// RR-type (two register operands)
    RR,
    /// RI7-type (immediate 7-bit)
    RI7,
    /// RI10-type (immediate 10-bit)
    RI10,
    /// RI16-type (immediate 16-bit)
    RI16,
    /// RI18-type (immediate 18-bit)
    RI18,
    /// Special forms
    Special,
    /// Unknown
    Unknown,
}

/// SPU instruction decoder
pub struct SpuDecoder;

impl SpuDecoder {
    /// Decode a 32-bit SPU instruction
    pub fn decode(opcode: u32) -> DecodedSpuInstruction {
        // SPU uses big-endian, variable-length opcodes
        let op11 = (opcode >> 21) & 0x7FF;
        let op10 = (opcode >> 22) & 0x3FF;
        let op9 = (opcode >> 23) & 0x1FF;
        let op8 = (opcode >> 24) & 0xFF;
        let op7 = (opcode >> 25) & 0x7F;
        let op4 = (opcode >> 28) & 0xF;

        // Determine instruction type based on opcode patterns
        let itype = if (op4 == 0b0100) || (op4 == 0b1100) {
            // RI18-type (branches)
            SpuInstructionType::RI18
        } else if (0b0100000..=0b0111111).contains(&op7) {
            // RI16-type
            SpuInstructionType::RI16
        } else if (0b00010000..=0b00011111).contains(&op8) {
            // RI10-type
            SpuInstructionType::RI10
        } else if op9 >= 0b011100000 {
            // RI7-type
            SpuInstructionType::RI7
        } else if op10 >= 0b0001100000 {
            // RR-type
            SpuInstructionType::RR
        } else if op11 >= 0b00001000000 {
            // RRR-type
            SpuInstructionType::RRR
        } else {
            SpuInstructionType::Unknown
        };

        DecodedSpuInstruction { opcode, itype }
    }

    /// Extract RRR-type fields: rc, rb, ra, rt
    #[inline]
    pub fn rrr_form(opcode: u32) -> (u8, u8, u8, u8) {
        let rt = (opcode & 0x7F) as u8;
        let ra = ((opcode >> 7) & 0x7F) as u8;
        let rb = ((opcode >> 14) & 0x7F) as u8;
        let rc = ((opcode >> 21) & 0x7F) as u8;
        (rc, rb, ra, rt)
    }

    /// Extract RR-type fields: rb, ra, rt
    #[inline]
    pub fn rr_form(opcode: u32) -> (u8, u8, u8) {
        let rt = (opcode & 0x7F) as u8;
        let ra = ((opcode >> 7) & 0x7F) as u8;
        let rb = ((opcode >> 14) & 0x7F) as u8;
        (rb, ra, rt)
    }

    /// Extract RI7-type fields: i7, ra, rt
    #[inline]
    pub fn ri7_form(opcode: u32) -> (i8, u8, u8) {
        let rt = (opcode & 0x7F) as u8;
        let ra = ((opcode >> 7) & 0x7F) as u8;
        let i7 = ((opcode >> 14) & 0x7F) as i8;
        // Sign extend from 7 bits
        let i7 = if i7 & 0x40 != 0 { i7 | !0x7F } else { i7 };
        (i7, ra, rt)
    }

    /// Extract RI10-type fields: i10, ra, rt
    #[inline]
    pub fn ri10_form(opcode: u32) -> (i16, u8, u8) {
        let rt = (opcode & 0x7F) as u8;
        let ra = ((opcode >> 7) & 0x7F) as u8;
        let i10 = ((opcode >> 14) & 0x3FF) as i16;
        // Sign extend from 10 bits
        let i10 = if i10 & 0x200 != 0 { i10 | !0x3FF } else { i10 };
        (i10, ra, rt)
    }

    /// Extract RI16-type fields: i16, rt
    #[inline]
    pub fn ri16_form(opcode: u32) -> (i16, u8) {
        let rt = (opcode & 0x7F) as u8;
        let i16_val = ((opcode >> 7) & 0xFFFF) as i16;
        (i16_val, rt)
    }

    /// Extract RI18-type fields: i18, rt
    #[inline]
    pub fn ri18_form(opcode: u32) -> (i32, u8) {
        let rt = (opcode & 0x7F) as u8;
        let i18 = ((opcode >> 7) & 0x3FFFF) as i32;
        // Sign extend from 18 bits
        let i18 = if i18 & 0x20000 != 0 { i18 | !0x3FFFF } else { i18 };
        (i18, rt)
    }

    /// Get the 11-bit opcode
    #[inline]
    pub fn op11(opcode: u32) -> u16 {
        ((opcode >> 21) & 0x7FF) as u16
    }

    /// Get the 10-bit opcode
    #[inline]
    pub fn op10(opcode: u32) -> u16 {
        ((opcode >> 22) & 0x3FF) as u16
    }

    /// Get the 9-bit opcode
    #[inline]
    pub fn op9(opcode: u32) -> u16 {
        ((opcode >> 23) & 0x1FF) as u16
    }

    /// Get the 8-bit opcode
    #[inline]
    pub fn op8(opcode: u32) -> u8 {
        ((opcode >> 24) & 0xFF) as u8
    }

    /// Get the 7-bit opcode
    #[inline]
    pub fn op7(opcode: u32) -> u8 {
        ((opcode >> 25) & 0x7F) as u8
    }

    /// Get the 4-bit opcode
    #[inline]
    pub fn op4(opcode: u32) -> u8 {
        ((opcode >> 28) & 0xF) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rr_form() {
        // Sample instruction
        let opcode = 0x00000000u32;
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
        assert_eq!(rt, 0);
        assert_eq!(ra, 0);
        assert_eq!(rb, 0);
    }

    #[test]
    fn test_ri10_form() {
        let opcode = 0x00000000u32;
        let (i10, ra, rt) = SpuDecoder::ri10_form(opcode);
        assert_eq!(rt, 0);
        assert_eq!(ra, 0);
        assert_eq!(i10, 0);
    }
}
