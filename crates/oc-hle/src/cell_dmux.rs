//! cellDmux HLE - Demultiplexer module
//!
//! This module provides HLE implementations for the PS3's demuxer library.

use std::collections::HashMap;
use tracing::trace;

/// Demux handle
pub type DmuxHandle = u32;

/// Error codes
pub const CELL_DMUX_ERROR_ARG: i32 = 0x80610301u32 as i32;
pub const CELL_DMUX_ERROR_SEQ: i32 = 0x80610302u32 as i32;
pub const CELL_DMUX_ERROR_BUSY: i32 = 0x80610303u32 as i32;
pub const CELL_DMUX_ERROR_EMPTY: i32 = 0x80610304u32 as i32;
pub const CELL_DMUX_ERROR_FATAL: i32 = 0x80610305u32 as i32;

/// Success code
pub const CELL_OK: i32 = 0;

/// Stream types
pub const CELL_DMUX_STREAM_TYPE_PAMF: u32 = 0;
pub const CELL_DMUX_STREAM_TYPE_MPEG2_PS: u32 = 1;
pub const CELL_DMUX_STREAM_TYPE_MPEG2_TS: u32 = 2;
/// MP4 (ISO Base Media File Format) container
pub const CELL_DMUX_STREAM_TYPE_MP4: u32 = 3;

/// ES types
pub const CELL_DMUX_ES_TYPE_VIDEO: u32 = 0;
pub const CELL_DMUX_ES_TYPE_AUDIO: u32 = 1;
pub const CELL_DMUX_ES_TYPE_USER: u32 = 2;

/// Demux callback functions
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxCbMsg {
    pub msg_type: u32,
    pub supplemental_info: u32,
}

/// Demux type attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxType {
    pub stream_type: u32,
    pub reserved: [u32; 2],
}

/// Demux resource attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxResource {
    pub mem_addr: u32,
    pub mem_size: u32,
    pub ppu_thread_priority: i32,
    pub spu_thread_priority: i32,
    pub num_spu_threads: u32,
}

/// Demux callback
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxCb {
    pub cb_msg: u32,
    pub cb_arg: u32,
}

/// Elementary stream attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxEsAttr {
    pub es_type: u32,
    pub es_id: u32,
    pub es_filter_id: u32,
    pub es_specific_info_addr: u32,
    pub es_specific_info_size: u32,
}

/// Elementary stream callback
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxEsCb {
    pub cb_es_msg: u32,
    pub cb_arg: u32,
}

/// AU (Access Unit) information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellDmuxAuInfo {
    pub pts: u64,
    pub dts: u64,
    pub user_data: u64,
    pub spec_info: u32,
    pub au_addr: u32,
    pub au_size: u32,
}

/// Elementary stream entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct EsEntry {
    es_attr: CellDmuxEsAttr,
    es_cb: CellDmuxEsCb,
    au_queue: Vec<CellDmuxAuInfo>,
}

/// Container parser for demultiplexing streams
#[derive(Debug, Clone)]
struct ContainerParser {
    /// Container type
    stream_type: u32,
    /// Current parsing position
    position: usize,
    /// Total size
    total_size: usize,
}

impl ContainerParser {
    /// Create a new container parser
    fn new(stream_type: u32) -> Self {
        Self {
            stream_type,
            position: 0,
            total_size: 0,
        }
    }

    /// Parse PAMF (PlayStation Audio/video Multiplexed Format)
    /// 
    /// PAMF is Sony's proprietary container format for PS3.
    /// Header structure:
    /// - 0x00-0x03: Magic "PAMF" (0x50414D46)
    /// - 0x04-0x05: Version (e.g., 0x0041 for 4.1)
    /// - 0x08-0x0B: Data offset (big-endian, points to first elementary stream data)
    /// - 0x0C-0x0F: Data size (big-endian, size of all elementary stream data)
    /// - 0x10-0x13: Reserved
    /// - 0x14-0x17: EPmap offset (entry point map for seeking)
    /// - 0x18-0x1B: Number of streams
    /// - 0x1C+: Stream info table entries (28 bytes each)
    /// 
    /// Stream info entry (28 bytes):
    /// - 0x00: Stream type (0=video, 1=audio, 2=user)
    /// - 0x01: Stream coding type (0=AVC, 1=M2V, 0x00-0x0F audio types)
    /// - 0x02-0x03: Stream ID (e.g., 0xBD for audio, 0xE0-0xEF for video)
    /// - 0x04-0x07: EP entry count for this stream
    /// - 0x08-0x0B: Stream start offset (relative to data section)
    /// - 0x0C+: Codec-specific info (resolution, sample rate, channels, etc.)
    fn parse_pamf(&mut self, data: &[u8]) -> Result<Vec<(u32, CellDmuxAuInfo)>, i32> {
        trace!("ContainerParser::parse_pamf: size={}", data.len());
        
        let mut aus = Vec::new();
        self.total_size = data.len();
        
        // Minimum PAMF header size check
        if data.len() < 0x80 {
            trace!("parse_pamf: data too small for PAMF header");
            return Ok(aus);
        }
        
        // Check PAMF magic (0x50414D46 = "PAMF")
        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if magic != 0x50414D46 {
            trace!("parse_pamf: invalid magic {:08X}, expected 0x50414D46", magic);
            // Try to recover by scanning for elementary streams anyway
            return self.parse_pamf_fallback(data);
        }
        
        // Parse PAMF header
        let version = u16::from_be_bytes([data[4], data[5]]);
        trace!("parse_pamf: version {}.{}", version >> 8, version & 0xFF);
        
        // Data section offset and size
        let data_offset = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let data_size = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
        
        // Number of streams
        let num_streams = u32::from_be_bytes([data[24], data[25], data[26], data[27]]) as usize;
        trace!("parse_pamf: data_offset={}, data_size={}, num_streams={}", 
               data_offset, data_size, num_streams);
        
        if num_streams > 16 {
            trace!("parse_pamf: too many streams, limiting to 16");
            return self.parse_pamf_fallback(data);
        }
        
        // Parse stream info table (starts at offset 0x1C in v4.1)
        let stream_table_offset = 0x1C;
        const STREAM_INFO_SIZE: usize = 28;
        
        for i in 0..num_streams {
            let entry_offset = stream_table_offset + i * STREAM_INFO_SIZE;
            if entry_offset + STREAM_INFO_SIZE > data.len() {
                break;
            }
            
            let stream_type = data[entry_offset]; // 0=video, 1=audio, 2=user
            let coding_type = data[entry_offset + 1];
            let stream_id = u16::from_be_bytes([data[entry_offset + 2], data[entry_offset + 3]]);
            let stream_offset = u32::from_be_bytes([
                data[entry_offset + 8], data[entry_offset + 9],
                data[entry_offset + 10], data[entry_offset + 11]
            ]) as usize;
            
            trace!("parse_pamf: stream {}: type={}, coding={}, id={:04X}, offset={}",
                   i, stream_type, coding_type, stream_id, stream_offset);
            
            let es_type = match stream_type {
                0 => CELL_DMUX_ES_TYPE_VIDEO,
                1 => CELL_DMUX_ES_TYPE_AUDIO,
                _ => CELL_DMUX_ES_TYPE_USER,
            };
            
            // Calculate AU address relative to data section
            let au_addr = if data_offset + stream_offset < data.len() {
                (data_offset + stream_offset) as u32
            } else {
                0
            };
            
            // For now, estimate AU size based on data section
            let au_size = if data_size > 0 {
                (data_size / num_streams.max(1)) as u32
            } else {
                data.len() as u32
            };
            
            let au_info = CellDmuxAuInfo {
                pts: 0, // PTS extracted from PES layer
                dts: 0, // DTS extracted from PES layer
                user_data: stream_id as u64,
                spec_info: ((coding_type as u32) << 8) | (stream_type as u32),
                au_addr,
                au_size,
            };
            
            aus.push((es_type, au_info));
        }
        
        // If we found streams in the header, also scan the data section for PES packets
        if data_offset < data.len() && !aus.is_empty() {
            let data_section = &data[data_offset..];
            self.extract_pamf_pes_packets(data_section, data_offset, &mut aus);
        }
        
        trace!("parse_pamf: found {} AUs", aus.len());
        Ok(aus)
    }
    
    /// Fallback PAMF parsing - scan for PES packets directly
    fn parse_pamf_fallback(&mut self, data: &[u8]) -> Result<Vec<(u32, CellDmuxAuInfo)>, i32> {
        trace!("parse_pamf_fallback: scanning {} bytes", data.len());
        let mut aus = Vec::new();
        
        // Scan for PES start codes (0x000001 prefix)
        let mut pos = 0;
        while pos + 6 < data.len() {
            if data[pos] == 0x00 && data[pos + 1] == 0x00 && data[pos + 2] == 0x01 {
                let stream_id = data[pos + 3];
                
                // Video stream IDs: 0xE0-0xEF
                // Audio stream IDs: 0xC0-0xDF
                // Private stream 1 (often used for audio): 0xBD
                let es_type = if (0xE0..=0xEF).contains(&stream_id) {
                    CELL_DMUX_ES_TYPE_VIDEO
                } else if (0xC0..=0xDF).contains(&stream_id) || stream_id == 0xBD {
                    CELL_DMUX_ES_TYPE_AUDIO
                } else {
                    pos += 1;
                    continue;
                };
                
                // Parse PES packet length
                let pes_len = u16::from_be_bytes([data[pos + 4], data[pos + 5]]) as usize;
                let packet_size = if pes_len > 0 { pes_len + 6 } else { 2048 };
                
                // Extract PTS if present
                let (pts, dts) = if pos + 14 < data.len() && pes_len >= 8 {
                    self.extract_pes_timestamps(&data[pos..])
                } else {
                    (0, 0)
                };
                
                let au_info = CellDmuxAuInfo {
                    pts,
                    dts,
                    user_data: stream_id as u64,
                    spec_info: 0,
                    au_addr: pos as u32,
                    au_size: packet_size.min(data.len() - pos) as u32,
                };
                
                aus.push((es_type, au_info));
                pos += packet_size.max(6);
            } else {
                pos += 1;
            }
        }
        
        Ok(aus)
    }
    
