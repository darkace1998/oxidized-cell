//! RSX shader type definitions
//!
//! Defines the instruction format unions and common types for RSX shaders.

use bitflags::bitflags;

bitflags! {
    /// Shader stage flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShaderStage: u8 {
        const VERTEX = 0x01;
        const FRAGMENT = 0x02;
    }
}

//=============================================================================
// VERTEX PROGRAM INSTRUCTION FORMAT
//=============================================================================

/// Vertex program vector opcodes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpVecOpcode {
    Nop = 0x00,
    Mov = 0x01,
    Mul = 0x02,
    Add = 0x03,
    Mad = 0x04,
    Dp3 = 0x05,
    Dph = 0x06,
    Dp4 = 0x07,
    Dst = 0x08,
    Min = 0x09,
    Max = 0x0A,
    Slt = 0x0B,
    Sge = 0x0C,
    Arl = 0x0D,
    Frc = 0x0E,
    Flr = 0x0F,
    Seq = 0x10,
    Sfl = 0x11,
    Sgt = 0x12,
    Sle = 0x13,
    Sne = 0x14,
    Str = 0x15,
    Ssg = 0x16,
    Txl = 0x19,
}

impl From<u8> for VpVecOpcode {
    fn from(v: u8) -> Self {
        match v {
            0x00 => VpVecOpcode::Nop,
            0x01 => VpVecOpcode::Mov,
            0x02 => VpVecOpcode::Mul,
            0x03 => VpVecOpcode::Add,
            0x04 => VpVecOpcode::Mad,
            0x05 => VpVecOpcode::Dp3,
            0x06 => VpVecOpcode::Dph,
            0x07 => VpVecOpcode::Dp4,
            0x08 => VpVecOpcode::Dst,
            0x09 => VpVecOpcode::Min,
            0x0A => VpVecOpcode::Max,
            0x0B => VpVecOpcode::Slt,
            0x0C => VpVecOpcode::Sge,
            0x0D => VpVecOpcode::Arl,
            0x0E => VpVecOpcode::Frc,
            0x0F => VpVecOpcode::Flr,
            0x10 => VpVecOpcode::Seq,
            0x11 => VpVecOpcode::Sfl,
            0x12 => VpVecOpcode::Sgt,
            0x13 => VpVecOpcode::Sle,
            0x14 => VpVecOpcode::Sne,
            0x15 => VpVecOpcode::Str,
            0x16 => VpVecOpcode::Ssg,
            0x19 => VpVecOpcode::Txl,
            _ => VpVecOpcode::Nop,
        }
    }
}

/// Vertex program scalar opcodes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpScaOpcode {
    Nop = 0x00,
    Mov = 0x01,
    Rcp = 0x02,
    Rcc = 0x03,
    Rsq = 0x04,
    Exp = 0x05,
    Log = 0x06,
    Lit = 0x07,
    Bra = 0x08,
    Bri = 0x09,
    Cal = 0x0A,
    Cli = 0x0B,
    Ret = 0x0C,
    Lg2 = 0x0D,
    Ex2 = 0x0E,
    Sin = 0x0F,
    Cos = 0x10,
    Brb = 0x11,
    Clb = 0x12,
    Psh = 0x13,
    Pop = 0x14,
}

impl From<u8> for VpScaOpcode {
    fn from(v: u8) -> Self {
        match v {
            0x00 => VpScaOpcode::Nop,
            0x01 => VpScaOpcode::Mov,
            0x02 => VpScaOpcode::Rcp,
            0x03 => VpScaOpcode::Rcc,
            0x04 => VpScaOpcode::Rsq,
            0x05 => VpScaOpcode::Exp,
            0x06 => VpScaOpcode::Log,
            0x07 => VpScaOpcode::Lit,
            0x08 => VpScaOpcode::Bra,
            0x09 => VpScaOpcode::Bri,
            0x0A => VpScaOpcode::Cal,
            0x0B => VpScaOpcode::Cli,
            0x0C => VpScaOpcode::Ret,
            0x0D => VpScaOpcode::Lg2,
            0x0E => VpScaOpcode::Ex2,
            0x0F => VpScaOpcode::Sin,
            0x10 => VpScaOpcode::Cos,
            0x11 => VpScaOpcode::Brb,
            0x12 => VpScaOpcode::Clb,
            0x13 => VpScaOpcode::Psh,
            0x14 => VpScaOpcode::Pop,
            _ => VpScaOpcode::Nop,
        }
    }
}

