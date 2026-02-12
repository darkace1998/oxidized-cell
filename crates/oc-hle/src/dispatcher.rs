//! HLE Function Dispatcher
//!
//! This module provides the main entry point for HLE function calls.
//! It maps stub addresses to HLE functions and dispatches calls with
//! proper argument marshalling.

use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::{debug, info, trace};

use crate::context::get_hle_context_mut;
use crate::memory::write_be32;

/// HLE function result codes
pub mod error {
    pub const CELL_OK: i64 = 0;
    pub const CELL_CANCEL: i64 = 1;
    pub const CELL_EFAULT: i64 = 0x80010002u32 as i64;
    pub const CELL_EINVAL: i64 = 0x80010003u32 as i64;
    pub const CELL_ENOMEM: i64 = 0x80010004u32 as i64;
    pub const CELL_ENOENT: i64 = 0x80010006u32 as i64;
    pub const CELL_EBUSY: i64 = 0x8001000Au32 as i64;
    pub const CELL_ENOTINIT: i64 = 0x80010013u32 as i64;
}

/// Maximum bytes to transfer in a single cellFsRead/cellFsWrite dispatcher call
const FS_MAX_TRANSFER_SIZE: u64 = 64 * 1024;

/// HLE function call context
#[derive(Debug, Clone)]
pub struct HleCallContext {
    /// Stub address that was called
    pub stub_addr: u32,
    /// Function arguments (from R3-R10)
    pub args: [u64; 8],
    /// TOC value (R2)
    pub toc: u64,
    /// Link Register (return address)
    pub lr: u64,
}

/// HLE function type - takes arguments, returns result in R3
pub type HleFn = fn(&HleCallContext) -> i64;

/// HLE function info
#[derive(Clone)]
pub struct HleFunctionInfo {
    /// Function name (for debugging)
    pub name: &'static str,
    /// Module name
    pub module: &'static str,
    /// Handler function
    pub handler: HleFn,
}

/// Global HLE dispatcher
pub static HLE_DISPATCHER: Lazy<RwLock<HleDispatcher>> = Lazy::new(|| {
    RwLock::new(HleDispatcher::new())
});

/// HLE Dispatcher - maps stub addresses to HLE functions
pub struct HleDispatcher {
    /// Map of stub address -> function info
    stub_map: HashMap<u32, HleFunctionInfo>,
    /// Next available stub ID for dynamic registration
    next_stub_id: u32,
    /// Base address for stub region
    stub_base: u32,
    /// Call statistics
    call_counts: HashMap<u32, u64>,
    /// NID → stub address mapping for NID-based dispatch
    nid_to_stub: HashMap<u32, u32>,
}

impl HleDispatcher {
    /// Create a new HLE dispatcher
    pub fn new() -> Self {
        Self {
            stub_map: HashMap::new(),
            next_stub_id: 0,
            stub_base: 0x2F00_0000,
            call_counts: HashMap::new(),
            nid_to_stub: HashMap::new(),
        }
    }

    /// Set the stub base address
    pub fn set_stub_base(&mut self, base: u32) {
        self.stub_base = base;
    }

    /// Register a stub address with an HLE function
    pub fn register_stub(&mut self, stub_addr: u32, info: HleFunctionInfo) {
        debug!("Registering HLE stub 0x{:08x} -> {}::{}", stub_addr, info.module, info.name);
        self.stub_map.insert(stub_addr, info);
    }

    /// Register a generic/unknown stub that just returns CELL_OK
    /// 
    /// This is used for dynamically discovered imports that don't have
    /// a specific HLE implementation. The stub will just return 0.
    pub fn register_generic_stub(&mut self, stub_addr: u32, desc_addr: u32) {
        // Use a generic handler that returns OK
        fn generic_stub_handler(_ctx: &HleCallContext) -> i64 {
            0 // CELL_OK
        }
        
        // We use static strings since the struct requires 'static lifetime
        debug!("Registering generic HLE stub 0x{:08x} for descriptor 0x{:08x}", stub_addr, desc_addr);
        self.stub_map.insert(stub_addr, HleFunctionInfo {
            name: "unknown_import",
            module: "unknown",
            handler: generic_stub_handler,
        });
    }

    /// Register a new HLE function and return its stub address
    pub fn register_function(&mut self, module: &'static str, name: &'static str, handler: HleFn) -> u32 {
        const STUB_SIZE: u32 = 16;
        let stub_addr = self.stub_base + self.next_stub_id * STUB_SIZE;
        self.next_stub_id += 1;

        self.register_stub(stub_addr, HleFunctionInfo {
            name,
            module,
            handler,
        });

        stub_addr
    }

    /// Check if an address is an HLE stub
    pub fn is_hle_stub(&self, addr: u32) -> bool {
        self.stub_map.contains_key(&addr)
    }

    /// Dispatch an HLE function call
    pub fn dispatch(&mut self, ctx: &HleCallContext) -> Option<i64> {
        if let Some(info) = self.stub_map.get(&ctx.stub_addr) {
            // Update call statistics
            *self.call_counts.entry(ctx.stub_addr).or_insert(0) += 1;

            trace!(
                "HLE call {}::{} (stub 0x{:08x}) args=[0x{:x}, 0x{:x}, 0x{:x}, 0x{:x}]",
                info.module, info.name, ctx.stub_addr,
                ctx.args[0], ctx.args[1], ctx.args[2], ctx.args[3]
            );

            let result = (info.handler)(ctx);

            trace!("HLE call {}::{} returned 0x{:x}", info.module, info.name, result);

            Some(result)
        } else {
            None
        }
    }

    /// Get function info for a stub address
    pub fn get_function_info(&self, stub_addr: u32) -> Option<&HleFunctionInfo> {
        self.stub_map.get(&stub_addr)
    }

    /// Get call count for a function
    pub fn get_call_count(&self, stub_addr: u32) -> u64 {
        *self.call_counts.get(&stub_addr).unwrap_or(&0)
    }

    /// Get iterator over all registered stubs
    pub fn iter_stubs(&self) -> impl Iterator<Item = (&u32, &HleFunctionInfo)> {
        self.stub_map.iter()
    }

    /// Get number of registered stubs
    pub fn stub_count(&self) -> usize {
        self.stub_map.len()
    }

    /// Register a NID → stub address mapping.
    ///
    /// Used by the loader when it discovers an import table entry and
    /// patches the PLT/GOT to point at one of the pre-registered HLE
    /// stubs.  Later, at runtime, the PPU interpreter can look up the
    /// NID to find the correct HLE handler.
    pub fn register_nid_stub(&mut self, nid: u32, stub_addr: u32) {
        debug!("Mapping NID 0x{:08x} -> stub 0x{:08x}", nid, stub_addr);
        self.nid_to_stub.insert(nid, stub_addr);
    }

