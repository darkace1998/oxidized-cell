//! PPU instruction decoder

/// Decoded PPU instruction
#[derive(Debug, Clone, Copy)]
pub struct DecodedInstruction {
    /// Raw opcode
    pub opcode: u32,
    /// Primary opcode (bits 0-5)
    pub op: u8,
    /// Extended opcode (various positions depending on instruction form)
    pub xo: u16,
    /// Instruction form
    pub form: InstructionForm,
}

/// PPU instruction forms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionForm {
    /// I-Form: Branch instructions
    I,
    /// B-Form: Conditional branch
    B,
    /// SC-Form: System call
    SC,
    /// D-Form: Load/store with displacement
    D,
    /// DS-Form: Load/store double with displacement
    DS,
    /// X-Form: Indexed load/store, misc
    X,
    /// XL-Form: Branch conditional to LR/CTR
    XL,
    /// XFX-Form: Move to/from special registers
    XFX,
    /// XFL-Form: Move to FPSCR
    XFL,
    /// XS-Form: Shift double
    XS,
    /// XO-Form: Integer arithmetic
    XO,
    /// A-Form: Floating-point multiply-add
    A,
    /// M-Form: Rotate and mask
    M,
    /// MD-Form: Rotate and mask (64-bit)
    MD,
    /// MDS-Form: Rotate and mask shift (64-bit)
    MDS,
    /// VA-Form: Vector three-operand
    VA,
    /// VX-Form: Vector two-operand
    VX,
    /// VXR-Form: Vector compare
    VXR,
    /// Unknown form
    Unknown,
}

/// PPU instruction decoder
pub struct PpuDecoder;