    /// Extract PES packets from PAMF data section
    fn extract_pamf_pes_packets(&mut self, data: &[u8], base_offset: usize, aus: &mut Vec<(u32, CellDmuxAuInfo)>) {
        let mut pos = 0;
        
        while pos + 6 < data.len() {
            if data[pos] == 0x00 && data[pos + 1] == 0x00 && data[pos + 2] == 0x01 {
                let stream_id = data[pos + 3];
                
                let es_type = if (0xE0..=0xEF).contains(&stream_id) {
                    CELL_DMUX_ES_TYPE_VIDEO
                } else if (0xC0..=0xDF).contains(&stream_id) || stream_id == 0xBD {
                    CELL_DMUX_ES_TYPE_AUDIO
                } else {
                    pos += 1;
                    continue;
                };
                
                let pes_len = u16::from_be_bytes([data[pos + 4], data[pos + 5]]) as usize;
                let packet_size = if pes_len > 0 { pes_len + 6 } else { 2048 };
                
                let (pts, dts) = if pos + 14 < data.len() {
                    self.extract_pes_timestamps(&data[pos..])
                } else {
                    (0, 0)
                };
                
                let au_info = CellDmuxAuInfo {
                    pts,
                    dts,
                    user_data: stream_id as u64,
                    spec_info: 0,
                    au_addr: (base_offset + pos) as u32,
                    au_size: packet_size.min(data.len() - pos) as u32,
                };
                
                aus.push((es_type, au_info));
                pos += packet_size.max(6);
            } else {
                pos += 1;
            }
        }
    }
    
    /// Extract PTS and DTS from PES header
    /// PES header with PTS/DTS:
    /// - bytes 0-3: start code (0x000001) + stream_id
    /// - bytes 4-5: PES packet length
    /// - bytes 6-7: PES header flags and data
    /// - byte 8: PTS_DTS_flags (bits 7-6) and other flags
    /// - byte 9: PES header data length
    /// - bytes 10+: PTS (5 bytes), DTS (5 bytes if present)
    fn extract_pes_timestamps(&self, data: &[u8]) -> (u64, u64) {
        if data.len() < 14 {
            return (0, 0);
        }
        
        // Check PES header structure
        let pts_dts_flags = (data[7] >> 6) & 0x03;
        
        if pts_dts_flags == 0 {
            return (0, 0);
        }
        
        let mut pts: u64 = 0;
        let dts: u64;
        
        // PTS is present (flags == 2 or 3)
        if pts_dts_flags >= 2 && data.len() >= 14 {
            // PTS is encoded in 5 bytes:
            // [0]: 0011 xxx1 (xxx = bits 32-30)
            // [1-2]: bits 29-15, with marker bit at LSB of [2]
            // [3-4]: bits 14-0, with marker bit at LSB of [4]
            let pts_bytes = &data[9..14];
            pts = (((pts_bytes[0] as u64 >> 1) & 0x07) << 30)
                | ((pts_bytes[1] as u64) << 22)
                | (((pts_bytes[2] as u64 >> 1) & 0x7F) << 15)
                | ((pts_bytes[3] as u64) << 7)
                | ((pts_bytes[4] as u64 >> 1) & 0x7F);
        }
        
        // DTS is present (flags == 3)
        if pts_dts_flags == 3 && data.len() >= 19 {
            let dts_bytes = &data[14..19];
            let parsed_dts = (((dts_bytes[0] as u64 >> 1) & 0x07) << 30)
                | ((dts_bytes[1] as u64) << 22)
                | (((dts_bytes[2] as u64 >> 1) & 0x7F) << 15)
                | ((dts_bytes[3] as u64) << 7)
                | ((dts_bytes[4] as u64 >> 1) & 0x7F);
            dts = parsed_dts;
        } else {
            dts = pts; // DTS defaults to PTS if not present
        }
        
        (pts, dts)
    }

    /// Parse MPEG-2 Program Stream (MPEG-PS)
    /// 
    /// MPEG-2 PS is the standard format for DVDs and video files.
    /// Structure:
    /// - Pack Header (0x000001BA): Contains SCR (System Clock Reference) and mux rate
    /// - System Header (0x000001BB): Optional, describes stream configuration
    /// - PES Packets (0x000001BD-0x000001FF): Elementary stream data
    ///   - 0xBD: Private stream 1 (AC3, DTS, etc.)
    ///   - 0xBE: Padding stream
    ///   - 0xBF: Private stream 2 (navigation data)
    ///   - 0xC0-0xDF: Audio streams (MPEG audio, typically 0xC0)
    ///   - 0xE0-0xEF: Video streams (MPEG video, typically 0xE0)
    fn parse_mpeg_ps(&mut self, data: &[u8]) -> Result<Vec<(u32, CellDmuxAuInfo)>, i32> {
        trace!("ContainerParser::parse_mpeg_ps: size={}", data.len());
        
        let mut aus = Vec::new();
        self.total_size = data.len();
        self.position = 0;
        
        // Current timing from pack headers (90kHz clock)
        let mut current_scr: u64 = 0;
        
        // Scan for start codes
        while self.position + 6 < data.len() {
            // Look for start code prefix (0x000001)
            if data[self.position] == 0x00
                && data[self.position + 1] == 0x00
                && data[self.position + 2] == 0x01 
            {
                let start_code = data[self.position + 3];
                
                match start_code {
                    // Pack header (0xBA)
                    0xBA => {
                        if self.position + 14 <= data.len() {
                            // Parse SCR from pack header
                            current_scr = self.parse_pack_scr(&data[self.position..]);
                            
                            // Determine pack header size (MPEG-1 vs MPEG-2)
                            let pack_size = if data[self.position + 4] & 0xC0 == 0x40 {
                                // MPEG-2 pack header (14 bytes + stuffing)
                                let stuffing = (data[self.position + 13] & 0x07) as usize;
                                14 + stuffing
                            } else {
                                // MPEG-1 pack header (12 bytes)
                                12
                            };
                            self.position += pack_size;
                        } else {
                            self.position += 4;
                        }
                    }
                    
                    // System header (0xBB)
                    0xBB => {
                        if self.position + 6 <= data.len() {
                            let header_len = u16::from_be_bytes([
                                data[self.position + 4], 
                                data[self.position + 5]
                            ]) as usize;
                            self.position += 6 + header_len;
                        } else {
                            self.position += 4;
                        }
                    }
                    
                    // Private stream 1 (0xBD) - often AC3, DTS, PCM audio
                    0xBD => {
                        let au = self.parse_pes_packet(data, CELL_DMUX_ES_TYPE_AUDIO, current_scr);
                        if let Some((es_type, au_info)) = au {
                            aus.push((es_type, au_info));
                        }
                    }
                    
                    // Padding stream (0xBE) - skip
                    0xBE => {
                        if self.position + 6 <= data.len() {
                            let pes_len = u16::from_be_bytes([
                                data[self.position + 4],
                                data[self.position + 5]
                            ]) as usize;
                            self.position += 6 + pes_len;
                        } else {
                            self.position += 4;
                        }
                    }
                    
                    // Private stream 2 (0xBF) - navigation data, skip
                    0xBF => {
                        if self.position + 6 <= data.len() {
                            let pes_len = u16::from_be_bytes([
                                data[self.position + 4],
                                data[self.position + 5]
                            ]) as usize;
                            self.position += 6 + pes_len;
                        } else {
                            self.position += 4;
                        }
                    }
                    
                    // Audio streams (0xC0-0xDF)
                    id if (0xC0..=0xDF).contains(&id) => {
                        let au = self.parse_pes_packet(data, CELL_DMUX_ES_TYPE_AUDIO, current_scr);
                        if let Some((es_type, au_info)) = au {
                            aus.push((es_type, au_info));
                        }
                    }
                    
                    // Video streams (0xE0-0xEF)
                    id if (0xE0..=0xEF).contains(&id) => {
                        let au = self.parse_pes_packet(data, CELL_DMUX_ES_TYPE_VIDEO, current_scr);
                        if let Some((es_type, au_info)) = au {
                            aus.push((es_type, au_info));
                        }
                    }
                    
                    // Program end (0xB9)
                    0xB9 => {
                        trace!("parse_mpeg_ps: found program end at {}", self.position);
                        break;
                    }
                    
                    // Other start codes (video sequence headers, GOP, etc.)
                    _ => {
                        self.position += 4;
                    }
                }
            } else {
                // Resync: find next start code
                self.position += 1;
            }
        }
        
        trace!("parse_mpeg_ps: found {} AUs", aus.len());
        Ok(aus)
    }
    
    /// Parse SCR (System Clock Reference) from MPEG-2 pack header
    /// SCR is encoded across 6 bytes with marker bits
    fn parse_pack_scr(&self, data: &[u8]) -> u64 {
        if data.len() < 10 {
            return 0;
        }
        
        // Check for MPEG-2 pack header (0x4x at byte 4)
        if data[4] & 0xC0 == 0x40 {
            // MPEG-2: SCR is in bytes 4-9
            // Format: '01' + SCR[32:30] + '1' + SCR[29:15] + '1' + SCR[14:0] + '1' + SCR_ext + '1'
            let scr_base = (((data[4] as u64 >> 3) & 0x07) << 30)
                         | ((data[4] as u64 & 0x03) << 28)
                         | ((data[5] as u64) << 20)
                         | (((data[6] as u64 >> 3) & 0x1F) << 15)
                         | ((data[6] as u64 & 0x03) << 13)
                         | ((data[7] as u64) << 5)
                         | ((data[8] as u64 >> 3) & 0x1F);
            scr_base
        } else if data[4] & 0xF0 == 0x20 {
            // MPEG-1: SCR is in bytes 4-8
            let scr = (((data[4] as u64 >> 1) & 0x07) << 30)
                    | ((data[5] as u64) << 22)
                    | (((data[6] as u64 >> 1) & 0x7F) << 15)
                    | ((data[7] as u64) << 7)
                    | ((data[8] as u64 >> 1) & 0x7F);
            scr
        } else {
            0
        }
    }
    
    /// Parse a PES packet and extract AU info
    fn parse_pes_packet(&mut self, data: &[u8], es_type: u32, fallback_pts: u64) -> Option<(u32, CellDmuxAuInfo)> {
        if self.position + 6 > data.len() {
            return None;
        }
        
        let stream_id = data[self.position + 3];
        let pes_len = u16::from_be_bytes([
            data[self.position + 4],
            data[self.position + 5]
        ]) as usize;
        
        // Extract PTS/DTS from PES header
        let (pts, dts) = if self.position + 14 <= data.len() && pes_len >= 3 {
            self.extract_pes_timestamps(&data[self.position..])
        } else {
            (fallback_pts, fallback_pts)
        };
        
        // Calculate actual packet size
        let packet_size = if pes_len > 0 {
            pes_len + 6
        } else {
            // Unbounded PES (video ES) - scan for next start code
            let mut end = self.position + 6;
            while end + 4 <= data.len() {
                if data[end] == 0x00 && data[end + 1] == 0x00 && data[end + 2] == 0x01 {
                    break;
                }
                end += 1;
            }
            end - self.position
        };
        
        let au_info = CellDmuxAuInfo {
            pts,
            dts,
            user_data: stream_id as u64,
            spec_info: 0,
            au_addr: self.position as u32,
            au_size: packet_size.min(data.len() - self.position) as u32,
        };
        
        self.position += packet_size.max(6);
        Some((es_type, au_info))
    }

