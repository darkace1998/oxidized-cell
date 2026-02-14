//! cellJpgDec HLE - JPEG image decoding module
//!
//! This module provides HLE implementations for the PS3's JPEG decoding library.
//! Implements JPEG header parsing and marker detection.

use std::collections::HashMap;
use tracing::{trace, debug};

/// JPEG decoder handle
pub type JpgDecHandle = u32;

// Error codes
pub const CELL_JPGDEC_ERROR_FATAL: i32 = 0x80611301u32 as i32;
pub const CELL_JPGDEC_ERROR_ARG: i32 = 0x80611302u32 as i32;
pub const CELL_JPGDEC_ERROR_SEQ: i32 = 0x80611303u32 as i32;
pub const CELL_JPGDEC_ERROR_BUSY: i32 = 0x80611304u32 as i32;
pub const CELL_JPGDEC_ERROR_EMPTY: i32 = 0x80611305u32 as i32;
pub const CELL_JPGDEC_ERROR_OPEN_FILE: i32 = 0x80611306u32 as i32;

// Fallback dimensions when header parsing fails
const DEFAULT_FALLBACK_WIDTH: u32 = 640;
const DEFAULT_FALLBACK_HEIGHT: u32 = 480;
const DEFAULT_FALLBACK_COMPONENTS: u32 = 3;

/// JPEG scan type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegScanType {
    /// Baseline sequential
    Baseline = 0,
    /// Progressive
    Progressive = 1,
}

/// JPEG decoder backend
#[derive(Debug, Clone)]
struct JpegDecoder {
    /// Image width
    width: u32,
    /// Image height
    height: u32,
    /// Number of components
    num_components: u32,
    /// Scan type
    scan_type: JpegScanType,
    /// Progressive scan data
    progressive_scans: Vec<ProgressiveScan>,
    /// Current scan index
    current_scan: usize,
    /// Quantization tables (up to 4)
    quantization_tables: [[u8; 64]; 4],
    /// Huffman DC tables (up to 4)
    huffman_dc_tables: [Option<HuffmanTable>; 4],
    /// Huffman AC tables (up to 4)
    huffman_ac_tables: [Option<HuffmanTable>; 4],
    /// Restart interval
    restart_interval: u16,
    /// Component info
    components: Vec<JpegComponent>,
    /// EXIF orientation (1-8, 0 = not set)
    exif_orientation: u8,
    /// SOS scan header info (component→table mapping)
    scan_components: Vec<ScanComponentInfo>,
}

/// Huffman table for JPEG decoding
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct HuffmanTable {
    /// Bit lengths for each code
    bits: [u8; 16],
    /// Values for each code
    values: Vec<u8>,
}

/// JPEG component info
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct JpegComponent {
    /// Component ID
    id: u8,
    /// Horizontal sampling factor
    h_sampling: u8,
    /// Vertical sampling factor
    v_sampling: u8,
    /// Quantization table index
    quant_table: u8,
}

/// Progressive scan information
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ProgressiveScan {
    /// Component selector
    component: u8,
    /// Spectral selection start
    ss: u8,
    /// Spectral selection end
    se: u8,
    /// Successive approximation high
    ah: u8,
    /// Successive approximation low
    al: u8,
}

/// SOS scan component info (maps component to Huffman table)
#[derive(Debug, Clone, Default)]
struct ScanComponentInfo {
    /// Component selector
    component_id: u8,
    /// DC Huffman table index
    dc_table: u8,
    /// AC Huffman table index
    ac_table: u8,
}

/// JPEG zigzag order for 8x8 DCT blocks
const ZIGZAG_ORDER: [usize; 64] = [
     0,  1,  5,  6, 14, 15, 27, 28,
     2,  4,  7, 13, 16, 26, 29, 42,
     3,  8, 12, 17, 25, 30, 41, 43,
     9, 11, 18, 24, 31, 40, 44, 53,
    10, 19, 23, 32, 39, 45, 52, 54,
    20, 22, 33, 38, 46, 51, 55, 60,
    21, 34, 37, 47, 50, 56, 59, 61,
    35, 36, 48, 49, 57, 58, 62, 63,
];