    /// Look up a stub address by NID.
    ///
    /// Returns the stub address that was previously registered for this
    /// NID, if any.
    pub fn get_stub_for_nid(&self, nid: u32) -> Option<u32> {
        self.nid_to_stub.get(&nid).copied()
    }

    /// Get number of NID → stub mappings.
    pub fn nid_stub_count(&self) -> usize {
        self.nid_to_stub.len()
    }

    /// Reset the dispatcher
    pub fn reset(&mut self) {
        self.stub_map.clear();
        self.next_stub_id = 0;
        self.call_counts.clear();
        self.nid_to_stub.clear();
    }
}

impl Default for HleDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HLE Function Implementations
// ============================================================================

// --- cellSysutil ---

fn hle_sysutil_check_callback(_ctx: &HleCallContext) -> i64 {
    trace!("cellSysutilCheckCallback()");
    // Check and process any pending system callbacks
    // Returns number of callbacks processed (usually 0)
    error::CELL_OK
}

fn hle_sysutil_register_callback(ctx: &HleCallContext) -> i64 {
    let slot = ctx.args[0] as u32;
    let func = ctx.args[1] as u32;
    let userdata = ctx.args[2] as u32;
    
    debug!("cellSysutilRegisterCallback(slot={}, func=0x{:08x}, userdata=0x{:08x})", slot, func, userdata);
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.sysutil.register_callback(slot, func, userdata) as i64
}

fn hle_sysutil_unregister_callback(ctx: &HleCallContext) -> i64 {
    let slot = ctx.args[0] as u32;
    
    debug!("cellSysutilUnregisterCallback(slot={})", slot);
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.sysutil.unregister_callback(slot) as i64
}

fn hle_sysutil_get_system_param_int(ctx: &HleCallContext) -> i64 {
    let id = ctx.args[0] as u32;
    let value_ptr = ctx.args[1] as u32;
    
    debug!("cellSysutilGetSystemParamInt(id=0x{:04x}, value_ptr=0x{:08x})", id, value_ptr);
    
    // Return sensible defaults for common parameters
    let value: i32 = match id {
        0x0111 => 1,  // CELL_SYSUTIL_SYSTEMPARAM_ID_LANG - English
        0x0112 => 1,  // CELL_SYSUTIL_SYSTEMPARAM_ID_ENTER_BUTTON_ASSIGN - Cross
        0x0114 => 0,  // CELL_SYSUTIL_SYSTEMPARAM_ID_DATE_FORMAT - YYYYMMDD
        0x0115 => 0,  // CELL_SYSUTIL_SYSTEMPARAM_ID_TIME_FORMAT - 24h
        0x0116 => 0,  // CELL_SYSUTIL_SYSTEMPARAM_ID_TIMEZONE
        0x0117 => 0,  // CELL_SYSUTIL_SYSTEMPARAM_ID_SUMMERTIME
        0x0121 => 0,  // CELL_SYSUTIL_SYSTEMPARAM_ID_GAME_PARENTAL_LEVEL
        _ => 0,
    };
    
    // Write value to memory at value_ptr
    if value_ptr != 0 {
        if let Err(_) = write_be32(value_ptr, value as u32) {
            return error::CELL_EFAULT;
        }
    }
    
    trace!("  returning value={}", value);
    error::CELL_OK
}

// --- cellPad ---

fn hle_pad_init(ctx: &HleCallContext) -> i64 {
    let max_port = ctx.args[0] as u32;
    debug!("cellPadInit(max_port={})", max_port);
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.pad.init(max_port) as i64
}

fn hle_pad_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellPadEnd()");
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.pad.end() as i64
}

fn hle_pad_get_info(ctx: &HleCallContext) -> i64 {
    let info_ptr = ctx.args[0] as u32;
    trace!("cellPadGetInfo(info_ptr=0x{:08x})", info_ptr);
    
    // Write pad info structure indicating 1 controller connected
    // CellPadInfo: max_connect, now_connect, system_info, port_status[7], port_setting[7], device_capability[7], device_type[7]
    if info_ptr != 0 {
        // max_connect: 7 (max ports)
        if let Err(_) = write_be32(info_ptr, 7) { return error::CELL_EFAULT; }
        // now_connect: 1 (one controller connected)
        if let Err(_) = write_be32(info_ptr + 4, 1) { return error::CELL_EFAULT; }
        // system_info: 0
        if let Err(_) = write_be32(info_ptr + 8, 0) { return error::CELL_EFAULT; }
        // port_status[0]: CELL_PAD_STATUS_CONNECTED | CELL_PAD_STATUS_ASSIGN_CHANGES = 0x11
        if let Err(_) = write_be32(info_ptr + 12, 0x11) { return error::CELL_EFAULT; }
        // port_setting[0]: 0
        if let Err(_) = write_be32(info_ptr + 40, 0) { return error::CELL_EFAULT; }
    }
    
    error::CELL_OK
}

fn hle_pad_get_info2(ctx: &HleCallContext) -> i64 {
    let info_ptr = ctx.args[0] as u32;
    trace!("cellPadGetInfo2(info_ptr=0x{:08x})", info_ptr);

    if info_ptr == 0 {
        return error::CELL_OK;
    }

    // CellPadInfo2: max_connect(u32), now_connect(u32), system_info(u32),
    //   port_status[CELL_PAD_MAX_PORT_NUM], port_setting[CELL_PAD_MAX_PORT_NUM],
    //   device_capability[CELL_PAD_MAX_PORT_NUM], device_type[CELL_PAD_MAX_PORT_NUM]
    // max_connect: 7
    if let Err(_) = write_be32(info_ptr, 7) { return error::CELL_EFAULT; }
    // now_connect: 1 (one pad connected)
    if let Err(_) = write_be32(info_ptr + 4, 1) { return error::CELL_EFAULT; }
    // system_info: 0
    if let Err(_) = write_be32(info_ptr + 8, 0) { return error::CELL_EFAULT; }
    // port_status[0]: CELL_PAD_STATUS_CONNECTED | CELL_PAD_STATUS_ASSIGN_CHANGES = 0x11
    if let Err(_) = write_be32(info_ptr + 12, 0x11) { return error::CELL_EFAULT; }
    // Zero remaining port_status[1..7]
    for i in 1u32..7 {
        if let Err(_) = write_be32(info_ptr + 12 + i * 4, 0) { return error::CELL_EFAULT; }
    }
    // port_setting[0]: 0 (no press/sensor mode)
    if let Err(_) = write_be32(info_ptr + 40, 0) { return error::CELL_EFAULT; }
    // Zero remaining port_setting[1..7]
    for i in 1u32..7 {
        if let Err(_) = write_be32(info_ptr + 40 + i * 4, 0) { return error::CELL_EFAULT; }
    }
    // device_capability[0]: 0x3 (digital + analog)
    if let Err(_) = write_be32(info_ptr + 68, 0x3) { return error::CELL_EFAULT; }
    // Zero remaining device_capability[1..7]
    for i in 1u32..7 {
        if let Err(_) = write_be32(info_ptr + 68 + i * 4, 0) { return error::CELL_EFAULT; }
    }
    // device_type[0]: 0 (standard controller)
    if let Err(_) = write_be32(info_ptr + 96, 0) { return error::CELL_EFAULT; }
    // Zero remaining device_type[1..7]
    for i in 1u32..7 {
        if let Err(_) = write_be32(info_ptr + 96 + i * 4, 0) { return error::CELL_EFAULT; }
    }

    error::CELL_OK
}