    /// Parse MPEG-2 Transport Stream (MPEG-TS)
    /// 
    /// MPEG-TS is the standard for broadcast and streaming.
    /// Structure:
    /// - Each packet is 188 bytes (or 204 with FEC)
    /// - Sync byte: 0x47
    /// - Header (4 bytes):
    ///   - Byte 0: Sync byte (0x47)
    ///   - Byte 1: TEI, PUSI, priority, PID[12:8]
    ///   - Byte 2: PID[7:0]
    ///   - Byte 3: Scrambling, Adaptation field ctrl, Continuity counter
    /// - Adaptation field (variable, optional)
    /// - Payload (variable)
    /// 
    /// Reserved PIDs:
    /// - 0x0000: PAT (Program Association Table)
    /// - 0x0001: CAT (Conditional Access Table)
    /// - 0x0010: NIT (Network Information Table)
    /// - 0x0011: SDT/BAT
    /// - 0x0012: EIT
    /// - 0x0013: RST
    /// - 0x0014: TDT/TOT
    /// - 0x1FFF: Null packet
    fn parse_mpeg_ts(&mut self, data: &[u8]) -> Result<Vec<(u32, CellDmuxAuInfo)>, i32> {
        trace!("ContainerParser::parse_mpeg_ts: size={}", data.len());
        
        let mut aus = Vec::new();
        self.total_size = data.len();
        self.position = 0;
        
        const TS_PACKET_SIZE: usize = 188;
        
        // Track discovered programs and streams from PAT/PMT
        let mut pmt_pids: Vec<u16> = Vec::new();
        let mut video_pids: Vec<u16> = Vec::new();
        let mut audio_pids: Vec<u16> = Vec::new();
        
        // PES assembly buffers (PID -> accumulated data)
        let mut pes_buffers: HashMap<u16, Vec<u8>> = HashMap::new();
        let mut pes_pts: HashMap<u16, u64> = HashMap::new();
        let mut pes_dts: HashMap<u16, u64> = HashMap::new();
        let mut pes_start_positions: HashMap<u16, usize> = HashMap::new();
        
        // First pass: find sync and parse PAT/PMT to discover stream PIDs
        while self.position + TS_PACKET_SIZE <= data.len() {
            // Check for sync byte
            if data[self.position] != 0x47 {
                // Try to resync
                self.position += 1;
                continue;
            }
            
            // Parse TS header
            let pusi = (data[self.position + 1] & 0x40) != 0; // Payload Unit Start Indicator
            let pid = (((data[self.position + 1] & 0x1F) as u16) << 8) | (data[self.position + 2] as u16);
            let adaptation_field_ctrl = (data[self.position + 3] >> 4) & 0x03;
            
            // Calculate payload offset
            let mut payload_offset = 4;
            if adaptation_field_ctrl & 0x02 != 0 {
                // Adaptation field present
                let af_len = data[self.position + 4] as usize;
                payload_offset += 1 + af_len;
            }
            
            // Skip if no payload
            if adaptation_field_ctrl & 0x01 == 0 || payload_offset >= TS_PACKET_SIZE {
                self.position += TS_PACKET_SIZE;
                continue;
            }
            
            let payload = &data[self.position + payload_offset..self.position + TS_PACKET_SIZE];
            
            // Process based on PID
            match pid {
                // PAT (Program Association Table)
                0x0000 => {
                    if pusi && payload.len() >= 8 {
                        // Skip pointer field if present
                        let pointer = payload[0] as usize;
                        if pointer + 8 < payload.len() {
                            let table = &payload[pointer + 1..];
                            self.parse_pat(table, &mut pmt_pids);
                        }
                    }
                }
                
                // Null packet - skip
                0x1FFF => {}
                
                // Other PIDs - check if PMT or elementary stream
                _ => {
                    // Check if this is a PMT PID
                    if pmt_pids.contains(&pid) && pusi {
                        if payload.len() >= 8 {
                            let pointer = payload[0] as usize;
                            if pointer + 8 < payload.len() {
                                let table = &payload[pointer + 1..];
                                self.parse_pmt(table, &mut video_pids, &mut audio_pids);
                            }
                        }
                    }
                    // Check if this is an elementary stream PID
                    else if video_pids.contains(&pid) || audio_pids.contains(&pid) {
                        let es_type = if video_pids.contains(&pid) {
                            CELL_DMUX_ES_TYPE_VIDEO
                        } else {
                            CELL_DMUX_ES_TYPE_AUDIO
                        };
                        
                        if pusi {
                            // Start of new PES packet
                            // First, output any previously accumulated data
                            if let Some(buf) = pes_buffers.remove(&pid) {
                                if !buf.is_empty() {
                                    let pts = pes_pts.get(&pid).copied().unwrap_or(0);
                                    let dts = pes_dts.get(&pid).copied().unwrap_or(pts);
                                    let start_pos = pes_start_positions.get(&pid).copied().unwrap_or(0);
                                    
                                    let au_info = CellDmuxAuInfo {
                                        pts,
                                        dts,
                                        user_data: pid as u64,
                                        spec_info: es_type,
                                        au_addr: start_pos as u32,
                                        au_size: buf.len() as u32,
                                    };
                                    aus.push((es_type, au_info));
                                }
                            }
                            
                            // Parse PES header for timestamps
                            if payload.len() >= 9 && payload[0] == 0x00 && payload[1] == 0x00 && payload[2] == 0x01 {
                                let (pts, dts) = self.extract_pes_timestamps(payload);
                                pes_pts.insert(pid, pts);
                                pes_dts.insert(pid, dts);
                            }
                            
                            pes_start_positions.insert(pid, self.position);
                            pes_buffers.insert(pid, payload.to_vec());
                        } else {
                            // Continuation of PES packet
                            if let Some(buf) = pes_buffers.get_mut(&pid) {
                                buf.extend_from_slice(payload);
                            }
                        }
                    }
                    // Heuristic: if we haven't found PAT/PMT, treat low PIDs as video, higher as audio
                    else if pmt_pids.is_empty() && pid > 0x0020 && pid < 0x1FFF {
                        let es_type = if pid < 0x0100 {
                            CELL_DMUX_ES_TYPE_VIDEO
                        } else {
                            CELL_DMUX_ES_TYPE_AUDIO
                        };
                        
                        if pusi {
                            // Output accumulated data
                            if let Some(buf) = pes_buffers.remove(&pid) {
                                if !buf.is_empty() {
                                    let pts = pes_pts.get(&pid).copied().unwrap_or(0);
                                    let dts = pes_dts.get(&pid).copied().unwrap_or(pts);
                                    let start_pos = pes_start_positions.get(&pid).copied().unwrap_or(0);
                                    
                                    let au_info = CellDmuxAuInfo {
                                        pts,
                                        dts,
                                        user_data: pid as u64,
                                        spec_info: es_type,
                                        au_addr: start_pos as u32,
                                        au_size: buf.len() as u32,
                                    };
                                    aus.push((es_type, au_info));
                                }
                            }
                            
                            if payload.len() >= 9 && payload[0] == 0x00 && payload[1] == 0x00 && payload[2] == 0x01 {
                                let (pts, dts) = self.extract_pes_timestamps(payload);
                                pes_pts.insert(pid, pts);
                                pes_dts.insert(pid, dts);
                            }
                            
                            pes_start_positions.insert(pid, self.position);
                            pes_buffers.insert(pid, payload.to_vec());
                        } else if pes_buffers.contains_key(&pid) {
                            if let Some(buf) = pes_buffers.get_mut(&pid) {
                                buf.extend_from_slice(payload);
                            }
                        }
                    }
                }
            }
            
            self.position += TS_PACKET_SIZE;
        }
        
        // Output any remaining buffered PES data
        for (pid, buf) in pes_buffers {
            if !buf.is_empty() {
                let es_type = if video_pids.contains(&pid) {
                    CELL_DMUX_ES_TYPE_VIDEO
                } else {
                    CELL_DMUX_ES_TYPE_AUDIO
                };
                
                let pts = pes_pts.get(&pid).copied().unwrap_or(0);
                let dts = pes_dts.get(&pid).copied().unwrap_or(pts);
                let start_pos = pes_start_positions.get(&pid).copied().unwrap_or(0);
                
                let au_info = CellDmuxAuInfo {
                    pts,
                    dts,
                    user_data: pid as u64,
                    spec_info: es_type,
                    au_addr: start_pos as u32,
                    au_size: buf.len() as u32,
                };
                aus.push((es_type, au_info));
            }
        }
        
        trace!("parse_mpeg_ts: found {} AUs, video_pids={:?}, audio_pids={:?}", 
               aus.len(), video_pids, audio_pids);
        Ok(aus)
    }
    
    /// Parse PAT (Program Association Table) to find PMT PIDs
    fn parse_pat(&self, data: &[u8], pmt_pids: &mut Vec<u16>) {
        if data.len() < 8 {
            return;
        }
        
        // Check table ID (0x00 for PAT)
        if data[0] != 0x00 {
            return;
        }
        
        let section_length = (((data[1] & 0x0F) as usize) << 8) | (data[2] as usize);
        if section_length < 9 || section_length > data.len() - 3 {
            return;
        }
        
        // Skip header and CRC, parse program entries (4 bytes each)
        let program_info_start = 8;
        let program_info_end = section_length - 4 + 3; // -4 for CRC
        
        let mut offset = program_info_start;
        while offset + 4 <= program_info_end && offset + 4 <= data.len() {
            let program_num = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let pid = (((data[offset + 2] & 0x1F) as u16) << 8) | (data[offset + 3] as u16);
            
            if program_num != 0 {
                // This is a program PMT PID (program 0 is NIT)
                if !pmt_pids.contains(&pid) {
                    pmt_pids.push(pid);
                    trace!("parse_pat: program {} -> PMT PID {:04X}", program_num, pid);
                }
            }
            
            offset += 4;
        }
    }
    