/// VP register type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpRegType {
    Temp = 0,
    Input = 1,
    Constant = 2,
    Unknown = 3,
}

impl From<u8> for VpRegType {
    fn from(v: u8) -> Self {
        match v & 0x3 {
            0 => VpRegType::Temp,
            1 => VpRegType::Input,
            2 => VpRegType::Constant,
            _ => VpRegType::Unknown,
        }
    }
}

/// Vertex program D0 word
#[derive(Debug, Clone, Copy)]
pub struct VpD0 {
    pub addr_swz: u8,           // bits 0-1
    pub mask_w: u8,             // bits 2-3
    pub mask_z: u8,             // bits 4-5
    pub mask_y: u8,             // bits 6-7
    pub mask_x: u8,             // bits 8-9
    pub cond: u8,               // bits 10-12
    pub cond_test_enable: bool, // bit 13
    pub cond_update_enable_0: bool, // bit 14
    pub dst_tmp: u8,            // bits 15-20
    pub src0_abs: bool,         // bit 21
    pub src1_abs: bool,         // bit 22
    pub src2_abs: bool,         // bit 23
    pub addr_reg_sel_1: bool,   // bit 24
    pub cond_reg_sel_1: bool,   // bit 25
    pub saturate: bool,         // bit 26
    pub index_input: bool,      // bit 27
    pub cond_update_enable_1: bool, // bit 29
    pub vec_result: bool,       // bit 30
}

impl VpD0 {
    pub fn decode(val: u32) -> Self {
        Self {
            addr_swz: (val & 0x3) as u8,
            mask_w: ((val >> 2) & 0x3) as u8,
            mask_z: ((val >> 4) & 0x3) as u8,
            mask_y: ((val >> 6) & 0x3) as u8,
            mask_x: ((val >> 8) & 0x3) as u8,
            cond: ((val >> 10) & 0x7) as u8,
            cond_test_enable: (val >> 13) & 1 != 0,
            cond_update_enable_0: (val >> 14) & 1 != 0,
            dst_tmp: ((val >> 15) & 0x3F) as u8,
            src0_abs: (val >> 21) & 1 != 0,
            src1_abs: (val >> 22) & 1 != 0,
            src2_abs: (val >> 23) & 1 != 0,
            addr_reg_sel_1: (val >> 24) & 1 != 0,
            cond_reg_sel_1: (val >> 25) & 1 != 0,
            saturate: (val >> 26) & 1 != 0,
            index_input: (val >> 27) & 1 != 0,
            cond_update_enable_1: (val >> 29) & 1 != 0,
            vec_result: (val >> 30) & 1 != 0,
        }
    }
}

/// Vertex program D1 word
#[derive(Debug, Clone, Copy)]
pub struct VpD1 {
    pub src0h: u16,         // bits 0-7 (src0 high part)
    pub input_src: u8,      // bits 8-11
    pub const_src: u16,     // bits 12-21
    pub vec_opcode: VpVecOpcode, // bits 22-26
    pub sca_opcode: VpScaOpcode, // bits 27-31
}

impl VpD1 {
    pub fn decode(val: u32) -> Self {
        Self {
            src0h: (val & 0xFF) as u16,
            input_src: ((val >> 8) & 0xF) as u8,
            const_src: ((val >> 12) & 0x3FF) as u16,
            vec_opcode: VpVecOpcode::from(((val >> 22) & 0x1F) as u8),
            sca_opcode: VpScaOpcode::from(((val >> 27) & 0x1F) as u8),
        }
    }
}

