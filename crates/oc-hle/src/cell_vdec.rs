//! cellVdec HLE - Video decoder module
//!
//! This module provides HLE implementations for the PS3's video decoder library.

use std::collections::{HashMap, VecDeque};
use tracing::trace;

/// Video decoder handle
pub type VdecHandle = u32;

/// Video codec type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellVdecCodecType {
    Mpeg2 = 0,
    Avc = 1,
    Divx = 2,
}

/// Video decoder type
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVdecType {
    pub codec_type: u32,
    pub profile_level: u32,
}

/// Video decoder resource attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVdecResource {
    pub mem_addr: u32,
    pub mem_size: u32,
    pub ppu_thread_priority: i32,
    pub spu_thread_priority: i32,
    pub ppu_thread_stack_size: u32,
}

/// Video decoder callback message
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVdecCbMsg {
    pub msg_type: u32,
    pub error_code: i32,
}

/// Video decoder callback
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVdecCb {
    pub cb_func: u32,
    pub cb_arg: u32,
}

/// Video decoder attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVdecAttr {
    pub decoder_mode: u32,
    pub au_info_num: u32,
    pub aux_info_size: u32,
}

/// Picture format
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellVdecPicFormat {
    pub alpha: u32,
    pub color_format: u32,
}

/// Picture information
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellVdecPicItem {
    pub codec_type: u32,
    pub start_addr: u32,
    pub size: u32,
    pub au_num: u32,
    pub au_info: [CellVdecAuInfo; 2],
    pub status: u32,
    pub attr: u32,
    pub pic_size: u32,
}

/// AU (Access Unit) information
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellVdecAuInfo {
    pub pts: u64,
    pub dts: u64,
    pub user_data: u64,
    pub codec_spec_info: u64,
}

// Error codes
pub const CELL_VDEC_ERROR_ARG: i32 = 0x80610901u32 as i32;
pub const CELL_VDEC_ERROR_SEQ: i32 = 0x80610902u32 as i32;
pub const CELL_VDEC_ERROR_BUSY: i32 = 0x80610903u32 as i32;
pub const CELL_VDEC_ERROR_EMPTY: i32 = 0x80610904u32 as i32;
pub const CELL_VDEC_ERROR_FATAL: i32 = 0x80610905u32 as i32;

/// Video decoder entry
#[derive(Debug)]
struct VdecEntry {
    codec_type: u32,
    profile_level: u32,
    is_seq_started: bool,
    picture_queue: VecDeque<CellVdecPicItem>,
    au_count: u32,
}

impl VdecEntry {
    fn new(codec_type: u32, profile_level: u32) -> Self {
        Self {
            codec_type,
            profile_level,
            is_seq_started: false,
            picture_queue: VecDeque::new(),
            au_count: 0,
        }
    }
}

/// Video decoder manager
pub struct VdecManager {
    decoders: HashMap<VdecHandle, VdecEntry>,
    next_handle: VdecHandle,
}

impl VdecManager {
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
            next_handle: 1,
        }
    }

    pub fn open(&mut self, codec_type: u32, profile_level: u32) -> Result<VdecHandle, i32> {
        let handle = self.next_handle;
        self.next_handle += 1;
        
        let entry = VdecEntry::new(codec_type, profile_level);
        self.decoders.insert(handle, entry);
        
        Ok(handle)
    }

    pub fn close(&mut self, handle: VdecHandle) -> Result<(), i32> {
        self.decoders
            .remove(&handle)
            .ok_or(CELL_VDEC_ERROR_ARG)?;
        Ok(())
    }

    pub fn start_seq(&mut self, handle: VdecHandle) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_VDEC_ERROR_ARG)?;
        
        if entry.is_seq_started {
            return Err(CELL_VDEC_ERROR_SEQ);
        }
        
        entry.is_seq_started = true;
        Ok(())
    }

    pub fn end_seq(&mut self, handle: VdecHandle) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_VDEC_ERROR_ARG)?;
        
        if !entry.is_seq_started {
            return Err(CELL_VDEC_ERROR_SEQ);
        }
        
        entry.is_seq_started = false;
        entry.picture_queue.clear();
        entry.au_count = 0;
        Ok(())
    }

    pub fn decode_au(&mut self, handle: VdecHandle, au_info: &CellVdecAuInfo) -> Result<(), i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_VDEC_ERROR_ARG)?;
        
        if !entry.is_seq_started {
            return Err(CELL_VDEC_ERROR_SEQ);
        }
        
        // Note: Would Integrate with actual video decoder in a full implementation with backend integration.
        // For now, increment AU count to simulate decoding
        entry.au_count += 1;
        
        Ok(())
    }

    pub fn get_picture(&mut self, handle: VdecHandle, pic_format: &CellVdecPicFormat) -> Result<CellVdecPicItem, i32> {
        let entry = self.decoders.get_mut(&handle).ok_or(CELL_VDEC_ERROR_ARG)?;
        
        if !entry.is_seq_started {
            return Err(CELL_VDEC_ERROR_SEQ);
        }
        
        entry.picture_queue.pop_front().ok_or(CELL_VDEC_ERROR_EMPTY)
    }

    pub fn set_frame_rate(&mut self, handle: VdecHandle, frame_rate: u32) -> Result<(), i32> {
        let _entry = self.decoders.get_mut(&handle).ok_or(CELL_VDEC_ERROR_ARG)?;
        
        // Note: Would store frame rate configuration. Requires implementation.
        Ok(())
    }
}