fn hle_pad_get_data(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let data_ptr = ctx.args[1] as u32;
    trace!("cellPadGetData(port={}, data_ptr=0x{:08x})", port, data_ptr);
    
    // Write empty pad data (no buttons pressed)
    // CellPadData: len, reserved[6], button[24]
    if data_ptr != 0 {
        // len: 24 (number of buttons in data)
        if let Err(_) = write_be32(data_ptr, 24) { return error::CELL_EFAULT; }
        // button[0]: digital buttons 1 (D-pad) = all released (0xFF)
        if let Err(_) = write_be32(data_ptr + 28, 0x00FF) { return error::CELL_EFAULT; }
        // button[1]: digital buttons 2 (face/shoulder) = all released (0xFF)
        if let Err(_) = write_be32(data_ptr + 32, 0x00FF) { return error::CELL_EFAULT; }
        // button[2]: analog right stick X = centered (0x80)
        if let Err(_) = write_be32(data_ptr + 36, 0x0080) { return error::CELL_EFAULT; }
        // button[3]: analog right stick Y = centered (0x80)
        if let Err(_) = write_be32(data_ptr + 40, 0x0080) { return error::CELL_EFAULT; }
        // button[4]: analog left stick X = centered (0x80)
        if let Err(_) = write_be32(data_ptr + 44, 0x0080) { return error::CELL_EFAULT; }
        // button[5]: analog left stick Y = centered (0x80)
        if let Err(_) = write_be32(data_ptr + 48, 0x0080) { return error::CELL_EFAULT; }
    }
    
    error::CELL_OK
}

fn hle_pad_set_press_mode(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let mode = ctx.args[1] as u32;
    debug!("cellPadSetPressMode(port={}, mode={})", port, mode);
    error::CELL_OK
}

fn hle_pad_set_sensor_mode(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let mode = ctx.args[1] as u32;
    debug!("cellPadSetSensorMode(port={}, mode={})", port, mode);
    error::CELL_OK
}

// --- cellGcmSys ---

fn hle_gcm_init(ctx: &HleCallContext) -> i64 {
    let context_addr = ctx.args[0] as u32;
    let cmd_size = ctx.args[1] as u32;
    let io_size = ctx.args[2] as u32;
    let io_addr = ctx.args[3] as u32;
    
    info!(
        "cellGcmInit(context=0x{:08x}, cmd_size=0x{:x}, io_size=0x{:x}, io_addr=0x{:08x})",
        context_addr, cmd_size, io_size, io_addr
    );
    
    let mut hle_ctx = get_hle_context_mut();
    // Note: GcmManager::init takes context_addr and context_size (using cmd_size as context_size)
    hle_ctx.gcm.init(context_addr, cmd_size) as i64
}

fn hle_gcm_get_configuration(ctx: &HleCallContext) -> i64 {
    let config_ptr = ctx.args[0] as u32;
    debug!("cellGcmGetConfiguration(config_ptr=0x{:08x})", config_ptr);
    
    // Write CellGcmConfig structure to memory
    // Structure: localAddress, ioSize, memoryFrequency, coreFrequency, localSize, ioAddress
    if config_ptr != 0 {
        let hle_ctx = get_hle_context_mut();
        let config = hle_ctx.gcm.get_configuration();
        
        // localAddress (offset 0)
        if let Err(_) = write_be32(config_ptr, config.local_addr) { return error::CELL_EFAULT; }
        // localSize (offset 4)
        if let Err(_) = write_be32(config_ptr + 4, config.local_size) { return error::CELL_EFAULT; }
        // ioAddress (offset 8)
        if let Err(_) = write_be32(config_ptr + 8, config.io_addr) { return error::CELL_EFAULT; }
        // ioSize (offset 12)
        if let Err(_) = write_be32(config_ptr + 12, config.io_size) { return error::CELL_EFAULT; }
        // memoryFrequency (offset 16)
        if let Err(_) = write_be32(config_ptr + 16, config.mem_frequency) { return error::CELL_EFAULT; }
        // coreFrequency (offset 20)
        if let Err(_) = write_be32(config_ptr + 20, config.core_frequency) { return error::CELL_EFAULT; }
    }
    
    error::CELL_OK
}

fn hle_gcm_set_flip_mode(ctx: &HleCallContext) -> i64 {
    let mode = ctx.args[0] as u32;
    debug!("cellGcmSetFlipMode(mode={})", mode);
    
    let mut hle_ctx = get_hle_context_mut();
    // Convert u32 to CellGcmFlipMode (1=Vsync, 2=Hsync)
    let flip_mode = if mode == 2 {
        crate::cell_gcm_sys::CellGcmFlipMode::Hsync
    } else {
        crate::cell_gcm_sys::CellGcmFlipMode::Vsync
    };
    hle_ctx.gcm.set_flip_mode(flip_mode) as i64
}

fn hle_gcm_get_tiled_pitch_size(ctx: &HleCallContext) -> i64 {
    let size = ctx.args[0] as u32;
    trace!("cellGcmGetTiledPitchSize(size={})", size);
    
    // Return aligned pitch size
    let aligned = (size + 0xFF) & !0xFF;
    aligned as i64
}

fn hle_gcm_set_display_buffer(ctx: &HleCallContext) -> i64 {
    let buffer_id = ctx.args[0] as u32;
    let offset = ctx.args[1] as u32;
    let pitch = ctx.args[2] as u32;
    let width = ctx.args[3] as u32;
    let height = ctx.args[4] as u32;
    
    debug!(
        "cellGcmSetDisplayBuffer(id={}, offset=0x{:x}, pitch={}, {}x{})",
        buffer_id, offset, pitch, width, height
    );
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.gcm.set_display_buffer(buffer_id, offset, pitch, width, height) as i64
}

fn hle_gcm_get_ctrl(_ctx: &HleCallContext) -> i64 {
    trace!("cellGcmGetControlRegister()");
    
    // Return pointer to GCM control register structure
    // This is typically mapped at a fixed RSX memory address
    // The control register contains put/get pointers for command buffer
    0xC010_0000u32 as i64
}