impl PpuDecoder {
    /// Decode a 32-bit PPU instruction
    pub fn decode(opcode: u32) -> DecodedInstruction {
        let op = ((opcode >> 26) & 0x3F) as u8;
        
        let (form, xo) = match op {
            // I-Form branches
            18 => (InstructionForm::I, 0),
            
            // B-Form conditional branches
            16 => (InstructionForm::B, 0),
            
            // SC-Form system call
            17 => (InstructionForm::SC, 0),
            
            // D-Form load/store
            14 | 15 | // addi, addis
            32..=39 | // lwz, lwzu, lbz, lbzu, stw, stwu, stb, stbu
            40..=47 | // lhz, lhzu, lha, lhau, sth, sthu, lmw, stmw
            48..=55 | // lfs, lfsu, lfd, lfdu, stfs, stfsu, stfd, stfdu
            8..=13 | // subfic, cmpli, cmpi, addic, addic., mulli
            24..=29 | // ori, oris, xori, xoris, andi., andis.
            2 | 3 | // tdi, twi
            7 | // mulli
            58 => (InstructionForm::D, 0),
            
            // DS-Form
            62 => (InstructionForm::DS, 0),
            
            // Special opcode groups
            19 => {
                let xo = ((opcode >> 1) & 0x3FF) as u16;
                (InstructionForm::XL, xo)
            }
            
            31 => {
                let xo = ((opcode >> 1) & 0x3FF) as u16;
                // Differentiate between X-form and XO-form based on extended opcode
                // XO-form arithmetic instructions use a 9-bit xo field (bits 22-30)
                let xo_9bit = ((opcode >> 1) & 0x1FF) as u16;
                
                match xo {
                    // XO-form: Integer arithmetic with OE bit
                    // Arithmetic operations
                    8 =>   // subfc - Subtract From Carrying
                           (InstructionForm::XO, xo_9bit),
                    10 =>  // addc - Add Carrying
                           (InstructionForm::XO, xo_9bit),
                    40 =>  // subf - Subtract From
                           (InstructionForm::XO, xo_9bit),
                    104 => // neg - Negate
                           (InstructionForm::XO, xo_9bit),
                    136 => // subfe - Subtract From Extended
                           (InstructionForm::XO, xo_9bit),
                    138 => // adde - Add Extended
                           (InstructionForm::XO, xo_9bit),
                    200 => // subfze - Subtract From Zero Extended
                           (InstructionForm::XO, xo_9bit),
                    202 => // addze - Add to Zero Extended
                           (InstructionForm::XO, xo_9bit),
                    232 => // subfme - Subtract From Minus One Extended
                           (InstructionForm::XO, xo_9bit),
                    234 => // addme - Add to Minus One Extended
                           (InstructionForm::XO, xo_9bit),
                    266 => // add - Add
                           (InstructionForm::XO, xo_9bit),
                    // Multiply operations
                    9 =>   // mulhdu - Multiply High Doubleword Unsigned
                           (InstructionForm::XO, xo_9bit),
                    11 =>  // mulhwu - Multiply High Word Unsigned
                           (InstructionForm::XO, xo_9bit),
                    73 =>  // mulhd - Multiply High Doubleword
                           (InstructionForm::XO, xo_9bit),
                    75 =>  // mulhw - Multiply High Word
                           (InstructionForm::XO, xo_9bit),
                    233 => // mulld - Multiply Low Doubleword
                           (InstructionForm::XO, xo_9bit),
                    235 => // mullw - Multiply Low Word
                           (InstructionForm::XO, xo_9bit),
                    // Division operations
                    457 => // divdu - Divide Doubleword Unsigned
                           (InstructionForm::XO, xo_9bit),
                    459 => // divwu - Divide Word Unsigned
                           (InstructionForm::XO, xo_9bit),
                    489 => // divd - Divide Doubleword
                           (InstructionForm::XO, xo_9bit),
                    491 => // divw - Divide Word
                           (InstructionForm::XO, xo_9bit),
                    _ => {
                        // X-form and other variants (10-bit xo)
                        (InstructionForm::X, xo)
                    }
                }
            }
            
            30 => {
                // MD/MDS-Form rotate
                let xo = ((opcode >> 2) & 0x7) as u16;
                (InstructionForm::MD, xo)
            }
            
            // M-Form rotate
            20..=23 => (InstructionForm::M, 0),
            
            // A-Form floating-point and X-form floating-point compare
            59 | 63 => {
                let xo_10bit = ((opcode >> 1) & 0x3FF) as u16;
                let xo_5bit = ((opcode >> 1) & 0x1F) as u16;
                // fcmpu (xo=0) and fcmpo (xo=32) are X-form within opcode 63 only
                // Opcode 59 has no X-form compare instructions
                if op == 63 && (xo_10bit == 0 || xo_10bit == 32) {
                    (InstructionForm::X, xo_10bit)
                } else {
                    (InstructionForm::A, xo_5bit)
                }
            }
            
            // Vector instructions
            4 => {
                let xo = (opcode & 0x3F) as u16;
                (InstructionForm::VA, xo)
            }
            
            // Reserved/invalid opcodes - provide better Unknown handling
            0 => {
                tracing::debug!("Reserved opcode 0 encountered - likely invalid code or data");
                (InstructionForm::Unknown, 0)
            }
            1 => {
                tracing::debug!("Reserved opcode 1 encountered - likely invalid code or data");
                (InstructionForm::Unknown, 0)
            }
            5 | 6 => {
                tracing::debug!("Reserved opcode {} encountered - likely invalid code or data", op);
                (InstructionForm::Unknown, 0)
            }
            56 | 57 | 60 | 61 => {
                // These are valid PowerPC opcodes but not commonly used on Cell BE
                tracing::debug!("Uncommon opcode {} - may need implementation", op);
                (InstructionForm::Unknown, 0)
            }
            
            _ => {
                tracing::debug!("Unknown primary opcode {} (0x{:02x})", op, op);
                (InstructionForm::Unknown, 0)
            }
        };
        
        DecodedInstruction {
            opcode,
            op,
            xo,
            form,
        }
    }
    
    /// Extract D-form fields
    #[inline]
    pub fn d_form(opcode: u32) -> (u8, u8, i16) {
        let rt = ((opcode >> 21) & 0x1F) as u8;
        let ra = ((opcode >> 16) & 0x1F) as u8;
        let d = (opcode & 0xFFFF) as i16;
        (rt, ra, d)
    }
    
    /// Extract X-form fields
    #[inline]
    pub fn x_form(opcode: u32) -> (u8, u8, u8, u16, bool) {
        let rt = ((opcode >> 21) & 0x1F) as u8;
        let ra = ((opcode >> 16) & 0x1F) as u8;
        let rb = ((opcode >> 11) & 0x1F) as u8;
        let xo = ((opcode >> 1) & 0x3FF) as u16;
        let rc = (opcode & 1) != 0;
        (rt, ra, rb, xo, rc)
    }
    
    /// Extract XO-form fields (integer arithmetic)
    #[inline]
    pub fn xo_form(opcode: u32) -> (u8, u8, u8, bool, u16, bool) {
        let rt = ((opcode >> 21) & 0x1F) as u8;
        let ra = ((opcode >> 16) & 0x1F) as u8;
        let rb = ((opcode >> 11) & 0x1F) as u8;
        let oe = ((opcode >> 10) & 1) != 0;
        let xo = ((opcode >> 1) & 0x1FF) as u16;
        let rc = (opcode & 1) != 0;
        (rt, ra, rb, oe, xo, rc)
    }
    