/// Vertex program D2 word
#[derive(Debug, Clone, Copy)]
pub struct VpD2 {
    pub src0l: u16,     // bits 0-8 (src0 low part)
    pub src1: u16,      // bits 9-25 (17 bits)
    pub src2h: u8,      // bits 26-31 (6 bits, src2 high part)
    pub tex_num: u8,    // bits 8-9 (for TXL)
}

impl VpD2 {
    pub fn decode(val: u32) -> Self {
        Self {
            src0l: (val & 0x1FF) as u16,
            src1: ((val >> 9) & 0x1FFFF) as u16,
            src2h: ((val >> 26) & 0x3F) as u8,
            tex_num: ((val >> 8) & 0x3) as u8,
        }
    }
}

/// Vertex program D3 word
#[derive(Debug, Clone, Copy)]
pub struct VpD3 {
    pub end: bool,              // bit 0
    pub index_const: bool,      // bit 1
    pub dst: u8,                // bits 2-6
    pub sca_dst_tmp: u8,        // bits 7-12
    pub vec_writemask_w: bool,  // bit 13
    pub vec_writemask_z: bool,  // bit 14
    pub vec_writemask_y: bool,  // bit 15
    pub vec_writemask_x: bool,  // bit 16
    pub sca_writemask_w: bool,  // bit 17
    pub sca_writemask_z: bool,  // bit 18
    pub sca_writemask_y: bool,  // bit 19
    pub sca_writemask_x: bool,  // bit 20
    pub src2l: u16,             // bits 21-31 (11 bits, src2 low part)
}

impl VpD3 {
    pub fn decode(val: u32) -> Self {
        Self {
            end: val & 1 != 0,
            index_const: (val >> 1) & 1 != 0,
            dst: ((val >> 2) & 0x1F) as u8,
            sca_dst_tmp: ((val >> 7) & 0x3F) as u8,
            vec_writemask_w: (val >> 13) & 1 != 0,
            vec_writemask_z: (val >> 14) & 1 != 0,
            vec_writemask_y: (val >> 15) & 1 != 0,
            vec_writemask_x: (val >> 16) & 1 != 0,
            sca_writemask_w: (val >> 17) & 1 != 0,
            sca_writemask_z: (val >> 18) & 1 != 0,
            sca_writemask_y: (val >> 19) & 1 != 0,
            sca_writemask_x: (val >> 20) & 1 != 0,
            src2l: ((val >> 21) & 0x7FF) as u16,
        }
    }
}

/// Vertex program source operand
#[derive(Debug, Clone, Copy)]
pub struct VpSource {
    pub reg_type: VpRegType,
    pub tmp_src: u8,        // temp register index
    pub swz_x: u8,
    pub swz_y: u8,
    pub swz_z: u8,
    pub swz_w: u8,
    pub neg: bool,
}

impl VpSource {
    /// Decode source 0 from D1/D2 (17 bits split across both words)
    pub fn decode_src0(d1: &VpD1, d2: &VpD2) -> Self {
        // Combine src0h (8 bits from D1) and src0l (9 bits from D2)
        let combined = ((d1.src0h as u32) << 9) | (d2.src0l as u32);
        Self::decode_17bit(combined)
    }

    /// Decode source 1 from D2 (17 bits)
    pub fn decode_src1(d2: &VpD2) -> Self {
        Self::decode_17bit(d2.src1 as u32)
    }

    /// Decode source 2 from D2/D3 (17 bits split)
    pub fn decode_src2(d2: &VpD2, d3: &VpD3) -> Self {
        // Combine src2h (6 bits from D2) and src2l (11 bits from D3)
        let combined = ((d2.src2h as u32) << 11) | (d3.src2l as u32);
        Self::decode_17bit(combined)
    }