fn hle_gcm_get_label_address(ctx: &HleCallContext) -> i64 {
    let index = ctx.args[0] as u32;
    trace!("cellGcmGetLabelAddress(index={})", index);
    
    // Return a label address (used for GPU synchronization)
    // Labels are typically at a fixed offset in RSX memory
    let label_base = 0xC000_0000u32; // RSX memory base
    (label_base + index * 16) as i64
}

fn hle_gcm_set_wait_flip(_ctx: &HleCallContext) -> i64 {
    trace!("cellGcmSetWaitFlip()");
    error::CELL_OK
}

fn hle_gcm_reset_flip_status(_ctx: &HleCallContext) -> i64 {
    trace!("cellGcmResetFlipStatus()");
    error::CELL_OK
}

fn hle_gcm_get_flip_status(_ctx: &HleCallContext) -> i64 {
    trace!("cellGcmGetFlipStatus()");
    0 // Not flipping
}

// --- cellFs ---

fn hle_fs_open(ctx: &HleCallContext) -> i64 {
    let path_ptr = ctx.args[0] as u32;
    let flags = ctx.args[1] as u32;
    let fd_ptr = ctx.args[2] as u32;
    let mode = ctx.args[3] as u32;
    let _arg = ctx.args[4] as u32;
    
    // Read path string from guest memory
    let path = match crate::memory::read_string(path_ptr, 1024) {
        Ok(p) => p,
        Err(_) => {
            debug!("cellFsOpen: failed to read path from 0x{:08x}", path_ptr);
            return error::CELL_EFAULT;
        }
    };
    
    debug!("cellFsOpen(path='{}', flags=0x{:x}, fd_ptr=0x{:08x}, mode=0x{:x})", path, flags, fd_ptr, mode);
    
    let mut hle_ctx = get_hle_context_mut();
    match hle_ctx.fs.open(&path, flags, mode) {
        Ok(fd) => {
            // Write fd to the output pointer
            if fd_ptr != 0 {
                if let Err(_) = write_be32(fd_ptr, fd as u32) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_close(ctx: &HleCallContext) -> i64 {
    let fd = ctx.args[0] as i32;
    debug!("cellFsClose(fd={})", fd);
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.fs.close(fd) as i64
}

fn hle_fs_read(ctx: &HleCallContext) -> i64 {
    let fd = ctx.args[0] as i32;
    let buf = ctx.args[1] as u32;
    let size = ctx.args[2] as u64;
    let nread_ptr = ctx.args[3] as u32;
    
    debug!("cellFsRead(fd={}, buf=0x{:08x}, size={}, nread_ptr=0x{:08x})", fd, buf, size, nread_ptr);
    
    // Allocate a temporary buffer to read into
    let read_size = size.min(FS_MAX_TRANSFER_SIZE) as usize;
    let mut temp_buf = vec![0u8; read_size];
    
    let mut hle_ctx = get_hle_context_mut();
    match hle_ctx.fs.read(fd, &mut temp_buf) {
        Ok(bytes_read) => {
            // Write data to guest memory
            if bytes_read > 0 {
                if let Err(_) = crate::memory::write_bytes(buf, &temp_buf[..bytes_read as usize]) {
                    return error::CELL_EFAULT;
                }
            }
            // Write bytes read count
            if nread_ptr != 0 {
                if let Err(_) = crate::memory::write_be64(nread_ptr, bytes_read) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_write(ctx: &HleCallContext) -> i64 {
    let fd = ctx.args[0] as i32;
    let buf = ctx.args[1] as u32;
    let size = ctx.args[2] as u64;
    let nwrite_ptr = ctx.args[3] as u32;
    
    debug!("cellFsWrite(fd={}, buf=0x{:08x}, size={}, nwrite_ptr=0x{:08x})", fd, buf, size, nwrite_ptr);
    
    // Read data from guest memory
    let write_size = size.min(FS_MAX_TRANSFER_SIZE) as u32;
    let data = match crate::memory::read_bytes(buf, write_size) {
        Ok(d) => d,
        Err(_) => return error::CELL_EFAULT,
    };
    
    let mut hle_ctx = get_hle_context_mut();
    match hle_ctx.fs.write(fd, &data) {
        Ok(bytes_written) => {
            if nwrite_ptr != 0 {
                if let Err(_) = crate::memory::write_be64(nwrite_ptr, bytes_written) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_stat(ctx: &HleCallContext) -> i64 {
    let path_ptr = ctx.args[0] as u32;
    let stat_ptr = ctx.args[1] as u32;
    
    // Read path string from guest memory
    let path = match crate::memory::read_string(path_ptr, 1024) {
        Ok(p) => p,
        Err(_) => {
            debug!("cellFsStat: failed to read path from 0x{:08x}", path_ptr);
            return error::CELL_EFAULT;
        }
    };
    
    debug!("cellFsStat(path='{}', stat_ptr=0x{:08x})", path, stat_ptr);
    
    let hle_ctx = crate::context::get_hle_context_mut();
    match hle_ctx.fs.stat(&path) {
        Ok(stat) => {
            if stat_ptr != 0 {
                // Write CellFsStat structure to guest memory
                // mode(u32), uid(u32), gid(u32), atime(u64), mtime(u64), ctime(u64), size(u64), blksize(u64)
                if let Err(_) = write_be32(stat_ptr, stat.mode) { return error::CELL_EFAULT; }
                if let Err(_) = write_be32(stat_ptr + 4, stat.uid) { return error::CELL_EFAULT; }
                if let Err(_) = write_be32(stat_ptr + 8, stat.gid) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 16, stat.atime) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 24, stat.mtime) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 32, stat.ctime) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 40, stat.size) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 48, stat.blksize) { return error::CELL_EFAULT; }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_fstat(ctx: &HleCallContext) -> i64 {
    let fd = ctx.args[0] as i32;
    let stat_ptr = ctx.args[1] as u32;
    
    debug!("cellFsFstat(fd={}, stat_ptr=0x{:08x})", fd, stat_ptr);
    
    let hle_ctx = crate::context::get_hle_context_mut();
    match hle_ctx.fs.fstat(fd) {
        Ok(stat) => {
            if stat_ptr != 0 {
                // Write CellFsStat structure to guest memory
                if let Err(_) = write_be32(stat_ptr, stat.mode) { return error::CELL_EFAULT; }
                if let Err(_) = write_be32(stat_ptr + 4, stat.uid) { return error::CELL_EFAULT; }
                if let Err(_) = write_be32(stat_ptr + 8, stat.gid) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 16, stat.atime) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 24, stat.mtime) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 32, stat.ctime) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 40, stat.size) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_be64(stat_ptr + 48, stat.blksize) { return error::CELL_EFAULT; }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_opendir(ctx: &HleCallContext) -> i64 {
    let path_ptr = ctx.args[0] as u32;
    let fd_ptr = ctx.args[1] as u32;
    
    let path = match crate::memory::read_string(path_ptr, 1024) {
        Ok(p) => p,
        Err(_) => {
            debug!("cellFsOpendir: failed to read path from 0x{:08x}", path_ptr);
            return error::CELL_EFAULT;
        }
    };
    
    debug!("cellFsOpendir(path='{}', fd_ptr=0x{:08x})", path, fd_ptr);
    
    let mut hle_ctx = get_hle_context_mut();
    match hle_ctx.fs.opendir(&path) {
        Ok(fd) => {
            if fd_ptr != 0 {
                if let Err(_) = write_be32(fd_ptr, fd as u32) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_readdir(ctx: &HleCallContext) -> i64 {
    let fd = ctx.args[0] as i32;
    let dirent_ptr = ctx.args[1] as u32;
    
    debug!("cellFsReaddir(fd={}, dirent_ptr=0x{:08x})", fd, dirent_ptr);
    
    let mut hle_ctx = get_hle_context_mut();
    match hle_ctx.fs.readdir(fd) {
        Ok(Some(dirent)) => {
            if dirent_ptr != 0 {
                // Write CellFsDirent: d_type(u8), d_namlen(u8), d_name[256]
                if let Err(_) = crate::memory::write_u8(dirent_ptr, dirent.d_type) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_u8(dirent_ptr + 1, dirent.d_namlen) { return error::CELL_EFAULT; }
                // Write name bytes
                let name_len = dirent.d_namlen as usize;
                if name_len > 0 {
                    if let Err(_) = crate::memory::write_bytes(dirent_ptr + 2, &dirent.d_name[..name_len]) {
                        return error::CELL_EFAULT;
                    }
                }
                // Null-terminate the name
                if let Err(_) = crate::memory::write_u8(dirent_ptr + 2 + name_len as u32, 0) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Ok(None) => {
            // End of directory — write zero-length entry
            if dirent_ptr != 0 {
                if let Err(_) = crate::memory::write_u8(dirent_ptr, 0) { return error::CELL_EFAULT; }
                if let Err(_) = crate::memory::write_u8(dirent_ptr + 1, 0) { return error::CELL_EFAULT; }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_fs_lseek(ctx: &HleCallContext) -> i64 {
    let fd = ctx.args[0] as i32;
    let offset = ctx.args[1] as i64;
    let whence = ctx.args[2] as u32;
    let pos_ptr = ctx.args[3] as u32;
    
    debug!("cellFsLseek(fd={}, offset={}, whence={}, pos_ptr=0x{:08x})", fd, offset, whence, pos_ptr);
    
    let mut hle_ctx = get_hle_context_mut();
    match hle_ctx.fs.lseek(fd, offset, whence) {
        Ok(new_pos) => {
            if pos_ptr != 0 {
                if let Err(_) = crate::memory::write_be64(pos_ptr, new_pos) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

// --- cellAudio ---

fn hle_audio_init(_ctx: &HleCallContext) -> i64 {
    info!("cellAudioInit()");
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.audio.init();
    error::CELL_OK
}

fn hle_audio_quit(_ctx: &HleCallContext) -> i64 {
    info!("cellAudioQuit()");
    
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.audio.quit();
    error::CELL_OK
}

fn hle_audio_port_open(ctx: &HleCallContext) -> i64 {
    let config_ptr = ctx.args[0] as u32;
    let port_num_ptr = ctx.args[1] as u32;
    
    debug!("cellAudioPortOpen(config=0x{:08x}, port_num_ptr=0x{:08x})", config_ptr, port_num_ptr);
    
    // Create audio port with default config and return port number
    let mut hle_ctx = get_hle_context_mut();
    
    // Open port with default parameters: 2 channels, 8 blocks, no special attributes, 1.0 volume
    let port_result = hle_ctx.audio.port_open(2, 8, 0, 1.0);
    
    match port_result {
        Ok(port_num) => {
            // Write port number to output pointer
            if port_num_ptr != 0 {
                if let Err(_) = write_be32(port_num_ptr, port_num) {
                    return error::CELL_EFAULT;
                }
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_audio_port_close(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    debug!("cellAudioPortClose(port={})", port_num);
    error::CELL_OK
}

fn hle_audio_port_start(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    debug!("cellAudioPortStart(port={})", port_num);
    error::CELL_OK
}

fn hle_audio_port_stop(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    debug!("cellAudioPortStop(port={})", port_num);
    error::CELL_OK
}

fn hle_audio_get_port_config(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    let config_ptr = ctx.args[1] as u32;
    
    debug!("cellAudioGetPortConfig(port={}, config_ptr=0x{:08x})", port_num, config_ptr);
    error::CELL_OK
}

// --- cellGame ---

fn hle_game_boot_check(ctx: &HleCallContext) -> i64 {
    let type_ptr = ctx.args[0] as u32;
    let attr_ptr = ctx.args[1] as u32;
    let size_ptr = ctx.args[2] as u32;
    let dir_name_ptr = ctx.args[3] as u32;
    
    debug!(
        "cellGameBootCheck(type=0x{:08x}, attr=0x{:08x}, size=0x{:08x}, dir=0x{:08x})",
        type_ptr, attr_ptr, size_ptr, dir_name_ptr
    );
    
    // Initialize the game manager
    let mut hle_ctx = get_hle_context_mut();
    hle_ctx.game.boot_check();
    drop(hle_ctx);
    
    // Write game type (disc game = 1)
    if type_ptr != 0 {
        if let Err(_) = write_be32(type_ptr, 1) { return error::CELL_EFAULT; }
    }
    
    // Write attributes (0 = normal)
    if attr_ptr != 0 {
        if let Err(_) = write_be32(attr_ptr, 0) { return error::CELL_EFAULT; }
    }
    
    // Write CellGameContentSize structure (hddFreeSizeKB, sizeKB, sysSizeKB)
    if size_ptr != 0 {
        // hddFreeSizeKB: 10GB free space
        if let Err(_) = write_be32(size_ptr, 10 * 1024 * 1024) { return error::CELL_EFAULT; }
        // sizeKB: current game data size = 0
        if let Err(_) = write_be32(size_ptr + 4, 0) { return error::CELL_EFAULT; }
        // sysSizeKB: system size = 0
        if let Err(_) = write_be32(size_ptr + 8, 0) { return error::CELL_EFAULT; }
    }
    
    // Write directory name — CELL_GAME_DIRNAME_SIZE is 32 bytes
    if dir_name_ptr != 0 {
        if let Err(_) = crate::memory::write_string(dir_name_ptr, "GAME00000", 32) {
            return error::CELL_EFAULT;
        }
    }
    
    error::CELL_OK
}

fn hle_game_data_check(ctx: &HleCallContext) -> i64 {
    let type_val = ctx.args[0] as u32;
    let dir_name_ptr = ctx.args[1] as u32;
    let size_ptr = ctx.args[2] as u32;
    
    debug!(
        "cellGameDataCheck(type={}, dir_name=0x{:08x}, size_ptr=0x{:08x})",
        type_val, dir_name_ptr, size_ptr
    );
    error::CELL_OK
}

fn hle_game_content_error_dialog(_ctx: &HleCallContext) -> i64 {
    debug!("cellGameContentErrorDialog()");
    error::CELL_OK
}

fn hle_game_get_param_int(ctx: &HleCallContext) -> i64 {
    let id = ctx.args[0] as u32;
    let value_ptr = ctx.args[1] as u32;
    
    debug!("cellGameGetParamInt(id={}, value_ptr=0x{:08x})", id, value_ptr);
    
    let hle_ctx = crate::context::get_hle_context_mut();
    let value = hle_ctx.game.get_param_int(id).unwrap_or(0);
    drop(hle_ctx);
    
    if value_ptr != 0 {
        if let Err(_) = write_be32(value_ptr, value as u32) {
            return error::CELL_EFAULT;
        }
    }
    
    error::CELL_OK
}

fn hle_game_get_param_string(ctx: &HleCallContext) -> i64 {
    let id = ctx.args[0] as u32;
    let buf_ptr = ctx.args[1] as u32;
    let buf_size = ctx.args[2] as u32;
    
    debug!("cellGameGetParamString(id={}, buf=0x{:08x}, size={})", id, buf_ptr, buf_size);
    
    let hle_ctx = crate::context::get_hle_context_mut();
    let value = hle_ctx.game.get_param_string(id)
        .unwrap_or("")
        .to_string();
    drop(hle_ctx);
    
    if buf_ptr != 0 && buf_size > 0 {
        if let Err(_) = crate::memory::write_string(buf_ptr, &value, buf_size) {
            return error::CELL_EFAULT;
        }
    }
    
    error::CELL_OK
}

fn hle_game_content_permit(ctx: &HleCallContext) -> i64 {
    let content_info_path_ptr = ctx.args[0] as u32;
    let usrdir_path_ptr = ctx.args[1] as u32;
    
    debug!(
        "cellGameContentPermit(contentInfoPath=0x{:08x}, usrdirPath=0x{:08x})",
        content_info_path_ptr, usrdir_path_ptr
    );
    
    // Ensure game manager is initialized
    let mut hle_ctx = get_hle_context_mut();
    if !hle_ctx.game.is_initialized() {
        hle_ctx.game.boot_check();
    }
    
    let content_info_path = hle_ctx.game.get_content_info_path().to_string();
    let usrdir_path = hle_ctx.game.get_usrdir_path().to_string();
    drop(hle_ctx);
    
    // Write content info path (e.g., "/dev_hdd0/game/GAME00000")
    if content_info_path_ptr != 0 {
        if let Err(_) = crate::memory::write_string(content_info_path_ptr, &content_info_path, 256) {
            return error::CELL_EFAULT;
        }
    }
    
    // Write USRDIR path (e.g., "/dev_hdd0/game/GAME00000/USRDIR")
    if usrdir_path_ptr != 0 {
        if let Err(_) = crate::memory::write_string(usrdir_path_ptr, &usrdir_path, 256) {
            return error::CELL_EFAULT;
        }
    }
    
    error::CELL_OK
}

// --- cellVideoOut ---

fn hle_video_out_get_state(ctx: &HleCallContext) -> i64 {
    let video_out = ctx.args[0] as u32;
    let device_index = ctx.args[1] as u32;
    let state_ptr = ctx.args[2] as u32;
    
    debug!(
        "cellVideoOutGetState(videoOut={}, deviceIndex={}, state_ptr=0x{:08x})",
        video_out, device_index, state_ptr
    );
    
    crate::cell_sysutil::cell_video_out_get_state(video_out, device_index, state_ptr) as i64
}

fn hle_video_out_configure(ctx: &HleCallContext) -> i64 {
    let video_out = ctx.args[0] as u32;
    let config_ptr = ctx.args[1] as u32;
    let option_ptr = ctx.args[2] as u32;
    let wait = ctx.args[3] as u32;
    
    debug!(
        "cellVideoOutConfigure(videoOut={}, config=0x{:08x}, option=0x{:08x}, wait={})",
        video_out, config_ptr, option_ptr, wait
    );
    
    crate::cell_sysutil::cell_video_out_configure(video_out, config_ptr, option_ptr, wait) as i64
}

fn hle_video_out_get_configuration(ctx: &HleCallContext) -> i64 {
    let video_out = ctx.args[0] as u32;
    let config_ptr = ctx.args[1] as u32;
    let option_ptr = ctx.args[2] as u32;
    
    debug!(
        "cellVideoOutGetConfiguration(videoOut={}, config=0x{:08x}, option=0x{:08x})",
        video_out, config_ptr, option_ptr
    );
    
    crate::cell_sysutil::cell_video_out_get_configuration(video_out, config_ptr, option_ptr) as i64
}

fn hle_video_out_get_resolution_availability(ctx: &HleCallContext) -> i64 {
    let video_out = ctx.args[0] as u32;
    let resolution_id = ctx.args[1] as u32;
    let aspect = ctx.args[2] as u32;
    let _option = ctx.args[3] as u32;
    
    trace!(
        "cellVideoOutGetResolutionAvailability(videoOut={}, resId={}, aspect={})",
        video_out, resolution_id, aspect
    );
    
    // All resolutions are available in our emulator
    // Returns 1 (available) for any valid resolution
    if video_out == 0 && resolution_id >= 1 && resolution_id <= 7 {
        1 // Available
    } else {
        0 // Not available
    }
}

// --- cellResc ---

fn hle_resc_init(ctx: &HleCallContext) -> i64 {
    let config_ptr = ctx.args[0] as u32;
    info!("cellRescInit(config_ptr=0x{:08x})", config_ptr);
    error::CELL_OK
}

fn hle_resc_exit(_ctx: &HleCallContext) -> i64 {
    info!("cellRescExit()");
    error::CELL_OK
}

fn hle_resc_set_display_mode(ctx: &HleCallContext) -> i64 {
    let mode = ctx.args[0] as u32;
    debug!("cellRescSetDisplayMode(mode={})", mode);
    error::CELL_OK
}

fn hle_resc_set_src(ctx: &HleCallContext) -> i64 {
    let idx = ctx.args[0] as u32;
    let src_ptr = ctx.args[1] as u32;
    debug!("cellRescSetSrc(idx={}, src_ptr=0x{:08x})", idx, src_ptr);
    error::CELL_OK
}

// --- cellSpurs ---

fn hle_spurs_initialize(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let num_spu = ctx.args[1] as u32;
    let spu_priority = ctx.args[2] as u32;
    let ppu_priority = ctx.args[3] as u32;
    let exit_if_no_work = ctx.args[4] != 0;
    
    info!(
        "cellSpursInitialize(spurs=0x{:08x}, num_spu={}, spu_pri={}, ppu_pri={}, exit_if_no_work={})",
        spurs_ptr, num_spu, spu_priority, ppu_priority, exit_if_no_work
    );
    
    // SPURS initialization is complex - just return success for now
    error::CELL_OK
}

fn hle_spurs_finalize(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    info!("cellSpursFinalize(spurs=0x{:08x})", spurs_ptr);
    error::CELL_OK
}

fn hle_spurs_attach_lv2_event_queue(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    debug!("cellSpursAttachLv2EventQueue(spurs=0x{:08x})", spurs_ptr);
    error::CELL_OK
}

// --- Generic stub ---

#[allow(dead_code)]
fn hle_stub_return_ok(_ctx: &HleCallContext) -> i64 {
    error::CELL_OK
}

// ============================================================================
// Registration
// ============================================================================

/// Register all known HLE functions
pub fn register_all_hle_functions(dispatcher: &mut HleDispatcher) {
    info!("Registering HLE functions...");
    
    // cellSysutil
    dispatcher.register_function("cellSysutil", "cellSysutilCheckCallback", hle_sysutil_check_callback);
    dispatcher.register_function("cellSysutil", "cellSysutilRegisterCallback", hle_sysutil_register_callback);
    dispatcher.register_function("cellSysutil", "cellSysutilUnregisterCallback", hle_sysutil_unregister_callback);
    dispatcher.register_function("cellSysutil", "cellSysutilGetSystemParamInt", hle_sysutil_get_system_param_int);
    
    // cellVideoOut (part of cellSysutil module)
    dispatcher.register_function("cellSysutil", "cellVideoOutGetState", hle_video_out_get_state);
    dispatcher.register_function("cellSysutil", "cellVideoOutConfigure", hle_video_out_configure);
    dispatcher.register_function("cellSysutil", "cellVideoOutGetConfiguration", hle_video_out_get_configuration);
    dispatcher.register_function("cellSysutil", "cellVideoOutGetResolutionAvailability", hle_video_out_get_resolution_availability);
    
    // cellPad
    dispatcher.register_function("cellPad", "cellPadInit", hle_pad_init);
    dispatcher.register_function("cellPad", "cellPadEnd", hle_pad_end);
    dispatcher.register_function("cellPad", "cellPadGetInfo", hle_pad_get_info);
    dispatcher.register_function("cellPad", "cellPadGetInfo2", hle_pad_get_info2);
    dispatcher.register_function("cellPad", "cellPadGetData", hle_pad_get_data);
    dispatcher.register_function("cellPad", "cellPadSetPressMode", hle_pad_set_press_mode);
    dispatcher.register_function("cellPad", "cellPadSetSensorMode", hle_pad_set_sensor_mode);
    
    // cellGcmSys
    dispatcher.register_function("cellGcmSys", "cellGcmInit", hle_gcm_init);
    dispatcher.register_function("cellGcmSys", "cellGcmGetConfiguration", hle_gcm_get_configuration);
    dispatcher.register_function("cellGcmSys", "cellGcmSetFlipMode", hle_gcm_set_flip_mode);
    dispatcher.register_function("cellGcmSys", "cellGcmGetTiledPitchSize", hle_gcm_get_tiled_pitch_size);
    dispatcher.register_function("cellGcmSys", "cellGcmSetDisplayBuffer", hle_gcm_set_display_buffer);
    dispatcher.register_function("cellGcmSys", "cellGcmGetControlRegister", hle_gcm_get_ctrl);
    dispatcher.register_function("cellGcmSys", "cellGcmGetLabelAddress", hle_gcm_get_label_address);
    dispatcher.register_function("cellGcmSys", "cellGcmSetWaitFlip", hle_gcm_set_wait_flip);
    dispatcher.register_function("cellGcmSys", "cellGcmResetFlipStatus", hle_gcm_reset_flip_status);
    dispatcher.register_function("cellGcmSys", "cellGcmGetFlipStatus", hle_gcm_get_flip_status);
    
    // cellFs
    dispatcher.register_function("cellFs", "cellFsOpen", hle_fs_open);
    dispatcher.register_function("cellFs", "cellFsClose", hle_fs_close);
    dispatcher.register_function("cellFs", "cellFsRead", hle_fs_read);
    dispatcher.register_function("cellFs", "cellFsWrite", hle_fs_write);
    dispatcher.register_function("cellFs", "cellFsStat", hle_fs_stat);
    dispatcher.register_function("cellFs", "cellFsFstat", hle_fs_fstat);
    dispatcher.register_function("cellFs", "cellFsOpendir", hle_fs_opendir);
    dispatcher.register_function("cellFs", "cellFsReaddir", hle_fs_readdir);
    dispatcher.register_function("cellFs", "cellFsLseek64", hle_fs_lseek);
    
    // cellAudio
    dispatcher.register_function("cellAudio", "cellAudioInit", hle_audio_init);
    dispatcher.register_function("cellAudio", "cellAudioQuit", hle_audio_quit);
    dispatcher.register_function("cellAudio", "cellAudioPortOpen", hle_audio_port_open);
    dispatcher.register_function("cellAudio", "cellAudioPortClose", hle_audio_port_close);
    dispatcher.register_function("cellAudio", "cellAudioPortStart", hle_audio_port_start);
    dispatcher.register_function("cellAudio", "cellAudioPortStop", hle_audio_port_stop);
    dispatcher.register_function("cellAudio", "cellAudioGetPortConfig", hle_audio_get_port_config);
    
    // cellGame
    dispatcher.register_function("cellGame", "cellGameBootCheck", hle_game_boot_check);
    dispatcher.register_function("cellGame", "cellGameDataCheck", hle_game_data_check);
    dispatcher.register_function("cellGame", "cellGameContentPermit", hle_game_content_permit);
    dispatcher.register_function("cellGame", "cellGameContentErrorDialog", hle_game_content_error_dialog);
    dispatcher.register_function("cellGame", "cellGameGetParamInt", hle_game_get_param_int);
    dispatcher.register_function("cellGame", "cellGameGetParamString", hle_game_get_param_string);
    
    // cellResc
    dispatcher.register_function("cellResc", "cellRescInit", hle_resc_init);
    dispatcher.register_function("cellResc", "cellRescExit", hle_resc_exit);
    dispatcher.register_function("cellResc", "cellRescSetDisplayMode", hle_resc_set_display_mode);
    dispatcher.register_function("cellResc", "cellRescSetSrc", hle_resc_set_src);
    
    // cellSpurs
    dispatcher.register_function("cellSpurs", "cellSpursInitialize", hle_spurs_initialize);
    dispatcher.register_function("cellSpurs", "cellSpursFinalize", hle_spurs_finalize);
    dispatcher.register_function("cellSpurs", "cellSpursAttachLv2EventQueue", hle_spurs_attach_lv2_event_queue);
    
    info!("Registered {} HLE functions", dispatcher.stub_map.len());
}

/// Get mutable access to the global HLE dispatcher
pub fn get_dispatcher_mut() -> std::sync::RwLockWriteGuard<'static, HleDispatcher> {
    HLE_DISPATCHER.write().expect("Failed to acquire HLE dispatcher lock")
}

/// Get read access to the global HLE dispatcher
pub fn get_dispatcher() -> std::sync::RwLockReadGuard<'static, HleDispatcher> {
    HLE_DISPATCHER.read().expect("Failed to acquire HLE dispatcher lock")
}

/// Initialize the HLE dispatcher with all functions
pub fn init_hle_dispatcher() {
    let mut dispatcher = get_dispatcher_mut();
    register_all_hle_functions(&mut dispatcher);
}

/// Dispatch an HLE call (convenience wrapper)
pub fn dispatch_hle_call(stub_addr: u32, args: &[u64; 8], toc: u64, lr: u64) -> Option<i64> {
    let ctx = HleCallContext {
        stub_addr,
        args: *args,
        toc,
        lr,
    };
    
    let mut dispatcher = get_dispatcher_mut();
    dispatcher.dispatch(&ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_creation() {
        let dispatcher = HleDispatcher::new();
        assert!(!dispatcher.is_hle_stub(0x2F000000));
    }

    #[test]
    fn test_function_registration() {
        let mut dispatcher = HleDispatcher::new();
        
        let addr = dispatcher.register_function("test", "test_func", hle_stub_return_ok);
        assert_eq!(addr, 0x2F000000);
        assert!(dispatcher.is_hle_stub(addr));
        
        let info = dispatcher.get_function_info(addr);
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "test_func");
    }

    #[test]
    fn test_dispatch() {
        let mut dispatcher = HleDispatcher::new();
        let addr = dispatcher.register_function("test", "test_func", hle_stub_return_ok);
        
        let ctx = HleCallContext {
            stub_addr: addr,
            args: [0; 8],
            toc: 0,
            lr: 0,
        };
        
        let result = dispatcher.dispatch(&ctx);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_nid_stub_registration() {
        let mut dispatcher = HleDispatcher::new();
        let addr = dispatcher.register_function("cellFs", "cellFsOpen", hle_stub_return_ok);

        // No NID mapping yet
        assert_eq!(dispatcher.nid_stub_count(), 0);
        assert!(dispatcher.get_stub_for_nid(0xB27C8AE7).is_none());

        // Register NID → stub
        dispatcher.register_nid_stub(0xB27C8AE7, addr);
        assert_eq!(dispatcher.nid_stub_count(), 1);
        assert_eq!(dispatcher.get_stub_for_nid(0xB27C8AE7), Some(addr));
    }

    #[test]
    fn test_nid_stub_reset() {
        let mut dispatcher = HleDispatcher::new();
        dispatcher.register_nid_stub(0x12345678, 0x2F000000);
        assert_eq!(dispatcher.nid_stub_count(), 1);
        dispatcher.reset();
        assert_eq!(dispatcher.nid_stub_count(), 0);
    }

    // Phase 1 dispatcher integration tests

    #[test]
    fn test_phase1_video_out_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // cellVideoOut functions should be registered
        let has_get_state = dispatcher.stub_map.values()
            .any(|info| info.name == "cellVideoOutGetState");
        let has_configure = dispatcher.stub_map.values()
            .any(|info| info.name == "cellVideoOutConfigure");
        let has_get_config = dispatcher.stub_map.values()
            .any(|info| info.name == "cellVideoOutGetConfiguration");
        let has_res_avail = dispatcher.stub_map.values()
            .any(|info| info.name == "cellVideoOutGetResolutionAvailability");
        
        assert!(has_get_state, "cellVideoOutGetState should be registered");
        assert!(has_configure, "cellVideoOutConfigure should be registered");
        assert!(has_get_config, "cellVideoOutGetConfiguration should be registered");
        assert!(has_res_avail, "cellVideoOutGetResolutionAvailability should be registered");
    }

    #[test]
    fn test_phase1_game_content_permit_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let has_content_permit = dispatcher.stub_map.values()
            .any(|info| info.name == "cellGameContentPermit");

        assert!(has_content_permit, "cellGameContentPermit should be registered");
    }

    #[test]
    fn test_phase1_fs_dir_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let has_opendir = dispatcher.stub_map.values()
            .any(|info| info.name == "cellFsOpendir");
        let has_readdir = dispatcher.stub_map.values()
            .any(|info| info.name == "cellFsReaddir");
        let has_lseek = dispatcher.stub_map.values()
            .any(|info| info.name == "cellFsLseek64");

        assert!(has_opendir, "cellFsOpendir should be registered");
        assert!(has_readdir, "cellFsReaddir should be registered");
        assert!(has_lseek, "cellFsLseek64 should be registered");
    }

    #[test]
    fn test_phase1_registration_count() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // Verify we have a reasonable number of functions registered
        // Previous count was ~37, new count should be ~47 (added ~10 new functions)
        assert!(dispatcher.stub_map.len() >= 45,
            "Expected at least 45 registered functions, got {}", dispatcher.stub_map.len());
    }

    #[test]
    fn test_video_out_get_resolution_availability() {
        let mut dispatcher = HleDispatcher::new();
        let addr = dispatcher.register_function(
            "cellSysutil", "cellVideoOutGetResolutionAvailability",
            hle_video_out_get_resolution_availability
        );

        // Test: video_out=0 (PRIMARY), resolution=5 (720p), aspect=0 → should be 1 (available)
        let ctx = HleCallContext {
            stub_addr: addr,
            args: [0, 5, 0, 0, 0, 0, 0, 0],
            toc: 0,
            lr: 0,
        };
        let result = dispatcher.dispatch(&ctx);
        assert_eq!(result, Some(1), "720p should be available on primary output");

        // Test: video_out=0, resolution=7 (1080p) → should be 1
        let ctx_1080 = HleCallContext {
            stub_addr: addr,
            args: [0, 7, 0, 0, 0, 0, 0, 0],
            toc: 0,
            lr: 0,
        };
        let result_1080 = dispatcher.dispatch(&ctx_1080);
        assert_eq!(result_1080, Some(1), "1080p should be available");

        // Test: video_out=99 (invalid), resolution=5 → should be 0 (not available)
        let ctx_invalid = HleCallContext {
            stub_addr: addr,
            args: [99, 5, 0, 0, 0, 0, 0, 0],
            toc: 0,
            lr: 0,
        };
        let result_invalid = dispatcher.dispatch(&ctx_invalid);
        assert_eq!(result_invalid, Some(0), "Invalid video output should return 0");
    }
}
