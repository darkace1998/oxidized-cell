//! SPU arithmetic instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Multiply - mpy rt, ra, rb
pub fn mpy(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        ((a[0] as i32).wrapping_mul(b[0] as i32)) as u32,
        ((a[1] as i32).wrapping_mul(b[1] as i32)) as u32,
        ((a[2] as i32).wrapping_mul(b[2] as i32)) as u32,
        ((a[3] as i32).wrapping_mul(b[3] as i32)) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply Unsigned - mpyu rt, ra, rb
pub fn mpyu(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (a[0] & 0xFFFF).wrapping_mul(b[0] & 0xFFFF),
        (a[1] & 0xFFFF).wrapping_mul(b[1] & 0xFFFF),
        (a[2] & 0xFFFF).wrapping_mul(b[2] & 0xFFFF),
        (a[3] & 0xFFFF).wrapping_mul(b[3] & 0xFFFF),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply High - mpyh rt, ra, rb
pub fn mpyh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        ((a[0] >> 16) * (b[0] & 0xFFFF)) << 16,
        ((a[1] >> 16) * (b[1] & 0xFFFF)) << 16,
        ((a[2] >> 16) * (b[2] & 0xFFFF)) << 16,
        ((a[3] >> 16) * (b[3] & 0xFFFF)) << 16,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Left Word - shl rt, ra, rb
pub fn shl(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        a[0] << (b[0] & 0x3F),
        a[1] << (b[1] & 0x3F),
        a[2] << (b[2] & 0x3F),
        a[3] << (b[3] & 0x3F),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Left Word Immediate - shli rt, ra, i7
pub fn shli(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x3F) as u32;
    let result = [
        a[0] << shift,
        a[1] << shift,
        a[2] << shift,
        a[3] << shift,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate Word - rot rt, ra, rb
pub fn rot(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        a[0].rotate_left(b[0] & 0x1F),
        a[1].rotate_left(b[1] & 0x1F),
        a[2].rotate_left(b[2] & 0x1F),
        a[3].rotate_left(b[3] & 0x1F),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate Word Immediate - roti rt, ra, i7
pub fn roti(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x1F) as u32;
    let result = [
        a[0].rotate_left(shift),
        a[1].rotate_left(shift),
        a[2].rotate_left(shift),
        a[3].rotate_left(shift),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Add Halfword - ah rt, ra, rb
pub fn ah(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = ((a[i] >> 16) as u16).wrapping_add((b[i] >> 16) as u16);
        let lo = ((a[i] & 0xFFFF) as u16).wrapping_add((b[i] & 0xFFFF) as u16);
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Subtract Halfword - sfh rt, ra, rb
pub fn sfh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = ((b[i] >> 16) as u16).wrapping_sub((a[i] >> 16) as u16);
        let lo = ((b[i] & 0xFFFF) as u16).wrapping_sub((a[i] & 0xFFFF) as u16);
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Left Halfword - shlh rt, ra, rb
pub fn shlh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift_hi = (b[i] >> 16) & 0x1F;
        let shift_lo = b[i] & 0x1F;
        let hi = if shift_hi < 16 { (a[i] >> 16) << shift_hi } else { 0 };
        let lo = if shift_lo < 16 { (a[i] & 0xFFFF) << shift_lo } else { 0 };
        result[i] = (hi << 16) | (lo & 0xFFFF);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Left Halfword Immediate - shlhi rt, ra, i7
pub fn shlhi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x1F) as u32;
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = if shift < 16 { (a[i] >> 16) << shift } else { 0 };
        let lo = if shift < 16 { (a[i] & 0xFFFF) << shift } else { 0 };
        result[i] = (hi << 16) | (lo & 0xFFFF);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate Halfword - roth rt, ra, rb
pub fn roth(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let rot_hi = (b[i] >> 16) & 0xF;
        let rot_lo = b[i] & 0xF;
        let a_hi = ((a[i] >> 16) & 0xFFFF) as u16;
        let a_lo = (a[i] & 0xFFFF) as u16;
        let hi = a_hi.rotate_left(rot_hi) as u32;
        let lo = a_lo.rotate_left(rot_lo) as u32;
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate Halfword Immediate - rothi rt, ra, i7
pub fn rothi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let rot = (i7 & 0xF) as u32;
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = ((a[i] >> 16) & 0xFFFF) as u16;
        let a_lo = (a[i] & 0xFFFF) as u16;
        let hi = a_hi.rotate_left(rot) as u32;
        let lo = a_lo.rotate_left(rot) as u32;
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Right Word - shr rt, ra, rb (shift right logical)
pub fn shr(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if (b[0] & 0x3F) < 32 { a[0] >> (b[0] & 0x3F) } else { 0 },
        if (b[1] & 0x3F) < 32 { a[1] >> (b[1] & 0x3F) } else { 0 },
        if (b[2] & 0x3F) < 32 { a[2] >> (b[2] & 0x3F) } else { 0 },
        if (b[3] & 0x3F) < 32 { a[3] >> (b[3] & 0x3F) } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Right Word Immediate - shri rt, ra, i7
pub fn shri(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x3F) as u32;
    let result = [
        if shift < 32 { a[0] >> shift } else { 0 },
        if shift < 32 { a[1] >> shift } else { 0 },
        if shift < 32 { a[2] >> shift } else { 0 },
        if shift < 32 { a[3] >> shift } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Word - rotm rt, ra, rb (shift right with negative count)
pub fn rotm(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        {
            let shift = (0u32.wrapping_sub(b[0])) & 0x3F;
            if shift < 32 { a[0] >> shift } else { 0 }
        },
        {
            let shift = (0u32.wrapping_sub(b[1])) & 0x3F;
            if shift < 32 { a[1] >> shift } else { 0 }
        },
        {
            let shift = (0u32.wrapping_sub(b[2])) & 0x3F;
            if shift < 32 { a[2] >> shift } else { 0 }
        },
        {
            let shift = (0u32.wrapping_sub(b[3])) & 0x3F;
            if shift < 32 { a[3] >> shift } else { 0 }
        },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Word Immediate - rotmi rt, ra, i7
pub fn rotmi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (0i8.wrapping_sub(i7) & 0x3F) as u32;
    let result = [
        if shift < 32 { a[0] >> shift } else { 0 },
        if shift < 32 { a[1] >> shift } else { 0 },
        if shift < 32 { a[2] >> shift } else { 0 },
        if shift < 32 { a[3] >> shift } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Algebraic Word - rotma rt, ra, rb (arithmetic right shift)
pub fn rotma(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        {
            let shift = (0u32.wrapping_sub(b[0])) & 0x3F;
            if shift < 32 {
                ((a[0] as i32) >> shift) as u32
            } else {
                ((a[0] as i32) >> 31) as u32
            }
        },
        {
            let shift = (0u32.wrapping_sub(b[1])) & 0x3F;
            if shift < 32 {
                ((a[1] as i32) >> shift) as u32
            } else {
                ((a[1] as i32) >> 31) as u32
            }
        },
        {
            let shift = (0u32.wrapping_sub(b[2])) & 0x3F;
            if shift < 32 {
                ((a[2] as i32) >> shift) as u32
            } else {
                ((a[2] as i32) >> 31) as u32
            }
        },
        {
            let shift = (0u32.wrapping_sub(b[3])) & 0x3F;
            if shift < 32 {
                ((a[3] as i32) >> shift) as u32
            } else {
                ((a[3] as i32) >> 31) as u32
            }
        },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Algebraic Word Immediate - rotmai rt, ra, i7
pub fn rotmai(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (0i8.wrapping_sub(i7) & 0x3F) as u32;
    let result = [
        if shift < 32 {
            ((a[0] as i32) >> shift) as u32
        } else {
            ((a[0] as i32) >> 31) as u32
        },
        if shift < 32 {
            ((a[1] as i32) >> shift) as u32
        } else {
            ((a[1] as i32) >> 31) as u32
        },
        if shift < 32 {
            ((a[2] as i32) >> shift) as u32
        } else {
            ((a[2] as i32) >> 31) as u32
        },
        if shift < 32 {
            ((a[3] as i32) >> shift) as u32
        } else {
            ((a[3] as i32) >> 31) as u32
        },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Algebraic Halfword - rotmah rt, ra, rb
pub fn rotmah(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift_hi = (0u32.wrapping_sub(b[i] >> 16)) & 0x1F;
        let shift_lo = (0u32.wrapping_sub(b[i] & 0xFFFF)) & 0x1F;
        let a_hi = (a[i] >> 16) as i16;
        let a_lo = (a[i] & 0xFFFF) as i16;
        let hi = if shift_hi < 16 {
            ((a_hi >> shift_hi) as u16) as u32
        } else {
            ((a_hi >> 15) as u16) as u32
        };
        let lo = if shift_lo < 16 {
            ((a_lo >> shift_lo) as u16) as u32
        } else {
            ((a_lo >> 15) as u16) as u32
        };
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Algebraic Halfword Immediate - rotmahi rt, ra, i7
pub fn rotmahi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (0i8.wrapping_sub(i7) & 0x1F) as u32;
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as i16;
        let a_lo = (a[i] & 0xFFFF) as i16;
        let hi = if shift < 16 {
            ((a_hi >> shift) as u16) as u32
        } else {
            ((a_hi >> 15) as u16) as u32
        };
        let lo = if shift < 16 {
            ((a_lo >> shift) as u16) as u32
        } else {
            ((a_lo >> 15) as u16) as u32
        };
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Carry Generate - cg rt, ra, rb
pub fn cg(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if (a[0] as u64) + (b[0] as u64) > 0xFFFFFFFF { 1 } else { 0 },
        if (a[1] as u64) + (b[1] as u64) > 0xFFFFFFFF { 1 } else { 0 },
        if (a[2] as u64) + (b[2] as u64) > 0xFFFFFFFF { 1 } else { 0 },
        if (a[3] as u64) + (b[3] as u64) > 0xFFFFFFFF { 1 } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Borrow Generate - bg rt, ra, rb
pub fn bg(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if b[0] >= a[0] { 1 } else { 0 },
        if b[1] >= a[1] { 1 } else { 0 },
        if b[2] >= a[2] { 1 } else { 0 },
        if b[3] >= a[3] { 1 } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Add Extended - addx rt, ra, rb, rc (add with carry-in)
pub fn addx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let result = [
        a[0].wrapping_add(b[0]).wrapping_add(t[0] & 1),
        a[1].wrapping_add(b[1]).wrapping_add(t[1] & 1),
        a[2].wrapping_add(b[2]).wrapping_add(t[2] & 1),
        a[3].wrapping_add(b[3]).wrapping_add(t[3] & 1),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Subtract from Extended - sfx rt, ra, rb (subtract with borrow-in)
pub fn sfx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let result = [
        b[0].wrapping_sub(a[0]).wrapping_sub(1).wrapping_add(t[0] & 1),
        b[1].wrapping_sub(a[1]).wrapping_sub(1).wrapping_add(t[1] & 1),
        b[2].wrapping_sub(a[2]).wrapping_sub(1).wrapping_add(t[2] & 1),
        b[3].wrapping_sub(a[3]).wrapping_sub(1).wrapping_add(t[3] & 1),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Carry Generate Extended - cgx rt, ra, rb
pub fn cgx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let result = [
        if (a[0] as u64) + (b[0] as u64) + ((t[0] & 1) as u64) > 0xFFFFFFFF { 1 } else { 0 },
        if (a[1] as u64) + (b[1] as u64) + ((t[1] & 1) as u64) > 0xFFFFFFFF { 1 } else { 0 },
        if (a[2] as u64) + (b[2] as u64) + ((t[2] & 1) as u64) > 0xFFFFFFFF { 1 } else { 0 },
        if (a[3] as u64) + (b[3] as u64) + ((t[3] & 1) as u64) > 0xFFFFFFFF { 1 } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Borrow Generate Extended - bgx rt, ra, rb
pub fn bgx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let result = [
        if (b[0] as i64) - (a[0] as i64) + ((t[0] & 1) as i64) - 1 >= 0 { 1 } else { 0 },
        if (b[1] as i64) - (a[1] as i64) + ((t[1] & 1) as i64) - 1 >= 0 { 1 } else { 0 },
        if (b[2] as i64) - (a[2] as i64) + ((t[2] & 1) as i64) - 1 >= 0 { 1 } else { 0 },
        if (b[3] as i64) - (a[3] as i64) + ((t[3] & 1) as i64) - 1 >= 0 { 1 } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply Immediate - mpyi rt, ra, i10
pub fn mpyi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm = i10 as i32;
    let result = [
        (((a[0] & 0xFFFF) as i16 as i32).wrapping_mul(imm)) as u32,
        (((a[1] & 0xFFFF) as i16 as i32).wrapping_mul(imm)) as u32,
        (((a[2] & 0xFFFF) as i16 as i32).wrapping_mul(imm)) as u32,
        (((a[3] & 0xFFFF) as i16 as i32).wrapping_mul(imm)) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply Unsigned Immediate - mpyui rt, ra, i10
pub fn mpyui(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm = (i10 as u16) as u32;
    let result = [
        (a[0] & 0xFFFF).wrapping_mul(imm),
        (a[1] & 0xFFFF).wrapping_mul(imm),
        (a[2] & 0xFFFF).wrapping_mul(imm),
        (a[3] & 0xFFFF).wrapping_mul(imm),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply High High - mpyhh rt, ra, rb (signed)
pub fn mpyhh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (((a[0] >> 16) as i16 as i32) * ((b[0] >> 16) as i16 as i32)) as u32,
        (((a[1] >> 16) as i16 as i32) * ((b[1] >> 16) as i16 as i32)) as u32,
        (((a[2] >> 16) as i16 as i32) * ((b[2] >> 16) as i16 as i32)) as u32,
        (((a[3] >> 16) as i16 as i32) * ((b[3] >> 16) as i16 as i32)) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply High High Unsigned - mpyhhu rt, ra, rb
pub fn mpyhhu(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (a[0] >> 16) * (b[0] >> 16),
        (a[1] >> 16) * (b[1] >> 16),
        (a[2] >> 16) * (b[2] >> 16),
        (a[3] >> 16) * (b[3] >> 16),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply and Add - mpya rt, ra, rb, rc
pub fn mpya(thread: &mut SpuThread, rc: u8, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let c = thread.regs.read_u32x4(rc as usize);
    let result = [
        ((((a[0] & 0xFFFF) as i16 as i32) * ((b[0] & 0xFFFF) as i16 as i32)) + (c[0] as i32)) as u32,
        ((((a[1] & 0xFFFF) as i16 as i32) * ((b[1] & 0xFFFF) as i16 as i32)) + (c[1] as i32)) as u32,
        ((((a[2] & 0xFFFF) as i16 as i32) * ((b[2] & 0xFFFF) as i16 as i32)) + (c[2] as i32)) as u32,
        ((((a[3] & 0xFFFF) as i16 as i32) * ((b[3] & 0xFFFF) as i16 as i32)) + (c[3] as i32)) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply High High and Add - mpyhha rt, ra, rb, rc (signed)
pub fn mpyhha(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let result = [
        ((((a[0] >> 16) as i16 as i32) * ((b[0] >> 16) as i16 as i32)) as u32).wrapping_add(t[0]),
        ((((a[1] >> 16) as i16 as i32) * ((b[1] >> 16) as i16 as i32)) as u32).wrapping_add(t[1]),
        ((((a[2] >> 16) as i16 as i32) * ((b[2] >> 16) as i16 as i32)) as u32).wrapping_add(t[2]),
        ((((a[3] >> 16) as i16 as i32) * ((b[3] >> 16) as i16 as i32)) as u32).wrapping_add(t[3]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply High High and Add Unsigned - mpyhhau rt, ra, rb
pub fn mpyhhau(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let result = [
        ((a[0] >> 16) * (b[0] >> 16)).wrapping_add(t[0]),
        ((a[1] >> 16) * (b[1] >> 16)).wrapping_add(t[1]),
        ((a[2] >> 16) * (b[2] >> 16)).wrapping_add(t[2]),
        ((a[3] >> 16) * (b[3] >> 16)).wrapping_add(t[3]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply and Shift Right - mpys rt, ra, rb
pub fn mpys(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (((((a[0] & 0xFFFF) as i16 as i32) * ((b[0] & 0xFFFF) as i16 as i32)) >> 16) as i16 as i32) as u32,
        (((((a[1] & 0xFFFF) as i16 as i32) * ((b[1] & 0xFFFF) as i16 as i32)) >> 16) as i16 as i32) as u32,
        (((((a[2] & 0xFFFF) as i16 as i32) * ((b[2] & 0xFFFF) as i16 as i32)) >> 16) as i16 as i32) as u32,
        (((((a[3] & 0xFFFF) as i16 as i32) * ((b[3] & 0xFFFF) as i16 as i32)) >> 16) as i16 as i32) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Count Leading Zeros - clz rt, ra
pub fn clz(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let result = [
        a[0].leading_zeros(),
        a[1].leading_zeros(),
        a[2].leading_zeros(),
        a[3].leading_zeros(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Count Ones in Bytes - cntb rt, ra
pub fn cntb(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes = a[i].to_be_bytes();
        let cnt0 = bytes[0].count_ones() as u8;
        let cnt1 = bytes[1].count_ones() as u8;
        let cnt2 = bytes[2].count_ones() as u8;
        let cnt3 = bytes[3].count_ones() as u8;
        result[i] = u32::from_be_bytes([cnt0, cnt1, cnt2, cnt3]);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Form Select Mask for Bytes - fsmb rt, ra
pub fn fsmb(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let mut result_bytes = [0u8; 16];
    for i in 0..16 {
        result_bytes[i] = if (a >> (15 - i)) & 1 != 0 { 0xFF } else { 0x00 };
    }
    let result = [
        u32::from_be_bytes([result_bytes[0], result_bytes[1], result_bytes[2], result_bytes[3]]),
        u32::from_be_bytes([result_bytes[4], result_bytes[5], result_bytes[6], result_bytes[7]]),
        u32::from_be_bytes([result_bytes[8], result_bytes[9], result_bytes[10], result_bytes[11]]),
        u32::from_be_bytes([result_bytes[12], result_bytes[13], result_bytes[14], result_bytes[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Form Select Mask for Halfwords - fsmh rt, ra
pub fn fsmh(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = if (a >> (7 - i * 2)) & 1 != 0 { 0xFFFF0000 } else { 0 };
        let lo = if (a >> (6 - i * 2)) & 1 != 0 { 0x0000FFFF } else { 0 };
        result[i] = hi | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Form Select Mask for Words - fsm rt, ra
pub fn fsm(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let result = [
        if (a >> 3) & 1 != 0 { 0xFFFFFFFF } else { 0 },
        if (a >> 2) & 1 != 0 { 0xFFFFFFFF } else { 0 },
        if (a >> 1) & 1 != 0 { 0xFFFFFFFF } else { 0 },
        if a & 1 != 0 { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Gather Bits from Bytes - gbb rt, ra
pub fn gbb(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let mut result_val = 0u32;
    for i in 0..4 {
        let bytes = a[i].to_be_bytes();
        for j in 0..4 {
            if bytes[j] & 1 != 0 {
                result_val |= 1 << (15 - (i * 4 + j));
            }
        }
    }
    thread.regs.write_u32x4(rt as usize, [result_val, 0, 0, 0]);
    thread.advance_pc();
    Ok(())
}

/// Gather Bits from Halfwords - gbh rt, ra
pub fn gbh(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let mut result_val = 0u32;
    for i in 0..4 {
        if (a[i] >> 16) & 1 != 0 {
            result_val |= 1 << (7 - i * 2);
        }
        if a[i] & 1 != 0 {
            result_val |= 1 << (6 - i * 2);
        }
    }
    thread.regs.write_u32x4(rt as usize, [result_val, 0, 0, 0]);
    thread.advance_pc();
    Ok(())
}

/// Gather Bits from Words - gb rt, ra
pub fn gb(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let result_val = ((a[0] & 1) << 3) | ((a[1] & 1) << 2) | ((a[2] & 1) << 1) | (a[3] & 1);
    thread.regs.write_u32x4(rt as usize, [result_val, 0, 0, 0]);
    thread.advance_pc();
    Ok(())
}

/// Average Bytes - avgb rt, ra, rb
pub fn avgb(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let r0 = ((a_bytes[0] as u16 + b_bytes[0] as u16 + 1) / 2) as u8;
        let r1 = ((a_bytes[1] as u16 + b_bytes[1] as u16 + 1) / 2) as u8;
        let r2 = ((a_bytes[2] as u16 + b_bytes[2] as u16 + 1) / 2) as u8;
        let r3 = ((a_bytes[3] as u16 + b_bytes[3] as u16 + 1) / 2) as u8;
        result[i] = u32::from_be_bytes([r0, r1, r2, r3]);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Absolute Differences of Bytes - absdb rt, ra, rb
pub fn absdb(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let r0 = (a_bytes[0] as i16 - b_bytes[0] as i16).unsigned_abs() as u8;
        let r1 = (a_bytes[1] as i16 - b_bytes[1] as i16).unsigned_abs() as u8;
        let r2 = (a_bytes[2] as i16 - b_bytes[2] as i16).unsigned_abs() as u8;
        let r3 = (a_bytes[3] as i16 - b_bytes[3] as i16).unsigned_abs() as u8;
        result[i] = u32::from_be_bytes([r0, r1, r2, r3]);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Sum Bytes into Halfwords - sumb rt, ra, rb
pub fn sumb(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let sum_a = (a_bytes[0] as u16) + (a_bytes[1] as u16) + (a_bytes[2] as u16) + (a_bytes[3] as u16);
        let sum_b = (b_bytes[0] as u16) + (b_bytes[1] as u16) + (b_bytes[2] as u16) + (b_bytes[3] as u16);
        result[i] = ((sum_b as u32) << 16) | (sum_a as u32);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Extend Sign Byte to Halfword - xsbh rt, ra
pub fn xsbh(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes = a[i].to_be_bytes();
        let hi = (bytes[1] as i8 as i16 as u16) as u32;
        let lo = (bytes[3] as i8 as i16 as u16) as u32;
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Extend Sign Halfword to Word - xshw rt, ra
pub fn xshw(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let result = [
        ((a[0] & 0xFFFF) as i16 as i32) as u32,
        ((a[1] & 0xFFFF) as i16 as i32) as u32,
        ((a[2] & 0xFFFF) as i16 as i32) as u32,
        ((a[3] & 0xFFFF) as i16 as i32) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Extend Sign Word to Doubleword - xswd rt, ra
pub fn xswd(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Sign-extend word 0 to doubleword in slots 0-1, word 2 to slots 2-3
    let ext0 = if a[0] & 0x80000000 != 0 { 0xFFFFFFFF } else { 0 };
    let ext2 = if a[2] & 0x80000000 != 0 { 0xFFFFFFFF } else { 0 };
    let result = [ext0, a[0], ext2, a[2]];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Add Byte - ab rt, ra, rb
pub fn ab(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let r0 = a_bytes[0].wrapping_add(b_bytes[0]);
        let r1 = a_bytes[1].wrapping_add(b_bytes[1]);
        let r2 = a_bytes[2].wrapping_add(b_bytes[2]);
        let r3 = a_bytes[3].wrapping_add(b_bytes[3]);
        result[i] = u32::from_be_bytes([r0, r1, r2, r3]);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Subtract Byte - sfb rt, ra, rb (b - a)
pub fn sfb(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let r0 = b_bytes[0].wrapping_sub(a_bytes[0]);
        let r1 = b_bytes[1].wrapping_sub(a_bytes[1]);
        let r2 = b_bytes[2].wrapping_sub(a_bytes[2]);
        let r3 = b_bytes[3].wrapping_sub(a_bytes[3]);
        result[i] = u32::from_be_bytes([r0, r1, r2, r3]);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Halfword - rotmh rt, ra, rb
pub fn rotmh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift_hi = (0u32.wrapping_sub(b[i] >> 16)) & 0x1F;
        let shift_lo = (0u32.wrapping_sub(b[i] & 0xFFFF)) & 0x1F;
        let hi = if shift_hi < 16 { (a[i] >> 16) >> shift_hi } else { 0 };
        let lo = if shift_lo < 16 { (a[i] & 0xFFFF) >> shift_lo } else { 0 };
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Halfword Immediate - rotmhi rt, ra, i7
pub fn rotmhi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (0i8.wrapping_sub(i7) & 0x1F) as u32;
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = if shift < 16 { (a[i] >> 16) >> shift } else { 0 };
        let lo = if shift < 16 { (a[i] & 0xFFFF) >> shift } else { 0 };
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Right Halfword - shrh rt, ra, rb
pub fn shrh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift_hi = (b[i] >> 16) & 0x1F;
        let shift_lo = b[i] & 0x1F;
        let hi = if shift_hi < 16 { (a[i] >> 16) >> shift_hi } else { 0 };
        let lo = if shift_lo < 16 { (a[i] & 0xFFFF) >> shift_lo } else { 0 };
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Right Halfword Immediate - shrhi rt, ra, i7
pub fn shrhi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x1F) as u32;
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = if shift < 16 { (a[i] >> 16) >> shift } else { 0 };
        let lo = if shift < 16 { (a[i] & 0xFFFF) >> shift } else { 0 };
        result[i] = (hi << 16) | lo;
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_memory::MemoryManager;

    fn create_test_thread() -> SpuThread {
        let memory = MemoryManager::new().unwrap();
        SpuThread::new(0, memory)
    }

    #[test]
    fn test_mpy() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [2, 3, 4, 5]);
        thread.regs.write_u32x4(2, [3, 4, 5, 6]);
        
        mpy(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [6, 12, 20, 30]);
    }

    #[test]
    fn test_shl() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [1, 2, 4, 8]);
        thread.regs.write_u32x4(2, [1, 2, 3, 4]);
        
        shl(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [2, 8, 32, 128]);
    }

    #[test]
    fn test_rot() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x12345678, 0xABCDEF00, 0x11223344, 0x55667788]);
        thread.regs.write_u32x4(2, [8, 16, 4, 12]);
        
        rot(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result[0], 0x12345678u32.rotate_left(8));
        assert_eq!(result[1], 0xABCDEF00u32.rotate_left(16));
    }
}