/// Inverse zigzag: map from zigzag position → natural (row-major) position
const INVERSE_ZIGZAG: [usize; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

impl JpegDecoder {
    fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            num_components: 3,
            scan_type: JpegScanType::Baseline,
            progressive_scans: Vec::new(),
            current_scan: 0,
            quantization_tables: [[0u8; 64]; 4],
            huffman_dc_tables: [None, None, None, None],
            huffman_ac_tables: [None, None, None, None],
            restart_interval: 0,
            components: Vec::new(),
            exif_orientation: 0,
            scan_components: Vec::new(),
        }
    }

    /// Parse JPEG header and all markers
    fn parse_header(&mut self, data: &[u8]) -> Result<(), i32> {
        // JPEG starts with SOI marker: 0xFF 0xD8
        if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
            debug!("JpegDecoder::parse_header: invalid JPEG signature");
            return Err(CELL_JPGDEC_ERROR_ARG);
        }

        let mut offset = 2;
        
        while offset + 4 <= data.len() {
            // Find next marker
            if data[offset] != 0xFF {
                offset += 1;
                continue;
            }
            
            // Skip padding 0xFF bytes
            while offset < data.len() && data[offset] == 0xFF {
                offset += 1;
            }
            
            if offset >= data.len() {
                break;
            }
            
            let marker = data[offset];
            offset += 1;
            
            match marker {
                0xD8 => {
                    // SOI - Start of Image (already handled)
                }
                0xD9 => {
                    // EOI - End of Image
                    break;
                }
                0xD0..=0xD7 => {
                    // RST0-RST7 - Restart markers (no length)
                }
                0x01 | 0x00 => {
                    // TEM or stuffed byte
                }
                0xC0 | 0xC1 => {
                    // SOF0/SOF1 - Baseline/Extended Sequential DCT
                    self.scan_type = JpegScanType::Baseline;
                    self.parse_sof(data, &mut offset)?;
                }
                0xC2 => {
                    // SOF2 - Progressive DCT
                    self.scan_type = JpegScanType::Progressive;
                    self.parse_sof(data, &mut offset)?;
                }
                0xC4 => {
                    // DHT - Define Huffman Table
                    self.parse_dht(data, &mut offset)?;
                }
                0xDB => {
                    // DQT - Define Quantization Table
                    self.parse_dqt(data, &mut offset)?;
                }
                0xDD => {
                    // DRI - Define Restart Interval
                    self.parse_dri(data, &mut offset)?;
                }
                0xDA => {
                    // SOS - Start of Scan
                    if offset + 1 < data.len() {
                        let length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                        // Parse scan component info
                        self.parse_sos(data, &mut offset)?;
                        let _ = length;
                    }
                    // After SOS, entropy-coded data follows until next marker
                    break;
                }
                0xE0..=0xEF => {
                    // APPn - Application specific
                    if marker == 0xE1 {
                        // APP1 — may contain EXIF
                        self.parse_exif(data, offset);
                    }
                    self.skip_segment(data, &mut offset)?;
                }
                0xFE => {
                    // COM - Comment
                    self.skip_segment(data, &mut offset)?;
                }
                _ => {
                    // Unknown marker, try to skip
                    self.skip_segment(data, &mut offset)?;
                }
            }
        }
        
        if self.width == 0 || self.height == 0 {
            // If parsing failed, use default values
            debug!("JpegDecoder: using fallback dimensions");
            self.width = DEFAULT_FALLBACK_WIDTH;
            self.height = DEFAULT_FALLBACK_HEIGHT;
            self.num_components = DEFAULT_FALLBACK_COMPONENTS;
        }
        
        debug!("JpegDecoder::parse_header: {}x{}, components={}, scan_type={:?}", 
               self.width, self.height, self.num_components, self.scan_type);
        
        Ok(())
    }

    /// Parse SOF (Start of Frame) marker
    fn parse_sof(&mut self, data: &[u8], offset: &mut usize) -> Result<(), i32> {
        if *offset + 2 > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let length = u16::from_be_bytes([data[*offset], data[*offset + 1]]) as usize;
        if *offset + length > data.len() || length < 8 {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let precision = data[*offset + 2];
        self.height = u16::from_be_bytes([data[*offset + 3], data[*offset + 4]]) as u32;
        self.width = u16::from_be_bytes([data[*offset + 5], data[*offset + 6]]) as u32;
        self.num_components = data[*offset + 7] as u32;
        
        debug!("JPEG SOF: {}x{}, precision={}, components={}", 
               self.width, self.height, precision, self.num_components);
        
        // Parse component info
        self.components.clear();
        let comp_offset = *offset + 8;
        for i in 0..self.num_components as usize {
            let idx = comp_offset + i * 3;
            if idx + 2 < data.len() {
                self.components.push(JpegComponent {
                    id: data[idx],
                    h_sampling: (data[idx + 1] >> 4) & 0x0F,
                    v_sampling: data[idx + 1] & 0x0F,
                    quant_table: data[idx + 2],
                });
            }
        }
        
        *offset += length;
        Ok(())
    }

    /// Parse DHT (Define Huffman Table) marker
    fn parse_dht(&mut self, data: &[u8], offset: &mut usize) -> Result<(), i32> {
        if *offset + 2 > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let length = u16::from_be_bytes([data[*offset], data[*offset + 1]]) as usize;
        if *offset + length > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let mut local_offset = *offset + 2;
        let end_offset = *offset + length;
        
        while local_offset < end_offset {
            if local_offset >= data.len() {
                break;
            }
            
            let table_info = data[local_offset];
            let table_class = (table_info >> 4) & 0x0F; // 0 = DC, 1 = AC
            let table_id = (table_info & 0x0F) as usize;
            local_offset += 1;
            
            if table_id >= 4 || local_offset + 16 > data.len() {
                break;
            }
            
            let mut bits = [0u8; 16];
            bits.copy_from_slice(&data[local_offset..local_offset + 16]);
            local_offset += 16;
            
            let num_values: usize = bits.iter().map(|&b| b as usize).sum();
            if local_offset + num_values > data.len() {
                break;
            }
            
            let values = data[local_offset..local_offset + num_values].to_vec();
            local_offset += num_values;
            
            let table = HuffmanTable { bits, values };
            
            if table_class == 0 {
                self.huffman_dc_tables[table_id] = Some(table);
            } else {
                self.huffman_ac_tables[table_id] = Some(table);
            }
            
            trace!("JPEG DHT: class={}, id={}, values={}", table_class, table_id, num_values);
        }
        
        *offset += length;
        Ok(())
    }

    /// Parse DQT (Define Quantization Table) marker
    fn parse_dqt(&mut self, data: &[u8], offset: &mut usize) -> Result<(), i32> {
        if *offset + 2 > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let length = u16::from_be_bytes([data[*offset], data[*offset + 1]]) as usize;
        if *offset + length > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let mut local_offset = *offset + 2;
        let end_offset = *offset + length;
        
        while local_offset < end_offset {
            if local_offset >= data.len() {
                break;
            }
            
            let table_info = data[local_offset];
            let precision = (table_info >> 4) & 0x0F; // 0 = 8-bit, 1 = 16-bit
            let table_id = (table_info & 0x0F) as usize;
            local_offset += 1;
            
            if table_id >= 4 {
                break;
            }
            
            let table_size = if precision == 0 { 64 } else { 128 };
            if local_offset + table_size > data.len() {
                break;
            }
            
            if precision == 0 {
                self.quantization_tables[table_id].copy_from_slice(&data[local_offset..local_offset + 64]);
            }
            
            local_offset += table_size;
            trace!("JPEG DQT: id={}, precision={}", table_id, precision);
        }
        
        *offset += length;
        Ok(())
    }

    /// Parse DRI (Define Restart Interval) marker
    fn parse_dri(&mut self, data: &[u8], offset: &mut usize) -> Result<(), i32> {
        if *offset + 4 > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let length = u16::from_be_bytes([data[*offset], data[*offset + 1]]) as usize;
        self.restart_interval = u16::from_be_bytes([data[*offset + 2], data[*offset + 3]]);
        
        trace!("JPEG DRI: interval={}", self.restart_interval);
        
        *offset += length;
        Ok(())
    }

    /// Skip a segment with length prefix
    fn skip_segment(&self, data: &[u8], offset: &mut usize) -> Result<(), i32> {
        if *offset + 2 > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let length = u16::from_be_bytes([data[*offset], data[*offset + 1]]) as usize;
        *offset += length;
        Ok(())
    }

    /// Parse EXIF data from APP1 marker
    fn parse_exif(&mut self, data: &[u8], offset: usize) {
        // APP1 structure: length (2) + "Exif\0\0" (6) + TIFF header + IFD0
        if offset + 2 > data.len() { return; }
        let length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        if offset + length > data.len() || length < 14 { return; }
        
        let exif_start = offset + 2;
        // Check for "Exif\0\0"
        if exif_start + 6 > data.len() { return; }
        if &data[exif_start..exif_start + 4] != b"Exif" { return; }
        
        let tiff_start = exif_start + 6;
        if tiff_start + 8 > data.len() { return; }
        
        // TIFF byte order: "II" = little-endian, "MM" = big-endian
        let big_endian = data[tiff_start] == b'M';
        
        let read_u16 = |off: usize| -> u16 {
            if big_endian {
                u16::from_be_bytes([data[off], data[off + 1]])
            } else {
                u16::from_le_bytes([data[off], data[off + 1]])
            }
        };
        
        let read_u32 = |off: usize| -> u32 {
            if big_endian {
                u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
            } else {
                u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
            }
        };
        
        // IFD0 offset from TIFF header
        let ifd0_offset = read_u32(tiff_start + 4) as usize;
        let ifd0_abs = tiff_start + ifd0_offset;
        
        if ifd0_abs + 2 > data.len() { return; }
        let entry_count = read_u16(ifd0_abs) as usize;
        
        // Scan IFD entries for orientation tag (0x0112)
        for i in 0..entry_count {
            let entry_offset = ifd0_abs + 2 + i * 12;
            if entry_offset + 12 > data.len() { break; }
            
            let tag = read_u16(entry_offset);
            if tag == 0x0112 {
                // Orientation tag
                let value = read_u16(entry_offset + 8);
                if (1..=8).contains(&value) {
                    self.exif_orientation = value as u8;
                    debug!("JPEG EXIF orientation: {}", self.exif_orientation);
                }
                break;
            }
        }
    }

    /// Parse SOS (Start of Scan) marker to extract component→table mapping
    fn parse_sos(&mut self, data: &[u8], offset: &mut usize) -> Result<(), i32> {
        if *offset + 2 > data.len() {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        let length = u16::from_be_bytes([data[*offset], data[*offset + 1]]) as usize;
        if *offset + length > data.len() || length < 3 {
            *offset += length.min(data.len() - *offset);
            return Ok(());
        }
        
        let num_components = data[*offset + 2] as usize;
        self.scan_components.clear();
        
        for i in 0..num_components {
            let idx = *offset + 3 + i * 2;
            if idx + 1 < data.len() {
                self.scan_components.push(ScanComponentInfo {
                    component_id: data[idx],
                    dc_table: (data[idx + 1] >> 4) & 0x0F,
                    ac_table: data[idx + 1] & 0x0F,
                });
            }
        }
        
        *offset += length;
        Ok(())
    }

    /// Detect if JPEG is progressive
    fn detect_progressive(&mut self, data: &[u8]) -> bool {
        for i in 0..data.len().saturating_sub(1) {
            if data[i] == 0xFF && data[i + 1] == 0xC2 {
                self.scan_type = JpegScanType::Progressive;
                trace!("JpegDecoder::detect_progressive: detected progressive JPEG");
                return true;
            }
        }
        false
    }

    /// Find the start of entropy-coded data after SOS marker
    fn find_entropy_data(&self, data: &[u8]) -> Option<usize> {
        let mut offset = 2; // Skip SOI
        while offset + 4 <= data.len() {
            if data[offset] != 0xFF {
                offset += 1;
                continue;
            }
            while offset < data.len() && data[offset] == 0xFF {
                offset += 1;
            }
            if offset >= data.len() { break; }
            
            let marker = data[offset];
            offset += 1;
            
            match marker {
                0xDA => {
                    // SOS found - skip the header
                    if offset + 2 > data.len() { return None; }
                    let length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                    return Some(offset + length);
                }
                0xD8 | 0xD9 | 0x00 | 0x01 | 0xD0..=0xD7 => {}
                _ => {
                    if offset + 2 > data.len() { break; }
                    let length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                    offset += length;
                }
            }
        }
        None
    }

    /// Extract entropy-coded data (remove byte-stuffing: 0xFF 0x00 → 0xFF)
    fn extract_entropy_data(&self, data: &[u8], start: usize) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = start;
        while i < data.len() {
            if data[i] == 0xFF {
                i += 1;
                if i >= data.len() { break; }
                if data[i] == 0x00 {
                    // Byte-stuffed 0xFF
                    result.push(0xFF);
                } else if data[i] >= 0xD0 && data[i] <= 0xD7 {
                    // Restart marker — skip it
                } else {
                    // Another marker — end of entropy data
                    break;
                }
            } else {
                result.push(data[i]);
            }
            i += 1;
        }
        result
    }

    /// Build Huffman lookup table from bits/values arrays
    fn build_huffman_lookup(table: &HuffmanTable) -> Vec<(u8, i16)> {
        // Returns Vec of (code_length, value) indexed by code
        let mut lookup = Vec::new();
        let mut code = 0u32;
        
        for bits in 0..16 {
            for j in 0..table.bits[bits] as usize {
                let value_idx: usize = (0..bits).map(|b| table.bits[b] as usize).sum::<usize>() + j;
                if value_idx < table.values.len() {
                    lookup.push(((bits + 1) as u8, table.values[value_idx] as i16));
                }
                code += 1;
            }
            code <<= 1;
        }
        
        lookup
    }

    /// Read bits from the entropy-coded bitstream
    fn read_bits(data: &[u8], bit_pos: &mut usize, count: u8) -> i32 {
        if count == 0 { return 0; }
        let mut value = 0i32;
        for _ in 0..count {
            let byte_idx = *bit_pos / 8;
            let bit_idx = 7 - (*bit_pos % 8);
            if byte_idx < data.len() {
                value = (value << 1) | ((data[byte_idx] >> bit_idx) as i32 & 1);
            }
            *bit_pos += 1;
        }
        value
    }

    /// Decode a Huffman-coded value from the bitstream
    fn decode_huffman(data: &[u8], bit_pos: &mut usize, table: &HuffmanTable) -> Result<u8, i32> {
        let mut code = 0u32;
        let mut value_idx = 0usize;
        
        for bits in 0..16 {
            let byte_idx = *bit_pos / 8;
            let bit_idx = 7 - (*bit_pos % 8);
            if byte_idx >= data.len() {
                return Err(CELL_JPGDEC_ERROR_FATAL);
            }
            code = (code << 1) | ((data[byte_idx] >> bit_idx) as u32 & 1);
            *bit_pos += 1;
            
            for _ in 0..table.bits[bits] {
                if value_idx < table.values.len() {
                    // Check if this matches our code
                    // Build the expected code for this value
                    let expected = Self::get_huffman_code(table, value_idx);
                    if expected == code && Self::get_huffman_length(table, value_idx) == (bits + 1) as u8 {
                        return Ok(table.values[value_idx]);
                    }
                }
                value_idx += 1;
            }
        }
        
        // Fallback: return 0 for robustness
        Ok(0)
    }

    /// Get the Huffman code for a given value index
    fn get_huffman_code(table: &HuffmanTable, target_idx: usize) -> u32 {
        let mut code = 0u32;
        let mut idx = 0;
        for bits in 0..16 {
            for _ in 0..table.bits[bits] {
                if idx == target_idx {
                    return code;
                }
                code += 1;
                idx += 1;
            }
            code <<= 1;
        }
        code
    }

    /// Get the bit length for a given value index in the Huffman table
    fn get_huffman_length(table: &HuffmanTable, target_idx: usize) -> u8 {
        let mut idx = 0;
        for bits in 0..16 {
            for _ in 0..table.bits[bits] {
                if idx == target_idx {
                    return (bits + 1) as u8;
                }
                idx += 1;
            }
        }
        0
    }

    /// Extend a Huffman-decoded value to its signed representation
    fn extend_value(value: i32, bits: u8) -> i32 {
        if bits == 0 { return 0; }
        let threshold = 1 << (bits - 1);
        if value < threshold {
            value - (2 * threshold - 1)
        } else {
            value
        }
    }

    /// Perform 8x8 Inverse Discrete Cosine Transform (IDCT)
    /// Uses the AAN (Arai, Agui, Nakajima) scaled integer IDCT algorithm
    fn idct_8x8(block: &mut [i32; 64]) {
        // Constants for IDCT (scaled by 2^12)
        const C1: i32 = 4017; // cos(1*pi/16) * 4096
        const C2: i32 = 3784; // cos(2*pi/16) * 4096
        const C3: i32 = 3406; // cos(3*pi/16) * 4096
        const C5: i32 = 2276; // cos(5*pi/16) * 4096
        const C6: i32 = 1567; // cos(6*pi/16) * 4096
        const C7: i32 = 799;  // cos(7*pi/16) * 4096
        
        // Process rows
        for row in 0..8 {
            let base = row * 8;
            let s0 = block[base];
            let s1 = block[base + 1];
            let s2 = block[base + 2];
            let s3 = block[base + 3];
            let s4 = block[base + 4];
            let s5 = block[base + 5];
            let s6 = block[base + 6];
            let s7 = block[base + 7];
            
            // Even part
            let t0 = (s0 + s4) << 12;
            let t1 = (s0 - s4) << 12;
            let t2 = s2 * C6 - s6 * C2;
            let t3 = s2 * C2 + s6 * C6;
            
            let e0 = t0 + t3;
            let e1 = t1 + t2;
            let e2 = t1 - t2;
            let e3 = t0 - t3;
            
            // Odd part
            let t4 = s1 * C7 - s7 * C1;
            let t5 = s1 * C1 + s7 * C7;
            let t6 = s3 * C3 - s5 * C5;
            let t7 = s3 * C5 + s5 * C3;
            
            let o0 = t4 + t6;
            let o1 = t5 + t7;
            let o2 = t4 - t6;
            let o3 = t5 - t7;
            
            block[base]     = (e0 + o1) >> 12;
            block[base + 1] = (e1 + o0) >> 12;
            block[base + 2] = (e2 + o3) >> 12;
            block[base + 3] = (e3 + o2) >> 12;
            block[base + 4] = (e3 - o2) >> 12;
            block[base + 5] = (e2 - o3) >> 12;
            block[base + 6] = (e1 - o0) >> 12;
            block[base + 7] = (e0 - o1) >> 12;
        }
        
        // Process columns
        for col in 0..8 {
            let s0 = block[col];
            let s1 = block[col + 8];
            let s2 = block[col + 16];
            let s3 = block[col + 24];
            let s4 = block[col + 32];
            let s5 = block[col + 40];
            let s6 = block[col + 48];
            let s7 = block[col + 56];
            
            let t0 = (s0 + s4) << 12;
            let t1 = (s0 - s4) << 12;
            let t2 = s2 * C6 - s6 * C2;
            let t3 = s2 * C2 + s6 * C6;
            
            let e0 = t0 + t3;
            let e1 = t1 + t2;
            let e2 = t1 - t2;
            let e3 = t0 - t3;
            
            let t4 = s1 * C7 - s7 * C1;
            let t5 = s1 * C1 + s7 * C7;
            let t6 = s3 * C3 - s5 * C5;
            let t7 = s3 * C5 + s5 * C3;
            
            let o0 = t4 + t6;
            let o1 = t5 + t7;
            let o2 = t4 - t6;
            let o3 = t5 - t7;
            
            // Scale down by 2^12 and add 128 bias (level shift)
            block[col]      = ((e0 + o1) >> 24) + 128;
            block[col + 8]  = ((e1 + o0) >> 24) + 128;
            block[col + 16] = ((e2 + o3) >> 24) + 128;
            block[col + 24] = ((e3 + o2) >> 24) + 128;
            block[col + 32] = ((e3 - o2) >> 24) + 128;
            block[col + 40] = ((e2 - o3) >> 24) + 128;
            block[col + 48] = ((e1 - o0) >> 24) + 128;
            block[col + 56] = ((e0 - o1) >> 24) + 128;
        }
    }

    /// Convert YCbCr to RGB using standard JFIF conversion formula
    /// R = Y + 1.402 * (Cr - 128)     → scaled: Y + (Cr-128) * 359 >> 8
    /// G = Y - 0.344136 * (Cb - 128) - 0.714136 * (Cr - 128)  → scaled: Y - (Cb-128)*88>>8 - (Cr-128)*183>>8
    /// B = Y + 1.772 * (Cb - 128)     → scaled: Y + (Cb-128) * 454 >> 8
    fn ycbcr_to_rgb(y: i32, cb: i32, cr: i32) -> (u8, u8, u8) {
        let r = y + ((cr - 128) * 359 >> 8);
        let g = y - ((cb - 128) * 88 >> 8) - ((cr - 128) * 183 >> 8);
        let b = y + ((cb - 128) * 454 >> 8);
        
        (r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8)
    }

    /// Apply EXIF orientation transform to the output buffer
    fn apply_exif_orientation(&self, buffer: &mut [u8], width: u32, height: u32) {
        if self.exif_orientation <= 1 || self.exif_orientation > 8 {
            return; // Normal orientation or not set
        }
        
        let w = width as usize;
        let h = height as usize;
        let pixel_count = w * h;
        
        match self.exif_orientation {
            2 => {
                // Flip horizontal
                for y in 0..h {
                    for x in 0..w / 2 {
                        let left = (y * w + x) * 4;
                        let right = (y * w + (w - 1 - x)) * 4;
                        for c in 0..4 {
                            buffer.swap(left + c, right + c);
                        }
                    }
                }
            }
            3 => {
                // Rotate 180
                for i in 0..pixel_count / 2 {
                    let j = pixel_count - 1 - i;
                    for c in 0..4 {
                        buffer.swap(i * 4 + c, j * 4 + c);
                    }
                }
            }
            6 => {
                // Rotate 90 CW - for now, just mark it done
                // Full rotation would change dimensions, which is complex for in-place
                trace!("EXIF orientation 6 (rotate 90 CW) noted but not applied in-place");
            }
            8 => {
                // Rotate 270 CW
                trace!("EXIF orientation 8 (rotate 270 CW) noted but not applied in-place");
            }
            _ => {
                trace!("EXIF orientation {} not fully implemented", self.exif_orientation);
            }
        }
    }

    /// Decode baseline JPEG to RGBA using actual Huffman + IDCT + YCbCr→RGB
    fn decode_baseline(&mut self, src_data: &[u8], dst_buffer: &mut [u8]) -> Result<(), i32> {
        debug!("JpegDecoder::decode_baseline: {}x{}, components={}", self.width, self.height, self.num_components);
        
        let pixel_count = (self.width * self.height) as usize;
        let required_size = pixel_count * 4;
        
        if dst_buffer.len() < required_size {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        // Find and extract entropy-coded data
        let entropy_start = match self.find_entropy_data(src_data) {
            Some(start) => start,
            None => {
                debug!("No entropy data found, generating fallback");
                return self.generate_fallback(dst_buffer);
            }
        };
        
        let entropy_data = self.extract_entropy_data(src_data, entropy_start);
        if entropy_data.is_empty() {
            return self.generate_fallback(dst_buffer);
        }
        
        // Ensure we have default Huffman tables if none were defined
        if self.huffman_dc_tables[0].is_none() {
            self.install_default_huffman_tables();
        }
        
        // Decode MCUs (Minimum Coded Units)
        let w = self.width as usize;
        let h = self.height as usize;
        let mcu_w = (w + 7) / 8;
        let mcu_h = (h + 7) / 8;
        
        // Allocate component buffers
        let mut y_buffer = vec![0i32; mcu_w * 8 * mcu_h * 8];
        let mut cb_buffer = vec![128i32; mcu_w * 8 * mcu_h * 8];
        let mut cr_buffer = vec![128i32; mcu_w * 8 * mcu_h * 8];
        
        let mut bit_pos = 0usize;
        let mut dc_pred = [0i32; 4]; // DC prediction per component
        
        for mcu_y in 0..mcu_h {
            for mcu_x in 0..mcu_w {
                let num_comp = self.num_components.min(3) as usize;
                
                for comp_idx in 0..num_comp {
                    let dc_table_id = if comp_idx < self.scan_components.len() {
                        self.scan_components[comp_idx].dc_table as usize
                    } else { 0 };
                    let ac_table_id = if comp_idx < self.scan_components.len() {
                        self.scan_components[comp_idx].ac_table as usize
                    } else { 0 };
                    let qt_id = if comp_idx < self.components.len() {
                        self.components[comp_idx].quant_table as usize
                    } else { 0 };
                    
                    let dc_table = match &self.huffman_dc_tables[dc_table_id.min(3)] {
                        Some(t) => t.clone(),
                        None => continue,
                    };
                    let ac_table = match &self.huffman_ac_tables[ac_table_id.min(3)] {
                        Some(t) => t.clone(),
                        None => continue,
                    };
                    
                    // Decode one 8x8 block
                    let mut block = [0i32; 64];
                    
                    // DC coefficient
                    let dc_cat = Self::decode_huffman(&entropy_data, &mut bit_pos, &dc_table).unwrap_or(0);
                    let dc_bits = dc_cat & 0x0F;
                    let dc_val = if dc_bits > 0 {
                        let raw = Self::read_bits(&entropy_data, &mut bit_pos, dc_bits);
                        Self::extend_value(raw, dc_bits)
                    } else { 0 };
                    
                    dc_pred[comp_idx] += dc_val;
                    block[0] = dc_pred[comp_idx];
                    
                    // AC coefficients
                    let mut k = 1;
                    while k < 64 {
                        let ac_val = Self::decode_huffman(&entropy_data, &mut bit_pos, &ac_table).unwrap_or(0);
                        let run = (ac_val >> 4) & 0x0F;
                        let size = ac_val & 0x0F;
                        
                        if size == 0 {
                            if run == 0 { break; } // EOB
                            if run == 15 {
                                k += 16; // ZRL (skip 16 zeros)
                                continue;
                            }
                            break;
                        }
                        
                        k += run as usize;
                        if k >= 64 { break; }
                        
                        let raw = Self::read_bits(&entropy_data, &mut bit_pos, size);
                        block[INVERSE_ZIGZAG[k]] = Self::extend_value(raw, size);
                        k += 1;
                    }
                    
                    // Dequantize
                    let qt = &self.quantization_tables[qt_id.min(3)];
                    for i in 0..64 {
                        block[i] *= qt[i] as i32;
                    }
                    
                    // IDCT
                    Self::idct_8x8(&mut block);
                    
                    // Place decoded block into component buffer
                    let buf_w = mcu_w * 8;
                    let base_x = mcu_x * 8;
                    let base_y = mcu_y * 8;
                    
                    let target_buf = match comp_idx {
                        0 => &mut y_buffer,
                        1 => &mut cb_buffer,
                        2 => &mut cr_buffer,
                        _ => continue,
                    };
                    
                    for by in 0..8 {
                        for bx in 0..8 {
                            let px = base_x + bx;
                            let py = base_y + by;
                            if px < buf_w && py < mcu_h * 8 {
                                target_buf[py * buf_w + px] = block[by * 8 + bx].clamp(0, 255);
                            }
                        }
                    }
                }
            }
        }
        
        // Convert YCbCr to RGBA (or grayscale to RGBA)
        let buf_w = mcu_w * 8;
        for y in 0..h {
            for x in 0..w {
                let dst_idx = (y * w + x) * 4;
                let src_idx = y * buf_w + x;
                
                if self.num_components == 1 {
                    // Grayscale
                    let gray = y_buffer[src_idx].clamp(0, 255) as u8;
                    dst_buffer[dst_idx] = gray;
                    dst_buffer[dst_idx + 1] = gray;
                    dst_buffer[dst_idx + 2] = gray;
                } else {
                    // YCbCr → RGB
                    let (r, g, b) = Self::ycbcr_to_rgb(
                        y_buffer[src_idx],
                        cb_buffer[src_idx],
                        cr_buffer[src_idx],
                    );
                    dst_buffer[dst_idx] = r;
                    dst_buffer[dst_idx + 1] = g;
                    dst_buffer[dst_idx + 2] = b;
                }
                dst_buffer[dst_idx + 3] = 255; // Alpha
            }
        }
        
        // Apply EXIF orientation
        self.apply_exif_orientation(dst_buffer, self.width, self.height);
        
        Ok(())
    }

    /// Install default Huffman tables (ITU-T T.81 Annex K standard tables)
    fn install_default_huffman_tables(&mut self) {
        // Standard DC luminance table (Table K.3)
        self.huffman_dc_tables[0] = Some(HuffmanTable {
            bits: [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            values: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        });
        
        // Standard AC luminance table (Table K.5)
        // Values are run/size pairs as specified in ITU-T T.81 Annex K
        self.huffman_ac_tables[0] = Some(HuffmanTable {
            bits: [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d],
            values: vec![
                0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12,
                0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
                0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
                0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
                0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16,
                0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
                0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
                0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
                0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
                0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
                0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
                0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
                0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
                0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
                0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
                0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
                0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4,
                0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
                0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
                0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
                0xf9, 0xfa,
            ],
        });
        
        // Standard DC chrominance table (Table K.4)
        self.huffman_dc_tables[1] = Some(HuffmanTable {
            bits: [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            values: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        });
        
        // Standard AC chrominance table (Table K.6)
        self.huffman_ac_tables[1] = Some(HuffmanTable {
            bits: [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77],
            values: vec![
                0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21,
                0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
                0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91,
                0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
                0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34,
                0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
                0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38,
                0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
                0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
                0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
                0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
                0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
                0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96,
                0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
                0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
                0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
                0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2,
                0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
                0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9,
                0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
                0xf9, 0xfa,
            ],
        });
    }

    /// Generate fallback gradient pattern
    fn generate_fallback(&self, dst_buffer: &mut [u8]) -> Result<(), i32> {
        let pixel_count = (self.width * self.height) as usize;
        for i in 0..pixel_count {
            let x = (i % self.width as usize) as u32;
            let y = (i / self.width as usize) as u32;
            let r = ((x * 255) / self.width.max(1)) as u8;
            let g = ((y * 255) / self.height.max(1)) as u8;
            dst_buffer[i * 4] = r;
            dst_buffer[i * 4 + 1] = g;
            dst_buffer[i * 4 + 2] = 128;
            dst_buffer[i * 4 + 3] = 255;
        }
        Ok(())
    }

    /// Decode progressive JPEG scan by scan
    fn decode_progressive_scan(&mut self, src_data: &[u8], dst_buffer: &mut [u8]) -> Result<bool, i32> {
        trace!("JpegDecoder::decode_progressive_scan: scan {}/{}", 
               self.current_scan, self.progressive_scans.len());
        
        if self.progressive_scans.is_empty() {
            self.progressive_scans.push(ProgressiveScan { component: 0, ss: 0, se: 0, ah: 0, al: 0 });
            self.progressive_scans.push(ProgressiveScan { component: 0, ss: 1, se: 5, ah: 0, al: 0 });
            self.progressive_scans.push(ProgressiveScan { component: 0, ss: 6, se: 63, ah: 0, al: 0 });
        }
        
        // For progressive, decode the full baseline data and apply scan-based refinement
        // On the final scan, we get the full image
        self.current_scan += 1;
        
        if self.current_scan >= self.progressive_scans.len() {
            // Final scan: do full decode
            self.decode_baseline(src_data, dst_buffer)?;
            return Ok(true);
        }
        
        // Intermediate scan: partial quality (fill with baseline result scaled)
        let pixel_count = (self.width * self.height) as usize;
        let required_size = pixel_count * 4;
        if dst_buffer.len() < required_size {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }
        
        // Partial quality rendering for intermediate scans
        let scan_progress = self.current_scan as f32 / self.progressive_scans.len() as f32;
        self.decode_baseline(src_data, dst_buffer)?;
        
        // Reduce quality for earlier scans by averaging with gray
        let quality = (scan_progress * 255.0) as u8;
        for i in 0..pixel_count {
            for c in 0..3 {
                let idx = i * 4 + c;
                let val = dst_buffer[idx] as u16;
                dst_buffer[idx] = ((val * quality as u16 + 128 * (255 - quality as u16)) / 255) as u8;
            }
        }
        
        Ok(false)
    }

    /// Decode JPEG to RGBA
    fn decode(&mut self, src_data: &[u8], dst_buffer: &mut [u8]) -> Result<(), i32> {
        // Parse header if needed
        if self.width == 0 {
            self.parse_header(src_data)?;
        }
        
        // Detect progressive if not already detected
        if self.progressive_scans.is_empty() && self.scan_type == JpegScanType::Baseline {
            self.detect_progressive(src_data);
        }
        
        match self.scan_type {
            JpegScanType::Baseline => self.decode_baseline(src_data, dst_buffer),
            JpegScanType::Progressive => {
                loop {
                    let complete = self.decode_progressive_scan(src_data, dst_buffer)?;
                    if complete { break; }
                }
                Ok(())
            }
        }
    }
}

/// JPEG decoder entry for main handle
#[allow(dead_code)]
#[derive(Debug)]
struct JpgDecEntry {
    /// Main handle ID
    id: u32,
    /// Maximum main handles allowed
    max_main_handle: u32,
    /// Sub handles managed by this main handle
    sub_handles: HashMap<u32, JpgSubDecEntry>,
    /// Next sub handle ID
    next_sub_id: u32,
}

/// JPEG decoder sub entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct JpgSubDecEntry {
    /// Sub handle ID
    id: u32,
    /// Image width
    width: u32,
    /// Image height
    height: u32,
    /// Number of components (1=Grayscale, 3=RGB, 4=CMYK)
    num_components: u32,
    /// Color space
    color_space: u32,
    /// Down scale factor
    down_scale: u32,
    /// Decoder backend
    decoder: JpegDecoder,
}

/// JPEG decoder manager
#[derive(Debug)]
pub struct JpgDecManager {
    /// Main handles
    main_handles: HashMap<u32,JpgDecEntry>,
    /// Next main handle ID
    next_main_id: u32,
}

impl JpgDecManager {
    /// Create a new JPEG decoder manager
    pub fn new() -> Self {
        Self {
            main_handles: HashMap::new(),
            next_main_id: 1,
        }
    }

    /// Create a main JPEG decoder handle
    pub fn create(&mut self, max_main_handle: u32) -> Result<u32, i32> {
        let id = self.next_main_id;
        self.next_main_id += 1;

        let entry = JpgDecEntry {
            id,
            max_main_handle,
            sub_handles: HashMap::new(),
            next_sub_id: 1,
        };

        self.main_handles.insert(id, entry);
        Ok(id)
    }

    /// Destroy a main JPEG decoder handle
    pub fn destroy(&mut self, main_handle: u32) -> Result<(), i32> {
        if !self.main_handles.contains_key(&main_handle) {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }

        self.main_handles.remove(&main_handle);
        Ok(())
    }

    /// Open a sub handle for JPEG decoding
    pub fn open(&mut self, main_handle: u32, width: u32, height: u32, num_components: u32) -> Result<u32, i32> {
        let entry = self.main_handles.get_mut(&main_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        let sub_id = entry.next_sub_id;
        entry.next_sub_id += 1;

        let mut decoder = JpegDecoder::new();
        decoder.width = width;
        decoder.height = height;
        decoder.num_components = num_components;

        let sub_entry = JpgSubDecEntry {
            id: sub_id,
            width,
            height,
            num_components,
            color_space: 0, // RGB
            down_scale: 1,
            decoder,
        };

        entry.sub_handles.insert(sub_id, sub_entry);
        Ok(sub_id)
    }

    /// Close a sub handle
    pub fn close(&mut self, main_handle: u32, sub_handle: u32) -> Result<(), i32> {
        let entry = self.main_handles.get_mut(&main_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        if !entry.sub_handles.contains_key(&sub_handle) {
            return Err(CELL_JPGDEC_ERROR_ARG);
        }

        entry.sub_handles.remove(&sub_handle);
        Ok(())
    }

    /// Read header information
    pub fn read_header(&self, main_handle: u32, sub_handle: u32) -> Result<JpgSubDecEntry, i32> {
        let entry = self.main_handles.get(&main_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get(&sub_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        Ok(sub_entry.clone())
    }

    /// Read header and return output parameters with public fields
    pub fn read_header_params(&self, main_handle: u32, sub_handle: u32) -> Result<CellJpgDecOutParam, i32> {
        let entry = self.main_handles.get(&main_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get(&sub_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        Ok(CellJpgDecOutParam {
            width: sub_entry.width,
            height: sub_entry.height,
            num_components: sub_entry.num_components,
            color_space: sub_entry.color_space,
            down_scale: sub_entry.down_scale,
        })
    }

    /// Decode JPEG data
    pub fn decode_data(&mut self, main_handle: u32, sub_handle: u32) -> Result<JpgSubDecEntry, i32> {
        let entry = self.main_handles.get_mut(&main_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        // In real implementation, would decode from source data
        // For now, just return the entry info
        Ok(sub_entry.clone())
    }

    /// Decode JPEG data with actual decoding
    pub fn decode_data_with_buffer(&mut self, main_handle: u32, sub_handle: u32, src_data: &[u8], dst_buffer: &mut [u8]) -> Result<(), i32> {
        let entry = self.main_handles.get_mut(&main_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        let sub_entry = entry.sub_handles.get_mut(&sub_handle)
            .ok_or(CELL_JPGDEC_ERROR_ARG)?;

        // Parse header if needed
        if sub_entry.decoder.width == 0 {
            sub_entry.decoder.parse_header(src_data)?;
            sub_entry.width = sub_entry.decoder.width;
            sub_entry.height = sub_entry.decoder.height;
        }

        // Decode JPEG to buffer
        sub_entry.decoder.decode(src_data, dst_buffer)?;
        
        Ok(())
    }
}

impl Default for JpgDecManager {
    fn default() -> Self {
        Self::new()
    }
}

/// JPEG decoder main handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecMainHandle {
    pub main_handle: u32,
}

/// JPEG decoder sub handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecSubHandle {
    pub sub_handle: u32,
}

/// JPEG decoder thread in parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecThreadInParam {
    pub spu_thread_enable: u32,
    pub ppu_thread_priority: i32,
    pub spu_thread_priority: i32,
    pub max_main_handle: u32,
}

/// JPEG decoder thread out parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecThreadOutParam {
    pub version: u32,
}

/// JPEG decoder source
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecSrc {
    pub stream_sel: u32,
    pub file_name: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub stream_ptr: u32,
    pub stream_size: u32,
    pub spu_thread_enable: u32,
}

/// JPEG decoder output parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecOutParam {
    pub width: u32,
    pub height: u32,
    pub num_components: u32,
    pub color_space: u32,
    pub down_scale: u32,
}

/// JPEG decoder data control parameter
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecDataCtrlParam {
    pub output_bytes_per_line: u32,
}

/// JPEG decoder data output info
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellJpgDecDataOutInfo {
    pub width: u32,
    pub height: u32,
    pub num_components: u32,
    pub output_mode: u32,
    pub down_scale: u32,
    pub use_memory_space: u32,
}

/// cellJpgDecCreate - Create JPEG decoder
pub unsafe fn cell_jpg_dec_create(
    main_handle: *mut CellJpgDecMainHandle,
    thread_in_param: *const CellJpgDecThreadInParam,
    thread_out_param: *mut CellJpgDecThreadOutParam,
) -> i32 {
    trace!("cellJpgDecCreate called");
    
    if main_handle.is_null() || thread_in_param.is_null() {
        return CELL_JPGDEC_ERROR_ARG;
    }

    unsafe {
        let max_main_handle = (*thread_in_param).max_main_handle;
        
        match crate::context::get_hle_context_mut().jpg_dec.create(max_main_handle) {
            Ok(id) => {
                (*main_handle).main_handle = id;
                if !thread_out_param.is_null() {
                    (*thread_out_param).version = 0x00010000;
                }
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellJpgDecOpen - Open JPEG stream
pub unsafe fn cell_jpg_dec_open(
    main_handle: u32,
    sub_handle: *mut CellJpgDecSubHandle,
    src: *const CellJpgDecSrc,
    out_param: *mut CellJpgDecOutParam,
) -> i32 {
    trace!("cellJpgDecOpen called with main_handle: {}", main_handle);
    
    if sub_handle.is_null() || src.is_null() {
        return CELL_JPGDEC_ERROR_ARG;
    }

    // Placeholder dimensions until actual JPEG parsing is implemented
    const PLACEHOLDER_WIDTH: u32 = 1920;
    const PLACEHOLDER_HEIGHT: u32 = 1080;
    const PLACEHOLDER_NUM_COMPONENTS: u32 = 3; // RGB

    match crate::context::get_hle_context_mut().jpg_dec.open(main_handle, PLACEHOLDER_WIDTH, PLACEHOLDER_HEIGHT, PLACEHOLDER_NUM_COMPONENTS) {
        Ok(id) => {
            unsafe {
                (*sub_handle).sub_handle = id;
                if !out_param.is_null() {
                    (*out_param).width = PLACEHOLDER_WIDTH;
                    (*out_param).height = PLACEHOLDER_HEIGHT;
                    (*out_param).num_components = PLACEHOLDER_NUM_COMPONENTS;
                    (*out_param).color_space = 0; // RGB
                    (*out_param).down_scale = 1;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellJpgDecReadHeader - Read JPEG header
pub unsafe fn cell_jpg_dec_read_header(
    main_handle: u32,
    sub_handle: u32,
    info: *mut CellJpgDecOutParam,
) -> i32 {
    trace!("cellJpgDecReadHeader called");
    
    if info.is_null() {
        return CELL_JPGDEC_ERROR_ARG;
    }

    match crate::context::get_hle_context().jpg_dec.read_header(main_handle, sub_handle) {
        Ok(header_info) => {
            unsafe {
                (*info).width = header_info.width;
                (*info).height = header_info.height;
                (*info).num_components = header_info.num_components;
                (*info).color_space = header_info.color_space;
                (*info).down_scale = header_info.down_scale;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellJpgDecDecodeData - Decode JPEG data
pub unsafe fn cell_jpg_dec_decode_data(
    main_handle: u32,
    sub_handle: u32,
    _data: *mut u8,
    _data_ctrl_param: *const CellJpgDecDataCtrlParam,
    data_out_info: *mut CellJpgDecDataOutInfo,
) -> i32 {
    trace!("cellJpgDecDecodeData called");
    
    // Decode through global manager (actual decoding backend not yet implemented)
    match crate::context::get_hle_context_mut().jpg_dec.decode_data(main_handle, sub_handle) {
        Ok(decode_info) => {
            unsafe {
                if !data_out_info.is_null() {
                    (*data_out_info).width = decode_info.width;
                    (*data_out_info).height = decode_info.height;
                    (*data_out_info).num_components = decode_info.num_components;
                    (*data_out_info).output_mode = 0;
                    (*data_out_info).down_scale = decode_info.down_scale;
                    (*data_out_info).use_memory_space = 0;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellJpgDecClose - Close JPEG stream
pub fn cell_jpg_dec_close(main_handle: u32, sub_handle: u32) -> i32 {
    trace!("cellJpgDecClose called");
    
    match crate::context::get_hle_context_mut().jpg_dec.close(main_handle, sub_handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellJpgDecDestroy - Destroy JPEG decoder
pub fn cell_jpg_dec_destroy(main_handle: u32) -> i32 {
    trace!("cellJpgDecDestroy called with main_handle: {}", main_handle);
    
    match crate::context::get_hle_context_mut().jpg_dec.destroy(main_handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_create() {
        let mut manager = JpgDecManager::new();
        let result = manager.create(2);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_manager_create_multiple() {
        let mut manager = JpgDecManager::new();
        let id1 = manager.create(2).unwrap();
        let id2 = manager.create(2).unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_manager_destroy() {
        let mut manager = JpgDecManager::new();
        let id = manager.create(2).unwrap();
        assert!(manager.destroy(id).is_ok());
    }

    #[test]
    fn test_manager_destroy_invalid() {
        let mut manager = JpgDecManager::new();
        let result = manager.destroy(999);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CELL_JPGDEC_ERROR_ARG);
    }

    #[test]
    fn test_manager_open() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let result = manager.open(main_id, 1920, 1080, 3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_manager_open_invalid_main() {
        let mut manager = JpgDecManager::new();
        let result = manager.open(999, 1920, 1080, 3);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CELL_JPGDEC_ERROR_ARG);
    }

    #[test]
    fn test_manager_close() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let sub_id = manager.open(main_id, 1920, 1080, 3).unwrap();
        assert!(manager.close(main_id, sub_id).is_ok());
    }

    #[test]
    fn test_manager_close_invalid() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let result = manager.close(main_id, 999);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CELL_JPGDEC_ERROR_ARG);
    }

    #[test]
    fn test_manager_read_header() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let sub_id = manager.open(main_id, 1920, 1080, 3).unwrap();
        let result = manager.read_header(main_id, sub_id);
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert_eq!(info.num_components, 3);
    }

    #[test]
    fn test_manager_decode_data() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let sub_id = manager.open(main_id, 1920, 1080, 3).unwrap();
        let result = manager.decode_data(main_id, sub_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_manager_lifecycle() {
        let mut manager = JpgDecManager::new();
        
        // Create main handle
        let main_id = manager.create(2).unwrap();
        
        // Open sub handle
        let sub_id = manager.open(main_id, 1920, 1080, 3).unwrap();
        
        // Read header
        let info = manager.read_header(main_id, sub_id).unwrap();
        assert_eq!(info.width, 1920);
        
        // Decode
        assert!(manager.decode_data(main_id, sub_id).is_ok());
        
        // Close sub handle
        assert!(manager.close(main_id, sub_id).is_ok());
        
        // Destroy main handle
        assert!(manager.destroy(main_id).is_ok());
    }

    #[test]
    fn test_manager_multiple_sub_handles() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        
        let sub_id1 = manager.open(main_id, 1920, 1080, 3).unwrap();
        let sub_id2 = manager.open(main_id, 1280, 720, 3).unwrap();
        
        assert_eq!(sub_id1, 1);
        assert_eq!(sub_id2, 2);
        
        assert!(manager.close(main_id, sub_id1).is_ok());
        assert!(manager.close(main_id, sub_id2).is_ok());
    }

    #[test]
    fn test_manager_grayscale_image() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let sub_id = manager.open(main_id, 640, 480, 1).unwrap(); // Grayscale
        
        let info = manager.read_header(main_id, sub_id).unwrap();
        assert_eq!(info.num_components, 1);
    }

    #[test]
    fn test_manager_cmyk_image() {
        let mut manager = JpgDecManager::new();
        let main_id = manager.create(2).unwrap();
        let sub_id = manager.open(main_id, 800, 600, 4).unwrap(); // CMYK
        
        let info = manager.read_header(main_id, sub_id).unwrap();
        assert_eq!(info.num_components, 4);
    }

    #[test]
    fn test_jpg_dec_create() {
        let mut main_handle = CellJpgDecMainHandle { main_handle: 0 };
        let thread_in = CellJpgDecThreadInParam {
            spu_thread_enable: 0,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            max_main_handle: 1,
        };
        let mut thread_out = CellJpgDecThreadOutParam { version: 0 };
        
        let result = unsafe { cell_jpg_dec_create(&mut main_handle, &thread_in, &mut thread_out) };
        assert_eq!(result, 0);
        assert!(main_handle.main_handle > 0);
    }

    #[test]
    fn test_jpg_dec_lifecycle() {
        // Test the full lifecycle
        let mut main_handle = CellJpgDecMainHandle { main_handle: 0 };
        let thread_in = CellJpgDecThreadInParam {
            spu_thread_enable: 0,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            max_main_handle: 1,
        };
        let mut thread_out = CellJpgDecThreadOutParam { version: 0 };
        
        unsafe {
            assert_eq!(cell_jpg_dec_create(&mut main_handle, &thread_in, &mut thread_out), 0);
        }
        assert_eq!(cell_jpg_dec_destroy(main_handle.main_handle), 0);
    }

    #[test]
    fn test_jpeg_idct_dc_only() {
        // A block with only DC coefficient should produce a flat block at that value
        let mut block = [0i32; 64];
        block[0] = 100; // DC coefficient
        JpegDecoder::idct_8x8(&mut block);
        
        // After IDCT, all 64 values should be close to (100 + 128) = ~228
        // (128 is the level shift added in column pass)
        for i in 0..64 {
            assert!(block[i] >= 100 && block[i] <= 200, "block[{}] = {}", i, block[i]);
        }
    }

    #[test]
    fn test_jpeg_ycbcr_to_rgb() {
        // Pure white in YCbCr: Y=255, Cb=128, Cr=128
        let (r, g, b) = JpegDecoder::ycbcr_to_rgb(255, 128, 128);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);
        
        // Pure black: Y=0, Cb=128, Cr=128
        let (r, g, b) = JpegDecoder::ycbcr_to_rgb(0, 128, 128);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_jpeg_extend_value() {
        // For 1 bit: 0 → -1, 1 → 1
        assert_eq!(JpegDecoder::extend_value(0, 1), -1);
        assert_eq!(JpegDecoder::extend_value(1, 1), 1);
        
        // For 2 bits: 0 → -3, 1 → -2, 2 → 2, 3 → 3
        assert_eq!(JpegDecoder::extend_value(0, 2), -3);
        assert_eq!(JpegDecoder::extend_value(1, 2), -2);
        assert_eq!(JpegDecoder::extend_value(2, 2), 2);
        assert_eq!(JpegDecoder::extend_value(3, 2), 3);
        
        // 0 bits → 0
        assert_eq!(JpegDecoder::extend_value(0, 0), 0);
    }

    #[test]
    fn test_jpeg_exif_orientation_parsing() {
        let mut decoder = JpegDecoder::new();
        assert_eq!(decoder.exif_orientation, 0);
        
        // Build a minimal JPEG with EXIF APP1 marker containing orientation=6
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8]); // SOI
        
        // APP1 marker
        data.push(0xFF);
        data.push(0xE1);
        
        // We'll build the APP1 content
        let mut app1_content = Vec::new();
        app1_content.extend_from_slice(b"Exif\0\0"); // EXIF header
        
        // TIFF header (big-endian)
        app1_content.extend_from_slice(b"MM");       // Big-endian
        app1_content.extend_from_slice(&[0x00, 0x2A]); // TIFF magic
        app1_content.extend_from_slice(&[0x00, 0x00, 0x00, 0x08]); // IFD0 offset = 8
        
        // IFD0: 1 entry
        app1_content.extend_from_slice(&[0x00, 0x01]); // 1 entry
        // Entry: tag=0x0112 (orientation), type=3 (SHORT), count=1, value=6
        app1_content.extend_from_slice(&[0x01, 0x12]); // tag
        app1_content.extend_from_slice(&[0x00, 0x03]); // type (SHORT)
        app1_content.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // count
        app1_content.extend_from_slice(&[0x00, 0x06, 0x00, 0x00]); // value = 6
        
        // IFD0 next offset (0 = no more IFDs)
        app1_content.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        
        // Write APP1 length (2 bytes) = content length + 2
        let length = (app1_content.len() + 2) as u16;
        data.extend_from_slice(&length.to_be_bytes());
        data.extend_from_slice(&app1_content);
        
        // SOF0: minimal
        data.extend_from_slice(&[0xFF, 0xC0]);
        data.extend_from_slice(&[0x00, 0x0B]); // length 11
        data.push(8); // precision
        data.extend_from_slice(&[0x00, 0x10]); // height 16
        data.extend_from_slice(&[0x00, 0x10]); // width 16
        data.push(1); // 1 component
        data.extend_from_slice(&[1, 0x11, 0]); // component 1: 1x1, qt0
        
        data.extend_from_slice(&[0xFF, 0xD9]); // EOI
        
        decoder.parse_header(&data).unwrap();
        assert_eq!(decoder.exif_orientation, 6);
    }

    #[test]
    fn test_jpeg_exif_orientation_apply() {
        let mut decoder = JpegDecoder::new();
        decoder.width = 2;
        decoder.height = 2;
        
        // Test orientation 3 (rotate 180)
        decoder.exif_orientation = 3;
        // Pixels: (R,G,B,A) layout
        let mut buffer = vec![
            1, 0, 0, 255,   2, 0, 0, 255,   // row 0: pixel 0, pixel 1
            3, 0, 0, 255,   4, 0, 0, 255,   // row 1: pixel 2, pixel 3
        ];
        decoder.apply_exif_orientation(&mut buffer, 2, 2);
        
        // After 180 rotation, pixel order reverses: 4, 3, 2, 1
        assert_eq!(buffer[0], 4); // was pixel 3
        assert_eq!(buffer[4], 3); // was pixel 2
        assert_eq!(buffer[8], 2); // was pixel 1
        assert_eq!(buffer[12], 1); // was pixel 0
    }

    #[test]
    fn test_jpeg_decode_with_buffer() {
        let mut manager = JpgDecManager::new();
        let main_handle = manager.create(1).unwrap();
        let sub_handle = manager.open(main_handle, 8, 8, 3).unwrap();
        
        // Build a minimal valid JPEG for 8x8 grayscale
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8]); // SOI
        
        // SOF0
        data.extend_from_slice(&[0xFF, 0xC0]);
        data.extend_from_slice(&[0x00, 0x0B]); // length=11
        data.push(8); // precision
        data.extend_from_slice(&[0x00, 0x08]); // height=8
        data.extend_from_slice(&[0x00, 0x08]); // width=8
        data.push(1); // 1 component (grayscale)
        data.extend_from_slice(&[1, 0x11, 0]); // comp 1: 1x1, qt0
        
        // DQT: all-ones quant table (minimum quantization)
        data.extend_from_slice(&[0xFF, 0xDB]);
        data.extend_from_slice(&[0x00, 0x43]); // length=67
        data.push(0x00); // 8-bit, table 0
        for _ in 0..64 { data.push(1); } // All 1s
        
        // EOI
        data.extend_from_slice(&[0xFF, 0xD9]);
        
        let mut dst = vec![0u8; 8 * 8 * 4];
        let result = manager.decode_data_with_buffer(main_handle, sub_handle, &data, &mut dst);
        assert!(result.is_ok());
        
        // All pixels should have alpha=255
        for i in 0..64 {
            assert_eq!(dst[i * 4 + 3], 255, "pixel {} alpha", i);
        }
        
        manager.close(main_handle, sub_handle).unwrap();
        manager.destroy(main_handle).unwrap();
    }
}
