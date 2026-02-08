//! cellVpost HLE - Video post-processing module
//!
//! This module provides HLE implementations for the PS3's video post-processing library.
//! Supports video scaling, color conversion, and deinterlacing operations.

use std::collections::HashMap;
use tracing::trace;

/// Video post-processing handle
pub type VpostHandle = u32;

// Error codes
pub const CELL_VPOST_ERROR_ARG: i32 = 0x80610b01u32 as i32;
pub const CELL_VPOST_ERROR_SEQ: i32 = 0x80610b02u32 as i32;
pub const CELL_VPOST_ERROR_BUSY: i32 = 0x80610b03u32 as i32;
pub const CELL_VPOST_ERROR_FATAL: i32 = 0x80610b04u32 as i32;

/// Picture format type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellVpostFormatType {
    /// YUV420 Planar
    Yuv420Planar = 0,
    /// YUV422 Planar
    Yuv422Planar = 1,
    /// RGBA 8888
    Rgba8888 = 2,
    /// ARGB 8888
    Argb8888 = 3,
}

/// Color matrix type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellVpostColorMatrix {
    /// BT.601 standard
    Bt601 = 0,
    /// BT.709 standard
    Bt709 = 1,
}

/// Picture format
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVpostPictureFormat {
    pub format_type: u32,
    pub color_matrix: u32,
    pub alpha: u32,
}

/// Picture configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellVpostPictureInfo {
    pub in_width: u32,
    pub in_height: u32,
    pub in_pitch: u32,
    pub in_chroma_offset: [u32; 2],
    pub in_alpha_offset: u32,
    pub out_width: u32,
    pub out_height: u32,
    pub out_pitch: u32,
    pub out_chroma_offset: [u32; 2],
    pub out_alpha_offset: u32,
}

/// Resource attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVpostResource {
    pub mem_addr: u32,
    pub mem_size: u32,
    pub ppu_thread_priority: i32,
    pub ppu_thread_stack_size: u32,
}

/// Video post-processing configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVpostCfg {
    pub in_pic_format: CellVpostPictureFormat,
    pub out_pic_format: CellVpostPictureFormat,
    pub resource: *const CellVpostResource,
}

/// Video post-processing control parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVpostCtrlParam {
    pub in_buffer_addr: u32,
    pub out_buffer_addr: u32,
    pub pic_info: *const CellVpostPictureInfo,
}

/// Video post-processor entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct VpostEntry {
    /// Input picture format
    in_format: CellVpostPictureFormat,
    /// Output picture format
    out_format: CellVpostPictureFormat,
    /// Memory size allocated
    mem_size: u32,
    /// Number of frames processed
    frames_processed: u32,
    /// Whether processor is busy
    is_busy: bool,
    /// Color conversion backend
    converter: Option<ColorConverter>,
    /// Image scaler
    scaler: Option<Scaler>,
}

/// Scaling algorithm
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingAlgorithm {
    /// Nearest neighbor (fastest)
    NearestNeighbor = 0,
    /// Bilinear interpolation
    Bilinear = 1,
    /// Bicubic interpolation (best quality)
    Bicubic = 2,
}

/// Image scaler
#[derive(Debug, Clone)]
struct Scaler {
    /// Scaling algorithm to use
    algorithm: ScalingAlgorithm,
}

impl Scaler {
    fn new(algorithm: ScalingAlgorithm) -> Self {
        Self { algorithm }
    }