    /// Extract I-form fields (branch)
    #[inline]
    pub fn i_form(opcode: u32) -> (i32, bool, bool) {
        let li = ((opcode >> 2) & 0xFFFFFF) as i32;
        // Sign extend from 24 bits
        let li = if li & 0x800000 != 0 {
            li | !0xFFFFFF
        } else {
            li
        } << 2;
        let aa = ((opcode >> 1) & 1) != 0;
        let lk = (opcode & 1) != 0;
        (li, aa, lk)
    }
    
    /// Extract B-form fields (conditional branch)
    #[inline]
    pub fn b_form(opcode: u32) -> (u8, u8, i16, bool, bool) {
        let bo = ((opcode >> 21) & 0x1F) as u8;
        let bi = ((opcode >> 16) & 0x1F) as u8;
        let bd = ((opcode >> 2) & 0x3FFF) as i16;
        // Sign extend from 14 bits
        let bd = if bd & 0x2000 != 0 {
            bd | !0x3FFF
        } else {
            bd
        } << 2;
        let aa = ((opcode >> 1) & 1) != 0;
        let lk = (opcode & 1) != 0;
        (bo, bi, bd, aa, lk)
    }
    
    /// Extract M-form fields (rotate)
    #[inline]
    pub fn m_form(opcode: u32) -> (u8, u8, u8, u8, u8, bool) {
        let rs = ((opcode >> 21) & 0x1F) as u8;
        let ra = ((opcode >> 16) & 0x1F) as u8;
        let rb = ((opcode >> 11) & 0x1F) as u8;
        let mb = ((opcode >> 6) & 0x1F) as u8;
        let me = ((opcode >> 1) & 0x1F) as u8;
        let rc = (opcode & 1) != 0;
        (rs, ra, rb, mb, me, rc)
    }
    
    /// Get a human-readable mnemonic for the instruction (best effort)
    pub fn get_mnemonic(opcode: u32) -> &'static str {
        let op = ((opcode >> 26) & 0x3F) as u8;
        
        match op {
            // Common D-form
            14 => "addi",
            15 => "addis",
            32 => "lwz",
            33 => "lwzu",
            34 => "lbz",
            35 => "lbzu",
            36 => "stw",
            37 => "stwu",
            38 => "stb",
            39 => "stbu",
            40 => "lhz",
            41 => "lhzu",
            42 => "lha",
            43 => "lhau",
            44 => "sth",
            45 => "sthu",
            46 => "lmw",
            47 => "stmw",
            48 => "lfs",
            49 => "lfsu",
            50 => "lfd",
            51 => "lfdu",
            52 => "stfs",
            53 => "stfsu",
            54 => "stfd",
            55 => "stfdu",
            24 => "ori",
            25 => "oris",
            26 => "xori",
            27 => "xoris",
            28 => "andi.",
            29 => "andis.",
            // Branch/Control
            16 => "bc",
            17 => "sc",
            18 => "b",
            19 => "xl-form",
            // Integer arithmetic extended
            31 => "x-form",
            // Rotate
            20 => "rlwimi",
            21 => "rlwinm",
            23 => "rlwnm",
            30 => "md-form",
            // FP/Vector
            4 => "vector",
            59 => "fp-single",
            63 => "fp-double",
            // DS-form
            58 => "ld/ldu/lwa",
            62 => "std/stdu",
            // Comparison/arith
            7 => "mulli",
            8 => "subfic",
            10 => "cmpli",
            11 => "cmpi",
            12 => "addic",
            13 => "addic.",
            // Reserved
            0 => "reserved-0",
            1 => "reserved-1",
            2 => "tdi",
            3 => "twi",
            _ => "unknown"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_addi() {
        // addi r3, r0, 100
        let opcode = 0x38600064u32;
        let decoded = PpuDecoder::decode(opcode);
        assert_eq!(decoded.op, 14);
        assert_eq!(decoded.form, InstructionForm::D);
    }

    #[test]
    fn test_d_form_extract() {
        // addi r3, r1, 8
        let opcode = 0x38610008u32;
        let (rt, ra, d) = PpuDecoder::d_form(opcode);
        assert_eq!(rt, 3);
        assert_eq!(ra, 1);
        assert_eq!(d, 8);
    }

    #[test]
    fn test_i_form_branch() {
        // b 0x100
        let opcode = 0x48000100u32;
        let (li, aa, lk) = PpuDecoder::i_form(opcode);
        assert_eq!(li, 0x100);
        assert!(!aa);
        assert!(!lk);
    }
}