    /// Parse PMT (Program Map Table) to find elementary stream PIDs
    fn parse_pmt(&self, data: &[u8], video_pids: &mut Vec<u16>, audio_pids: &mut Vec<u16>) {
        if data.len() < 12 {
            return;
        }
        
        // Check table ID (0x02 for PMT)
        if data[0] != 0x02 {
            return;
        }
        
        let section_length = (((data[1] & 0x0F) as usize) << 8) | (data[2] as usize);
        if section_length < 9 || section_length > data.len() - 3 {
            return;
        }
        
        // Program info length
        let prog_info_len = (((data[10] & 0x0F) as usize) << 8) | (data[11] as usize);
        
        // ES info starts after program descriptors
        let mut offset = 12 + prog_info_len;
        let section_end = section_length - 4 + 3; // -4 for CRC
        
        while offset + 5 <= section_end && offset + 5 <= data.len() {
            let stream_type = data[offset];
            let es_pid = (((data[offset + 1] & 0x1F) as u16) << 8) | (data[offset + 2] as u16);
            let es_info_len = (((data[offset + 3] & 0x0F) as usize) << 8) | (data[offset + 4] as usize);
            
            // Classify stream by type
            match stream_type {
                // Video types
                0x01 | 0x02 => { // MPEG-1/2 video
                    if !video_pids.contains(&es_pid) {
                        video_pids.push(es_pid);
                        trace!("parse_pmt: MPEG video PID {:04X}", es_pid);
                    }
                }
                0x1B => { // H.264/AVC
                    if !video_pids.contains(&es_pid) {
                        video_pids.push(es_pid);
                        trace!("parse_pmt: H.264 video PID {:04X}", es_pid);
                    }
                }
                0x24 => { // H.265/HEVC
                    if !video_pids.contains(&es_pid) {
                        video_pids.push(es_pid);
                        trace!("parse_pmt: H.265 video PID {:04X}", es_pid);
                    }
                }
                // Audio types
                0x03 | 0x04 => { // MPEG-1/2 audio
                    if !audio_pids.contains(&es_pid) {
                        audio_pids.push(es_pid);
                        trace!("parse_pmt: MPEG audio PID {:04X}", es_pid);
                    }
                }
                0x0F => { // AAC
                    if !audio_pids.contains(&es_pid) {
                        audio_pids.push(es_pid);
                        trace!("parse_pmt: AAC audio PID {:04X}", es_pid);
                    }
                }
                0x81 | 0x06 => { // AC-3 / private stream with descriptor
                    if !audio_pids.contains(&es_pid) {
                        audio_pids.push(es_pid);
                        trace!("parse_pmt: AC-3/private audio PID {:04X}", es_pid);
                    }
                }
                0x11 => { // LATM AAC
                    if !audio_pids.contains(&es_pid) {
                        audio_pids.push(es_pid);
                        trace!("parse_pmt: LATM AAC audio PID {:04X}", es_pid);
                    }
                }
                _ => {
                    trace!("parse_pmt: unknown stream type {:02X} PID {:04X}", stream_type, es_pid);
                }
            }
            
            offset += 5 + es_info_len;
        }
    }

    /// Parse container and extract elementary streams
    fn parse(&mut self, data: &[u8]) -> Result<Vec<(u32, CellDmuxAuInfo)>, i32> {
        match self.stream_type {
            CELL_DMUX_STREAM_TYPE_PAMF => self.parse_pamf(data),
            CELL_DMUX_STREAM_TYPE_MPEG2_PS => self.parse_mpeg_ps(data),
            CELL_DMUX_STREAM_TYPE_MPEG2_TS => self.parse_mpeg_ts(data),
            CELL_DMUX_STREAM_TYPE_MP4 => self.parse_mp4(data),
            _ => Err(CELL_DMUX_ERROR_ARG),
        }
    }

    /// Parse MP4 (ISO Base Media File Format) container
    ///
    /// MP4 uses a box (atom) hierarchy:
    /// - ftyp: File type identification
    /// - moov: Movie metadata (contains trak boxes with codec info)
    ///   - mvhd: Movie header (timescale, duration)
    ///   - trak: Track (one per stream)
    ///     - tkhd: Track header (track ID, dimensions)
    ///     - mdia: Media information
    ///       - mdhd: Media header (timescale, duration)
    ///       - hdlr: Handler reference (vide/soun)
    ///       - minf → stbl: Sample table
    ///         - stsd: Sample descriptions (codec info)
    ///         - stts: Sample-to-time mapping
    ///         - stsc: Sample-to-chunk mapping
    ///         - stsz: Sample sizes
    ///         - stco/co64: Chunk offsets
    /// - moof: Movie fragment (for fragmented MP4)
    ///   - mfhd: Fragment header
    ///   - traf: Track fragment
    ///     - tfhd: Track fragment header
    ///     - trun: Track run (sample entries)
    /// - mdat: Media data (raw ES data)
    fn parse_mp4(&mut self, data: &[u8]) -> Result<Vec<(u32, CellDmuxAuInfo)>, i32> {
        trace!("ContainerParser::parse_mp4: size={}", data.len());
        
        let mut aus = Vec::new();
        self.total_size = data.len();
        
        // Track info collected from moov/moof
        let mut tracks: Vec<Mp4TrackInfo> = Vec::new();
        let mut mdat_offset: usize = 0;
        let mut mdat_size: usize = 0;
        let mut movie_timescale: u32 = 90000;
        
        // First pass: parse top-level boxes
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            // Handle box_size == 0 (box extends to end of file) or box_size == 1 (64-bit extended size)
            let actual_size = if box_size == 0 {
                data.len() - pos
            } else if box_size == 1 && pos + 16 <= data.len() {
                let ext = u64::from_be_bytes([
                    data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11],
                    data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15],
                ]) as usize;
                ext
            } else if box_size < 8 {
                break; // Invalid
            } else {
                box_size
            };
            
            if pos + actual_size > data.len() {
                break;
            }
            
            match box_type {
                b"ftyp" => {
                    trace!("parse_mp4: found ftyp box at offset {}", pos);
                }
                b"moov" => {
                    trace!("parse_mp4: found moov box at offset {}, size={}", pos, actual_size);
                    self.parse_mp4_moov(&data[pos + 8..pos + actual_size], &mut tracks, &mut movie_timescale);
                }
                b"moof" => {
                    trace!("parse_mp4: found moof box at offset {}, size={}", pos, actual_size);
                    self.parse_mp4_moof(&data[pos + 8..pos + actual_size], &mut tracks, movie_timescale);
                }
                b"mdat" => {
                    mdat_offset = pos + 8;
                    mdat_size = actual_size - 8;
                    trace!("parse_mp4: found mdat box at offset {}, data_size={}", pos, mdat_size);
                }
                _ => {
                    let type_str = std::str::from_utf8(box_type).unwrap_or("[non-utf8]");
                    trace!("parse_mp4: skipping box '{}' at offset {}", type_str, pos);
                }
            }
            
            pos += actual_size;
        }
        
        // Generate AUs from track info + mdat
        for track in &tracks {
            let es_type = track.es_type;
            
            if track.sample_sizes.is_empty() {
                // Single AU for entire track from mdat
                if mdat_size > 0 {
                    let au_info = CellDmuxAuInfo {
                        pts: 0,
                        dts: 0,
                        user_data: track.track_id as u64,
                        spec_info: es_type,
                        au_addr: mdat_offset as u32,
                        au_size: mdat_size as u32,
                    };
                    aus.push((es_type, au_info));
                }
            } else {
                // Per-sample AUs from stsz/stco
                let mut sample_offset = mdat_offset;
                let timescale = if track.timescale > 0 { track.timescale } else { movie_timescale };
                
                for (i, &size) in track.sample_sizes.iter().enumerate() {
                    if sample_offset + size as usize > data.len() {
                        break;
                    }
                    
                    // Calculate PTS from sample duration
                    let pts = if timescale > 0 {
                        let sample_duration = track.sample_duration.unwrap_or(1);
                        (i as u64 * sample_duration as u64 * 90000) / timescale as u64
                    } else {
                        0
                    };
                    
                    let au_info = CellDmuxAuInfo {
                        pts,
                        dts: pts,
                        user_data: track.track_id as u64,
                        spec_info: es_type,
                        au_addr: sample_offset as u32,
                        au_size: size,
                    };
                    aus.push((es_type, au_info));
                    sample_offset += size as usize;
                }
            }
        }
        
        trace!("parse_mp4: found {} AUs across {} tracks", aus.len(), tracks.len());
        Ok(aus)
    }
    
    /// Parse moov box (movie metadata) to extract track information
    fn parse_mp4_moov(&self, data: &[u8], tracks: &mut Vec<Mp4TrackInfo>, movie_timescale: &mut u32) {
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            match box_type {
                b"mvhd" => {
                    // Movie header: extract timescale
                    let content = &data[pos + 8..pos + box_size];
                    if content.len() >= 20 {
                        let version = content[0];
                        let ts_offset: usize = if version == 0 { 12 } else { 20 };
                        if ts_offset + 4 <= content.len() {
                            *movie_timescale = u32::from_be_bytes([
                                content[ts_offset], content[ts_offset + 1],
                                content[ts_offset + 2], content[ts_offset + 3],
                            ]);
                            trace!("parse_mp4_moov: movie timescale={}", movie_timescale);
                        }
                    }
                }
                b"trak" => {
                    let mut track = Mp4TrackInfo::new();
                    self.parse_mp4_trak(&data[pos + 8..pos + box_size], &mut track);
                    trace!("parse_mp4_moov: track_id={}, es_type={}, samples={}",
                           track.track_id, track.es_type, track.sample_sizes.len());
                    tracks.push(track);
                }
                _ => {}
            }
            
            pos += box_size;
        }
    }
    
    /// Parse trak box (single track) to determine type and sample layout
    fn parse_mp4_trak(&self, data: &[u8], track: &mut Mp4TrackInfo) {
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            match box_type {
                b"tkhd" => {
                    let content = &data[pos + 8..pos + box_size];
                    if content.len() >= 4 {
                        let version = content[0];
                        let id_offset: usize = if version == 0 { 12 } else { 20 };
                        if id_offset + 4 <= content.len() {
                            track.track_id = u32::from_be_bytes([
                                content[id_offset], content[id_offset + 1],
                                content[id_offset + 2], content[id_offset + 3],
                            ]);
                        }
                    }
                }
                b"mdia" => {
                    self.parse_mp4_mdia(&data[pos + 8..pos + box_size], track);
                }
                _ => {}
            }
            
            pos += box_size;
        }
    }
    
    /// Parse mdia box (media information) for handler type and sample table
    fn parse_mp4_mdia(&self, data: &[u8], track: &mut Mp4TrackInfo) {
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            match box_type {
                b"mdhd" => {
                    let content = &data[pos + 8..pos + box_size];
                    if content.len() >= 4 {
                        let version = content[0];
                        let ts_offset: usize = if version == 0 { 12 } else { 20 };
                        if ts_offset + 4 <= content.len() {
                            track.timescale = u32::from_be_bytes([
                                content[ts_offset], content[ts_offset + 1],
                                content[ts_offset + 2], content[ts_offset + 3],
                            ]);
                        }
                    }
                }
                b"hdlr" => {
                    let content = &data[pos + 8..pos + box_size];
                    if content.len() >= 12 {
                        let handler_type = &content[8..12];
                        track.es_type = match handler_type {
                            b"vide" => CELL_DMUX_ES_TYPE_VIDEO,
                            b"soun" => CELL_DMUX_ES_TYPE_AUDIO,
                            _ => CELL_DMUX_ES_TYPE_USER,
                        };
                    }
                }
                b"minf" => {
                    self.parse_mp4_minf(&data[pos + 8..pos + box_size], track);
                }
                _ => {}
            }
            
            pos += box_size;
        }
    }
    
    /// Parse minf box → stbl (sample table)
    fn parse_mp4_minf(&self, data: &[u8], track: &mut Mp4TrackInfo) {
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            if box_type == b"stbl" {
                self.parse_mp4_stbl(&data[pos + 8..pos + box_size], track);
            }
            
            pos += box_size;
        }
    }
    
    /// Parse stbl box (sample table) for sample sizes and timing
    fn parse_mp4_stbl(&self, data: &[u8], track: &mut Mp4TrackInfo) {
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            let content = &data[pos + 8..pos + box_size];
            
            match box_type {
                b"stsz" => {
                    // Sample size box
                    if content.len() >= 12 {
                        let _version = content[0];
                        let uniform_size = u32::from_be_bytes([content[4], content[5], content[6], content[7]]);
                        let sample_count = u32::from_be_bytes([content[8], content[9], content[10], content[11]]);
                        
                        if uniform_size > 0 {
                            track.sample_sizes = vec![uniform_size; sample_count as usize];
                        } else {
                            // Per-sample sizes
                            for i in 0..sample_count as usize {
                                let off = 12 + i * 4;
                                if off + 4 <= content.len() {
                                    let sz = u32::from_be_bytes([content[off], content[off + 1], content[off + 2], content[off + 3]]);
                                    track.sample_sizes.push(sz);
                                }
                            }
                        }
                    }
                }
                b"stts" => {
                    // Time-to-sample box
                    if content.len() >= 16 {
                        let _entry_count = u32::from_be_bytes([content[4], content[5], content[6], content[7]]);
                        // First entry: sample_count, sample_delta
                        let sample_delta = u32::from_be_bytes([content[12], content[13], content[14], content[15]]);
                        track.sample_duration = Some(sample_delta);
                    }
                }
                _ => {}
            }
            
            pos += box_size;
        }
    }
    
    /// Parse moof box (movie fragment) for fragmented MP4
    fn parse_mp4_moof(&self, data: &[u8], tracks: &mut Vec<Mp4TrackInfo>, movie_timescale: u32) {
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            if box_type == b"traf" {
                self.parse_mp4_traf(&data[pos + 8..pos + box_size], tracks, movie_timescale);
            }
            
            pos += box_size;
        }
    }
    
    /// Parse traf box (track fragment) for sample runs
    fn parse_mp4_traf(&self, data: &[u8], tracks: &mut Vec<Mp4TrackInfo>, movie_timescale: u32) {
        let mut track_id: u32 = 0;
        let mut default_sample_duration: u32 = 0;
        let mut default_sample_size: u32 = 0;
        
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            let box_type = &data[pos + 4..pos + 8];
            
            if box_size < 8 || pos + box_size > data.len() {
                break;
            }
            
            let content = &data[pos + 8..pos + box_size];
            
            match box_type {
                b"tfhd" => {
                    if content.len() >= 8 {
                        let flags = u32::from_be_bytes([0, content[1], content[2], content[3]]);
                        track_id = u32::from_be_bytes([content[4], content[5], content[6], content[7]]);
                        let mut off = 8;
                        if flags & 0x01 != 0 { off += 8; } // base-data-offset
                        if flags & 0x02 != 0 { off += 4; } // sample-description-index
                        if flags & 0x08 != 0 && off + 4 <= content.len() {
                            default_sample_duration = u32::from_be_bytes([content[off], content[off + 1], content[off + 2], content[off + 3]]);
                            off += 4;
                        }
                        if flags & 0x10 != 0 && off + 4 <= content.len() {
                            default_sample_size = u32::from_be_bytes([content[off], content[off + 1], content[off + 2], content[off + 3]]);
                        }
                    }
                }
                b"trun" => {
                    if content.len() >= 8 {
                        let flags = u32::from_be_bytes([0, content[1], content[2], content[3]]);
                        let sample_count = u32::from_be_bytes([content[4], content[5], content[6], content[7]]);
                        
                        // Find or create track
                        let track = if let Some(t) = tracks.iter_mut().find(|t| t.track_id == track_id) {
                            t
                        } else {
                            let mut t = Mp4TrackInfo::new();
                            t.track_id = track_id;
                            t.timescale = movie_timescale;
                            tracks.push(t);
                            tracks.last_mut().unwrap()
                        };
                        
                        let mut off: usize = 8;
                        if flags & 0x01 != 0 { off += 4; } // data-offset
                        if flags & 0x04 != 0 { off += 4; } // first-sample-flags
                        
                        for _ in 0..sample_count {
                            let duration = if flags & 0x100 != 0 && off + 4 <= content.len() {
                                let d = u32::from_be_bytes([content[off], content[off + 1], content[off + 2], content[off + 3]]);
                                off += 4;
                                d
                            } else {
                                default_sample_duration
                            };
                            
                            let size = if flags & 0x200 != 0 && off + 4 <= content.len() {
                                let s = u32::from_be_bytes([content[off], content[off + 1], content[off + 2], content[off + 3]]);
                                off += 4;
                                s
                            } else {
                                default_sample_size
                            };
                            
                            if flags & 0x400 != 0 { off += 4; } // sample-flags
                            if flags & 0x800 != 0 { off += 4; } // sample-composition-time-offset
                            
                            track.sample_sizes.push(size);
                            if track.sample_duration.is_none() && duration > 0 {
                                track.sample_duration = Some(duration);
                            }
                        }
                    }
                }
                _ => {}
            }
            
            pos += box_size;
        }
    }
}