    fn decode_17bit(val: u32) -> Self {
        Self {
            reg_type: VpRegType::from((val & 0x3) as u8),
            tmp_src: ((val >> 2) & 0x3F) as u8,
            swz_w: ((val >> 8) & 0x3) as u8,
            swz_z: ((val >> 10) & 0x3) as u8,
            swz_y: ((val >> 12) & 0x3) as u8,
            swz_x: ((val >> 14) & 0x3) as u8,
            neg: (val >> 16) & 1 != 0,
        }
    }
}

//=============================================================================
// FRAGMENT PROGRAM INSTRUCTION FORMAT
//=============================================================================

/// Fragment program opcodes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpOpcode {
    Nop = 0x00,
    Mov = 0x01,
    Mul = 0x02,
    Add = 0x03,
    Mad = 0x04,
    Dp3 = 0x05,
    Dp4 = 0x06,
    Dst = 0x07,
    Min = 0x08,
    Max = 0x09,
    Slt = 0x0A,
    Sge = 0x0B,
    Sle = 0x0C,
    Sgt = 0x0D,
    Sne = 0x0E,
    Seq = 0x0F,
    Frc = 0x10,
    Flr = 0x11,
    Kil = 0x12,
    Pk4 = 0x13,
    Up4 = 0x14,
    Ddx = 0x15,
    Ddy = 0x16,
    Tex = 0x17,
    Txp = 0x18,
    Txd = 0x19,
    Rcp = 0x1A,
    Rsq = 0x1B,
    Ex2 = 0x1C,
    Lg2 = 0x1D,
    Lit = 0x1E,
    Lrp = 0x1F,
    Str = 0x20,
    Sfl = 0x21,
    Cos = 0x22,
    Sin = 0x23,
    Pk2 = 0x24,
    Up2 = 0x25,
    Pow = 0x26,
    Pkb = 0x27,
    Upb = 0x28,
    Pk16 = 0x29,
    Up16 = 0x2A,
    Bem = 0x2B,
    Pkg = 0x2C,
    Upg = 0x2D,
    Dp2a = 0x2E,
    Txl = 0x2F,
    Txb = 0x31,
    Refl = 0x36,
    Dp2 = 0x38,
    Nrm = 0x39,
    Div = 0x3A,
    Divsq = 0x3B,
    Lif = 0x3C,
    Fenct = 0x3D,
    Fencb = 0x3E,
    Brk = 0x40,
    Cal = 0x41,
    Ife = 0x42,
    Loop = 0x43,
    Rep = 0x44,
    Ret = 0x45,
}

impl From<u8> for FpOpcode {
    fn from(v: u8) -> Self {
        match v {
            0x00 => FpOpcode::Nop,
            0x01 => FpOpcode::Mov,
            0x02 => FpOpcode::Mul,
            0x03 => FpOpcode::Add,
            0x04 => FpOpcode::Mad,
            0x05 => FpOpcode::Dp3,
            0x06 => FpOpcode::Dp4,
            0x07 => FpOpcode::Dst,
            0x08 => FpOpcode::Min,
            0x09 => FpOpcode::Max,
            0x0A => FpOpcode::Slt,
            0x0B => FpOpcode::Sge,
            0x0C => FpOpcode::Sle,
            0x0D => FpOpcode::Sgt,
            0x0E => FpOpcode::Sne,
            0x0F => FpOpcode::Seq,
            0x10 => FpOpcode::Frc,
            0x11 => FpOpcode::Flr,
            0x12 => FpOpcode::Kil,
            0x17 => FpOpcode::Tex,
            0x18 => FpOpcode::Txp,
            0x1A => FpOpcode::Rcp,
            0x1B => FpOpcode::Rsq,
            0x1C => FpOpcode::Ex2,
            0x1D => FpOpcode::Lg2,
            0x1E => FpOpcode::Lit,
            0x1F => FpOpcode::Lrp,
            0x22 => FpOpcode::Cos,
            0x23 => FpOpcode::Sin,
            0x26 => FpOpcode::Pow,
            0x2F => FpOpcode::Txl,
            0x38 => FpOpcode::Dp2,
            0x39 => FpOpcode::Nrm,
            0x3A => FpOpcode::Div,
            0x3B => FpOpcode::Divsq,
            0x40 => FpOpcode::Brk,
            0x41 => FpOpcode::Cal,
            0x42 => FpOpcode::Ife,
            0x43 => FpOpcode::Loop,
            0x44 => FpOpcode::Rep,
            0x45 => FpOpcode::Ret,
            _ => FpOpcode::Nop,
        }
    }
}

