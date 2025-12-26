//! cellAdec HLE - Audio decoder module
//!
//! This module provides HLE implementations for the PS3's audio decoder library.

use std::collections::{HashMap, VecDeque};
use tracing::trace;

/// Audio decoder handle
pub type AdecHandle = u32;

/// Audio codec type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellAdecCodecType {
    Lpcm = 0,
    Ac3 = 1,
    Atrac3 = 2,
    Atrac3Plus = 3,
    Mp3 = 4,
    Aac = 5,
    Wma = 6,
}

/// Audio decoder type
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellAdecType {
    pub audio_codec_type: u32,
}

/// Audio decoder resource attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellAdecResource {
    pub mem_addr: u32,
    pub mem_size: u32,
    pub ppu_thread_priority: i32,
    pub spu_thread_priority: i32,
    pub ppu_thread_stack_size: u32,
}

/// Audio decoder callback message
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellAdecCbMsg {
    pub msg_type: u32,
    pub error_code: i32,
}

/// Audio decoder callback
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellAdecCb {
    pub cb_func: u32,
    pub cb_arg: u32,
}

/// Audio decoder attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellAdecAttr {
    pub decoder_mode: u32,
    pub au_info_num: u32,
}

/// Audio PCM format
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellAdecPcmFormat {
    pub num_channels: u32,
    pub sample_rate: u32,
    pub bit_depth: u32,
}

/// Audio PCM item
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellAdecPcmItem {
    pub start_addr: u32,
    pub size: u32,
    pub status: u32,
    pub au_info: CellAdecAuInfo,
}

/// AU (Access Unit) information
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellAdecAuInfo {
    pub pts: u64,
    pub size: u32,
    pub start_addr: u32,
    pub user_data: u64,
}

// Error codes
pub const CELL_ADEC_ERROR_ARG: i32 = 0x80610a01u32 as i32;
pub const CELL_ADEC_ERROR_SEQ: i32 = 0x80610a02u32 as i32;
pub const CELL_ADEC_ERROR_BUSY: i32 = 0x80610a03u32 as i32;
pub const CELL_ADEC_ERROR_EMPTY: i32 = 0x80610a04u32 as i32;
pub const CELL_ADEC_ERROR_FATAL: i32 = 0x80610a05u32 as i32;

/// Audio decoder entry
#[derive(Debug)]
struct AdecEntry {
    codec_type: u32,
    is_seq_started: bool,
    pcm_queue: VecDeque<CellAdecPcmItem>,
    au_count: u32,
}

impl AdecEntry {
    fn new(codec_type: u32) -> Self {
        Self {
            codec_type,
            is_seq_started: false,
            pcm_queue: VecDeque::new(),
            au_count: 0,
        }
    }
}

/// Audio decoder manager
pub struct AdecManager {
    decoders: HashMap<AdecHandle, AdecEntry>,
    next_handle: AdecHandle,
}

impl AdecManager {
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
            next_handle: 1,
        }
    }

    pub fn open(&mut self, codec_type: u32) -> Result<AdecHandle, i32> {
        let handle = self.next_handle;
        self.next_handle += 1;
        
        let entry = AdecEntry::new(codec_type);
        self.decoders.insert(handle, entry);
        
        Ok(handle)
    }

    pub fn close(&mut self, handle: AdecHandle) -> Result<(), i32> {
        self.decoders
            .remove(&handle)
            .ok_or(CELL_ADEC_ERROR_ARG)?;
        Ok(())
    }

    pub fn start_seq(&mut self, handle: AdecHandle) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_ADEC_ERROR_ARG)?;
        
        if entry.is_seq_started {
            return Err(CELL_ADEC_ERROR_SEQ);
        }
        
        entry.is_seq_started = true;
        Ok(())
    }

    pub fn end_seq(&mut self, handle: AdecHandle) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_ADEC_ERROR_ARG)?;
        
        if !entry.is_seq_started {
            return Err(CELL_ADEC_ERROR_SEQ);
        }
        
        entry.is_seq_started = false;
        entry.pcm_queue.clear();
        entry.au_count = 0;
        Ok(())
    }

    pub fn decode_au(&mut self, handle: AdecHandle, au_info: &CellAdecAuInfo) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_ADEC_ERROR_ARG)?;
        
        if !entry.is_seq_started {
            return Err(CELL_ADEC_ERROR_SEQ);
        }
        
        // Note: Would Integrate with actual audio decoder in a full implementation with backend integration.
        // For now, increment AU count to simulate decoding
        entry.au_count += 1;
        
        Ok(())
    }

    pub fn get_pcm(&mut self, handle: AdecHandle) -> Result<CellAdecPcmItem, i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_ADEC_ERROR_ARG)?;
        
        if !entry.is_seq_started {
            return Err(CELL_ADEC_ERROR_SEQ);
        }
        
        entry.pcm_queue.pop_front().ok_or(CELL_ADEC_ERROR_EMPTY)
    }
}