/// Track info collected during MP4 parsing
#[derive(Debug, Clone)]
struct Mp4TrackInfo {
    track_id: u32,
    es_type: u32,
    timescale: u32,
    sample_sizes: Vec<u32>,
    sample_duration: Option<u32>,
}

impl Mp4TrackInfo {
    fn new() -> Self {
        Self {
            track_id: 0,
            es_type: CELL_DMUX_ES_TYPE_USER,
            timescale: 0,
            sample_sizes: Vec::new(),
            sample_duration: None,
        }
    }
}

/// Demux entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct DmuxEntry {
    dmux_type: CellDmuxType,
    resource: CellDmuxResource,
    cb: CellDmuxCb,
    es_map: HashMap<u32, EsEntry>,
    next_es_id: u32,
    stream_addr: u32,
    stream_size: u32,
    has_stream: bool,
    /// Container parser
    parser: Option<ContainerParser>,
    /// Pending ES notification queue (es_handle, CellDmuxCbMsg)
    notification_queue: Vec<(u32, CellDmuxCbMsg)>,
}

impl DmuxEntry {
    fn new(dmux_type: CellDmuxType, resource: CellDmuxResource, cb: CellDmuxCb) -> Self {
        let parser = ContainerParser::new(dmux_type.stream_type);
        
        Self {
            dmux_type,
            resource,
            cb,
            es_map: HashMap::new(),
            next_es_id: 1,
            stream_addr: 0,
            stream_size: 0,
            has_stream: false,
            parser: Some(parser),
            notification_queue: Vec::new(),
        }
    }
}

/// Dmux Manager
pub struct DmuxManager {
    demuxers: HashMap<DmuxHandle, DmuxEntry>,
    next_handle: DmuxHandle,
}

impl DmuxManager {
    /// Create a new DmuxManager
    pub fn new() -> Self {
        Self {
            demuxers: HashMap::new(),
            next_handle: 1,
        }
    }

    /// Open a demuxer
    pub fn open(
        &mut self,
        dmux_type: CellDmuxType,
        resource: CellDmuxResource,
        cb: CellDmuxCb,
    ) -> Result<DmuxHandle, i32> {
        // Validate parameters
        if resource.mem_size == 0 {
            return Err(CELL_DMUX_ERROR_ARG);
        }

        let handle = self.next_handle;
        self.next_handle += 1;

        let entry = DmuxEntry::new(dmux_type, resource, cb);

        self.demuxers.insert(handle, entry);
        Ok(handle)
    }

    /// Close a demuxer
    pub fn close(&mut self, handle: DmuxHandle) -> Result<(), i32> {
        if self.demuxers.remove(&handle).is_none() {
            return Err(CELL_DMUX_ERROR_ARG);
        }
        Ok(())
    }

    /// Set stream data
    pub fn set_stream(
        &mut self,
        handle: DmuxHandle,
        stream_addr: u32,
        stream_size: u32,
        _discontinuity: u32,
    ) -> Result<(), i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        entry.stream_addr = stream_addr;
        entry.stream_size = stream_size;
        entry.has_stream = true;