/// FP register type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpRegType {
    Temp = 0,
    Input = 1,
    Constant = 2,
    Unknown = 3,
}

impl From<u8> for FpRegType {
    fn from(v: u8) -> Self {
        match v & 0x3 {
            0 => FpRegType::Temp,
            1 => FpRegType::Input,
            2 => FpRegType::Constant,
            _ => FpRegType::Unknown,
        }
    }
}

/// Fragment program OPDEST word (word 0)
#[derive(Debug, Clone, Copy)]
pub struct FpOpDest {
    pub no_dest: bool,      // bit 0
    pub saturate: bool,     // bit 1
    pub scale: u8,          // bits 2-3
    pub fp16: bool,         // bit 4 (output precision)
    pub test_mask: u8,      // bits 5-7 (condition mask)
    pub end: bool,          // bit 8
    pub dest_reg: u8,       // bits 9-14 (destination register)
    pub writemask: u8,      // bits 15-18 (xyzw)
    pub opcode: FpOpcode,   // bits 24-29
    pub set_cond: bool,     // bit 30
}

impl FpOpDest {
    pub fn decode(val: u32) -> Self {
        Self {
            no_dest: val & 1 != 0,
            saturate: (val >> 1) & 1 != 0,
            scale: ((val >> 2) & 0x3) as u8,
            fp16: (val >> 4) & 1 != 0,
            test_mask: ((val >> 5) & 0x7) as u8,
            end: (val >> 8) & 1 != 0,
            dest_reg: ((val >> 9) & 0x3F) as u8,
            writemask: ((val >> 15) & 0xF) as u8,
            opcode: FpOpcode::from(((val >> 24) & 0x3F) as u8),
            set_cond: (val >> 30) & 1 != 0,
        }
    }
}

/// Fragment program source operand
#[derive(Debug, Clone, Copy)]
pub struct FpSource {
    pub reg_type: FpRegType,
    pub reg_index: u8,
    pub swz_x: u8,
    pub swz_y: u8,
    pub swz_z: u8,
    pub swz_w: u8,
    pub neg: bool,
    pub abs: bool,
    pub fp16: bool,
}

impl FpSource {
    pub fn decode(val: u32, is_src0: bool) -> Self {
        if is_src0 {
            // SRC0 encoding
            Self {
                reg_type: FpRegType::from(((val >> 1) & 0x3) as u8),
                reg_index: ((val >> 3) & 0x3F) as u8,
                fp16: (val >> 9) & 1 != 0,
                swz_x: ((val >> 10) & 0x3) as u8,
                swz_y: ((val >> 12) & 0x3) as u8,
                swz_z: ((val >> 14) & 0x3) as u8,
                swz_w: ((val >> 16) & 0x3) as u8,
                neg: (val >> 18) & 1 != 0,
                abs: (val >> 19) & 1 != 0,
            }
        } else {
            // SRC1/SRC2 encoding (similar structure)
            Self {
                reg_type: FpRegType::from(((val >> 0) & 0x3) as u8),
                reg_index: ((val >> 2) & 0x3F) as u8,
                fp16: (val >> 8) & 1 != 0,
                swz_x: ((val >> 9) & 0x3) as u8,
                swz_y: ((val >> 11) & 0x3) as u8,
                swz_z: ((val >> 13) & 0x3) as u8,
                swz_w: ((val >> 15) & 0x3) as u8,
                neg: (val >> 17) & 1 != 0,
                abs: (val >> 18) & 1 != 0,
            }
        }
    }
}