impl Default for AdecManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellAdecQueryAttr - Query decoder attributes
pub fn cell_adec_query_attr(
    adec_type: *const CellAdecType,
    attr: *mut CellAdecAttr,
) -> i32 {
    trace!("cellAdecQueryAttr called");
    
    if adec_type.is_null() || attr.is_null() {
        return CELL_ADEC_ERROR_ARG;
    }
    
    unsafe {
        (*attr).decoder_mode = 0;
        (*attr).au_info_num = 1;
    }
    
    0 // CELL_OK
}

/// cellAdecOpen - Open audio decoder
pub fn cell_adec_open(
    adec_type: *const CellAdecType,
    _resource: *const CellAdecResource,
    _cb: *const CellAdecCb,
    handle: *mut AdecHandle,
) -> i32 {
    trace!("cellAdecOpen called");
    
    if adec_type.is_null() || handle.is_null() {
        return CELL_ADEC_ERROR_ARG;
    }
    
    unsafe {
        match crate::context::get_hle_context_mut().adec.open((*adec_type).audio_codec_type) {
            Ok(h) => {
                *handle = h;
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellAdecClose - Close audio decoder
pub fn cell_adec_close(handle: AdecHandle) -> i32 {
    trace!("cellAdecClose called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().adec.close(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellAdecStartSeq - Start sequence
pub fn cell_adec_start_seq(handle: AdecHandle, _param: u32) -> i32 {
    trace!("cellAdecStartSeq called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().adec.start_seq(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellAdecEndSeq - End sequence
pub fn cell_adec_end_seq(handle: AdecHandle) -> i32 {
    trace!("cellAdecEndSeq called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().adec.end_seq(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellAdecDecodeAu - Decode access unit
pub fn cell_adec_decode_au(
    handle: AdecHandle,
    au_info: *const CellAdecAuInfo,
) -> i32 {
    trace!("cellAdecDecodeAu called");
    
    if au_info.is_null() {
        return CELL_ADEC_ERROR_ARG;
    }
    
    unsafe {
        match crate::context::get_hle_context_mut().adec.decode_au(handle, &*au_info) {
            Ok(_) => 0, // CELL_OK
            Err(e) => e,
        }
    }
}

/// cellAdecGetPcm - Get decoded PCM data
pub fn cell_adec_get_pcm(
    handle: AdecHandle,
    pcm_item: *mut CellAdecPcmItem,
) -> i32 {
    trace!("cellAdecGetPcm called");
    
    if pcm_item.is_null() {
        return CELL_ADEC_ERROR_ARG;
    }
    
    match crate::context::get_hle_context_mut().adec.get_pcm(handle) {
        Ok(pcm) => {
            unsafe {
                *pcm_item = pcm;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellAdecGetPcmItem - Get PCM item
pub fn cell_adec_get_pcm_item(
    _handle: AdecHandle,
    pcm_item_addr: *mut u32,
) -> i32 {
    trace!("cellAdecGetPcmItem called");
    
    if pcm_item_addr.is_null() {
        return CELL_ADEC_ERROR_ARG;
    }
    
    // Note: Would implement PCM item retrieval via global context. Requires memory manager integration.
    
    CELL_ADEC_ERROR_EMPTY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adec_manager_new() {
        let manager = AdecManager::new();
        assert_eq!(manager.decoders.len(), 0);
        assert_eq!(manager.next_handle, 1);
    }

    #[test]
    fn test_adec_open_close() {
        let mut manager = AdecManager::new();
        
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        assert!(handle > 0);
        assert_eq!(manager.decoders.len(), 1);
        
        manager.close(handle).unwrap();
        assert_eq!(manager.decoders.len(), 0);
    }

    #[test]
    fn test_adec_multiple_decoders() {
        let mut manager = AdecManager::new();
        
        let handle1 = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        let handle2 = manager.open(CellAdecCodecType::Aac as u32).unwrap();
        
        assert_ne!(handle1, handle2);
        assert_eq!(manager.decoders.len(), 2);
    }

    #[test]
    fn test_adec_start_end_seq() {
        let mut manager = AdecManager::new();
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        
        manager.start_seq(handle).unwrap();
        
        // Starting sequence twice should fail
        assert_eq!(manager.start_seq(handle), Err(CELL_ADEC_ERROR_SEQ));
        
        manager.end_seq(handle).unwrap();
        
        // Ending sequence twice should fail
        assert_eq!(manager.end_seq(handle), Err(CELL_ADEC_ERROR_SEQ));
    }

    #[test]
    fn test_adec_decode_without_seq() {
        let mut manager = AdecManager::new();
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        
        let au_info = CellAdecAuInfo {
            pts: 0,
            size: 100,
            start_addr: 0x10000000,
            user_data: 0,
        };
        
        // Decoding without starting sequence should fail
        assert_eq!(manager.decode_au(handle, &au_info), Err(CELL_ADEC_ERROR_SEQ));
    }

    #[test]
    fn test_adec_decode_au() {
        let mut manager = AdecManager::new();
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        manager.start_seq(handle).unwrap();
        
        let au_info = CellAdecAuInfo {
            pts: 1000,
            size: 256,
            start_addr: 0x10000000,
            user_data: 0,
        };
        
        manager.decode_au(handle, &au_info).unwrap();
        
        let entry = manager.decoders.get(&handle).unwrap();
        assert_eq!(entry.au_count, 1);
    }

    #[test]
    fn test_adec_get_pcm_empty() {
        let mut manager = AdecManager::new();
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        manager.start_seq(handle).unwrap();
        
        // No PCM decoded yet
        assert_eq!(manager.get_pcm(handle), Err(CELL_ADEC_ERROR_EMPTY));
    }

    #[test]
    fn test_adec_invalid_handle() {
        let mut manager = AdecManager::new();
        
        assert_eq!(manager.close(999), Err(CELL_ADEC_ERROR_ARG));
        assert_eq!(manager.start_seq(999), Err(CELL_ADEC_ERROR_ARG));
    }

    #[test]
    fn test_adec_lifecycle() {
        let mut manager = AdecManager::new();
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        assert!(handle > 0);
        manager.close(handle).unwrap();
    }

    #[test]
    fn test_adec_sequence() {
        let mut manager = AdecManager::new();
        let handle = manager.open(CellAdecCodecType::Mp3 as u32).unwrap();
        
        manager.start_seq(handle).unwrap();
        manager.end_seq(handle).unwrap();
    }

    #[test]
    fn test_codec_types() {
        assert_eq!(CellAdecCodecType::Lpcm as u32, 0);
        assert_eq!(CellAdecCodecType::Ac3 as u32, 1);
        assert_eq!(CellAdecCodecType::Mp3 as u32, 4);
        assert_eq!(CellAdecCodecType::Aac as u32, 5);
    }
}