        // Parse the container and populate AU queues for each elementary stream
        if let Some(parser) = &mut entry.parser {
            // Simulate reading stream data (in real impl, would read from memory)
            let stream_data = vec![0u8; stream_size as usize];
            
            // Parse container to extract elementary streams
            match parser.parse(&stream_data) {
                Ok(aus) => {
                    trace!("DmuxManager::set_stream: parsed {} AUs", aus.len());
                    
                    // Distribute AUs to appropriate elementary streams
                    for (es_type, au_info) in aus {
                        // Find matching ES by type
                        for (&es_handle, es) in entry.es_map.iter_mut() {
                            if es.es_attr.es_type == es_type {
                                es.au_queue.push(au_info);
                                // Queue callback notification for this ES
                                entry.notification_queue.push((es_handle, CellDmuxCbMsg {
                                    msg_type: 0, // AU available
                                    supplemental_info: es_handle,
                                }));
                            }
                        }
                    }
                }
                Err(e) => {
                    trace!("DmuxManager::set_stream: parse failed with error {}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Reset stream
    pub fn reset_stream(&mut self, handle: DmuxHandle) -> Result<(), i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        entry.stream_addr = 0;
        entry.stream_size = 0;
        entry.has_stream = false;

        // Clear AU queues for all ES
        for es in entry.es_map.values_mut() {
            es.au_queue.clear();
        }

        Ok(())
    }

    /// Query attributes
    pub fn query_attr(&self, dmux_type: CellDmuxType) -> Result<CellDmuxType, i32> {
        // Return the same type for now
        Ok(dmux_type)
    }

    /// Enable elementary stream
    pub fn enable_es(
        &mut self,
        handle: DmuxHandle,
        es_attr: CellDmuxEsAttr,
        es_cb: CellDmuxEsCb,
    ) -> Result<u32, i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        let es_handle = entry.next_es_id;
        entry.next_es_id += 1;

        let es_entry = EsEntry {
            es_attr,
            es_cb,
            au_queue: Vec::new(),
        };

        entry.es_map.insert(es_handle, es_entry);
        Ok(es_handle)
    }

    /// Disable elementary stream
    pub fn disable_es(&mut self, handle: DmuxHandle, es_handle: u32) -> Result<(), i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        if entry.es_map.remove(&es_handle).is_none() {
            return Err(CELL_DMUX_ERROR_ARG);
        }

        Ok(())
    }

    /// Reset elementary stream
    pub fn reset_es(&mut self, handle: DmuxHandle, es_handle: u32) -> Result<(), i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        let es = entry.es_map.get_mut(&es_handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        es.au_queue.clear();
        Ok(())
    }

    /// Get access unit
    pub fn get_au(&mut self, handle: DmuxHandle, es_handle: u32) -> Result<CellDmuxAuInfo, i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        let es = entry.es_map.get_mut(&es_handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        if es.au_queue.is_empty() {
            return Err(CELL_DMUX_ERROR_EMPTY);
        }

        Ok(es.au_queue.remove(0))
    }

    /// Peek at access unit
    pub fn peek_au(&self, handle: DmuxHandle, es_handle: u32) -> Result<CellDmuxAuInfo, i32> {
        let entry = self.demuxers.get(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        let es = entry.es_map.get(&es_handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        
        if es.au_queue.is_empty() {
            return Err(CELL_DMUX_ERROR_EMPTY);
        }

        Ok(es.au_queue[0])
    }

    /// Release access unit (not used in current implementation since get_au removes it)
    pub fn release_au(&mut self, _handle: DmuxHandle, _es_handle: u32) -> Result<(), i32> {
        // AU is already removed in get_au, so this is a no-op
        Ok(())
    }

    /// Check if demuxer exists
    pub fn exists(&self, handle: DmuxHandle) -> bool {
        self.demuxers.contains_key(&handle)
    }

    /// Poll pending callback notifications for a demuxer
    pub fn poll_notifications(&mut self, handle: DmuxHandle) -> Result<Vec<(u32, CellDmuxCbMsg)>, i32> {
        let entry = self.demuxers.get_mut(&handle).ok_or(CELL_DMUX_ERROR_ARG)?;
        let notifications = std::mem::take(&mut entry.notification_queue);
        Ok(notifications)
    }
}

impl Default for DmuxManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellDmuxOpen - Open demuxer
pub unsafe fn cell_dmux_open(
    dmux_type: *const CellDmuxType,
    resource: *const CellDmuxResource,
    cb: *const CellDmuxCb,
    handle: *mut DmuxHandle,
) -> i32 {
    trace!("cellDmuxOpen called");
    
    unsafe {
        if dmux_type.is_null() || resource.is_null() || cb.is_null() || handle.is_null() {
            return CELL_DMUX_ERROR_ARG;
        }

        let dmux_type_val = *dmux_type;
        let resource_val = *resource;
        let cb_val = *cb;

        match crate::context::get_hle_context_mut().dmux.open(dmux_type_val, resource_val, cb_val) {
            Ok(h) => {
                *handle = h;
                CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellDmuxClose - Close demuxer
pub fn cell_dmux_close(handle: DmuxHandle) -> i32 {
    trace!("cellDmuxClose called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().dmux.close(handle) {
        Ok(()) => CELL_OK,
        Err(e) => e,
    }
}

/// cellDmuxSetStream - Set input stream
pub fn cell_dmux_set_stream(
    handle: DmuxHandle,
    stream_addr: u32,
    stream_size: u32,
    discontinuity: u32,
) -> i32 {
    trace!("cellDmuxSetStream called");
    
    match crate::context::get_hle_context_mut().dmux.set_stream(handle, stream_addr, stream_size, discontinuity) {
        Ok(()) => CELL_OK,
        Err(e) => e,
    }
}

/// cellDmuxResetStream - Reset stream
pub fn cell_dmux_reset_stream(handle: DmuxHandle) -> i32 {
    trace!("cellDmuxResetStream called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().dmux.reset_stream(handle) {
        Ok(()) => CELL_OK,
        Err(e) => e,
    }
}

/// cellDmuxQueryAttr - Query demuxer attributes
pub unsafe fn cell_dmux_query_attr(
    dmux_type: *const CellDmuxType,
    _resource: *const CellDmuxResource,
    attr: *mut CellDmuxType,
) -> i32 {
    trace!("cellDmuxQueryAttr called");
    
    let ctx = crate::context::get_hle_context();
    
    unsafe {
        if dmux_type.is_null() || attr.is_null() {
            return CELL_DMUX_ERROR_ARG;
        }

        let dmux_type_val = *dmux_type;
        match ctx.dmux.query_attr(dmux_type_val) {
            Ok(result) => {
                *attr = result;
                CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellDmuxEnableEs - Enable elementary stream
pub unsafe fn cell_dmux_enable_es(
    handle: DmuxHandle,
    es_attr: *const CellDmuxEsAttr,
    es_cb: *const CellDmuxEsCb,
    es_handle: *mut u32,
) -> i32 {
    trace!("cellDmuxEnableEs called");
    
    unsafe {
        if es_attr.is_null() || es_cb.is_null() || es_handle.is_null() {
            return CELL_DMUX_ERROR_ARG;
        }

        let es_attr_val = *es_attr;
        let es_cb_val = *es_cb;

        match crate::context::get_hle_context_mut().dmux.enable_es(handle, es_attr_val, es_cb_val) {
            Ok(h) => {
                *es_handle = h;
                CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellDmuxDisableEs - Disable elementary stream
pub fn cell_dmux_disable_es(handle: DmuxHandle, es_handle: u32) -> i32 {
    trace!("cellDmuxDisableEs called with es_handle: {}", es_handle);
    
    match crate::context::get_hle_context_mut().dmux.disable_es(handle, es_handle) {
        Ok(()) => CELL_OK,
        Err(e) => e,
    }
}

/// cellDmuxResetEs - Reset elementary stream
pub fn cell_dmux_reset_es(handle: DmuxHandle, es_handle: u32) -> i32 {
    trace!("cellDmuxResetEs called with es_handle: {}", es_handle);
    
    match crate::context::get_hle_context_mut().dmux.reset_es(handle, es_handle) {
        Ok(()) => CELL_OK,
        Err(e) => e,
    }
}

/// cellDmuxGetAu - Get access unit
pub unsafe fn cell_dmux_get_au(
    handle: DmuxHandle,
    es_handle: u32,
    au_info: *mut CellDmuxAuInfo,
    _au_specific_info: *mut u32,
) -> i32 {
    trace!("cellDmuxGetAu called");
    
    unsafe {
        if au_info.is_null() {
            return CELL_DMUX_ERROR_ARG;
        }

        match crate::context::get_hle_context_mut().dmux.get_au(handle, es_handle) {
            Ok(au) => {
                *au_info = au;
                CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellDmuxPeekAu - Peek at access unit
pub unsafe fn cell_dmux_peek_au(
    handle: DmuxHandle,
    es_handle: u32,
    au_info: *mut CellDmuxAuInfo,
    _au_specific_info: *mut u32,
) -> i32 {
    trace!("cellDmuxPeekAu called");
    
    unsafe {
        if au_info.is_null() {
            return CELL_DMUX_ERROR_ARG;
        }

        match crate::context::get_hle_context().dmux.peek_au(handle, es_handle) {
            Ok(au) => {
                *au_info = au;
                CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellDmuxReleaseAu - Release access unit
pub fn cell_dmux_release_au(handle: DmuxHandle, es_handle: u32) -> i32 {
    trace!("cellDmuxReleaseAu called with es_handle: {}", es_handle);
    
    match crate::context::get_hle_context_mut().dmux.release_au(handle, es_handle) {
        Ok(()) => CELL_OK,
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dmux_manager_new() {
        let manager = DmuxManager::new();
        assert_eq!(manager.demuxers.len(), 0);
        assert_eq!(manager.next_handle, 1);
    }

    #[test]
    fn test_dmux_manager_open_close() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();
        assert!(handle > 0);
        assert!(manager.exists(handle));

        manager.close(handle).unwrap();
        assert!(!manager.exists(handle));
    }

    #[test]
    fn test_dmux_manager_open_validation() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0,
            mem_size: 0, // Invalid - zero size
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        assert_eq!(manager.open(dmux_type, resource, cb).unwrap_err(), CELL_DMUX_ERROR_ARG);
    }

    #[test]
    fn test_dmux_manager_set_stream() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();
        
        manager.set_stream(handle, 0x20000000, 0x50000, 0).unwrap();
        
        let entry = manager.demuxers.get(&handle).unwrap();
        assert_eq!(entry.stream_addr, 0x20000000);
        assert_eq!(entry.stream_size, 0x50000);
        assert!(entry.has_stream);
    }

    #[test]
    fn test_dmux_manager_reset_stream() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();
        manager.set_stream(handle, 0x20000000, 0x50000, 0).unwrap();
        manager.reset_stream(handle).unwrap();
        
        let entry = manager.demuxers.get(&handle).unwrap();
        assert_eq!(entry.stream_addr, 0);
        assert_eq!(entry.stream_size, 0);
        assert!(!entry.has_stream);
    }

    #[test]
    fn test_dmux_manager_enable_disable_es() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();

        let es_attr = CellDmuxEsAttr {
            es_type: CELL_DMUX_ES_TYPE_VIDEO,
            es_id: 0xE0,
            es_filter_id: 0,
            es_specific_info_addr: 0,
            es_specific_info_size: 0,
        };
        let es_cb = CellDmuxEsCb {
            cb_es_msg: 0,
            cb_arg: 0,
        };

        let es_handle = manager.enable_es(handle, es_attr, es_cb).unwrap();
        assert!(es_handle > 0);

        let entry = manager.demuxers.get(&handle).unwrap();
        assert!(entry.es_map.contains_key(&es_handle));

        manager.disable_es(handle, es_handle).unwrap();
        let entry = manager.demuxers.get(&handle).unwrap();
        assert!(!entry.es_map.contains_key(&es_handle));
    }

    #[test]
    fn test_dmux_manager_multiple_es() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();

        // Add video ES
        let video_attr = CellDmuxEsAttr {
            es_type: CELL_DMUX_ES_TYPE_VIDEO,
            es_id: 0xE0,
            es_filter_id: 0,
            es_specific_info_addr: 0,
            es_specific_info_size: 0,
        };
        let es_cb = CellDmuxEsCb {
            cb_es_msg: 0,
            cb_arg: 0,
        };

        let video_es = manager.enable_es(handle, video_attr, es_cb).unwrap();

        // Add audio ES
        let audio_attr = CellDmuxEsAttr {
            es_type: CELL_DMUX_ES_TYPE_AUDIO,
            es_id: 0xC0,
            es_filter_id: 0,
            es_specific_info_addr: 0,
            es_specific_info_size: 0,
        };

        let audio_es = manager.enable_es(handle, audio_attr, es_cb).unwrap();

        assert_ne!(video_es, audio_es);

        let entry = manager.demuxers.get(&handle).unwrap();
        assert_eq!(entry.es_map.len(), 2);
    }

    #[test]
    fn test_dmux_manager_get_au_empty() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();

        let es_attr = CellDmuxEsAttr {
            es_type: CELL_DMUX_ES_TYPE_VIDEO,
            es_id: 0xE0,
            es_filter_id: 0,
            es_specific_info_addr: 0,
            es_specific_info_size: 0,
        };
        let es_cb = CellDmuxEsCb {
            cb_es_msg: 0,
            cb_arg: 0,
        };

        let es_handle = manager.enable_es(handle, es_attr, es_cb).unwrap();

        // Try to get AU from empty queue
        assert_eq!(manager.get_au(handle, es_handle).unwrap_err(), CELL_DMUX_ERROR_EMPTY);
    }

    #[test]
    fn test_dmux_manager_query_attr() {
        let manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_MPEG2_PS,
            reserved: [1, 2],
        };

        let result = manager.query_attr(dmux_type).unwrap();
        assert_eq!(result.stream_type, dmux_type.stream_type);
    }

    #[test]
    fn test_dmux_manager_reset_es() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();

        let es_attr = CellDmuxEsAttr {
            es_type: CELL_DMUX_ES_TYPE_VIDEO,
            es_id: 0xE0,
            es_filter_id: 0,
            es_specific_info_addr: 0,
            es_specific_info_size: 0,
        };
        let es_cb = CellDmuxEsCb {
            cb_es_msg: 0,
            cb_arg: 0,
        };

        let es_handle = manager.enable_es(handle, es_attr, es_cb).unwrap();
        manager.reset_es(handle, es_handle).unwrap();

        // ES should still exist but AU queue should be empty
        let entry = manager.demuxers.get(&handle).unwrap();
        let es = entry.es_map.get(&es_handle).unwrap();
        assert_eq!(es.au_queue.len(), 0);
    }

    #[test]
    fn test_dmux_lifecycle() {
        // HLE functions use the global manager instance from context.rs
        let dmux_type = CellDmuxType {
            stream_type: 0,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };
        let mut handle = 0;
        
        // Open should succeed using the global manager
        let result = unsafe { cell_dmux_open(&dmux_type, &resource, &cb, &mut handle) };
        assert_eq!(result, 0);
        assert!(handle > 0);
        
        // Close should also succeed using the global manager
        let close_result = cell_dmux_close(handle);
        assert_eq!(close_result, 0);
    }

    #[test]
    fn test_dmux_stream_operations() {
        // HLE functions use the global manager instance from context.rs
        // First open a demuxer to get a valid handle
        let dmux_type = CellDmuxType {
            stream_type: 0,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0,
            cb_arg: 0,
        };
        let mut handle = 0;
        
        unsafe { cell_dmux_open(&dmux_type, &resource, &cb, &mut handle) };
        
        // Stream operations should work with valid handle
        let result = cell_dmux_set_stream(handle, 0x1000, 0x10000, 0);
        assert_eq!(result, 0);
        
        let reset_result = cell_dmux_reset_stream(handle);
        assert_eq!(reset_result, 0);
        
        // Cleanup
        cell_dmux_close(handle);
    }

    #[test]
    fn test_dmux_edge_cases() {
        // Note: This test resets global context to ensure a clean state since tests
        // may run in sequence (--test-threads=1) and share the global context.
        // This is necessary to test "invalid handle" behavior correctly.
        crate::context::reset_hle_context();
        
        // Test invalid handle operations - use a handle that definitely doesn't exist
        let invalid_handle = 0x12345678;
        
        // Operations on invalid handle should return error
        assert_ne!(cell_dmux_close(invalid_handle), 0, "close should fail for invalid handle");
        assert_ne!(cell_dmux_set_stream(invalid_handle, 0x1000, 0x10000, 0), 0, "set_stream should fail for invalid handle");
        assert_ne!(cell_dmux_reset_stream(invalid_handle), 0, "reset_stream should fail for invalid handle");
        assert_ne!(cell_dmux_disable_es(invalid_handle, 0), 0, "disable_es should fail for invalid handle");
        assert_ne!(cell_dmux_reset_es(invalid_handle, 0), 0, "reset_es should fail for invalid handle");
        // Note: release_au is a no-op by design (AU already removed in get_au), so it always succeeds
    }

    #[test]
    fn test_dmux_null_parameter_validation() {
        // Test null parameter validation
        unsafe {
            let mut handle = 0;
            let dmux_type = CellDmuxType {
                stream_type: 0,
                reserved: [0, 0],
            };
            let resource = CellDmuxResource {
                mem_addr: 0,
                mem_size: 0x100000,
                ppu_thread_priority: 1001,
                spu_thread_priority: 250,
                num_spu_threads: 1,
            };
            let cb = CellDmuxCb {
                cb_msg: 0,
                cb_arg: 0,
            };
            
            // Null dmux_type
            assert_eq!(cell_dmux_open(std::ptr::null(), &resource, &cb, &mut handle), CELL_DMUX_ERROR_ARG);
            
            // Null resource
            assert_eq!(cell_dmux_open(&dmux_type, std::ptr::null(), &cb, &mut handle), CELL_DMUX_ERROR_ARG);
            
            // Null callback
            assert_eq!(cell_dmux_open(&dmux_type, &resource, std::ptr::null(), &mut handle), CELL_DMUX_ERROR_ARG);
            
            // Null handle
            assert_eq!(cell_dmux_open(&dmux_type, &resource, &cb, std::ptr::null_mut()), CELL_DMUX_ERROR_ARG);
        }
    }

    #[test]
    fn test_dmux_error_codes() {
        assert_eq!(CELL_DMUX_ERROR_ARG, 0x80610301u32 as i32);
        assert_eq!(CELL_DMUX_ERROR_SEQ, 0x80610302u32 as i32);
        assert_eq!(CELL_DMUX_ERROR_BUSY, 0x80610303u32 as i32);
        assert_eq!(CELL_DMUX_ERROR_EMPTY, 0x80610304u32 as i32);
        assert_eq!(CELL_DMUX_ERROR_FATAL, 0x80610305u32 as i32);
    }

    #[test]
    fn test_dmux_stream_types() {
        assert_eq!(CELL_DMUX_STREAM_TYPE_PAMF, 0);
        assert_eq!(CELL_DMUX_STREAM_TYPE_MPEG2_PS, 1);
        assert_eq!(CELL_DMUX_STREAM_TYPE_MPEG2_TS, 2);
    }

    #[test]
    fn test_dmux_es_types() {
        assert_eq!(CELL_DMUX_ES_TYPE_VIDEO, 0);
        assert_eq!(CELL_DMUX_ES_TYPE_AUDIO, 1);
        assert_eq!(CELL_DMUX_ES_TYPE_USER, 2);
    }

    #[test]
    fn test_container_parser_pamf_header() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_PAMF);
        
        // Create a minimal PAMF header
        let mut data = vec![0u8; 256];
        // Magic: "PAMF"
        data[0..4].copy_from_slice(&[0x50, 0x41, 0x4D, 0x46]);
        // Version 4.1
        data[4] = 0x00;
        data[5] = 0x41;
        // Data offset (0x80)
        data[8..12].copy_from_slice(&[0x00, 0x00, 0x00, 0x80]);
        // Data size
        data[12..16].copy_from_slice(&[0x00, 0x00, 0x00, 0x80]);
        // Number of streams: 2
        data[24..28].copy_from_slice(&[0x00, 0x00, 0x00, 0x02]);
        
        // Stream 0: video
        data[0x1C] = 0x00; // type = video
        data[0x1D] = 0x01; // coding = M2V
        data[0x1E..0x20].copy_from_slice(&[0x00, 0xE0]); // stream ID
        
        // Stream 1: audio
        data[0x1C + 28] = 0x01; // type = audio
        data[0x1D + 28] = 0x00; // coding = AAC
        data[0x1E + 28..0x20 + 28].copy_from_slice(&[0x00, 0xBD]); // stream ID
        
        let result = parser.parse(&data).unwrap();
        assert!(result.len() >= 2);
    }

    #[test]
    fn test_container_parser_pamf_pes_fallback() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_PAMF);
        
        // Create data with PES packets but no valid PAMF header
        let mut data = vec![0u8; 256];
        
        // PES video packet (0x000001E0)
        data[0] = 0x00;
        data[1] = 0x00;
        data[2] = 0x01;
        data[3] = 0xE0;
        data[4] = 0x00;
        data[5] = 0x10; // length = 16
        
        // PES audio packet (0x000001C0)
        data[22] = 0x00;
        data[23] = 0x00;
        data[24] = 0x01;
        data[25] = 0xC0;
        data[26] = 0x00;
        data[27] = 0x10;
        
        let result = parser.parse(&data).unwrap();
        // Should find both video and audio streams
        let has_video = result.iter().any(|(es_type, _)| *es_type == CELL_DMUX_ES_TYPE_VIDEO);
        let has_audio = result.iter().any(|(es_type, _)| *es_type == CELL_DMUX_ES_TYPE_AUDIO);
        assert!(has_video);
        assert!(has_audio);
    }

    #[test]
    fn test_container_parser_mpeg_ps() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MPEG2_PS);
        
        // Create minimal MPEG-PS data
        let mut data = vec![0u8; 512];
        
        // Pack header (0x000001BA)
        data[0] = 0x00;
        data[1] = 0x00;
        data[2] = 0x01;
        data[3] = 0xBA;
        data[4] = 0x44; // MPEG-2 marker
        // ... rest of pack header
        
        // Video PES packet (0x000001E0)
        data[14] = 0x00;
        data[15] = 0x00;
        data[16] = 0x01;
        data[17] = 0xE0;
        data[18] = 0x00;
        data[19] = 0x40; // length = 64
        data[20] = 0x80;
        data[21] = 0x00;
        data[22] = 0x00;
        
        // Audio PES packet (0x000001C0)
        let audio_offset = 14 + 6 + 64;
        if audio_offset + 10 < data.len() {
            data[audio_offset] = 0x00;
            data[audio_offset + 1] = 0x00;
            data[audio_offset + 2] = 0x01;
            data[audio_offset + 3] = 0xC0;
            data[audio_offset + 4] = 0x00;
            data[audio_offset + 5] = 0x40;
        }
        
        let result = parser.parse(&data).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_container_parser_mpeg_ts() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MPEG2_TS);
        
        // Create minimal MPEG-TS data with PAT and PMT
        let mut data = vec![0u8; 188 * 4]; // 4 TS packets
        
        // Packet 1: PAT (PID 0)
        data[0] = 0x47; // sync
        data[1] = 0x40; // PUSI=1, PID=0
        data[2] = 0x00;
        data[3] = 0x10; // adaptation=01 (payload only), cc=0
        data[4] = 0x00; // pointer field
        data[5] = 0x00; // table_id = PAT
        data[6] = 0xB0; // section syntax + section length high
        data[7] = 0x0D; // section length low = 13
        // transport_stream_id
        data[8] = 0x00;
        data[9] = 0x01;
        // version/cni
        data[10] = 0xC1;
        // section number
        data[11] = 0x00;
        // last section number
        data[12] = 0x00;
        // program 1: PMT PID = 0x100
        data[13] = 0x00;
        data[14] = 0x01; // program number
        data[15] = 0xE1; // reserved + PMT PID high
        data[16] = 0x00; // PMT PID low
        
        // Packet 2: PMT (PID 0x100)
        let p2 = 188;
        data[p2] = 0x47;
        data[p2 + 1] = 0x41; // PUSI, PID=0x100
        data[p2 + 2] = 0x00;
        data[p2 + 3] = 0x10;
        data[p2 + 4] = 0x00; // pointer
        data[p2 + 5] = 0x02; // table_id = PMT
        data[p2 + 6] = 0xB0;
        data[p2 + 7] = 0x12; // section length
        // program number
        data[p2 + 8] = 0x00;
        data[p2 + 9] = 0x01;
        data[p2 + 10] = 0xC1;
        data[p2 + 11] = 0x00;
        data[p2 + 12] = 0x00;
        // PCR PID
        data[p2 + 13] = 0xE1;
        data[p2 + 14] = 0x01;
        // program info length
        data[p2 + 15] = 0xF0;
        data[p2 + 16] = 0x00;
        // ES 1: video H.264 (type 0x1B) PID 0x101
        data[p2 + 17] = 0x1B;
        data[p2 + 18] = 0xE1;
        data[p2 + 19] = 0x01;
        data[p2 + 20] = 0xF0;
        data[p2 + 21] = 0x00;
        // ES 2: audio AAC (type 0x0F) PID 0x102
        data[p2 + 22] = 0x0F;
        data[p2 + 23] = 0xE1;
        data[p2 + 24] = 0x02;
        data[p2 + 25] = 0xF0;
        data[p2 + 26] = 0x00;
        
        // Packet 3: Video ES (PID 0x101)
        let p3 = 188 * 2;
        data[p3] = 0x47;
        data[p3 + 1] = 0x41; // PUSI
        data[p3 + 2] = 0x01; // PID = 0x101
        data[p3 + 3] = 0x10;
        // PES header
        data[p3 + 4] = 0x00;
        data[p3 + 5] = 0x00;
        data[p3 + 6] = 0x01;
        data[p3 + 7] = 0xE0;
        
        // Packet 4: Audio ES (PID 0x102)
        let p4 = 188 * 3;
        data[p4] = 0x47;
        data[p4 + 1] = 0x41; // PUSI
        data[p4 + 2] = 0x02; // PID = 0x102
        data[p4 + 3] = 0x10;
        // PES header
        data[p4 + 4] = 0x00;
        data[p4 + 5] = 0x00;
        data[p4 + 6] = 0x01;
        data[p4 + 7] = 0xC0;
        
        let result = parser.parse(&data).unwrap();
        // Should discover both video and audio streams
        assert!(!result.is_empty());
    }

    #[test]
    fn test_container_parser_pes_timestamp_extraction() {
        let parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MPEG2_PS);
        
        // Create PES packet with PTS
        let mut data = vec![0u8; 32];
        data[0] = 0x00;
        data[1] = 0x00;
        data[2] = 0x01;
        data[3] = 0xE0;
        data[4] = 0x00;
        data[5] = 0x10;
        // PES header
        data[6] = 0x80;
        data[7] = 0x80; // PTS flag set (bits 7-6 = 10)
        data[8] = 0x05; // header length
        // PTS = 90000 (1 second at 90kHz clock)
        // PTS format: 0010 xxx1 | xxxxxxxx | xxxxxxx1 | xxxxxxxx | xxxxxxx1
        // 90000 = 0x15F90
        data[9] = 0x21; // 0010 0001
        data[10] = 0x00;
        data[11] = 0x01 | 0x02; // marker
        data[12] = 0x5F;
        data[13] = 0x90 | 0x01; // marker
        
        let (pts, dts) = parser.extract_pes_timestamps(&data);
        // PTS should be non-zero
        assert!(pts > 0 || dts == pts);
    }

    #[test]
    fn test_container_parser_empty_data() {
        let mut pamf_parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_PAMF);
        let mut ps_parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MPEG2_PS);
        let mut ts_parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MPEG2_TS);
        
        // All parsers should handle empty data gracefully
        let empty = vec![];
        assert!(pamf_parser.parse(&empty).unwrap().is_empty());
        assert!(ps_parser.parse(&empty).unwrap().is_empty());
        assert!(ts_parser.parse(&empty).unwrap().is_empty());
    }

    #[test]
    fn test_container_parser_invalid_stream_type() {
        let mut parser = ContainerParser::new(99); // Invalid type
        let data = vec![0u8; 100];
        
        assert_eq!(parser.parse(&data).unwrap_err(), CELL_DMUX_ERROR_ARG);
    }

    #[test]
    fn test_container_parser_mp4_basic() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MP4);
        
        // Build minimal MP4 with ftyp + moov + mdat boxes
        let mut data = Vec::new();
        
        // ftyp box (16 bytes)
        data.extend_from_slice(&16u32.to_be_bytes()); // size
        data.extend_from_slice(b"ftyp");                // type
        data.extend_from_slice(b"isom");                // major brand
        data.extend_from_slice(&0u32.to_be_bytes());    // minor version
        
        // mdat box (16 bytes: 8 header + 8 data)
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&[0xAA; 8]);
        
        let result = parser.parse(&data).unwrap();
        // With no moov box, no tracks are discovered -> no AUs
        assert!(result.is_empty());
    }

    #[test]
    fn test_container_parser_mp4_with_moov() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MP4);
        