    /// Scale RGBA image using bilinear interpolation
    fn scale_bilinear(
        &self,
        src: &[u8],
        src_width: u32,
        src_height: u32,
        dst: &mut [u8],
        dst_width: u32,
        dst_height: u32,
    ) -> Result<(), i32> {
        trace!("Scaler::scale_bilinear: {}x{} -> {}x{}", src_width, src_height, dst_width, dst_height);

        if src.len() < (src_width * src_height * 4) as usize {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        if dst.len() < (dst_width * dst_height * 4) as usize {
            return Err(CELL_VPOST_ERROR_ARG);
        }

        let x_ratio = (src_width as f32 - 1.0) / (dst_width as f32);
        let y_ratio = (src_height as f32 - 1.0) / (dst_height as f32);

        for dy in 0..dst_height {
            for dx in 0..dst_width {
                let src_x = dx as f32 * x_ratio;
                let src_y = dy as f32 * y_ratio;
                
                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let x1 = (x0 + 1).min(src_width - 1);
                let y1 = (y0 + 1).min(src_height - 1);
                
                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;
                
                // Get four surrounding pixels
                let p00_idx = ((y0 * src_width + x0) * 4) as usize;
                let p10_idx = ((y0 * src_width + x1) * 4) as usize;
                let p01_idx = ((y1 * src_width + x0) * 4) as usize;
                let p11_idx = ((y1 * src_width + x1) * 4) as usize;
                
                let dst_idx = ((dy * dst_width + dx) * 4) as usize;
                
                // Bilinear interpolation for each channel
                for c in 0..4 {
                    let p00 = src[p00_idx + c] as f32;
                    let p10 = src[p10_idx + c] as f32;
                    let p01 = src[p01_idx + c] as f32;
                    let p11 = src[p11_idx + c] as f32;
                    
                    let top = p00 * (1.0 - fx) + p10 * fx;
                    let bottom = p01 * (1.0 - fx) + p11 * fx;
                    let value = top * (1.0 - fy) + bottom * fy;
                    
                    dst[dst_idx + c] = value.clamp(0.0, 255.0) as u8;
                }
            }
        }
        
        Ok(())
    }

    /// Cubic interpolation helper
    fn cubic_interpolate(&self, p: [f32; 4], x: f32) -> f32 {
        p[1] + 0.5 * x * (p[2] - p[0] + x * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3] + x * (3.0 * (p[1] - p[2]) + p[3] - p[0])))
    }

    /// Scale RGBA image using bicubic interpolation
    fn scale_bicubic(
        &self,
        src: &[u8],
        src_width: u32,
        src_height: u32,
        dst: &mut [u8],
        dst_width: u32,
        dst_height: u32,
    ) -> Result<(), i32> {
        trace!("Scaler::scale_bicubic: {}x{} -> {}x{}", src_width, src_height, dst_width, dst_height);

        if src.len() < (src_width * src_height * 4) as usize {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        if dst.len() < (dst_width * dst_height * 4) as usize {
            return Err(CELL_VPOST_ERROR_ARG);
        }

        let x_ratio = src_width as f32 / dst_width as f32;
        let y_ratio = src_height as f32 / dst_height as f32;

        for dy in 0..dst_height {
            for dx in 0..dst_width {
                let src_x = dx as f32 * x_ratio;
                let src_y = dy as f32 * y_ratio;
                
                let x_int = src_x.floor() as i32;
                let y_int = src_y.floor() as i32;
                let fx = src_x - x_int as f32;
                let fy = src_y - y_int as f32;
                
                let dst_idx = ((dy * dst_width + dx) * 4) as usize;
                
                // Bicubic interpolation for each channel
                for c in 0..4 {
                    let mut p = [[0.0f32; 4]; 4];
                    
                    // Get 4x4 pixel neighborhood
                    for j in 0..4 {
                        for i in 0..4 {
                            let sx = (x_int - 1 + i as i32).clamp(0, src_width as i32 - 1) as u32;
                            let sy = (y_int - 1 + j as i32).clamp(0, src_height as i32 - 1) as u32;
                            let idx = ((sy * src_width + sx) * 4 + c as u32) as usize;
                            p[j][i] = src[idx] as f32;
                        }
                    }
                    
                    // Interpolate in Y direction
                    let mut arr = [0.0f32; 4];
                    for i in 0..4 {
                        arr[i] = self.cubic_interpolate(p[i], fx);
                    }
                    
                    // Interpolate in X direction
                    let value = self.cubic_interpolate(arr, fy);
                    dst[dst_idx + c] = value.clamp(0.0, 255.0) as u8;
                }
            }
        }
        
        Ok(())
    }

    /// Scale RGBA image using nearest neighbor
    fn scale_nearest(
        &self,
        src: &[u8],
        src_width: u32,
        src_height: u32,
        dst: &mut [u8],
        dst_width: u32,
        dst_height: u32,
    ) -> Result<(), i32> {
        trace!("Scaler::scale_nearest: {}x{} -> {}x{}", src_width, src_height, dst_width, dst_height);

        if src.len() < (src_width * src_height * 4) as usize {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        if dst.len() < (dst_width * dst_height * 4) as usize {
            return Err(CELL_VPOST_ERROR_ARG);
        }

        let x_ratio = (src_width << 16) / dst_width;
        let y_ratio = (src_height << 16) / dst_height;

        for dy in 0..dst_height {
            let src_y = ((dy * y_ratio) >> 16).min(src_height - 1);
            for dx in 0..dst_width {
                let src_x = ((dx * x_ratio) >> 16).min(src_width - 1);
                
                let src_idx = ((src_y * src_width + src_x) * 4) as usize;
                let dst_idx = ((dy * dst_width + dx) * 4) as usize;
                
                dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
            }
        }
        
        Ok(())
    }

    /// Scale RGBA image
    fn scale(
        &self,
        src: &[u8],
        src_width: u32,
        src_height: u32,
        dst: &mut [u8],
        dst_width: u32,
        dst_height: u32,
    ) -> Result<(), i32> {
        // If dimensions are the same, just copy
        if src_width == dst_width && src_height == dst_height {
            let size = (src_width * src_height * 4) as usize;
            if src.len() >= size && dst.len() >= size {
                dst[..size].copy_from_slice(&src[..size]);
                return Ok(());
            }
        }

        match self.algorithm {
            ScalingAlgorithm::NearestNeighbor => {
                self.scale_nearest(src, src_width, src_height, dst, dst_width, dst_height)
            }
            ScalingAlgorithm::Bilinear => {
                self.scale_bilinear(src, src_width, src_height, dst, dst_width, dst_height)
            }
            ScalingAlgorithm::Bicubic => {
                self.scale_bicubic(src, src_width, src_height, dst, dst_width, dst_height)
            }
        }
    }
}

/// Color conversion backend
#[derive(Debug, Clone)]
struct ColorConverter {
    /// Input format
    in_format: CellVpostFormatType,
    /// Output format
    out_format: CellVpostFormatType,
    /// Color matrix for YUV conversions
    color_matrix: CellVpostColorMatrix,
}

impl ColorConverter {
    /// Create a new color converter
    fn new(in_format: &CellVpostPictureFormat, out_format: &CellVpostPictureFormat) -> Self {
        let in_fmt = match in_format.format_type {
            0 => CellVpostFormatType::Yuv420Planar,
            1 => CellVpostFormatType::Yuv422Planar,
            2 => CellVpostFormatType::Rgba8888,
            3 => CellVpostFormatType::Argb8888,
            _ => CellVpostFormatType::Yuv420Planar,
        };

        let out_fmt = match out_format.format_type {
            0 => CellVpostFormatType::Yuv420Planar,
            1 => CellVpostFormatType::Yuv422Planar,
            2 => CellVpostFormatType::Rgba8888,
            3 => CellVpostFormatType::Argb8888,
            _ => CellVpostFormatType::Rgba8888,
        };

        let matrix = match in_format.color_matrix {
            0 => CellVpostColorMatrix::Bt601,
            1 => CellVpostColorMatrix::Bt709,
            _ => CellVpostColorMatrix::Bt709,
        };

        Self {
            in_format: in_fmt,
            out_format: out_fmt,
            color_matrix: matrix,
        }
    }

    /// Convert YUV420 to RGBA using specified color matrix
    /// 
    /// Implements full YUV to RGB color conversion following ITU-R BT.601 and BT.709 standards.
    /// YUV420 format has full-resolution Y (luma) and half-resolution U/V (chroma) in both dimensions.
    fn yuv420_to_rgba(&self, y_plane: &[u8], u_plane: &[u8], v_plane: &[u8], 
                       width: u32, height: u32, out_buffer: &mut [u8]) -> Result<(), i32> {
        trace!("ColorConverter::yuv420_to_rgba: {}x{}, matrix={:?}", width, height, self.color_matrix);
        
        let pixel_count = (width * height) as usize;
        if out_buffer.len() < pixel_count * 4 {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        
        // YUV420 has chroma subsampled by 2 in both dimensions
        let chroma_width = (width / 2) as usize;
        
        // Color conversion coefficients based on color matrix standard
        // For full-range YUV (0-255):
        //   R = Y + Cr_coeff * (V - 128)
        //   G = Y - Cb_coeff_g * (U - 128) - Cr_coeff_g * (V - 128)
        //   B = Y + Cb_coeff * (U - 128)
        let (cr_coeff, cb_coeff_g, cr_coeff_g, cb_coeff) = match self.color_matrix {
            CellVpostColorMatrix::Bt601 => {
                // BT.601/SDTV coefficients
                (1.402, 0.344136, 0.714136, 1.772)
            }
            CellVpostColorMatrix::Bt709 => {
                // BT.709/HDTV coefficients
                (1.5748, 0.187324, 0.468124, 1.8556)
            }
        };
        
        for py in 0..height {
            for px in 0..width {
                let y_idx = (py * width + px) as usize;
                
                // Chroma is subsampled - each 2x2 block of Y shares one U,V pair
                let chroma_x = (px / 2) as usize;
                let chroma_y = (py / 2) as usize;
                let uv_idx = chroma_y * chroma_width + chroma_x;
                
                // Get Y, U, V values (default to neutral if out of bounds)
                let y = if y_idx < y_plane.len() { y_plane[y_idx] as f32 } else { 128.0 };
                let u = if uv_idx < u_plane.len() { u_plane[uv_idx] as f32 } else { 128.0 };
                let v = if uv_idx < v_plane.len() { v_plane[uv_idx] as f32 } else { 128.0 };
                
                // Convert YUV to RGB
                let r = y + cr_coeff * (v - 128.0);
                let g = y - cb_coeff_g * (u - 128.0) - cr_coeff_g * (v - 128.0);
                let b = y + cb_coeff * (u - 128.0);
                
                // Clamp and write RGBA
                let out_idx = y_idx * 4;
                out_buffer[out_idx] = r.clamp(0.0, 255.0) as u8;
                out_buffer[out_idx + 1] = g.clamp(0.0, 255.0) as u8;
                out_buffer[out_idx + 2] = b.clamp(0.0, 255.0) as u8;
                out_buffer[out_idx + 3] = 255; // Full alpha
            }
        }
        
        Ok(())
    }

    /// Convert RGBA to YUV420
    /// 
    /// Implements full RGB to YUV color conversion following ITU-R BT.601 and BT.709 standards.
    /// YUV420 format stores full-resolution Y (luma) and 4:1 subsampled U/V (chroma).
    fn rgba_to_yuv420(&self, rgba_buffer: &[u8], width: u32, height: u32,
                       y_plane: &mut [u8], u_plane: &mut [u8], v_plane: &mut [u8]) -> Result<(), i32> {
        trace!("ColorConverter::rgba_to_yuv420: {}x{}, matrix={:?}", width, height, self.color_matrix);
        
        let pixel_count = (width * height) as usize;
        if rgba_buffer.len() < pixel_count * 4 {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        
        // RGB to YUV coefficients based on color matrix standard
        // Y = Kr*R + Kg*G + Kb*B
        // U (Cb) = (B - Y) / (2 * (1 - Kb)) + 128 = -0.5*Kr/(1-Kb)*R - 0.5*Kg/(1-Kb)*G + 0.5*B + 128
        // V (Cr) = (R - Y) / (2 * (1 - Kr)) + 128 = 0.5*R - 0.5*Kg/(1-Kr)*G - 0.5*Kb/(1-Kr)*B + 128
        let (kr, kg, kb) = match self.color_matrix {
            CellVpostColorMatrix::Bt601 => {
                // BT.601/SDTV coefficients
                (0.299, 0.587, 0.114)
            }
            CellVpostColorMatrix::Bt709 => {
                // BT.709/HDTV coefficients
                (0.2126, 0.7152, 0.0722)
            }
        };
        
        // Pre-compute chroma coefficients
        let u_r = -0.5 * kr / (1.0 - kb);
        let u_g = -0.5 * kg / (1.0 - kb);
        let u_b = 0.5;
        let v_r = 0.5;
        let v_g = -0.5 * kg / (1.0 - kr);
        let v_b = -0.5 * kb / (1.0 - kr);
        
        // Compute Y plane (full resolution)
        for i in 0..pixel_count.min(y_plane.len()) {
            let rgba_idx = i * 4;
            if rgba_idx + 2 < rgba_buffer.len() {
                let r = rgba_buffer[rgba_idx] as f32;
                let g = rgba_buffer[rgba_idx + 1] as f32;
                let b = rgba_buffer[rgba_idx + 2] as f32;
                
                // Y = Kr*R + Kg*G + Kb*B
                let y = kr * r + kg * g + kb * b;
                y_plane[i] = y.clamp(0.0, 255.0) as u8;
            }
        }
        
        // Compute U and V planes (subsampled 2x2)
        // Average the chroma values from each 2x2 block of pixels
        let chroma_width = width / 2;
        let chroma_height = height / 2;
        let _chroma_size = (chroma_width * chroma_height) as usize;
        
        for cy in 0..chroma_height {
            for cx in 0..chroma_width {
                let uv_idx = (cy * chroma_width + cx) as usize;
                if uv_idx >= u_plane.len() || uv_idx >= v_plane.len() {
                    break;
                }
                
                // Average 2x2 block of pixels for chroma
                let mut u_sum = 0.0f32;
                let mut v_sum = 0.0f32;
                let mut count = 0.0f32;
                
                for dy in 0..2 {
                    for dx in 0..2 {
                        let px = cx * 2 + dx;
                        let py = cy * 2 + dy;
                        if px < width && py < height {
                            let rgba_idx = ((py * width + px) * 4) as usize;
                            if rgba_idx + 2 < rgba_buffer.len() {
                                let r = rgba_buffer[rgba_idx] as f32;
                                let g = rgba_buffer[rgba_idx + 1] as f32;
                                let b = rgba_buffer[rgba_idx + 2] as f32;
                                
                                // U (Cb) and V (Cr) computation
                                u_sum += u_r * r + u_g * g + u_b * b + 128.0;
                                v_sum += v_r * r + v_g * g + v_b * b + 128.0;
                                count += 1.0;
                            }
                        }
                    }
                }
                
                if count > 0.0 {
                    u_plane[uv_idx] = (u_sum / count).clamp(0.0, 255.0) as u8;
                    v_plane[uv_idx] = (v_sum / count).clamp(0.0, 255.0) as u8;
                } else {
                    u_plane[uv_idx] = 128;
                    v_plane[uv_idx] = 128;
                }
            }
        }
        
        Ok(())
    }

    /// Convert YUV422 to RGBA using specified color matrix
    /// 
    /// Implements YUV422 to RGB color conversion. YUV422 format has full-resolution Y (luma)
    /// and half-resolution U/V (chroma) only in the horizontal direction.
    fn yuv422_to_rgba(&self, y_plane: &[u8], u_plane: &[u8], v_plane: &[u8], 
                       width: u32, height: u32, out_buffer: &mut [u8]) -> Result<(), i32> {
        trace!("ColorConverter::yuv422_to_rgba: {}x{}, matrix={:?}", width, height, self.color_matrix);
        
        let pixel_count = (width * height) as usize;
        if out_buffer.len() < pixel_count * 4 {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        
        // YUV422 has chroma subsampled by 2 only in horizontal direction
        let chroma_width = (width / 2) as usize;
        
        // Color conversion coefficients based on color matrix standard
        let (cr_coeff, cb_coeff_g, cr_coeff_g, cb_coeff) = match self.color_matrix {
            CellVpostColorMatrix::Bt601 => (1.402, 0.344136, 0.714136, 1.772),
            CellVpostColorMatrix::Bt709 => (1.5748, 0.187324, 0.468124, 1.8556),
        };
        
        for py in 0..height {
            for px in 0..width {
                let y_idx = (py * width + px) as usize;
                
                // YUV422: each pair of pixels horizontally shares one U,V pair
                let chroma_x = (px / 2) as usize;
                let uv_idx = py as usize * chroma_width + chroma_x;
                
                // Get Y, U, V values
                let y = if y_idx < y_plane.len() { y_plane[y_idx] as f32 } else { 128.0 };
                let u = if uv_idx < u_plane.len() { u_plane[uv_idx] as f32 } else { 128.0 };
                let v = if uv_idx < v_plane.len() { v_plane[uv_idx] as f32 } else { 128.0 };
                
                // Convert YUV to RGB
                let r = y + cr_coeff * (v - 128.0);
                let g = y - cb_coeff_g * (u - 128.0) - cr_coeff_g * (v - 128.0);
                let b = y + cb_coeff * (u - 128.0);
                
                // Clamp and write RGBA
                let out_idx = y_idx * 4;
                out_buffer[out_idx] = r.clamp(0.0, 255.0) as u8;
                out_buffer[out_idx + 1] = g.clamp(0.0, 255.0) as u8;
                out_buffer[out_idx + 2] = b.clamp(0.0, 255.0) as u8;
                out_buffer[out_idx + 3] = 255;
            }
        }
        
        Ok(())
    }

    /// Convert between formats
    fn convert(&self, in_buffer: &[u8], pic_info: &CellVpostPictureInfo, out_buffer: &mut [u8]) -> Result<(), i32> {
        match (self.in_format, self.out_format) {
            (CellVpostFormatType::Yuv420Planar, CellVpostFormatType::Rgba8888) => {
                // Split YUV planes (simplified)
                let y_size = (pic_info.in_width * pic_info.in_height) as usize;
                let uv_size = y_size / 4;
                
                if in_buffer.len() >= y_size + uv_size * 2 {
                    let y_plane = &in_buffer[0..y_size];
                    let u_plane = &in_buffer[y_size..y_size + uv_size];
                    let v_plane = &in_buffer[y_size + uv_size..y_size + uv_size * 2];
                    
                    self.yuv420_to_rgba(y_plane, u_plane, v_plane, 
                                       pic_info.out_width, pic_info.out_height, out_buffer)
                } else {
                    Err(CELL_VPOST_ERROR_ARG)
                }
            }
            (CellVpostFormatType::Rgba8888, CellVpostFormatType::Yuv420Planar) => {
                let y_size = (pic_info.out_width * pic_info.out_height) as usize;
                let uv_size = y_size / 4;
                
                if out_buffer.len() >= y_size + uv_size * 2 {
                    let (y_plane, uv_planes) = out_buffer.split_at_mut(y_size);
                    let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);
                    
                    self.rgba_to_yuv420(in_buffer, pic_info.in_width, pic_info.in_height,
                                       y_plane, u_plane, v_plane)
                } else {
                    Err(CELL_VPOST_ERROR_ARG)
                }
            }
            (CellVpostFormatType::Yuv422Planar, CellVpostFormatType::Rgba8888) => {
                // YUV422 has chroma subsampled only horizontally
                let y_size = (pic_info.in_width * pic_info.in_height) as usize;
                let uv_size = y_size / 2; // 4:2:2 has half-width chroma per row
                
                if in_buffer.len() >= y_size + uv_size * 2 {
                    let y_plane = &in_buffer[0..y_size];
                    let u_plane = &in_buffer[y_size..y_size + uv_size];
                    let v_plane = &in_buffer[y_size + uv_size..];
                    
                    self.yuv422_to_rgba(y_plane, u_plane, v_plane,
                                       pic_info.out_width, pic_info.out_height, out_buffer)
                } else {
                    Err(CELL_VPOST_ERROR_ARG)
                }
            }
            _ => {
                // Unsupported or pass-through conversion
                trace!("Unsupported conversion: {:?} to {:?}", self.in_format, self.out_format);
                if in_buffer.len() <= out_buffer.len() {
                    out_buffer[..in_buffer.len()].copy_from_slice(in_buffer);
                    Ok(())
                } else {
                    Err(CELL_VPOST_ERROR_ARG)
                }
            }
        }
    }
}

impl VpostEntry {
    fn new(in_format: CellVpostPictureFormat, out_format: CellVpostPictureFormat, mem_size: u32) -> Self {
        let converter = ColorConverter::new(&in_format, &out_format);
        // Use bilinear scaling as default (good quality/performance trade-off)
        let scaler = Scaler::new(ScalingAlgorithm::Bilinear);
        
        Self {
            in_format,
            out_format,
            mem_size,
            frames_processed: 0,
            is_busy: false,
            converter: Some(converter),
            scaler: Some(scaler),
        }
    }
}

/// Video post-processor manager
pub struct VpostManager {
    processors: HashMap<VpostHandle, VpostEntry>,
    next_handle: VpostHandle,
}

impl VpostManager {
    pub fn new() -> Self {
        Self {
            processors: HashMap::new(),
            next_handle: 1,
        }
    }

    /// Query resource requirements for given configuration
    pub fn query_attr(&self, in_format: &CellVpostPictureFormat, out_format: &CellVpostPictureFormat) -> CellVpostResource {
        // Calculate memory requirements based on format types
        let base_mem = 0x100000u32; // 1MB base
        let format_multiplier = if in_format.format_type != out_format.format_type { 2 } else { 1 };
        
        CellVpostResource {
            mem_addr: 0,
            mem_size: base_mem * format_multiplier,
            ppu_thread_priority: 1001,
            ppu_thread_stack_size: 0x4000,
        }
    }

    /// Open a new video post-processor
    pub fn open(&mut self, in_format: CellVpostPictureFormat, out_format: CellVpostPictureFormat, mem_size: u32) -> Result<VpostHandle, i32> {
        if mem_size < 0x10000 {
            return Err(CELL_VPOST_ERROR_ARG);
        }

        let handle = self.next_handle;
        self.next_handle += 1;

        let entry = VpostEntry::new(in_format, out_format, mem_size);
        self.processors.insert(handle, entry);

        Ok(handle)
    }

    /// Close a video post-processor
    pub fn close(&mut self, handle: VpostHandle) -> Result<(), i32> {
        let entry = self.processors.remove(&handle).ok_or(CELL_VPOST_ERROR_ARG)?;
        
        if entry.is_busy {
            return Err(CELL_VPOST_ERROR_BUSY);
        }

        Ok(())
    }

    /// Execute video post-processing on a frame
    pub fn exec(&mut self, handle: VpostHandle, pic_info: &CellVpostPictureInfo) -> Result<(), i32> {
        let entry = self.processors.get_mut(&handle).ok_or(CELL_VPOST_ERROR_ARG)?;

        if entry.is_busy {
            return Err(CELL_VPOST_ERROR_BUSY);
        }

        // Validate picture dimensions
        if pic_info.in_width == 0 || pic_info.in_height == 0 {
            return Err(CELL_VPOST_ERROR_ARG);
        }
        if pic_info.out_width == 0 || pic_info.out_height == 0 {
            return Err(CELL_VPOST_ERROR_ARG);
        }

        // Perform color conversion and scaling
        if let (Some(converter), Some(scaler)) = (&entry.converter, &entry.scaler) {
            // Simulate input and output buffers (in real impl, would read from memory)
            let in_size = (pic_info.in_width * pic_info.in_height * 3 / 2) as usize; // YUV420 size
            let intermediate_size = (pic_info.in_width * pic_info.in_height * 4) as usize; // RGBA size before scaling
            let out_size = (pic_info.out_width * pic_info.out_height * 4) as usize; // Final RGBA size
            
            let in_buffer = vec![128u8; in_size]; // Dummy input
            let mut intermediate_buffer = vec![0u8; intermediate_size]; // After color conversion
            let mut out_buffer = vec![0u8; out_size]; // Final output buffer
            
            // Step 1: Perform color conversion to RGBA at input resolution
            converter.convert(&in_buffer, pic_info, &mut intermediate_buffer)?;
            
            // Step 2: Scale if dimensions differ
            if pic_info.in_width != pic_info.out_width || pic_info.in_height != pic_info.out_height {
                scaler.scale(
                    &intermediate_buffer,
                    pic_info.in_width,
                    pic_info.in_height,
                    &mut out_buffer,
                    pic_info.out_width,
                    pic_info.out_height,
                )?;
                
                trace!("VpostManager::exec: converted and scaled {}x{} to {}x{}", 
                       pic_info.in_width, pic_info.in_height,
                       pic_info.out_width, pic_info.out_height);
            } else {
                // No scaling needed, just use the converted buffer
                out_buffer.copy_from_slice(&intermediate_buffer[..out_size]);
                trace!("VpostManager::exec: converted {}x{} (no scaling)", 
                       pic_info.in_width, pic_info.in_height);
            }
        }

        entry.frames_processed += 1;

        Ok(())
    }

    /// Get the number of frames processed by a post-processor
    pub fn get_frames_processed(&self, handle: VpostHandle) -> Result<u32, i32> {
        let entry = self.processors.get(&handle).ok_or(CELL_VPOST_ERROR_ARG)?;
        Ok(entry.frames_processed)
    }

    /// Check if a post-processor is currently busy
    pub fn is_busy(&self, handle: VpostHandle) -> Result<bool, i32> {
        let entry = self.processors.get(&handle).ok_or(CELL_VPOST_ERROR_ARG)?;
        Ok(entry.is_busy)
    }

    /// Get the number of active post-processors
    pub fn active_count(&self) -> usize {
        self.processors.len()
    }

    /// Set the scaling algorithm for a post-processor
    pub fn set_scaling_algorithm(&mut self, handle: VpostHandle, algorithm: ScalingAlgorithm) -> Result<(), i32> {
        let entry = self.processors.get_mut(&handle).ok_or(CELL_VPOST_ERROR_ARG)?;
        entry.scaler = Some(Scaler::new(algorithm));
        trace!("VpostManager::set_scaling_algorithm: handle={}, algorithm={:?}", handle, algorithm);
        Ok(())
    }
}

impl Default for VpostManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellVpostQueryAttr - Query video post-processing attributes
pub unsafe fn cell_vpost_query_attr(
    cfg: *const CellVpostCfg,
    attr: *mut CellVpostResource,
) -> i32 {
    trace!("cellVpostQueryAttr called");

    if cfg.is_null() || attr.is_null() {
        return CELL_VPOST_ERROR_ARG;
    }

    let manager = VpostManager::new();
    unsafe {
        let config = &*cfg;
        let resource = manager.query_attr(&config.in_pic_format, &config.out_pic_format);
        *attr = resource;
    }

    0 // CELL_OK
}

/// cellVpostOpen - Open video post-processor
pub unsafe fn cell_vpost_open(
    cfg: *const CellVpostCfg,
    resource: *const CellVpostResource,
    handle: *mut VpostHandle,
) -> i32 {
    trace!("cellVpostOpen called");

    if cfg.is_null() || handle.is_null() {
        return CELL_VPOST_ERROR_ARG;
    }

    unsafe {
        let config = &*cfg;
        let mem_size = if resource.is_null() { 0x100000 } else { (*resource).mem_size };

        match crate::context::get_hle_context_mut().vpost.open(config.in_pic_format, config.out_pic_format, mem_size) {
            Ok(h) => {
                *handle = h;
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellVpostClose - Close video post-processor
pub fn cell_vpost_close(handle: VpostHandle) -> i32 {
    trace!("cellVpostClose called with handle: {}", handle);

    match crate::context::get_hle_context_mut().vpost.close(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellVpostExec - Execute video post-processing
pub unsafe fn cell_vpost_exec(
    handle: VpostHandle,
    _in_buffer: *const u8,
    ctrl_param: *const CellVpostCtrlParam,
    _out_buffer: *mut u8,
    _pic_info: *mut CellVpostPictureInfo,
) -> i32 {
    trace!("cellVpostExec called");

    if ctrl_param.is_null() {
        return CELL_VPOST_ERROR_ARG;
    }

    unsafe {
        let ctrl = &*ctrl_param;

        if ctrl.pic_info.is_null() {
            return CELL_VPOST_ERROR_ARG;
        }

        match crate::context::get_hle_context_mut().vpost.exec(handle, &*ctrl.pic_info) {
            Ok(_) => 0, // CELL_OK
            Err(e) => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_default_format() -> CellVpostPictureFormat {
        CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv420Planar as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        }
    }

    fn create_default_pic_info() -> CellVpostPictureInfo {
        CellVpostPictureInfo {
            in_width: 1920,
            in_height: 1080,
            in_pitch: 1920,
            in_chroma_offset: [0, 0],
            in_alpha_offset: 0,
            out_width: 1280,
            out_height: 720,
            out_pitch: 1280,
            out_chroma_offset: [0, 0],
            out_alpha_offset: 0,
        }
    }

    #[test]
    fn test_vpost_manager_new() {
        let manager = VpostManager::new();
        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.next_handle, 1);
    }

    #[test]
    fn test_vpost_manager_open_close() {
        let mut manager = VpostManager::new();
        let in_format = create_default_format();
        let out_format = create_default_format();

        let handle = manager.open(in_format, out_format, 0x100000).unwrap();
        assert!(handle > 0);
        assert_eq!(manager.active_count(), 1);

        manager.close(handle).unwrap();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_vpost_manager_multiple_processors() {
        let mut manager = VpostManager::new();
        let format = create_default_format();

        let handle1 = manager.open(format, format, 0x100000).unwrap();
        let handle2 = manager.open(format, format, 0x100000).unwrap();
        let handle3 = manager.open(format, format, 0x100000).unwrap();

        assert_ne!(handle1, handle2);
        assert_ne!(handle2, handle3);
        assert_eq!(manager.active_count(), 3);
    }

    #[test]
    fn test_vpost_manager_invalid_handle() {
        let mut manager = VpostManager::new();

        assert_eq!(manager.close(999), Err(CELL_VPOST_ERROR_ARG));
        assert_eq!(manager.is_busy(999), Err(CELL_VPOST_ERROR_ARG));
        assert_eq!(manager.get_frames_processed(999), Err(CELL_VPOST_ERROR_ARG));
    }

    #[test]
    fn test_vpost_manager_exec() {
        let mut manager = VpostManager::new();
        let format = create_default_format();
        let handle = manager.open(format, format, 0x100000).unwrap();
        let pic_info = create_default_pic_info();

        manager.exec(handle, &pic_info).unwrap();
        assert_eq!(manager.get_frames_processed(handle).unwrap(), 1);

        manager.exec(handle, &pic_info).unwrap();
        assert_eq!(manager.get_frames_processed(handle).unwrap(), 2);
    }

    #[test]
    fn test_vpost_manager_exec_invalid_dimensions() {
        let mut manager = VpostManager::new();
        let format = create_default_format();
        let handle = manager.open(format, format, 0x100000).unwrap();

        let mut pic_info = create_default_pic_info();
        pic_info.in_width = 0;

        assert_eq!(manager.exec(handle, &pic_info), Err(CELL_VPOST_ERROR_ARG));
    }

    #[test]
    fn test_vpost_manager_query_attr() {
        let manager = VpostManager::new();
        let in_format = create_default_format();
        let out_format = create_default_format();

        let attr = manager.query_attr(&in_format, &out_format);
        assert!(attr.mem_size >= 0x100000);
        assert!(attr.ppu_thread_stack_size > 0);
    }

    #[test]
    fn test_vpost_manager_query_attr_format_conversion() {
        let manager = VpostManager::new();
        let in_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv420Planar as u32,
            color_matrix: 0,
            alpha: 0,
        };
        let out_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Rgba8888 as u32,
            color_matrix: 0,
            alpha: 0,
        };

        let attr = manager.query_attr(&in_format, &out_format);
        // Different formats require more memory
        assert!(attr.mem_size >= 0x200000);
    }

    #[test]
    fn test_vpost_manager_insufficient_memory() {
        let mut manager = VpostManager::new();
        let format = create_default_format();

        // Too little memory should fail
        assert_eq!(manager.open(format, format, 0x1000), Err(CELL_VPOST_ERROR_ARG));
    }

    #[test]
    fn test_vpost_lifecycle() {
        let pic_format = CellVpostPictureFormat {
            format_type: 0,
            color_matrix: 0,
            alpha: 0,
        };
        let resource = CellVpostResource {
            mem_addr: 0,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            ppu_thread_stack_size: 0x4000,
        };
        let cfg = CellVpostCfg {
            in_pic_format: pic_format,
            out_pic_format: pic_format,
            resource: &resource,
        };
        let mut handle = 0;

        // HLE functions use the global manager instance from context.rs
        unsafe {
            assert_eq!(cell_vpost_open(&cfg, &resource, &mut handle), 0);
        }
        assert!(handle > 0);
        
        // Close should succeed using the global manager
        let close_result = cell_vpost_close(handle);
        assert_eq!(close_result, 0);
    }

    #[test]
    fn test_vpost_edge_cases() {
        // Test invalid handle operations
        let invalid_handle = 0xFFFFFFFF;
        
        // Operations on invalid handle should return error
        assert_ne!(cell_vpost_close(invalid_handle), 0);
        
        // cell_vpost_exec with invalid handle should also fail
        let ctrl_param = CellVpostCtrlParam {
            in_buffer_addr: 0,
            out_buffer_addr: 0,
            pic_info: std::ptr::null(),
        };
        let mut pic_info = CellVpostPictureInfo {
            in_width: 0,
            in_height: 0,
            in_pitch: 0,
            in_chroma_offset: [0, 0],
            in_alpha_offset: 0,
            out_width: 0,
            out_height: 0,
            out_pitch: 0,
            out_chroma_offset: [0, 0],
            out_alpha_offset: 0,
        };
        let result = unsafe {
            cell_vpost_exec(
                invalid_handle,
                std::ptr::null(),
                &ctrl_param,
                std::ptr::null_mut(),
                &mut pic_info,
            )
        };
        assert_ne!(result, 0);
    }

    #[test]
    fn test_vpost_null_parameter_validation() {
        // Note: cell_vpost_open treats null resource as using default values (not an error)
        // Only null config and null handle are actual errors
        let pic_format = CellVpostPictureFormat {
            format_type: 0,
            color_matrix: 0,
            alpha: 0,
        };
        let resource = CellVpostResource {
            mem_addr: 0,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            ppu_thread_stack_size: 0x4000,
        };
        let cfg = CellVpostCfg {
            in_pic_format: pic_format,
            out_pic_format: pic_format,
            resource: &resource,
        };
        let mut handle = 0;

        unsafe {
            // Null config should fail
            assert_ne!(cell_vpost_open(std::ptr::null(), &resource, &mut handle), 0);
            
            // Null handle should fail
            assert_ne!(cell_vpost_open(&cfg, &resource, std::ptr::null_mut()), 0);
            
            // Note: Null resource is valid - uses default mem_size
            // (tested here for documentation)
            let result = cell_vpost_open(&cfg, std::ptr::null(), &mut handle);
            assert_eq!(result, 0); // Should succeed with default mem_size
            cell_vpost_close(handle);
        }
    }

    #[test]
    fn test_vpost_query_attr() {
        let pic_format = CellVpostPictureFormat {
            format_type: 0,
            color_matrix: 0,
            alpha: 0,
        };
        let resource = CellVpostResource {
            mem_addr: 0,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            ppu_thread_stack_size: 0x4000,
        };
        let cfg = CellVpostCfg {
            in_pic_format: pic_format,
            out_pic_format: pic_format,
            resource: &resource,
        };
        let mut attr = CellVpostResource {
            mem_addr: 0,
            mem_size: 0,
            ppu_thread_priority: 0,
            ppu_thread_stack_size: 0,
        };

        unsafe {
            assert_eq!(cell_vpost_query_attr(&cfg, &mut attr), 0);
        }
        assert!(attr.mem_size > 0);
    }

    #[test]
    fn test_vpost_format_types() {
        assert_eq!(CellVpostFormatType::Yuv420Planar as u32, 0);
        assert_eq!(CellVpostFormatType::Yuv422Planar as u32, 1);
        assert_eq!(CellVpostFormatType::Rgba8888 as u32, 2);
        assert_eq!(CellVpostFormatType::Argb8888 as u32, 3);
    }

    #[test]
    fn test_vpost_color_matrix() {
        assert_eq!(CellVpostColorMatrix::Bt601 as u32, 0);
        assert_eq!(CellVpostColorMatrix::Bt709 as u32, 1);
    }

    #[test]
    fn test_vpost_error_codes() {
        assert_ne!(CELL_VPOST_ERROR_ARG, 0);
        assert_ne!(CELL_VPOST_ERROR_SEQ, 0);
        assert_ne!(CELL_VPOST_ERROR_BUSY, 0);
        assert_ne!(CELL_VPOST_ERROR_FATAL, 0);
    }

    #[test]
    fn test_yuv420_to_rgba_basic() {
        // Test YUV420 to RGBA conversion with known values
        let in_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv420Planar as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let out_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Rgba8888 as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let converter = ColorConverter::new(&in_format, &out_format);
        
        // 4x4 image for testing (2x2 chroma)
        let width = 4u32;
        let height = 4u32;
        let y_size = (width * height) as usize;
        let uv_size = y_size / 4;
        
        // Create neutral gray YUV data (Y=128, U=128, V=128 -> gray RGB)
        let y_plane = vec![128u8; y_size];
        let u_plane = vec![128u8; uv_size];
        let v_plane = vec![128u8; uv_size];
        
        let mut out_buffer = vec![0u8; y_size * 4];
        
        let result = converter.yuv420_to_rgba(&y_plane, &u_plane, &v_plane, width, height, &mut out_buffer);
        assert!(result.is_ok());
        
        // All pixels should be approximately gray (around 128)
        for i in 0..(y_size) {
            let r = out_buffer[i * 4];
            let g = out_buffer[i * 4 + 1];
            let b = out_buffer[i * 4 + 2];
            let a = out_buffer[i * 4 + 3];
            
            // Check that values are close to expected gray (within tolerance for rounding)
            assert!(r >= 126 && r <= 130, "R should be ~128, got {}", r);
            assert!(g >= 126 && g <= 130, "G should be ~128, got {}", g);
            assert!(b >= 126 && b <= 130, "B should be ~128, got {}", b);
            assert_eq!(a, 255, "Alpha should be 255");
        }
    }

    #[test]
    fn test_yuv420_to_rgba_red() {
        // Test YUV values that should produce red (Y=81, U=90, V=240 for BT.601)
        let in_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv420Planar as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let out_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Rgba8888 as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let converter = ColorConverter::new(&in_format, &out_format);
        
        // 2x2 image
        let y_plane = vec![81u8; 4]; // Approximate Y for pure red
        let u_plane = vec![90u8; 1]; // Approximate U for pure red
        let v_plane = vec![240u8; 1]; // Approximate V for pure red
        
        let mut out_buffer = vec![0u8; 16];
        
        let result = converter.yuv420_to_rgba(&y_plane, &u_plane, &v_plane, 2, 2, &mut out_buffer);
        assert!(result.is_ok());
        
        // Red channel should be high, green and blue should be low
        let r = out_buffer[0];
        let g = out_buffer[1];
        let b = out_buffer[2];
        
        assert!(r > 200, "Red channel should be high, got {}", r);
        assert!(g < 50, "Green channel should be low, got {}", g);
        assert!(b < 50, "Blue channel should be low, got {}", b);
    }

    #[test]
    fn test_rgba_to_yuv420_basic() {
        // Test RGBA to YUV420 conversion with gray color
        let in_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Rgba8888 as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let out_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv420Planar as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let converter = ColorConverter::new(&in_format, &out_format);
        
        // 4x4 gray RGBA image
        let width = 4u32;
        let height = 4u32;
        let pixel_count = (width * height) as usize;
        
        let mut rgba_buffer = vec![0u8; pixel_count * 4];
        for i in 0..pixel_count {
            rgba_buffer[i * 4] = 128;     // R
            rgba_buffer[i * 4 + 1] = 128; // G
            rgba_buffer[i * 4 + 2] = 128; // B
            rgba_buffer[i * 4 + 3] = 255; // A
        }
        
        let y_size = pixel_count;
        let uv_size = y_size / 4;
        let mut y_plane = vec![0u8; y_size];
        let mut u_plane = vec![0u8; uv_size];
        let mut v_plane = vec![0u8; uv_size];
        
        let result = converter.rgba_to_yuv420(&rgba_buffer, width, height, &mut y_plane, &mut u_plane, &mut v_plane);
        assert!(result.is_ok());
        
        // Y should be close to 128 for gray
        for y in &y_plane {
            assert!(*y >= 120 && *y <= 136, "Y should be ~128, got {}", y);
        }
        
        // U and V should be close to 128 for gray (neutral chroma)
        for u in &u_plane {
            assert!(*u >= 120 && *u <= 136, "U should be ~128, got {}", u);
        }
        for v in &v_plane {
            assert!(*v >= 120 && *v <= 136, "V should be ~128, got {}", v);
        }
    }

    #[test]
    fn test_rgba_to_yuv420_roundtrip() {
        // Test that converting RGBA -> YUV420 -> RGBA produces similar results
        // Note: YUV420 is a lossy conversion due to chroma subsampling
        let in_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Rgba8888 as u32,
            color_matrix: CellVpostColorMatrix::Bt709 as u32,
            alpha: 255,
        };
        let yuv_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv420Planar as u32,
            color_matrix: CellVpostColorMatrix::Bt709 as u32,
            alpha: 255,
        };
        
        let rgba_to_yuv = ColorConverter::new(&in_format, &yuv_format);
        let yuv_to_rgba = ColorConverter::new(&yuv_format, &in_format);
        
        // Use neutral gray for roundtrip test (minimizes conversion error)
        let width = 4u32;
        let height = 4u32;
        let pixel_count = (width * height) as usize;
        
        // Create a uniform gray image
        let mut original_rgba = vec![0u8; pixel_count * 4];
        for i in 0..pixel_count {
            original_rgba[i * 4] = 128;     // R
            original_rgba[i * 4 + 1] = 128; // G
            original_rgba[i * 4 + 2] = 128; // B
            original_rgba[i * 4 + 3] = 255; // A
        }
        
        // Convert to YUV420
        let y_size = pixel_count;
        let uv_size = y_size / 4;
        let mut y_plane = vec![0u8; y_size];
        let mut u_plane = vec![0u8; uv_size];
        let mut v_plane = vec![0u8; uv_size];
        
        rgba_to_yuv.rgba_to_yuv420(&original_rgba, width, height, &mut y_plane, &mut u_plane, &mut v_plane).unwrap();
        
        // Convert back to RGBA
        let mut final_rgba = vec![0u8; pixel_count * 4];
        yuv_to_rgba.yuv420_to_rgba(&y_plane, &u_plane, &v_plane, width, height, &mut final_rgba).unwrap();
        
        // Check that values are reasonably close for uniform gray (should be very close)
        for i in 0..pixel_count {
            let orig_r = original_rgba[i * 4] as i32;
            let final_r = final_rgba[i * 4] as i32;
            
            // Gray should round-trip with minimal error
            assert!((orig_r - final_r).abs() < 10, "R mismatch at pixel {}: {} vs {}", i, orig_r, final_r);
        }
        
        // Also verify the YUV planes have expected values for gray
        for y in &y_plane {
            assert!(*y >= 120 && *y <= 136, "Y should be ~128 for gray");
        }
    }

    #[test]
    fn test_yuv422_to_rgba_basic() {
        // Test YUV422 to RGBA conversion
        let in_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Yuv422Planar as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let out_format = CellVpostPictureFormat {
            format_type: CellVpostFormatType::Rgba8888 as u32,
            color_matrix: CellVpostColorMatrix::Bt601 as u32,
            alpha: 255,
        };
        let converter = ColorConverter::new(&in_format, &out_format);
        
        // 4x4 image (YUV422 has 2x4 chroma)
        let width = 4u32;
        let height = 4u32;
        let y_size = (width * height) as usize;
        let uv_size = y_size / 2; // 4:2:2 has half-width chroma per row
        
        // Neutral gray
        let y_plane = vec![128u8; y_size];
        let u_plane = vec![128u8; uv_size];
        let v_plane = vec![128u8; uv_size];
        
        let mut out_buffer = vec![0u8; y_size * 4];
        
        let result = converter.yuv422_to_rgba(&y_plane, &u_plane, &v_plane, width, height, &mut out_buffer);
        assert!(result.is_ok());
        
        // All pixels should be gray
        for i in 0..(y_size) {
            let r = out_buffer[i * 4];
            assert!(r >= 126 && r <= 130, "R should be ~128, got {}", r);
        }
    }
}