impl Default for VdecManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellVdecQueryAttr - Query decoder attributes
pub fn cell_vdec_query_attr(
    vdec_type: *const CellVdecType,
    attr: *mut CellVdecAttr,
) -> i32 {
    trace!("cellVdecQueryAttr called");
    
    if vdec_type.is_null() || attr.is_null() {
        return CELL_VDEC_ERROR_ARG;
    }
    
    unsafe {
        (*attr).decoder_mode = 0;
        (*attr).au_info_num = 1;
        (*attr).aux_info_size = 0;
    }
    
    0 // CELL_OK
}

/// cellVdecOpen - Open video decoder
pub fn cell_vdec_open(
    vdec_type: *const CellVdecType,
    _resource: *const CellVdecResource,
    _cb: *const CellVdecCb,
    handle: *mut VdecHandle,
) -> i32 {
    trace!("cellVdecOpen called");
    
    if vdec_type.is_null() || handle.is_null() {
        return CELL_VDEC_ERROR_ARG;
    }
    
    unsafe {
        match crate::context::get_hle_context_mut().vdec.open((*vdec_type).codec_type, (*vdec_type).profile_level) {
            Ok(h) => {
                *handle = h;
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellVdecClose - Close video decoder
pub fn cell_vdec_close(handle: VdecHandle) -> i32 {
    trace!("cellVdecClose called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().vdec.close(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellVdecStartSeq - Start sequence
pub fn cell_vdec_start_seq(handle: VdecHandle) -> i32 {
    trace!("cellVdecStartSeq called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().vdec.start_seq(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellVdecEndSeq - End sequence
pub fn cell_vdec_end_seq(handle: VdecHandle) -> i32 {
    trace!("cellVdecEndSeq called with handle: {}", handle);
    
    match crate::context::get_hle_context_mut().vdec.end_seq(handle) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellVdecDecodeAu - Decode access unit
pub fn cell_vdec_decode_au(
    handle: VdecHandle,
    _mode: u32,
    au_info: *const CellVdecAuInfo,
) -> i32 {
    trace!("cellVdecDecodeAu called");
    
    if au_info.is_null() {
        return CELL_VDEC_ERROR_ARG;
    }
    
    unsafe {
        match crate::context::get_hle_context_mut().vdec.decode_au(handle, &*au_info) {
            Ok(_) => 0, // CELL_OK
            Err(e) => e,
        }
    }
}

/// cellVdecGetPicture - Get decoded picture
pub fn cell_vdec_get_picture(
    handle: VdecHandle,
    pic_format: *const CellVdecPicFormat,
    pic_item: *mut CellVdecPicItem,
) -> i32 {
    trace!("cellVdecGetPicture called");
    
    if pic_format.is_null() || pic_item.is_null() {
        return CELL_VDEC_ERROR_ARG;
    }
    
    unsafe {
        match crate::context::get_hle_context_mut().vdec.get_picture(handle, &*pic_format) {
            Ok(pic) => {
                *pic_item = pic;
                0 // CELL_OK
            }
            Err(e) => e,
        }
    }
}

/// cellVdecGetPicItem - Get picture item
pub fn cell_vdec_get_pic_item(
    _handle: VdecHandle,
    pic_item_addr: *mut u32,
) -> i32 {
    trace!("cellVdecGetPicItem called");
    
    if pic_item_addr.is_null() {
        return CELL_VDEC_ERROR_ARG;
    }
    
    // Note: Would implement picture item retrieval through global context. Requires memory manager integration.
    
    CELL_VDEC_ERROR_EMPTY
}

/// cellVdecSetFrameRate - Set frame rate
pub fn cell_vdec_set_frame_rate(handle: VdecHandle, frame_rate: u32) -> i32 {
    trace!("cellVdecSetFrameRate called with frame_rate: {}", frame_rate);
    
    match crate::context::get_hle_context_mut().vdec.set_frame_rate(handle, frame_rate) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vdec_manager_new() {
        let manager = VdecManager::new();
        assert_eq!(manager.decoders.len(), 0);
        assert_eq!(manager.next_handle, 1);
    }

    #[test]
    fn test_vdec_open_close() {
        let mut manager = VdecManager::new();
        
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        assert!(handle > 0);
        assert_eq!(manager.decoders.len(), 1);
        
        manager.close(handle).unwrap();
        assert_eq!(manager.decoders.len(), 0);
    }

    #[test]
    fn test_vdec_multiple_decoders() {
        let mut manager = VdecManager::new();
        
        let handle1 = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        let handle2 = manager.open(CellVdecCodecType::Mpeg2 as u32, 0x10).unwrap();
        
        assert_ne!(handle1, handle2);
        assert_eq!(manager.decoders.len(), 2);
    }

    #[test]
    fn test_vdec_start_end_seq() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        
        manager.start_seq(handle).unwrap();
        
        // Starting sequence twice should fail
        assert_eq!(manager.start_seq(handle), Err(CELL_VDEC_ERROR_SEQ));
        
        manager.end_seq(handle).unwrap();
        
        // Ending sequence twice should fail
        assert_eq!(manager.end_seq(handle), Err(CELL_VDEC_ERROR_SEQ));
    }

    #[test]
    fn test_vdec_decode_without_seq() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        
        let au_info = CellVdecAuInfo {
            pts: 0,
            dts: 0,
            user_data: 0,
            codec_spec_info: 0,
        };
        
        // Decoding without starting sequence should fail
        assert_eq!(manager.decode_au(handle, &au_info), Err(CELL_VDEC_ERROR_SEQ));
    }

    #[test]
    fn test_vdec_decode_au() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        manager.start_seq(handle).unwrap();
        
        let au_info = CellVdecAuInfo {
            pts: 1000,
            dts: 900,
            user_data: 0,
            codec_spec_info: 0,
        };
        
        manager.decode_au(handle, &au_info).unwrap();
        
        let entry = manager.decoders.get(&handle).unwrap();
        assert_eq!(entry.au_count, 1);
    }

    #[test]
    fn test_vdec_get_picture_empty() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        manager.start_seq(handle).unwrap();
        
        let pic_format = CellVdecPicFormat {
            alpha: 0,
            color_format: 0,
        };
        
        // No pictures decoded yet
        assert_eq!(manager.get_picture(handle, &pic_format), Err(CELL_VDEC_ERROR_EMPTY));
    }

    #[test]
    fn test_vdec_set_frame_rate() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        
        manager.set_frame_rate(handle, 30).unwrap();
    }

    #[test]
    fn test_vdec_invalid_handle() {
        let mut manager = VdecManager::new();
        
        assert_eq!(manager.close(999), Err(CELL_VDEC_ERROR_ARG));
        assert_eq!(manager.start_seq(999), Err(CELL_VDEC_ERROR_ARG));
    }

    #[test]
    fn test_vdec_lifecycle() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        assert!(handle > 0);
        manager.close(handle).unwrap();
    }

    #[test]
    fn test_vdec_sequence() {
        let mut manager = VdecManager::new();
        let handle = manager.open(CellVdecCodecType::Avc as u32, 0x42).unwrap();
        
        manager.start_seq(handle).unwrap();
        manager.end_seq(handle).unwrap();
    }

    #[test]
    fn test_codec_types() {
        assert_eq!(CellVdecCodecType::Mpeg2 as u32, 0);
        assert_eq!(CellVdecCodecType::Avc as u32, 1);
        assert_eq!(CellVdecCodecType::Divx as u32, 2);
    }
}