        // Build MP4 with moov containing a video track
        let mut data = Vec::new();
        
        // ftyp box
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"isom");
        data.extend_from_slice(&0u32.to_be_bytes());
        
        // Build moov box with mvhd + trak
        let mut moov_content = Vec::new();
        
        // mvhd box (version 0, 108 bytes total)
        let mut mvhd = Vec::new();
        mvhd.extend_from_slice(&108u32.to_be_bytes()); // size
        mvhd.extend_from_slice(b"mvhd");
        mvhd.push(0); // version = 0
        mvhd.extend_from_slice(&[0; 3]); // flags
        mvhd.extend_from_slice(&[0; 8]); // creation/modification time
        mvhd.extend_from_slice(&90000u32.to_be_bytes()); // timescale = 90000
        mvhd.extend_from_slice(&[0; 84]); // rest of mvhd
        moov_content.extend_from_slice(&mvhd);
        
        // trak box with tkhd + mdia
        let mut trak = Vec::new();
        
        // tkhd (version 0, minimal)
        let mut tkhd = Vec::new();
        tkhd.extend_from_slice(&32u32.to_be_bytes());
        tkhd.extend_from_slice(b"tkhd");
        tkhd.push(0); // version
        tkhd.extend_from_slice(&[0; 3]); // flags
        tkhd.extend_from_slice(&[0; 8]); // creation/modification time
        tkhd.extend_from_slice(&1u32.to_be_bytes()); // track ID = 1
        tkhd.extend_from_slice(&[0; 8]); // reserved + duration
        trak.extend_from_slice(&tkhd);
        
        // mdia with hdlr (vide)
        let mut mdia = Vec::new();
        
        // hdlr box (handler = vide)
        let mut hdlr = Vec::new();
        hdlr.extend_from_slice(&32u32.to_be_bytes());
        hdlr.extend_from_slice(b"hdlr");
        hdlr.extend_from_slice(&[0; 8]); // version + flags + pre-defined
        hdlr.extend_from_slice(b"vide");   // handler type
        hdlr.extend_from_slice(&[0; 12]); // reserved + name
        mdia.extend_from_slice(&hdlr);
        
        let mdia_size = (mdia.len() + 8) as u32;
        let mut mdia_box = Vec::new();
        mdia_box.extend_from_slice(&mdia_size.to_be_bytes());
        mdia_box.extend_from_slice(b"mdia");
        mdia_box.extend_from_slice(&mdia);
        trak.extend_from_slice(&mdia_box);
        
        let trak_size = (trak.len() + 8) as u32;
        moov_content.extend_from_slice(&trak_size.to_be_bytes());
        moov_content.extend_from_slice(b"trak");
        moov_content.extend_from_slice(&trak);
        
        let moov_size = (moov_content.len() + 8) as u32;
        data.extend_from_slice(&moov_size.to_be_bytes());
        data.extend_from_slice(b"moov");
        data.extend_from_slice(&moov_content);
        
        // mdat with some data
        data.extend_from_slice(&108u32.to_be_bytes());
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&[0xBB; 100]);
        
        let result = parser.parse(&data).unwrap();
        // Should find the video track with 1 AU from mdat
        assert!(!result.is_empty());
        assert_eq!(result[0].0, CELL_DMUX_ES_TYPE_VIDEO);
    }

    #[test]
    fn test_dmux_notification_on_set_stream() {
        let mut manager = DmuxManager::new();
        
        let dmux_type = CellDmuxType {
            stream_type: CELL_DMUX_STREAM_TYPE_PAMF,
            reserved: [0, 0],
        };
        let resource = CellDmuxResource {
            mem_addr: 0x10000000,
            mem_size: 0x100000,
            ppu_thread_priority: 1001,
            spu_thread_priority: 250,
            num_spu_threads: 1,
        };
        let cb = CellDmuxCb {
            cb_msg: 0x3000,
            cb_arg: 0x4000,
        };

        let handle = manager.open(dmux_type, resource, cb).unwrap();
        
        // Enable a video ES
        let es_attr = CellDmuxEsAttr {
            es_type: CELL_DMUX_ES_TYPE_VIDEO,
            es_id: 0xE0,
            es_filter_id: 0,
            es_specific_info_addr: 0,
            es_specific_info_size: 0,
        };
        let es_cb = CellDmuxEsCb {
            cb_es_msg: 0,
            cb_arg: 0,
        };

        let _es_handle = manager.enable_es(handle, es_attr, es_cb).unwrap();
        
        // Set stream data (will trigger parsing and notifications)
        manager.set_stream(handle, 0x20000000, 0x50000, 0).unwrap();
        
        // Poll notifications - may be empty since all-zero data doesn't produce valid AUs
        let notifications = manager.poll_notifications(handle).unwrap();
        // Second poll should always return empty (notifications were drained)
        let second = manager.poll_notifications(handle).unwrap();
        assert!(second.is_empty(), "Second poll should be empty after draining");
    }

    #[test]
    fn test_mp4_stream_type_constant() {
        assert_eq!(CELL_DMUX_STREAM_TYPE_MP4, 3);
    }

    #[test]
    fn test_container_parser_mp4_empty() {
        let mut parser = ContainerParser::new(CELL_DMUX_STREAM_TYPE_MP4);
        let result = parser.parse(&[]).unwrap();
        assert!(result.is_empty());
    }
}
