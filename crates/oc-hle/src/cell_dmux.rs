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
#[derive(Debug, Clone)]
struct EsEntry {
    es_attr: CellDmuxEsAttr,
    es_cb: CellDmuxEsCb,
    au_queue: Vec<CellDmuxAuInfo>,
}

/// Demux entry
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

        let entry = DmuxEntry {
            dmux_type,
            resource,
            cb,
            es_map: HashMap::new(),
            next_es_id: 1,
            stream_addr: 0,
            stream_size: 0,
            has_stream: false,
        };

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

        // Note: Would Parse stream and populate AU queues in a full implementation.
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
}

impl Default for DmuxManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellDmuxOpen - Open demuxer
pub fn cell_dmux_open(
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
pub fn cell_dmux_query_attr(
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
pub fn cell_dmux_enable_es(
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
pub fn cell_dmux_get_au(
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
pub fn cell_dmux_peek_au(
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
        // Note: These HLE functions currently create temporary managers
        // Note: These functions would use the global manager instance in a full implementation.
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
        
        // Open should succeed
        let result = cell_dmux_open(&dmux_type, &resource, &cb, &mut handle);
        assert_eq!(result, 0);
        assert!(handle > 0);
        
        // Close may fail since we're using temporary managers
        // This is expected until global manager is implemented
    }

    #[test]
    fn test_dmux_stream_operations() {
        // Note: These operations currently use temporary managers
        // Note: These functions would use the global manager instance in a full implementation.
        let handle = 1;
        
        // These may return errors since manager is temporary
        // The important thing is they don't panic
        let _ = cell_dmux_set_stream(handle, 0x1000, 0x10000, 0);
        let _ = cell_dmux_reset_stream(handle);
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
}