//=============================================================================
// DECODED INSTRUCTION TYPES
//=============================================================================

/// Decoded vertex program instruction
#[derive(Debug, Clone)]
pub struct DecodedVpInstruction {
    pub vec_opcode: VpVecOpcode,
    pub sca_opcode: VpScaOpcode,
    pub sources: [VpSource; 3],
    pub vec_dst: u8,
    pub sca_dst: u8,
    pub vec_writemask: u8,  // xyzw bits
    pub sca_writemask: u8,  // xyzw bits
    pub saturate: bool,
    pub end: bool,
    pub d0: VpD0,
    pub d1: VpD1,
    pub d2: VpD2,
    pub d3: VpD3,
}

/// Decoded fragment program instruction
#[derive(Debug, Clone)]
pub struct DecodedFpInstruction {
    pub opcode: FpOpcode,
    pub dest: FpOpDest,
    pub sources: [FpSource; 3],
    pub tex_unit: u8,
    pub end: bool,
}

//=============================================================================
// PROGRAM DESCRIPTORS
//=============================================================================

/// Vertex program descriptor
#[derive(Debug, Clone)]
pub struct VertexProgram {
    pub instructions: Vec<u32>,
    pub input_mask: u32,
    pub output_mask: u32,
    /// Constants indexed by register number (0-511)
    pub constants: Vec<[f32; 4]>,
    /// Constant index range used by this program (start, end)
    pub constant_range: (u32, u32),
    pub decoded: Vec<DecodedVpInstruction>,
}

impl VertexProgram {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            input_mask: 0,
            output_mask: 0,
            constants: Vec::new(),
            constant_range: (0, 0),
            decoded: Vec::new(),
        }
    }

    pub fn from_data(data: &[u32]) -> Self {
        Self {
            instructions: data.to_vec(),
            input_mask: 0,
            output_mask: 0,
            constants: Vec::new(),
            constant_range: (0, 0),
            decoded: Vec::new(),
        }
    }
    
    /// Set constants from RSX state
    pub fn set_constants(&mut self, constants: &[[f32; 4]; 512]) {
        // Copy only the range we need
        let start = self.constant_range.0 as usize;
        let end = (self.constant_range.1 as usize).min(512);
        if start < end {
            self.constants = constants[start..end].to_vec();
        }
    }
}

impl Default for VertexProgram {
    fn default() -> Self {
        Self::new()
    }
}

/// Fragment program descriptor
#[derive(Debug, Clone)]
pub struct FragmentProgram {
    pub instructions: Vec<u32>,
    pub texture_mask: u32,
    pub constants: Vec<[f32; 4]>,
    pub decoded: Vec<DecodedFpInstruction>,
}

impl FragmentProgram {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            texture_mask: 0,
            constants: Vec::new(),
            decoded: Vec::new(),
        }
    }

    pub fn from_data(data: &[u32]) -> Self {
        Self {
            instructions: data.to_vec(),
            texture_mask: 0,
            constants: Vec::new(),
            decoded: Vec::new(),
        }
    }
}

impl Default for FragmentProgram {
    fn default() -> Self {
        Self::new()
    }
}

/// SPIR-V shader module
#[derive(Clone)]
pub struct SpirVModule {
    pub bytecode: Vec<u32>,
    pub stage: ShaderStage,
}

impl SpirVModule {
    pub fn new(stage: ShaderStage) -> Self {
        Self {
            bytecode: Vec::new(),
            stage,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.bytecode)
    }
}
