//! HLE Function Dispatcher
//!
//! This module provides the main entry point for HLE function calls.
//! It maps stub addresses to HLE functions and dispatches calls with
//! proper argument marshalling.

use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::{debug, info, trace};

use crate::context::{get_hle_context, get_hle_context_mut};
use crate::memory::{read_be32, read_string, write_be32};

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
    crate::cell_audio::cell_audio_init() as i64
}

fn hle_audio_quit(_ctx: &HleCallContext) -> i64 {
    info!("cellAudioQuit()");
    crate::cell_audio::cell_audio_quit() as i64
}

fn hle_audio_port_open(ctx: &HleCallContext) -> i64 {
    let config_ptr = ctx.args[0] as u32;
    let port_num_ptr = ctx.args[1] as u32;
    
    debug!("cellAudioPortOpen(config=0x{:08x}, port_num_ptr=0x{:08x})", config_ptr, port_num_ptr);
    crate::cell_audio::cell_audio_port_open(config_ptr, port_num_ptr) as i64
}

fn hle_audio_port_close(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    debug!("cellAudioPortClose(port={})", port_num);
    crate::cell_audio::cell_audio_port_close(port_num) as i64
}

fn hle_audio_port_start(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    debug!("cellAudioPortStart(port={})", port_num);
    crate::cell_audio::cell_audio_port_start(port_num) as i64
}

fn hle_audio_port_stop(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    debug!("cellAudioPortStop(port={})", port_num);
    crate::cell_audio::cell_audio_port_stop(port_num) as i64
}

fn hle_audio_get_port_config(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    let config_ptr = ctx.args[1] as u32;
    
    debug!("cellAudioGetPortConfig(port={}, config_ptr=0x{:08x})", port_num, config_ptr);
    crate::cell_audio::cell_audio_get_port_config(port_num, config_ptr) as i64
}

fn hle_audio_get_port_timestamp(ctx: &HleCallContext) -> i64 {
    let port_num = ctx.args[0] as u32;
    let tag = ctx.args[1];
    let stamp_addr = ctx.args[2] as u32;
    
    debug!("cellAudioGetPortTimestamp(port={}, tag={}, stamp=0x{:08x})", port_num, tag, stamp_addr);
    crate::cell_audio::cell_audio_get_port_timestamp(port_num, tag, stamp_addr) as i64
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
    
    // Read CellRescInitConfig from guest memory
    let config = crate::cell_resc::CellRescInitConfig {
        size: read_be32(config_ptr).unwrap_or(20),
        resource_policy: read_be32(config_ptr + 4).unwrap_or(0),
        display_modes: read_be32(config_ptr + 8).unwrap_or(0x0F),
        interpolation_mode: read_be32(config_ptr + 12).unwrap_or(0),
        interlace_filter: read_be32(config_ptr + 16).unwrap_or(0),
    };
    
    let mut ctx_guard = get_hle_context_mut();
    ctx_guard.resc.init(config) as i64
}

fn hle_resc_exit(_ctx: &HleCallContext) -> i64 {
    info!("cellRescExit()");
    let mut ctx = get_hle_context_mut();
    ctx.resc.exit() as i64
}

fn hle_resc_set_display_mode(ctx: &HleCallContext) -> i64 {
    let mode = ctx.args[0] as u32;
    debug!("cellRescSetDisplayMode(mode={})", mode);
    let mut ctx_guard = get_hle_context_mut();
    ctx_guard.resc.set_display_mode(mode) as i64
}

fn hle_resc_set_src(ctx: &HleCallContext) -> i64 {
    let src_ptr = ctx.args[0] as u32;
    debug!("cellRescSetSrc(src_ptr=0x{:08x})", src_ptr);
    
    // Read CellRescSrc from guest memory
    let src = crate::cell_resc::CellRescSrc {
        format: read_be32(src_ptr).unwrap_or(0),
        pitch: read_be32(src_ptr + 4).unwrap_or(0),
        width: (read_be32(src_ptr + 8).unwrap_or(0) >> 16) as u16,
        height: (read_be32(src_ptr + 8).unwrap_or(0) & 0xFFFF) as u16,
        offset: read_be32(src_ptr + 12).unwrap_or(0),
    };
    
    let mut ctx_guard = get_hle_context_mut();
    ctx_guard.resc.set_src(src) as i64
}

fn hle_resc_set_convert_and_flip(ctx: &HleCallContext) -> i64 {
    let buffer_id = ctx.args[0] as u32;
    debug!("cellRescSetConvertAndFlip(buffer_id={})", buffer_id);
    // Set convert-and-flip triggers the RSX to scale and present
    // For now, just record the buffer ID
    let ctx_guard = get_hle_context();
    if !ctx_guard.resc.is_initialized() {
        return crate::cell_resc::CELL_RESC_ERROR_NOT_INITIALIZED as i64;
    }
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
    
    crate::cell_spurs::cell_spurs_initialize(spurs_ptr, num_spu, spu_priority, ppu_priority, exit_if_no_work) as i64
}

fn hle_spurs_finalize(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    info!("cellSpursFinalize(spurs=0x{:08x})", spurs_ptr);
    crate::cell_spurs::cell_spurs_finalize(spurs_ptr) as i64
}

fn hle_spurs_attach_lv2_event_queue(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let queue = ctx.args[1] as u32;
    let port = ctx.args[2] as u32;
    let is_dynamic = ctx.args[3] != 0;
    debug!(
        "cellSpursAttachLv2EventQueue(spurs=0x{:08x}, queue={}, port={}, dynamic={})",
        spurs_ptr, queue, port, is_dynamic
    );
    crate::cell_spurs::cell_spurs_attach_lv2_event_queue(spurs_ptr, queue, port, is_dynamic) as i64
}

fn hle_spurs_create_taskset(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let taskset_ptr = ctx.args[1] as u32;
    debug!(
        "cellSpursCreateTaskset(spurs=0x{:08x}, taskset=0x{:08x})",
        spurs_ptr, taskset_ptr
    );
    crate::cell_spurs::cell_spurs_create_taskset(spurs_ptr, taskset_ptr) as i64
}

fn hle_spurs_taskset_attribute_set_name(ctx: &HleCallContext) -> i64 {
    let attr_ptr = ctx.args[0] as u32;
    let name_ptr = ctx.args[1] as u32;
    debug!(
        "cellSpursTasksetAttributeSetName(attr=0x{:08x}, name=0x{:08x})",
        attr_ptr, name_ptr
    );
    crate::cell_spurs::cell_spurs_taskset_attribute_set_name(attr_ptr, name_ptr) as i64
}

fn hle_spurs_create_task(ctx: &HleCallContext) -> i64 {
    let taskset_ptr = ctx.args[0] as u32;
    let task_id_ptr = ctx.args[1] as u32;
    let elf_addr = ctx.args[2] as u32;
    let context_addr = ctx.args[3] as u32;
    let ls_size = ctx.args[4] as u32;
    let ls_pattern_ptr = ctx.args[5] as u32;
    debug!(
        "cellSpursCreateTask(taskset=0x{:08x}, taskId=0x{:08x}, elf=0x{:08x}, ctx=0x{:08x}, ls=0x{:x}, pat=0x{:08x})",
        taskset_ptr, task_id_ptr, elf_addr, context_addr, ls_size, ls_pattern_ptr
    );
    crate::cell_spurs::cell_spurs_create_task(
        taskset_ptr, task_id_ptr, elf_addr, context_addr, ls_size, ls_pattern_ptr,
    ) as i64
}

fn hle_spurs_set_max_contention(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let wid = ctx.args[1] as u32;
    let max_contention = ctx.args[2] as u32;
    debug!(
        "cellSpursSetMaxContention(spurs=0x{:08x}, wid={}, max={})",
        spurs_ptr, wid, max_contention
    );
    crate::cell_spurs::cell_spurs_set_max_contention(spurs_ptr, wid, max_contention) as i64
}

fn hle_spurs_set_priorities(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let wid = ctx.args[1] as u32;
    let priorities_addr = ctx.args[2] as u32;
    debug!(
        "cellSpursSetPriorities(spurs=0x{:08x}, wid={}, pri=0x{:08x})",
        spurs_ptr, wid, priorities_addr
    );
    crate::cell_spurs::cell_spurs_set_priorities(spurs_ptr, wid, priorities_addr) as i64
}

fn hle_spurs_get_spu_thread_id(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let thread = ctx.args[1] as u32;
    let thread_id_addr = ctx.args[2] as u32;
    debug!(
        "cellSpursGetSpuThreadId(spurs=0x{:08x}, thread={}, out=0x{:08x})",
        spurs_ptr, thread, thread_id_addr
    );
    crate::cell_spurs::cell_spurs_get_spu_thread_id(spurs_ptr, thread, thread_id_addr) as i64
}

fn hle_spurs_detach_lv2_event_queue(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let port = ctx.args[1] as u32;
    debug!(
        "cellSpursDetachLv2EventQueue(spurs=0x{:08x}, port={})",
        spurs_ptr, port
    );
    crate::cell_spurs::cell_spurs_detach_lv2_event_queue(spurs_ptr, port) as i64
}

fn hle_spurs_add_policy_module(ctx: &HleCallContext) -> i64 {
    let image_addr = ctx.args[0] as u32;
    let image_size = ctx.args[1] as u32;
    debug!(
        "cellSpursAddPolicyModule(addr=0x{:08x}, size={})",
        image_addr, image_size
    );
    crate::cell_spurs::cell_spurs_add_policy_module(image_addr, image_size) as i64
}

fn hle_spurs_initialize_with_attribute(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let attr_ptr = ctx.args[1] as u32;
    debug!(
        "cellSpursInitializeWithAttribute(spurs=0x{:08x}, attr=0x{:08x})",
        spurs_ptr, attr_ptr
    );
    crate::cell_spurs::cell_spurs_initialize_with_attribute(spurs_ptr, attr_ptr) as i64
}

fn hle_spurs_attribute_initialize(ctx: &HleCallContext) -> i64 {
    let attr_ptr = ctx.args[0] as u32;
    let n_spus = ctx.args[1] as u32;
    let spu_priority = ctx.args[2] as u32;
    let ppu_priority = ctx.args[3] as u32;
    let exit_if_no_work = ctx.args[4] != 0;
    debug!(
        "cellSpursAttributeInitialize(attr=0x{:08x}, nSpus={}, spuPri={}, ppuPri={})",
        attr_ptr, n_spus, spu_priority, ppu_priority
    );
    crate::cell_spurs::cell_spurs_attribute_initialize(
        attr_ptr, n_spus, spu_priority, ppu_priority, exit_if_no_work,
    ) as i64
}

fn hle_spurs_attribute_set_name_prefix(ctx: &HleCallContext) -> i64 {
    let attr_ptr = ctx.args[0] as u32;
    let prefix_addr = ctx.args[1] as u32;
    let size = ctx.args[2] as u32;
    debug!(
        "cellSpursAttributeSetNamePrefix(attr=0x{:08x}, prefix=0x{:08x}, size={})",
        attr_ptr, prefix_addr, size
    );
    crate::cell_spurs::cell_spurs_attribute_set_name_prefix(attr_ptr, prefix_addr, size) as i64
}

fn hle_spurs_get_info(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let info_addr = ctx.args[1] as u32;
    debug!(
        "cellSpursGetInfo(spurs=0x{:08x}, info=0x{:08x})",
        spurs_ptr, info_addr
    );
    crate::cell_spurs::cell_spurs_get_info(spurs_ptr, info_addr) as i64
}

fn hle_spurs_wake_up(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    debug!("cellSpursWakeUp(spurs=0x{:08x})", spurs_ptr);
    crate::cell_spurs::cell_spurs_wake_up(spurs_ptr) as i64
}

fn hle_spurs_request_idle_spu(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let spu_id_addr = ctx.args[1] as u32;
    debug!(
        "cellSpursRequestIdleSpu(spurs=0x{:08x}, out=0x{:08x})",
        spurs_ptr, spu_id_addr
    );
    crate::cell_spurs::cell_spurs_request_idle_spu(spurs_ptr, spu_id_addr) as i64
}

fn hle_spurs_get_spu_thread_group_id(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let group_id_addr = ctx.args[1] as u32;
    debug!(
        "cellSpursGetSpuThreadGroupId(spurs=0x{:08x}, out=0x{:08x})",
        spurs_ptr, group_id_addr
    );
    crate::cell_spurs::cell_spurs_get_spu_thread_group_id(spurs_ptr, group_id_addr) as i64
}

fn hle_spurs_shutdown_taskset(ctx: &HleCallContext) -> i64 {
    let taskset_ptr = ctx.args[0] as u32;
    debug!("cellSpursShutdownTaskset(taskset=0x{:08x})", taskset_ptr);
    crate::cell_spurs::cell_spurs_shutdown_taskset(taskset_ptr) as i64
}

fn hle_spurs_join_taskset(ctx: &HleCallContext) -> i64 {
    let taskset_ptr = ctx.args[0] as u32;
    debug!("cellSpursJoinTaskset(taskset=0x{:08x})", taskset_ptr);
    crate::cell_spurs::cell_spurs_join_taskset(taskset_ptr) as i64
}

fn hle_spurs_create_taskset_with_attribute(ctx: &HleCallContext) -> i64 {
    let spurs_ptr = ctx.args[0] as u32;
    let taskset_ptr = ctx.args[1] as u32;
    let attr_ptr = ctx.args[2] as u32;
    debug!(
        "cellSpursCreateTasksetWithAttribute(spurs=0x{:08x}, taskset=0x{:08x}, attr=0x{:08x})",
        spurs_ptr, taskset_ptr, attr_ptr
    );
    crate::cell_spurs::cell_spurs_create_taskset_with_attribute(spurs_ptr, taskset_ptr, attr_ptr) as i64
}

// --- libsre (Regular Expression Library) ---

fn hle_sre_compile(ctx: &HleCallContext) -> i64 {
    let pattern_addr = ctx.args[0] as u32;
    let flags = ctx.args[1] as u32;
    let compiled_addr = ctx.args[2] as u32;
    debug!(
        "cellSreCompile(pattern=0x{:08x}, flags={}, out=0x{:08x})",
        pattern_addr, flags, compiled_addr
    );
    // Read pattern from memory and compile
    if pattern_addr == 0 || compiled_addr == 0 {
        return crate::libsre::SRE_ERROR_INVALID_PARAMETER as i64;
    }
    let pattern_str = if crate::memory::is_hle_memory_initialized() {
        crate::memory::read_string(pattern_addr, 256).unwrap_or_default()
    } else {
        return crate::libsre::SRE_ERROR_INVALID_PARAMETER as i64;
    };
    match crate::context::get_hle_context_mut().regex.compile(&pattern_str, flags) {
        Ok(pattern_id) => {
            if crate::memory::is_hle_memory_initialized() {
                let _ = crate::memory::write_be32(compiled_addr, pattern_id);
            }
            0i64
        }
        Err(e) => e as i64,
    }
}

fn hle_sre_free(ctx: &HleCallContext) -> i64 {
    let pattern = ctx.args[0] as u32;
    debug!("cellSreFree(pattern={})", pattern);
    crate::context::get_hle_context_mut().regex.free(pattern) as i64
}

fn hle_sre_match(ctx: &HleCallContext) -> i64 {
    let pattern = ctx.args[0] as u32;
    let _text_addr = ctx.args[1] as u32;
    let _text_len = ctx.args[2] as u32;
    let _matches_addr = ctx.args[3] as u32;
    let _max_matches = ctx.args[4] as u32;
    let num_matches_addr = ctx.args[5] as u32;
    debug!("cellSreMatch(pattern={})", pattern);
    // Stub: return success with 0 matches
    if num_matches_addr != 0 && crate::memory::is_hle_memory_initialized() {
        let _ = crate::memory::write_be32(num_matches_addr, 0);
    }
    0i64
}

fn hle_sre_search(ctx: &HleCallContext) -> i64 {
    let pattern = ctx.args[0] as u32;
    debug!("cellSreSearch(pattern={})", pattern);
    // Stub: return not found
    -1i64
}

fn hle_sre_replace(ctx: &HleCallContext) -> i64 {
    let pattern = ctx.args[0] as u32;
    debug!("cellSreReplace(pattern={})", pattern);
    // Stub: return error (no replacement done)
    crate::libsre::SRE_ERROR_INVALID_PARAMETER as i64
}

fn hle_sre_get_error(ctx: &HleCallContext) -> i64 {
    let error_code = ctx.args[0] as i32;
    let _buffer_addr = ctx.args[1] as u32;
    let _buffer_size = ctx.args[2] as u32;
    debug!("cellSreGetError(code={})", error_code);
    0i64
}

// --- Generic stub ---

// --- cellPngDec ---

fn hle_png_dec_create(ctx: &HleCallContext) -> i64 {
    let main_handle_addr = ctx.args[0] as u32;
    let thread_in_param_addr = ctx.args[1] as u32;
    let thread_out_param_addr = ctx.args[2] as u32;
    debug!("cellPngDecCreate(main_handle_addr=0x{:08x})", main_handle_addr);
    crate::cell_png_dec::cell_png_dec_create(main_handle_addr, thread_in_param_addr, thread_out_param_addr) as i64
}

fn hle_png_dec_destroy(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    trace!("cellPngDecDestroy(main_handle={})", main_handle);
    crate::cell_png_dec::cell_png_dec_destroy(main_handle) as i64
}

fn hle_png_dec_open(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle_addr = ctx.args[1] as u32;
    let src_addr = ctx.args[2] as u32;
    let open_info_addr = ctx.args[3] as u32;
    debug!("cellPngDecOpen(main_handle={}, sub_handle_addr=0x{:08x})", main_handle, sub_handle_addr);
    crate::cell_png_dec::cell_png_dec_open(main_handle, sub_handle_addr, src_addr, open_info_addr) as i64
}

fn hle_png_dec_close(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    trace!("cellPngDecClose(main_handle={}, sub_handle={})", main_handle, sub_handle);
    crate::cell_png_dec::cell_png_dec_close(main_handle, sub_handle) as i64
}

fn hle_png_dec_read_header(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    let info_addr = ctx.args[2] as u32;
    debug!("cellPngDecReadHeader(main_handle={}, sub_handle={})", main_handle, sub_handle);
    crate::cell_png_dec::cell_png_dec_read_header(main_handle, sub_handle, info_addr) as i64
}

fn hle_png_dec_set_parameter(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    let in_param_addr = ctx.args[2] as u32;
    let out_param_addr = ctx.args[3] as u32;
    trace!("cellPngDecSetParameter(main_handle={}, sub_handle={})", main_handle, sub_handle);
    crate::cell_png_dec::cell_png_dec_set_parameter(main_handle, sub_handle, in_param_addr, out_param_addr) as i64
}

fn hle_png_dec_decode_data(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    let data_addr = ctx.args[2] as u32;
    let data_out_info_addr = ctx.args[3] as u32;
    debug!("cellPngDecDecodeData(main_handle={}, sub_handle={})", main_handle, sub_handle);
    crate::cell_png_dec::cell_png_dec_decode_data(main_handle, sub_handle, data_addr, data_out_info_addr) as i64
}

// --- cellJpgDec ---

fn hle_jpg_dec_create(ctx: &HleCallContext) -> i64 {
    let main_handle_addr = ctx.args[0] as u32;
    let thread_in_param_addr = ctx.args[1] as u32;
    debug!("cellJpgDecCreate(main_handle_addr=0x{:08x})", main_handle_addr);

    if main_handle_addr == 0 || thread_in_param_addr == 0 {
        return 0x80611001u32 as i64; // CELL_JPGDEC_ERROR_ARG
    }

    // Create decoder through global manager (max 4 handles)
    match crate::context::get_hle_context_mut().jpg_dec.create(4) {
        Ok(handle_id) => {
            if let Err(e) = write_be32(main_handle_addr, handle_id) {
                return e as i64;
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_jpg_dec_destroy(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    trace!("cellJpgDecDestroy(main_handle={})", main_handle);
    crate::cell_jpg_dec::cell_jpg_dec_destroy(main_handle) as i64
}

fn hle_jpg_dec_open(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle_addr = ctx.args[1] as u32;
    debug!("cellJpgDecOpen(main_handle={}, sub_handle_addr=0x{:08x})", main_handle, sub_handle_addr);

    if sub_handle_addr == 0 {
        return 0x80611001u32 as i64; // CELL_JPGDEC_ERROR_ARG
    }

    // Placeholder dimensions — actual JPEG header parsing happens in the
    // decoder backend when source data is provided via memory subsystem
    match crate::context::get_hle_context_mut().jpg_dec.open(main_handle, 1920, 1080, 3) {
        Ok(sub_id) => {
            if let Err(e) = write_be32(sub_handle_addr, sub_id) {
                return e as i64;
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_jpg_dec_close(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    trace!("cellJpgDecClose(main_handle={}, sub_handle={})", main_handle, sub_handle);
    crate::cell_jpg_dec::cell_jpg_dec_close(main_handle, sub_handle) as i64
}

fn hle_jpg_dec_read_header(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    let info_addr = ctx.args[2] as u32;
    debug!("cellJpgDecReadHeader(main_handle={}, sub_handle={})", main_handle, sub_handle);

    if info_addr == 0 {
        return 0x80611001u32 as i64; // CELL_JPGDEC_ERROR_ARG
    }

    match crate::context::get_hle_context().jpg_dec.read_header_params(main_handle, sub_handle) {
        Ok(header_info) => {
            // Write CellJpgDecInfo to guest memory (width, height, numComponents, colorSpace, downScale)
            if let Err(e) = write_be32(info_addr, header_info.width) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 4, header_info.height) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 8, header_info.num_components) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 12, header_info.color_space) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 16, header_info.down_scale) { return e as i64; }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_jpg_dec_decode_data(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    debug!("cellJpgDecDecodeData(main_handle={}, sub_handle={})", main_handle, sub_handle);

    match crate::context::get_hle_context_mut().jpg_dec.decode_data(main_handle, sub_handle) {
        Ok(_decode_info) => error::CELL_OK,
        Err(e) => e as i64,
    }
}

// --- cellGifDec ---

fn hle_gif_dec_create(ctx: &HleCallContext) -> i64 {
    let main_handle_addr = ctx.args[0] as u32;
    let thread_in_param_addr = ctx.args[1] as u32;
    debug!("cellGifDecCreate(main_handle_addr=0x{:08x})", main_handle_addr);

    if main_handle_addr == 0 || thread_in_param_addr == 0 {
        return 0x80621001u32 as i64; // CELL_GIFDEC_ERROR_ARG
    }

    // Create decoder through global manager (max 4 handles)
    match crate::context::get_hle_context_mut().gif_dec.create(4) {
        Ok(handle_id) => {
            if let Err(e) = write_be32(main_handle_addr, handle_id) {
                return e as i64;
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_gif_dec_destroy(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    trace!("cellGifDecDestroy(main_handle={})", main_handle);
    crate::cell_gif_dec::cell_gif_dec_destroy(main_handle) as i64
}

fn hle_gif_dec_open(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle_addr = ctx.args[1] as u32;
    let src_addr = ctx.args[2] as u32;
    debug!("cellGifDecOpen(main_handle={}, sub_handle_addr=0x{:08x})", main_handle, sub_handle_addr);

    if sub_handle_addr == 0 || src_addr == 0 {
        return 0x80621001u32 as i64; // CELL_GIFDEC_ERROR_ARG
    }

    // Read source info from guest memory (stream_ptr, stream_size at src_addr)
    let stream_ptr = crate::memory::read_be32(src_addr).unwrap_or(0);
    let stream_size = crate::memory::read_be32(src_addr + 4).unwrap_or(0);

    match crate::context::get_hle_context_mut().gif_dec.open(main_handle, stream_ptr, stream_size) {
        Ok(sub_id) => {
            if let Err(e) = write_be32(sub_handle_addr, sub_id) {
                return e as i64;
            }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_gif_dec_close(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    trace!("cellGifDecClose(main_handle={}, sub_handle={})", main_handle, sub_handle);
    crate::cell_gif_dec::cell_gif_dec_close(main_handle, sub_handle) as i64
}

fn hle_gif_dec_read_header(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    let info_addr = ctx.args[2] as u32;
    debug!("cellGifDecReadHeader(main_handle={}, sub_handle={})", main_handle, sub_handle);

    if info_addr == 0 {
        return 0x80621001u32 as i64; // CELL_GIFDEC_ERROR_ARG
    }

    match crate::context::get_hle_context().gif_dec.get_info(main_handle, sub_handle) {
        Ok(info) => {
            // Write CellGifDecInfo to guest memory
            if let Err(e) = write_be32(info_addr, info.width) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 4, info.height) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 8, info.num_components) { return e as i64; }
            if let Err(e) = write_be32(info_addr + 12, info.color_space) { return e as i64; }
            error::CELL_OK
        }
        Err(e) => e as i64,
    }
}

fn hle_gif_dec_decode_data(ctx: &HleCallContext) -> i64 {
    let main_handle = ctx.args[0] as u32;
    let sub_handle = ctx.args[1] as u32;
    debug!("cellGifDecDecodeData(main_handle={}, sub_handle={})", main_handle, sub_handle);

    // Validate handle and return success — actual pixel data decoding
    // requires memory subsystem to provide source data and destination buffer
    match crate::context::get_hle_context().gif_dec.get_info(main_handle, sub_handle) {
        Ok(_info) => error::CELL_OK,
        Err(e) => e as i64,
    }
}

// --- cellFont ---

fn hle_font_init(ctx: &HleCallContext) -> i64 {
    let config_addr = ctx.args[0] as u32;
    debug!("cellFontInit(config_addr=0x{:08x})", config_addr);
    crate::cell_font::cell_font_init(config_addr) as i64
}

fn hle_font_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellFontEnd()");
    crate::cell_font::cell_font_end() as i64
}

fn hle_font_open_font_memory(ctx: &HleCallContext) -> i64 {
    let library = ctx.args[0] as u32;
    let font_addr = ctx.args[1] as u32;
    let font_size = ctx.args[2] as u32;
    let sub_num = ctx.args[3] as u32;
    let unique_id = ctx.args[4] as u32;
    let font_handle_addr = ctx.args[5] as u32;
    debug!("cellFontOpenFontMemory(fontAddr=0x{:08x}, fontSize={})", font_addr, font_size);
    crate::cell_font::cell_font_open_font_memory(library, font_addr, font_size, sub_num, unique_id, font_handle_addr) as i64
}

fn hle_font_close_font(ctx: &HleCallContext) -> i64 {
    let font = ctx.args[0] as u32;
    trace!("cellFontCloseFont(font={})", font);
    crate::cell_font::cell_font_close_font(font) as i64
}

fn hle_font_create_renderer(ctx: &HleCallContext) -> i64 {
    let library = ctx.args[0] as u32;
    let config_addr = ctx.args[1] as u32;
    let renderer_addr = ctx.args[2] as u32;
    debug!("cellFontCreateRenderer(renderer_addr=0x{:08x})", renderer_addr);
    crate::cell_font::cell_font_create_renderer(library, config_addr, renderer_addr) as i64
}

fn hle_font_destroy_renderer(ctx: &HleCallContext) -> i64 {
    let renderer = ctx.args[0] as u32;
    trace!("cellFontDestroyRenderer(renderer={})", renderer);
    crate::cell_font::cell_font_destroy_renderer(renderer) as i64
}

fn hle_font_render_char_glyph_image(ctx: &HleCallContext) -> i64 {
    let font = ctx.args[0] as u32;
    let code = ctx.args[1] as u32;
    let renderer = ctx.args[2] as u32;
    let glyph_addr = ctx.args[3] as u32;
    trace!("cellFontRenderCharGlyphImage(font={}, code=0x{:x})", font, code);
    crate::cell_font::cell_font_render_char_glyph_image(font, code, renderer, glyph_addr) as i64
}

fn hle_font_get_horizontal_layout(ctx: &HleCallContext) -> i64 {
    let font = ctx.args[0] as u32;
    let layout_addr = ctx.args[1] as u32;
    trace!("cellFontGetHorizontalLayout(font={})", font);
    crate::cell_font::cell_font_get_horizontal_layout(font, layout_addr) as i64
}

fn hle_font_open_font_file(ctx: &HleCallContext) -> i64 {
    let library = ctx.args[0] as u32;
    let font_path_addr = ctx.args[1] as u32;
    let sub_num = ctx.args[2] as u32;
    let unique_id = ctx.args[3] as u32;
    let font_handle_addr = ctx.args[4] as u32;
    debug!("cellFontOpenFontFile(path_addr=0x{:08x})", font_path_addr);
    crate::cell_font::cell_font_open_font_file(library, font_path_addr, sub_num, unique_id, font_handle_addr) as i64
}

// --- cellFontFT ---

fn hle_font_ft_init(ctx: &HleCallContext) -> i64 {
    let config_addr = ctx.args[0] as u32;
    debug!("cellFontFTInit(config_addr=0x{:08x})", config_addr);
    crate::cell_font_ft::cell_font_ft_init(config_addr) as i64
}

fn hle_font_ft_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellFontFTEnd()");
    crate::cell_font_ft::cell_font_ft_end() as i64
}

fn hle_font_ft_load_glyph(ctx: &HleCallContext) -> i64 {
    let face = ctx.args[0] as u32;
    let glyph_index = ctx.args[1] as u32;
    let flags = ctx.args[2] as u32;
    trace!("cellFontFTLoadGlyph(face={}, glyph_index={})", face, glyph_index);
    crate::cell_font_ft::cell_font_ft_load_glyph(face, glyph_index, flags) as i64
}

fn hle_font_ft_set_char_size(ctx: &HleCallContext) -> i64 {
    let face = ctx.args[0] as u32;
    let char_width = ctx.args[1] as u32;
    let char_height = ctx.args[2] as u32;
    trace!("cellFontFTSetCharSize(face={}, w={}, h={})", face, char_width, char_height);
    crate::cell_font_ft::cell_font_ft_set_char_size(face, char_width, char_height) as i64
}

fn hle_font_ft_open_font_memory(ctx: &HleCallContext) -> i64 {
    let font_addr = ctx.args[0] as u32;
    let font_size = ctx.args[1] as u32;
    let face_index = ctx.args[2] as u32;
    let face_addr = ctx.args[3] as u32;
    debug!("cellFontFTOpenFontMemory(fontAddr=0x{:08x}, size={})", font_addr, font_size);
    crate::cell_font_ft::cell_font_ft_open_font_memory(font_addr, font_size, face_index, face_addr) as i64
}

fn hle_font_ft_get_char_index(ctx: &HleCallContext) -> i64 {
    let face = ctx.args[0] as u32;
    let char_code = ctx.args[1] as u32;
    trace!("cellFontFTGetCharIndex(face={}, code=0x{:x})", face, char_code);
    crate::cell_font_ft::cell_font_ft_get_char_index(face, char_code) as i64
}

// --- cellSaveData ---

fn hle_save_data_list_load2(ctx: &HleCallContext) -> i64 {
    let set_list_addr = ctx.args[0] as u32;
    let set_buf_addr = ctx.args[1] as u32;
    let func_list = ctx.args[2] as u32;
    let func_stat = ctx.args[3] as u32;
    let func_file = ctx.args[4] as u32;
    debug!("cellSaveDataListLoad2(setList=0x{:08x}, setBuf=0x{:08x})", set_list_addr, set_buf_addr);
    
    let ctx_guard = get_hle_context();
    let dirs = ctx_guard.save_data.list_directories();
    
    // Write directory count to set_list result if addr valid
    if set_list_addr != 0 {
        let _ = write_be32(set_list_addr, dirs.len() as u32);
    }
    
    drop(ctx_guard);
    // TODO: Invoke func_list/func_stat/func_file callbacks on PPU thread for proper
    // save data directory selection and file I/O. Currently returns CELL_OK with
    // directory count written, which lets games proceed past save-check screens.
    let _ = (func_list, func_stat, func_file);
    error::CELL_OK
}

fn hle_save_data_list_save2(ctx: &HleCallContext) -> i64 {
    let set_list_addr = ctx.args[0] as u32;
    let set_buf_addr = ctx.args[1] as u32;
    debug!("cellSaveDataListSave2(setList=0x{:08x}, setBuf=0x{:08x})", set_list_addr, set_buf_addr);
    
    let ctx_guard = get_hle_context();
    let dirs = ctx_guard.save_data.list_directories();
    
    if set_list_addr != 0 {
        let _ = write_be32(set_list_addr, dirs.len() as u32);
    }
    
    error::CELL_OK
}

fn hle_save_data_auto_load2(ctx: &HleCallContext) -> i64 {
    let dir_name_addr = ctx.args[0] as u32;
    debug!("cellSaveDataAutoLoad2(dirName=0x{:08x})", dir_name_addr);
    
    let dir_name = read_string(dir_name_addr, 64).unwrap_or_default();
    let ctx_guard = get_hle_context();
    
    if !ctx_guard.save_data.directory_exists(&dir_name) {
        return error::CELL_ENOENT;
    }
    
    error::CELL_OK
}

fn hle_save_data_auto_save2(ctx: &HleCallContext) -> i64 {
    let dir_name_addr = ctx.args[0] as u32;
    debug!("cellSaveDataAutoSave2(dirName=0x{:08x})", dir_name_addr);
    
    let dir_name = read_string(dir_name_addr, 64).unwrap_or_default();
    let mut ctx_guard = get_hle_context_mut();
    
    if !ctx_guard.save_data.directory_exists(&dir_name) {
        ctx_guard.save_data.create_directory(&dir_name);
    }
    
    error::CELL_OK
}

fn hle_save_data_fixed_load2(ctx: &HleCallContext) -> i64 {
    let set_list_addr = ctx.args[0] as u32;
    debug!("cellSaveDataFixedLoad2(setList=0x{:08x})", set_list_addr);
    error::CELL_OK
}

fn hle_save_data_fixed_save2(ctx: &HleCallContext) -> i64 {
    let set_list_addr = ctx.args[0] as u32;
    debug!("cellSaveDataFixedSave2(setList=0x{:08x})", set_list_addr);
    error::CELL_OK
}

fn hle_save_data_delete2(ctx: &HleCallContext) -> i64 {
    let dir_name_addr = ctx.args[0] as u32;
    debug!("cellSaveDataDelete2(dirName=0x{:08x})", dir_name_addr);
    
    let dir_name = read_string(dir_name_addr, 64).unwrap_or_default();
    let mut ctx_guard = get_hle_context_mut();
    ctx_guard.save_data.delete_directory(&dir_name) as i64
}

// --- cellMsgDialog ---

fn hle_msg_dialog_open2(ctx: &HleCallContext) -> i64 {
    let dialog_type = ctx.args[0] as u32;
    let msg_addr = ctx.args[1] as u32;
    let callback = ctx.args[2] as u32;
    let userdata = ctx.args[3] as u32;
    debug!("cellMsgDialogOpen2(type={}, msg=0x{:08x})", dialog_type, msg_addr);
    crate::cell_sysutil::cell_msg_dialog_open(dialog_type, msg_addr, callback, userdata) as i64
}

fn hle_msg_dialog_close(ctx: &HleCallContext) -> i64 {
    let result = ctx.args[0] as u32;
    debug!("cellMsgDialogClose(result={})", result);
    crate::cell_sysutil::cell_msg_dialog_close(result) as i64
}

fn hle_msg_dialog_progress_bar_set_msg(ctx: &HleCallContext) -> i64 {
    let bar_index = ctx.args[0] as u32;
    let msg_addr = ctx.args[1] as u32;
    trace!("cellMsgDialogProgressBarSetMsg(bar={}, msg=0x{:08x})", bar_index, msg_addr);
    crate::cell_sysutil::cell_msg_dialog_progress_bar_set_msg(bar_index, msg_addr) as i64
}

fn hle_msg_dialog_progress_bar_inc(ctx: &HleCallContext) -> i64 {
    let bar_index = ctx.args[0] as u32;
    let delta = ctx.args[1] as u32;
    trace!("cellMsgDialogProgressBarInc(bar={}, delta={})", bar_index, delta);
    crate::cell_sysutil::cell_msg_dialog_progress_bar_inc(bar_index, delta) as i64
}

// --- cellSysutil BGM ---

fn hle_sysutil_get_bgm_playback_status(ctx: &HleCallContext) -> i64 {
    let status_addr = ctx.args[0] as u32;
    trace!("cellSysutilGetBgmPlaybackStatus(status=0x{:08x})", status_addr);
    crate::cell_sysutil::cell_sysutil_get_bgm_playback_status(status_addr) as i64
}

fn hle_sysutil_enable_bgm_playback(_ctx: &HleCallContext) -> i64 {
    debug!("cellSysutilEnableBgmPlayback()");
    crate::cell_sysutil::cell_sysutil_enable_bgm_playback() as i64
}

fn hle_sysutil_disable_bgm_playback(_ctx: &HleCallContext) -> i64 {
    debug!("cellSysutilDisableBgmPlayback()");
    crate::cell_sysutil::cell_sysutil_disable_bgm_playback() as i64
}

fn hle_sysutil_set_bgm_playback_volume(ctx: &HleCallContext) -> i64 {
    let volume = ctx.args[0] as u32;
    debug!("cellSysutilSetBgmPlaybackVolume(volume={})", volume);
    crate::cell_sysutil::cell_sysutil_set_bgm_playback_volume(volume) as i64
}

// --- cellVdec (Video Decoder) ---

fn hle_vdec_query_attr(ctx: &HleCallContext) -> i64 {
    let vdec_type_addr = ctx.args[0] as u32;
    let attr_addr = ctx.args[1] as u32;
    debug!("cellVdecQueryAttr(type=0x{:08x}, attr=0x{:08x})", vdec_type_addr, attr_addr);
    unsafe {
        crate::cell_vdec::cell_vdec_query_attr(
            vdec_type_addr as *const crate::cell_vdec::CellVdecType,
            attr_addr as *mut crate::cell_vdec::CellVdecAttr,
        ) as i64
    }
}

fn hle_vdec_open(ctx: &HleCallContext) -> i64 {
    let vdec_type_addr = ctx.args[0] as u32;
    let resource_addr = ctx.args[1] as u32;
    let cb_addr = ctx.args[2] as u32;
    let handle_addr = ctx.args[3] as u32;
    debug!("cellVdecOpen(type=0x{:08x}, handle=0x{:08x})", vdec_type_addr, handle_addr);
    unsafe {
        crate::cell_vdec::cell_vdec_open(
            vdec_type_addr as *const crate::cell_vdec::CellVdecType,
            resource_addr as *const crate::cell_vdec::CellVdecResource,
            cb_addr as *const crate::cell_vdec::CellVdecCb,
            handle_addr as *mut crate::cell_vdec::VdecHandle,
        ) as i64
    }
}

fn hle_vdec_close(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellVdecClose(handle={})", handle);
    crate::cell_vdec::cell_vdec_close(handle) as i64
}

fn hle_vdec_start_seq(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellVdecStartSeq(handle={})", handle);
    crate::cell_vdec::cell_vdec_start_seq(handle) as i64
}

fn hle_vdec_end_seq(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellVdecEndSeq(handle={})", handle);
    crate::cell_vdec::cell_vdec_end_seq(handle) as i64
}

fn hle_vdec_decode_au(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let mode = ctx.args[1] as u32;
    let au_info_addr = ctx.args[2] as u32;
    debug!("cellVdecDecodeAu(handle={}, mode={})", handle, mode);
    unsafe {
        crate::cell_vdec::cell_vdec_decode_au(
            handle,
            mode,
            au_info_addr as *const crate::cell_vdec::CellVdecAuInfo,
        ) as i64
    }
}

fn hle_vdec_get_picture(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let pic_format_addr = ctx.args[1] as u32;
    let pic_item_addr = ctx.args[2] as u32;
    debug!("cellVdecGetPicture(handle={})", handle);
    unsafe {
        crate::cell_vdec::cell_vdec_get_picture(
            handle,
            pic_format_addr as *const crate::cell_vdec::CellVdecPicFormat,
            pic_item_addr as *mut crate::cell_vdec::CellVdecPicItem,
        ) as i64
    }
}

fn hle_vdec_get_pic_item(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let pic_item_addr = ctx.args[1] as u32;
    debug!("cellVdecGetPicItem(handle={})", handle);
    crate::cell_vdec::cell_vdec_get_pic_item(
        handle,
        pic_item_addr as *mut u32,
    ) as i64
}

fn hle_vdec_set_frame_rate(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let frame_rate = ctx.args[1] as u32;
    trace!("cellVdecSetFrameRate(handle={}, rate={})", handle, frame_rate);
    crate::cell_vdec::cell_vdec_set_frame_rate(handle, frame_rate) as i64
}

// --- cellAdec (Audio Decoder) ---

fn hle_adec_query_attr(ctx: &HleCallContext) -> i64 {
    let adec_type_addr = ctx.args[0] as u32;
    let attr_addr = ctx.args[1] as u32;
    debug!("cellAdecQueryAttr(type=0x{:08x}, attr=0x{:08x})", adec_type_addr, attr_addr);
    unsafe {
        crate::cell_adec::cell_adec_query_attr(
            adec_type_addr as *const crate::cell_adec::CellAdecType,
            attr_addr as *mut crate::cell_adec::CellAdecAttr,
        ) as i64
    }
}

fn hle_adec_open(ctx: &HleCallContext) -> i64 {
    let adec_type_addr = ctx.args[0] as u32;
    let resource_addr = ctx.args[1] as u32;
    let cb_addr = ctx.args[2] as u32;
    let handle_addr = ctx.args[3] as u32;
    debug!("cellAdecOpen(type=0x{:08x}, handle=0x{:08x})", adec_type_addr, handle_addr);
    unsafe {
        crate::cell_adec::cell_adec_open(
            adec_type_addr as *const crate::cell_adec::CellAdecType,
            resource_addr as *const crate::cell_adec::CellAdecResource,
            cb_addr as *const crate::cell_adec::CellAdecCb,
            handle_addr as *mut crate::cell_adec::AdecHandle,
        ) as i64
    }
}

fn hle_adec_close(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellAdecClose(handle={})", handle);
    crate::cell_adec::cell_adec_close(handle) as i64
}

fn hle_adec_start_seq(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let start_param = ctx.args[1] as u32;
    trace!("cellAdecStartSeq(handle={})", handle);
    crate::cell_adec::cell_adec_start_seq(handle, start_param) as i64
}

fn hle_adec_end_seq(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellAdecEndSeq(handle={})", handle);
    crate::cell_adec::cell_adec_end_seq(handle) as i64
}

fn hle_adec_decode_au(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let au_info_addr = ctx.args[1] as u32;
    debug!("cellAdecDecodeAu(handle={})", handle);
    unsafe {
        crate::cell_adec::cell_adec_decode_au(
            handle,
            au_info_addr as *const crate::cell_adec::CellAdecAuInfo,
        ) as i64
    }
}

fn hle_adec_get_pcm(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let pcm_item_addr = ctx.args[1] as u32;
    debug!("cellAdecGetPcm(handle={})", handle);
    unsafe {
        crate::cell_adec::cell_adec_get_pcm(
            handle,
            pcm_item_addr as *mut crate::cell_adec::CellAdecPcmItem,
        ) as i64
    }
}

fn hle_adec_get_pcm_item(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let pcm_item_addr = ctx.args[1] as u32;
    debug!("cellAdecGetPcmItem(handle={})", handle);
    crate::cell_adec::cell_adec_get_pcm_item(
        handle,
        pcm_item_addr as *mut u32,
    ) as i64
}

// --- cellDmux (Demultiplexer) ---

fn hle_dmux_query_attr(ctx: &HleCallContext) -> i64 {
    let dmux_type_addr = ctx.args[0] as u32;
    let resource_addr = ctx.args[1] as u32;
    let attr_addr = ctx.args[2] as u32;
    debug!("cellDmuxQueryAttr(type=0x{:08x})", dmux_type_addr);
    unsafe {
        crate::cell_dmux::cell_dmux_query_attr(
            dmux_type_addr as *const crate::cell_dmux::CellDmuxType,
            resource_addr as *const crate::cell_dmux::CellDmuxResource,
            attr_addr as *mut crate::cell_dmux::CellDmuxType,
        ) as i64
    }
}

fn hle_dmux_open(ctx: &HleCallContext) -> i64 {
    let dmux_type_addr = ctx.args[0] as u32;
    let resource_addr = ctx.args[1] as u32;
    let cb_addr = ctx.args[2] as u32;
    let handle_addr = ctx.args[3] as u32;
    debug!("cellDmuxOpen(type=0x{:08x}, handle=0x{:08x})", dmux_type_addr, handle_addr);
    unsafe {
        crate::cell_dmux::cell_dmux_open(
            dmux_type_addr as *const crate::cell_dmux::CellDmuxType,
            resource_addr as *const crate::cell_dmux::CellDmuxResource,
            cb_addr as *const crate::cell_dmux::CellDmuxCb,
            handle_addr as *mut crate::cell_dmux::DmuxHandle,
        ) as i64
    }
}

fn hle_dmux_close(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellDmuxClose(handle={})", handle);
    crate::cell_dmux::cell_dmux_close(handle) as i64
}

fn hle_dmux_set_stream(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let stream_addr = ctx.args[1] as u32;
    let stream_size = ctx.args[2] as u32;
    let discontinuity = ctx.args[3] as u32;
    debug!("cellDmuxSetStream(handle={}, addr=0x{:08x}, size={})", handle, stream_addr, stream_size);
    crate::cell_dmux::cell_dmux_set_stream(handle, stream_addr, stream_size, discontinuity) as i64
}

fn hle_dmux_reset_stream(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellDmuxResetStream(handle={})", handle);
    crate::cell_dmux::cell_dmux_reset_stream(handle) as i64
}

fn hle_dmux_enable_es(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let es_attr_addr = ctx.args[1] as u32;
    let es_cb_addr = ctx.args[2] as u32;
    let es_handle_addr = ctx.args[3] as u32;
    debug!("cellDmuxEnableEs(handle={})", handle);
    unsafe {
        crate::cell_dmux::cell_dmux_enable_es(
            handle,
            es_attr_addr as *const crate::cell_dmux::CellDmuxEsAttr,
            es_cb_addr as *const crate::cell_dmux::CellDmuxEsCb,
            es_handle_addr as *mut u32,
        ) as i64
    }
}

fn hle_dmux_disable_es(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let es_handle = ctx.args[1] as u32;
    trace!("cellDmuxDisableEs(handle={}, es={})", handle, es_handle);
    crate::cell_dmux::cell_dmux_disable_es(handle, es_handle) as i64
}

fn hle_dmux_reset_es(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let es_handle = ctx.args[1] as u32;
    trace!("cellDmuxResetEs(handle={}, es={})", handle, es_handle);
    crate::cell_dmux::cell_dmux_reset_es(handle, es_handle) as i64
}

fn hle_dmux_get_au(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let es_handle = ctx.args[1] as u32;
    let au_info_addr = ctx.args[2] as u32;
    let au_specific_info_addr = ctx.args[3] as u32;
    debug!("cellDmuxGetAu(handle={}, es={})", handle, es_handle);
    unsafe {
        crate::cell_dmux::cell_dmux_get_au(
            handle,
            es_handle,
            au_info_addr as *mut crate::cell_dmux::CellDmuxAuInfo,
            au_specific_info_addr as *mut u32,
        ) as i64
    }
}

fn hle_dmux_peek_au(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let es_handle = ctx.args[1] as u32;
    let au_info_addr = ctx.args[2] as u32;
    let au_specific_info_addr = ctx.args[3] as u32;
    debug!("cellDmuxPeekAu(handle={}, es={})", handle, es_handle);
    unsafe {
        crate::cell_dmux::cell_dmux_peek_au(
            handle,
            es_handle,
            au_info_addr as *mut crate::cell_dmux::CellDmuxAuInfo,
            au_specific_info_addr as *mut u32,
        ) as i64
    }
}

fn hle_dmux_release_au(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let es_handle = ctx.args[1] as u32;
    trace!("cellDmuxReleaseAu(handle={}, es={})", handle, es_handle);
    crate::cell_dmux::cell_dmux_release_au(handle, es_handle) as i64
}

// --- cellVpost (Video Post-Processor) ---

fn hle_vpost_query_attr(ctx: &HleCallContext) -> i64 {
    let cfg_addr = ctx.args[0] as u32;
    let attr_addr = ctx.args[1] as u32;
    debug!("cellVpostQueryAttr(cfg=0x{:08x}, attr=0x{:08x})", cfg_addr, attr_addr);
    unsafe {
        crate::cell_vpost::cell_vpost_query_attr(
            cfg_addr as *const crate::cell_vpost::CellVpostCfg,
            attr_addr as *mut crate::cell_vpost::CellVpostResource,
        ) as i64
    }
}

fn hle_vpost_open(ctx: &HleCallContext) -> i64 {
    let cfg_addr = ctx.args[0] as u32;
    let resource_addr = ctx.args[1] as u32;
    let handle_addr = ctx.args[2] as u32;
    debug!("cellVpostOpen(cfg=0x{:08x}, handle=0x{:08x})", cfg_addr, handle_addr);
    unsafe {
        crate::cell_vpost::cell_vpost_open(
            cfg_addr as *const crate::cell_vpost::CellVpostCfg,
            resource_addr as *const crate::cell_vpost::CellVpostResource,
            handle_addr as *mut crate::cell_vpost::VpostHandle,
        ) as i64
    }
}

fn hle_vpost_close(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    trace!("cellVpostClose(handle={})", handle);
    crate::cell_vpost::cell_vpost_close(handle) as i64
}

fn hle_vpost_exec(ctx: &HleCallContext) -> i64 {
    let handle = ctx.args[0] as u32;
    let in_buffer = ctx.args[1] as u32;
    let ctrl_param_addr = ctx.args[2] as u32;
    let out_buffer = ctx.args[3] as u32;
    let pic_info_addr = ctx.args[4] as u32;
    debug!("cellVpostExec(handle={})", handle);
    unsafe {
        crate::cell_vpost::cell_vpost_exec(
            handle,
            in_buffer as *const u8,
            ctrl_param_addr as *const crate::cell_vpost::CellVpostCtrlParam,
            out_buffer as *mut u8,
            pic_info_addr as *mut crate::cell_vpost::CellVpostPictureInfo,
        ) as i64
    }
}

// --- cellNetCtl (Network Control) ---

fn hle_net_ctl_init(_ctx: &HleCallContext) -> i64 {
    debug!("cellNetCtlInit()");
    crate::cell_net_ctl::cell_net_ctl_init() as i64
}

fn hle_net_ctl_term(_ctx: &HleCallContext) -> i64 {
    debug!("cellNetCtlTerm()");
    crate::cell_net_ctl::cell_net_ctl_term() as i64
}

fn hle_net_ctl_get_state(ctx: &HleCallContext) -> i64 {
    let state_addr = ctx.args[0] as u32;
    trace!("cellNetCtlGetState(state=0x{:08x})", state_addr);
    crate::cell_net_ctl::cell_net_ctl_get_state(state_addr) as i64
}

fn hle_net_ctl_get_info(ctx: &HleCallContext) -> i64 {
    let code = ctx.args[0] as u32;
    let info_addr = ctx.args[1] as u32;
    trace!("cellNetCtlGetInfo(code={}, info=0x{:08x})", code, info_addr);
    crate::cell_net_ctl::cell_net_ctl_get_info(code, info_addr) as i64
}

fn hle_net_ctl_net_start_dialog_load_async(ctx: &HleCallContext) -> i64 {
    let param_addr = ctx.args[0] as u32;
    debug!("cellNetCtlNetStartDialogLoadAsync(param=0x{:08x})", param_addr);
    crate::cell_net_ctl::cell_net_ctl_net_start_dialog_load_async(param_addr) as i64
}

fn hle_net_ctl_net_start_dialog_unload_async(ctx: &HleCallContext) -> i64 {
    let result_addr = ctx.args[0] as u32;
    debug!("cellNetCtlNetStartDialogUnloadAsync(result=0x{:08x})", result_addr);
    crate::cell_net_ctl::cell_net_ctl_net_start_dialog_unload_async(result_addr) as i64
}

fn hle_net_ctl_get_nat_info(ctx: &HleCallContext) -> i64 {
    let nat_info_addr = ctx.args[0] as u32;
    trace!("cellNetCtlGetNatInfo(info=0x{:08x})", nat_info_addr);
    crate::cell_net_ctl::cell_net_ctl_get_nat_info(nat_info_addr) as i64
}

fn hle_net_ctl_add_handler(ctx: &HleCallContext) -> i64 {
    let handler = ctx.args[0] as u32;
    let arg = ctx.args[1] as u32;
    let hid_addr = ctx.args[2] as u32;
    debug!("cellNetCtlAddHandler(handler=0x{:08x}, hid=0x{:08x})", handler, hid_addr);
    crate::cell_net_ctl::cell_net_ctl_add_handler(handler, arg, hid_addr) as i64
}

fn hle_net_ctl_del_handler(ctx: &HleCallContext) -> i64 {
    let hid = ctx.args[0] as u32;
    trace!("cellNetCtlDelHandler(hid={})", hid);
    crate::cell_net_ctl::cell_net_ctl_del_handler(hid) as i64
}

// --- cellHttp (HTTP Client) ---

fn hle_http_init(ctx: &HleCallContext) -> i64 {
    let pool_size = ctx.args[0] as u32;
    debug!("cellHttpInit(pool_size={})", pool_size);
    crate::cell_http::cell_http_init(pool_size) as i64
}

fn hle_http_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellHttpEnd()");
    crate::cell_http::cell_http_end() as i64
}

fn hle_http_create_client(ctx: &HleCallContext) -> i64 {
    let client_addr = ctx.args[0] as u32;
    debug!("cellHttpCreateClient(client=0x{:08x})", client_addr);
    crate::cell_http::cell_http_create_client(client_addr) as i64
}

fn hle_http_destroy_client(ctx: &HleCallContext) -> i64 {
    let client = ctx.args[0] as u32;
    trace!("cellHttpDestroyClient(client={})", client);
    crate::cell_http::cell_http_destroy_client(client) as i64
}

fn hle_http_create_transaction(ctx: &HleCallContext) -> i64 {
    let client = ctx.args[0] as u32;
    let method = ctx.args[1] as u32;
    let url_addr = ctx.args[2] as u32;
    let transaction_addr = ctx.args[3] as u32;
    debug!("cellHttpCreateTransaction(client={}, method={})", client, method);
    crate::cell_http::cell_http_create_transaction(client, method, url_addr, transaction_addr) as i64
}

fn hle_http_destroy_transaction(ctx: &HleCallContext) -> i64 {
    let transaction = ctx.args[0] as u32;
    trace!("cellHttpDestroyTransaction(transaction={})", transaction);
    crate::cell_http::cell_http_destroy_transaction(transaction) as i64
}

fn hle_http_send_request(ctx: &HleCallContext) -> i64 {
    let transaction = ctx.args[0] as u32;
    let data_addr = ctx.args[1] as u32;
    let size = ctx.args[2];
    debug!("cellHttpSendRequest(transaction={}, size={})", transaction, size);
    crate::cell_http::cell_http_send_request(transaction, data_addr, size) as i64
}

fn hle_http_recv_response(ctx: &HleCallContext) -> i64 {
    let transaction = ctx.args[0] as u32;
    let data_addr = ctx.args[1] as u32;
    let size = ctx.args[2];
    debug!("cellHttpRecvResponse(transaction={}, size={})", transaction, size);
    crate::cell_http::cell_http_recv_response(transaction, data_addr, size)
}

fn hle_http_add_request_header(ctx: &HleCallContext) -> i64 {
    let transaction = ctx.args[0] as u32;
    let name_addr = ctx.args[1] as u32;
    let value_addr = ctx.args[2] as u32;
    trace!("cellHttpAddRequestHeader(transaction={})", transaction);
    crate::cell_http::cell_http_add_request_header(transaction, name_addr, value_addr) as i64
}

fn hle_http_get_status_code(ctx: &HleCallContext) -> i64 {
    let transaction = ctx.args[0] as u32;
    let status_code_addr = ctx.args[1] as u32;
    trace!("cellHttpGetStatusCode(transaction={})", transaction);
    crate::cell_http::cell_http_get_status_code(transaction, status_code_addr) as i64
}

fn hle_http_get_response_header(ctx: &HleCallContext) -> i64 {
    let transaction = ctx.args[0] as u32;
    let name_addr = ctx.args[1] as u32;
    let value_addr = ctx.args[2] as u32;
    let value_len_addr = ctx.args[3] as u32;
    trace!("cellHttpGetResponseHeader(transaction={})", transaction);
    crate::cell_http::cell_http_get_response_header(transaction, name_addr, value_addr, value_len_addr) as i64
}

fn hle_http_set_proxy(ctx: &HleCallContext) -> i64 {
    let client = ctx.args[0] as u32;
    let host_addr = ctx.args[1] as u32;
    let port = ctx.args[2] as u16;
    debug!("cellHttpSetProxy(client={}, port={})", client, port);
    crate::cell_http::cell_http_set_proxy(client, host_addr, port) as i64
}

// --- cellSsl (SSL/TLS) ---

fn hle_ssl_init(ctx: &HleCallContext) -> i64 {
    let pool_size = ctx.args[0] as u32;
    debug!("cellSslInit(pool_size={})", pool_size);
    crate::cell_ssl::cell_ssl_init(pool_size) as i64
}

fn hle_ssl_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellSslEnd()");
    crate::cell_ssl::cell_ssl_end() as i64
}

fn hle_ssl_cert_get_serial_number(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let serial_addr = ctx.args[1] as u32;
    let length_addr = ctx.args[2] as u32;
    trace!("cellSslCertGetSerialNumber(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_serial_number(cert_id, serial_addr as *mut u8, length_addr as *mut u32) as i64
}

fn hle_ssl_cert_get_public_key(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let key_addr = ctx.args[1] as u32;
    let length_addr = ctx.args[2] as u32;
    trace!("cellSslCertGetPublicKey(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_public_key(cert_id, key_addr as *mut u8, length_addr as *mut u32) as i64
}

fn hle_ssl_cert_get_rsa_public_key_modulus(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let modulus_addr = ctx.args[1] as u32;
    let length_addr = ctx.args[2] as u32;
    trace!("cellSslCertGetRsaPublicKeyModulus(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_rsa_public_key_modulus(cert_id, modulus_addr as *mut u8, length_addr as *mut u32) as i64
}

fn hle_ssl_cert_get_rsa_public_key_exponent(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let exponent_addr = ctx.args[1] as u32;
    let length_addr = ctx.args[2] as u32;
    trace!("cellSslCertGetRsaPublicKeyExponent(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_rsa_public_key_exponent(cert_id, exponent_addr as *mut u8, length_addr as *mut u32) as i64
}

fn hle_ssl_cert_get_not_before(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let begin_addr = ctx.args[1] as u32;
    trace!("cellSslCertGetNotBefore(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_not_before(cert_id, begin_addr as *mut u64) as i64
}

fn hle_ssl_cert_get_not_after(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let limit_addr = ctx.args[1] as u32;
    trace!("cellSslCertGetNotAfter(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_not_after(cert_id, limit_addr as *mut u64) as i64
}

fn hle_ssl_cert_get_subject_name(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let subject_addr = ctx.args[1] as u32;
    let length_addr = ctx.args[2] as u32;
    trace!("cellSslCertGetSubjectName(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_subject_name(cert_id, subject_addr as *mut u8, length_addr as *mut u32) as i64
}

fn hle_ssl_cert_get_issuer_name(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    let issuer_addr = ctx.args[1] as u32;
    let length_addr = ctx.args[2] as u32;
    trace!("cellSslCertGetIssuerName(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_get_issuer_name(cert_id, issuer_addr as *mut u8, length_addr as *mut u32) as i64
}

fn hle_ssl_cert_unload(ctx: &HleCallContext) -> i64 {
    let cert_id = ctx.args[0] as u32;
    trace!("cellSslCertUnload(cert={})", cert_id);
    crate::cell_ssl::cell_ssl_cert_unload(cert_id) as i64
}

// --- cellKb (Keyboard Input) ---

fn hle_kb_init(ctx: &HleCallContext) -> i64 {
    let max_connect = ctx.args[0] as u32;
    debug!("cellKbInit(max_connect={})", max_connect);
    crate::cell_kb::cell_kb_init(max_connect) as i64
}

fn hle_kb_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellKbEnd()");
    crate::cell_kb::cell_kb_end() as i64
}

fn hle_kb_get_info(ctx: &HleCallContext) -> i64 {
    let info_addr = ctx.args[0] as u32;
    trace!("cellKbGetInfo(info=0x{:08x})", info_addr);
    crate::cell_kb::cell_kb_get_info(info_addr) as i64
}

fn hle_kb_read(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let data_addr = ctx.args[1] as u32;
    trace!("cellKbRead(port={}, data=0x{:08x})", port, data_addr);
    crate::cell_kb::cell_kb_read(port, data_addr) as i64
}

fn hle_kb_set_read_mode(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let read_mode = ctx.args[1] as u32;
    trace!("cellKbSetReadMode(port={}, mode={})", port, read_mode);
    crate::cell_kb::cell_kb_set_read_mode(port, read_mode) as i64
}

fn hle_kb_set_code_type(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let code_type = ctx.args[1] as u32;
    trace!("cellKbSetCodeType(port={}, type={})", port, code_type);
    crate::cell_kb::cell_kb_set_code_type(port, code_type) as i64
}

fn hle_kb_set_led_status(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let led = ctx.args[1] as u32;
    trace!("cellKbSetLedStatus(port={}, led={})", port, led);
    crate::cell_kb::cell_kb_set_led_status(port, led) as i64
}

fn hle_kb_clear_buf(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    trace!("cellKbClearBuf(port={})", port);
    crate::cell_kb::cell_kb_clear_buf(port) as i64
}

// --- cellMouse (Mouse Input) ---

fn hle_mouse_init(ctx: &HleCallContext) -> i64 {
    let max_connect = ctx.args[0] as u32;
    debug!("cellMouseInit(max_connect={})", max_connect);
    crate::cell_mouse::cell_mouse_init(max_connect) as i64
}

fn hle_mouse_end(_ctx: &HleCallContext) -> i64 {
    debug!("cellMouseEnd()");
    crate::cell_mouse::cell_mouse_end() as i64
}

fn hle_mouse_get_info(ctx: &HleCallContext) -> i64 {
    let info_addr = ctx.args[0] as u32;
    trace!("cellMouseGetInfo(info=0x{:08x})", info_addr);
    crate::cell_mouse::cell_mouse_get_info(info_addr) as i64
}

fn hle_mouse_get_data(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let data_addr = ctx.args[1] as u32;
    trace!("cellMouseGetData(port={}, data=0x{:08x})", port, data_addr);
    crate::cell_mouse::cell_mouse_get_data(port, data_addr) as i64
}

fn hle_mouse_get_data_list(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let data_addr = ctx.args[1] as u32;
    trace!("cellMouseGetDataList(port={}, data=0x{:08x})", port, data_addr);
    crate::cell_mouse::cell_mouse_get_data_list(port, data_addr) as i64
}

fn hle_mouse_get_raw_data(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    let data_addr = ctx.args[1] as u32;
    trace!("cellMouseGetRawData(port={}, data=0x{:08x})", port, data_addr);
    crate::cell_mouse::cell_mouse_get_raw_data(port, data_addr) as i64
}

fn hle_mouse_clear_buf(ctx: &HleCallContext) -> i64 {
    let port = ctx.args[0] as u32;
    trace!("cellMouseClearBuf(port={})", port);
    crate::cell_mouse::cell_mouse_clear_buf(port) as i64
}

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
    dispatcher.register_function("cellAudio", "cellAudioGetPortTimestamp", hle_audio_get_port_timestamp);
    
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
    dispatcher.register_function("cellResc", "cellRescSetConvertAndFlip", hle_resc_set_convert_and_flip);
    
    // cellSpurs
    dispatcher.register_function("cellSpurs", "cellSpursInitialize", hle_spurs_initialize);
    dispatcher.register_function("cellSpurs", "cellSpursFinalize", hle_spurs_finalize);
    dispatcher.register_function("cellSpurs", "cellSpursAttachLv2EventQueue", hle_spurs_attach_lv2_event_queue);
    dispatcher.register_function("cellSpurs", "cellSpursCreateTaskset", hle_spurs_create_taskset);
    dispatcher.register_function("cellSpurs", "cellSpursTasksetAttributeSetName", hle_spurs_taskset_attribute_set_name);
    dispatcher.register_function("cellSpurs", "cellSpursCreateTask", hle_spurs_create_task);
    dispatcher.register_function("cellSpurs", "cellSpursSetMaxContention", hle_spurs_set_max_contention);
    dispatcher.register_function("cellSpurs", "cellSpursSetPriorities", hle_spurs_set_priorities);
    dispatcher.register_function("cellSpurs", "cellSpursGetSpuThreadId", hle_spurs_get_spu_thread_id);
    dispatcher.register_function("cellSpurs", "cellSpursDetachLv2EventQueue", hle_spurs_detach_lv2_event_queue);
    dispatcher.register_function("cellSpurs", "cellSpursAddPolicyModule", hle_spurs_add_policy_module);
    dispatcher.register_function("cellSpurs", "cellSpursInitializeWithAttribute", hle_spurs_initialize_with_attribute);
    dispatcher.register_function("cellSpurs", "cellSpursAttributeInitialize", hle_spurs_attribute_initialize);
    dispatcher.register_function("cellSpurs", "cellSpursAttributeSetNamePrefix", hle_spurs_attribute_set_name_prefix);
    dispatcher.register_function("cellSpurs", "cellSpursGetInfo", hle_spurs_get_info);
    dispatcher.register_function("cellSpurs", "cellSpursWakeUp", hle_spurs_wake_up);
    dispatcher.register_function("cellSpurs", "cellSpursRequestIdleSpu", hle_spurs_request_idle_spu);
    dispatcher.register_function("cellSpurs", "cellSpursGetSpuThreadGroupId", hle_spurs_get_spu_thread_group_id);
    dispatcher.register_function("cellSpurs", "cellSpursShutdownTaskset", hle_spurs_shutdown_taskset);
    dispatcher.register_function("cellSpurs", "cellSpursJoinTaskset", hle_spurs_join_taskset);
    dispatcher.register_function("cellSpurs", "cellSpursCreateTasksetWithAttribute", hle_spurs_create_taskset_with_attribute);
    
    // libsre (Regular Expression Library)
    dispatcher.register_function("libsre", "cellSreCompile", hle_sre_compile);
    dispatcher.register_function("libsre", "cellSreFree", hle_sre_free);
    dispatcher.register_function("libsre", "cellSreMatch", hle_sre_match);
    dispatcher.register_function("libsre", "cellSreSearch", hle_sre_search);
    dispatcher.register_function("libsre", "cellSreReplace", hle_sre_replace);
    dispatcher.register_function("libsre", "cellSreGetError", hle_sre_get_error);
    
    // cellPngDec
    dispatcher.register_function("cellPngDec", "cellPngDecCreate", hle_png_dec_create);
    dispatcher.register_function("cellPngDec", "cellPngDecDestroy", hle_png_dec_destroy);
    dispatcher.register_function("cellPngDec", "cellPngDecOpen", hle_png_dec_open);
    dispatcher.register_function("cellPngDec", "cellPngDecClose", hle_png_dec_close);
    dispatcher.register_function("cellPngDec", "cellPngDecReadHeader", hle_png_dec_read_header);
    dispatcher.register_function("cellPngDec", "cellPngDecSetParameter", hle_png_dec_set_parameter);
    dispatcher.register_function("cellPngDec", "cellPngDecDecodeData", hle_png_dec_decode_data);
    
    // cellJpgDec
    dispatcher.register_function("cellJpgDec", "cellJpgDecCreate", hle_jpg_dec_create);
    dispatcher.register_function("cellJpgDec", "cellJpgDecDestroy", hle_jpg_dec_destroy);
    dispatcher.register_function("cellJpgDec", "cellJpgDecOpen", hle_jpg_dec_open);
    dispatcher.register_function("cellJpgDec", "cellJpgDecClose", hle_jpg_dec_close);
    dispatcher.register_function("cellJpgDec", "cellJpgDecReadHeader", hle_jpg_dec_read_header);
    dispatcher.register_function("cellJpgDec", "cellJpgDecDecodeData", hle_jpg_dec_decode_data);
    
    // cellGifDec
    dispatcher.register_function("cellGifDec", "cellGifDecCreate", hle_gif_dec_create);
    dispatcher.register_function("cellGifDec", "cellGifDecDestroy", hle_gif_dec_destroy);
    dispatcher.register_function("cellGifDec", "cellGifDecOpen", hle_gif_dec_open);
    dispatcher.register_function("cellGifDec", "cellGifDecClose", hle_gif_dec_close);
    dispatcher.register_function("cellGifDec", "cellGifDecReadHeader", hle_gif_dec_read_header);
    dispatcher.register_function("cellGifDec", "cellGifDecDecodeData", hle_gif_dec_decode_data);
    
    // cellFont
    dispatcher.register_function("cellFont", "cellFontInit", hle_font_init);
    dispatcher.register_function("cellFont", "cellFontEnd", hle_font_end);
    dispatcher.register_function("cellFont", "cellFontOpenFontMemory", hle_font_open_font_memory);
    dispatcher.register_function("cellFont", "cellFontOpenFontFile", hle_font_open_font_file);
    dispatcher.register_function("cellFont", "cellFontCloseFont", hle_font_close_font);
    dispatcher.register_function("cellFont", "cellFontCreateRenderer", hle_font_create_renderer);
    dispatcher.register_function("cellFont", "cellFontDestroyRenderer", hle_font_destroy_renderer);
    dispatcher.register_function("cellFont", "cellFontRenderCharGlyphImage", hle_font_render_char_glyph_image);
    dispatcher.register_function("cellFont", "cellFontGetHorizontalLayout", hle_font_get_horizontal_layout);
    
    // cellFontFT (FreeType path)
    dispatcher.register_function("cellFontFT", "cellFontFTInit", hle_font_ft_init);
    dispatcher.register_function("cellFontFT", "cellFontFTEnd", hle_font_ft_end);
    dispatcher.register_function("cellFontFT", "cellFontFTOpenFontMemory", hle_font_ft_open_font_memory);
    dispatcher.register_function("cellFontFT", "cellFontFTLoadGlyph", hle_font_ft_load_glyph);
    dispatcher.register_function("cellFontFT", "cellFontFTSetCharSize", hle_font_ft_set_char_size);
    dispatcher.register_function("cellFontFT", "cellFontFTGetCharIndex", hle_font_ft_get_char_index);
    
    // cellSaveData
    dispatcher.register_function("cellSaveData", "cellSaveDataListLoad2", hle_save_data_list_load2);
    dispatcher.register_function("cellSaveData", "cellSaveDataListSave2", hle_save_data_list_save2);
    dispatcher.register_function("cellSaveData", "cellSaveDataAutoLoad2", hle_save_data_auto_load2);
    dispatcher.register_function("cellSaveData", "cellSaveDataAutoSave2", hle_save_data_auto_save2);
    dispatcher.register_function("cellSaveData", "cellSaveDataFixedLoad2", hle_save_data_fixed_load2);
    dispatcher.register_function("cellSaveData", "cellSaveDataFixedSave2", hle_save_data_fixed_save2);
    dispatcher.register_function("cellSaveData", "cellSaveDataDelete2", hle_save_data_delete2);
    
    // cellMsgDialog
    dispatcher.register_function("cellMsgDialog", "cellMsgDialogOpen2", hle_msg_dialog_open2);
    dispatcher.register_function("cellMsgDialog", "cellMsgDialogClose", hle_msg_dialog_close);
    dispatcher.register_function("cellMsgDialog", "cellMsgDialogProgressBarSetMsg", hle_msg_dialog_progress_bar_set_msg);
    dispatcher.register_function("cellMsgDialog", "cellMsgDialogProgressBarInc", hle_msg_dialog_progress_bar_inc);
    
    // cellSysutil - BGM playback
    dispatcher.register_function("cellSysutil", "cellSysutilGetBgmPlaybackStatus", hle_sysutil_get_bgm_playback_status);
    dispatcher.register_function("cellSysutil", "cellSysutilEnableBgmPlayback", hle_sysutil_enable_bgm_playback);
    dispatcher.register_function("cellSysutil", "cellSysutilDisableBgmPlayback", hle_sysutil_disable_bgm_playback);
    dispatcher.register_function("cellSysutil", "cellSysutilSetBgmPlaybackVolume", hle_sysutil_set_bgm_playback_volume);
    
    // cellVdec - Video Decoder
    dispatcher.register_function("cellVdec", "cellVdecQueryAttr", hle_vdec_query_attr);
    dispatcher.register_function("cellVdec", "cellVdecOpen", hle_vdec_open);
    dispatcher.register_function("cellVdec", "cellVdecClose", hle_vdec_close);
    dispatcher.register_function("cellVdec", "cellVdecStartSeq", hle_vdec_start_seq);
    dispatcher.register_function("cellVdec", "cellVdecEndSeq", hle_vdec_end_seq);
    dispatcher.register_function("cellVdec", "cellVdecDecodeAu", hle_vdec_decode_au);
    dispatcher.register_function("cellVdec", "cellVdecGetPicture", hle_vdec_get_picture);
    dispatcher.register_function("cellVdec", "cellVdecGetPicItem", hle_vdec_get_pic_item);
    dispatcher.register_function("cellVdec", "cellVdecSetFrameRate", hle_vdec_set_frame_rate);
    
    // cellAdec - Audio Decoder
    dispatcher.register_function("cellAdec", "cellAdecQueryAttr", hle_adec_query_attr);
    dispatcher.register_function("cellAdec", "cellAdecOpen", hle_adec_open);
    dispatcher.register_function("cellAdec", "cellAdecClose", hle_adec_close);
    dispatcher.register_function("cellAdec", "cellAdecStartSeq", hle_adec_start_seq);
    dispatcher.register_function("cellAdec", "cellAdecEndSeq", hle_adec_end_seq);
    dispatcher.register_function("cellAdec", "cellAdecDecodeAu", hle_adec_decode_au);
    dispatcher.register_function("cellAdec", "cellAdecGetPcm", hle_adec_get_pcm);
    dispatcher.register_function("cellAdec", "cellAdecGetPcmItem", hle_adec_get_pcm_item);
    
    // cellDmux - Demultiplexer
    dispatcher.register_function("cellDmux", "cellDmuxQueryAttr", hle_dmux_query_attr);
    dispatcher.register_function("cellDmux", "cellDmuxOpen", hle_dmux_open);
    dispatcher.register_function("cellDmux", "cellDmuxClose", hle_dmux_close);
    dispatcher.register_function("cellDmux", "cellDmuxSetStream", hle_dmux_set_stream);
    dispatcher.register_function("cellDmux", "cellDmuxResetStream", hle_dmux_reset_stream);
    dispatcher.register_function("cellDmux", "cellDmuxEnableEs", hle_dmux_enable_es);
    dispatcher.register_function("cellDmux", "cellDmuxDisableEs", hle_dmux_disable_es);
    dispatcher.register_function("cellDmux", "cellDmuxResetEs", hle_dmux_reset_es);
    dispatcher.register_function("cellDmux", "cellDmuxGetAu", hle_dmux_get_au);
    dispatcher.register_function("cellDmux", "cellDmuxPeekAu", hle_dmux_peek_au);
    dispatcher.register_function("cellDmux", "cellDmuxReleaseAu", hle_dmux_release_au);
    
    // cellVpost - Video Post-Processor
    dispatcher.register_function("cellVpost", "cellVpostQueryAttr", hle_vpost_query_attr);
    dispatcher.register_function("cellVpost", "cellVpostOpen", hle_vpost_open);
    dispatcher.register_function("cellVpost", "cellVpostClose", hle_vpost_close);
    dispatcher.register_function("cellVpost", "cellVpostExec", hle_vpost_exec);
    
    // cellNetCtl - Network Control
    dispatcher.register_function("cellNetCtl", "cellNetCtlInit", hle_net_ctl_init);
    dispatcher.register_function("cellNetCtl", "cellNetCtlTerm", hle_net_ctl_term);
    dispatcher.register_function("cellNetCtl", "cellNetCtlGetState", hle_net_ctl_get_state);
    dispatcher.register_function("cellNetCtl", "cellNetCtlGetInfo", hle_net_ctl_get_info);
    dispatcher.register_function("cellNetCtl", "cellNetCtlNetStartDialogLoadAsync", hle_net_ctl_net_start_dialog_load_async);
    dispatcher.register_function("cellNetCtl", "cellNetCtlNetStartDialogUnloadAsync", hle_net_ctl_net_start_dialog_unload_async);
    dispatcher.register_function("cellNetCtl", "cellNetCtlGetNatInfo", hle_net_ctl_get_nat_info);
    dispatcher.register_function("cellNetCtl", "cellNetCtlAddHandler", hle_net_ctl_add_handler);
    dispatcher.register_function("cellNetCtl", "cellNetCtlDelHandler", hle_net_ctl_del_handler);
    
    // cellHttp - HTTP Client
    dispatcher.register_function("cellHttp", "cellHttpInit", hle_http_init);
    dispatcher.register_function("cellHttp", "cellHttpEnd", hle_http_end);
    dispatcher.register_function("cellHttp", "cellHttpCreateClient", hle_http_create_client);
    dispatcher.register_function("cellHttp", "cellHttpDestroyClient", hle_http_destroy_client);
    dispatcher.register_function("cellHttp", "cellHttpCreateTransaction", hle_http_create_transaction);
    dispatcher.register_function("cellHttp", "cellHttpDestroyTransaction", hle_http_destroy_transaction);
    dispatcher.register_function("cellHttp", "cellHttpSendRequest", hle_http_send_request);
    dispatcher.register_function("cellHttp", "cellHttpRecvResponse", hle_http_recv_response);
    dispatcher.register_function("cellHttp", "cellHttpAddRequestHeader", hle_http_add_request_header);
    dispatcher.register_function("cellHttp", "cellHttpGetStatusCode", hle_http_get_status_code);
    dispatcher.register_function("cellHttp", "cellHttpGetResponseHeader", hle_http_get_response_header);
    dispatcher.register_function("cellHttp", "cellHttpSetProxy", hle_http_set_proxy);
    
    // cellSsl - SSL/TLS
    dispatcher.register_function("cellSsl", "cellSslInit", hle_ssl_init);
    dispatcher.register_function("cellSsl", "cellSslEnd", hle_ssl_end);
    dispatcher.register_function("cellSsl", "cellSslCertGetSerialNumber", hle_ssl_cert_get_serial_number);
    dispatcher.register_function("cellSsl", "cellSslCertGetPublicKey", hle_ssl_cert_get_public_key);
    dispatcher.register_function("cellSsl", "cellSslCertGetRsaPublicKeyModulus", hle_ssl_cert_get_rsa_public_key_modulus);
    dispatcher.register_function("cellSsl", "cellSslCertGetRsaPublicKeyExponent", hle_ssl_cert_get_rsa_public_key_exponent);
    dispatcher.register_function("cellSsl", "cellSslCertGetNotBefore", hle_ssl_cert_get_not_before);
    dispatcher.register_function("cellSsl", "cellSslCertGetNotAfter", hle_ssl_cert_get_not_after);
    dispatcher.register_function("cellSsl", "cellSslCertGetSubjectName", hle_ssl_cert_get_subject_name);
    dispatcher.register_function("cellSsl", "cellSslCertGetIssuerName", hle_ssl_cert_get_issuer_name);
    dispatcher.register_function("cellSsl", "cellSslCertUnload", hle_ssl_cert_unload);
    
    // cellKb - Keyboard Input
    dispatcher.register_function("cellKb", "cellKbInit", hle_kb_init);
    dispatcher.register_function("cellKb", "cellKbEnd", hle_kb_end);
    dispatcher.register_function("cellKb", "cellKbGetInfo", hle_kb_get_info);
    dispatcher.register_function("cellKb", "cellKbRead", hle_kb_read);
    dispatcher.register_function("cellKb", "cellKbSetReadMode", hle_kb_set_read_mode);
    dispatcher.register_function("cellKb", "cellKbSetCodeType", hle_kb_set_code_type);
    dispatcher.register_function("cellKb", "cellKbSetLedStatus", hle_kb_set_led_status);
    dispatcher.register_function("cellKb", "cellKbClearBuf", hle_kb_clear_buf);
    
    // cellMouse - Mouse Input
    dispatcher.register_function("cellMouse", "cellMouseInit", hle_mouse_init);
    dispatcher.register_function("cellMouse", "cellMouseEnd", hle_mouse_end);
    dispatcher.register_function("cellMouse", "cellMouseGetInfo", hle_mouse_get_info);
    dispatcher.register_function("cellMouse", "cellMouseGetData", hle_mouse_get_data);
    dispatcher.register_function("cellMouse", "cellMouseGetDataList", hle_mouse_get_data_list);
    dispatcher.register_function("cellMouse", "cellMouseGetRawData", hle_mouse_get_raw_data);
    dispatcher.register_function("cellMouse", "cellMouseClearBuf", hle_mouse_clear_buf);
    
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

    #[test]
    fn test_phase3_spurs_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // Verify all Phase 3 SPURS functions are registered
        let spurs_funcs = [
            "cellSpursInitialize",
            "cellSpursFinalize",
            "cellSpursAttachLv2EventQueue",
            "cellSpursCreateTaskset",
            "cellSpursTasksetAttributeSetName",
            "cellSpursCreateTask",
            "cellSpursSetMaxContention",
            "cellSpursSetPriorities",
            "cellSpursGetSpuThreadId",
        ];

        for func_name in &spurs_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "SPURS function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase3_registration_count() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // Phase 3 adds 6 new SPURS functions (was ~47, now ~53)
        assert!(dispatcher.stub_map.len() >= 50,
            "Expected at least 50 registered functions, got {}", dispatcher.stub_map.len());
    }

    #[test]
    fn test_phase4_audio_timestamp_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // Verify cellAudioGetPortTimestamp is registered
        let found = dispatcher.stub_map.values().any(|entry| entry.name == "cellAudioGetPortTimestamp");
        assert!(found, "cellAudioGetPortTimestamp should be registered");
    }

    #[test]
    fn test_phase4_audio_functions_wired() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // All Phase 4 audio functions should be registered
        let audio_funcs = [
            "cellAudioInit",
            "cellAudioQuit",
            "cellAudioPortOpen",
            "cellAudioPortClose",
            "cellAudioPortStart",
            "cellAudioPortStop",
            "cellAudioGetPortConfig",
            "cellAudioGetPortTimestamp",
        ];

        for func_name in &audio_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "Audio function '{}' should be registered", func_name);
        }
    }

    // Phase 5 tests

    #[test]
    fn test_phase5_png_dec_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let png_funcs = [
            "cellPngDecCreate",
            "cellPngDecDestroy",
            "cellPngDecOpen",
            "cellPngDecClose",
            "cellPngDecReadHeader",
            "cellPngDecSetParameter",
            "cellPngDecDecodeData",
        ];

        for func_name in &png_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "PNG decoder function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase5_jpg_dec_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let jpg_funcs = [
            "cellJpgDecCreate",
            "cellJpgDecDestroy",
            "cellJpgDecOpen",
            "cellJpgDecClose",
            "cellJpgDecReadHeader",
            "cellJpgDecDecodeData",
        ];

        for func_name in &jpg_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "JPG decoder function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase5_gif_dec_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let gif_funcs = [
            "cellGifDecCreate",
            "cellGifDecDestroy",
            "cellGifDecOpen",
            "cellGifDecClose",
            "cellGifDecReadHeader",
            "cellGifDecDecodeData",
        ];

        for func_name in &gif_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "GIF decoder function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase5_font_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let font_funcs = [
            "cellFontInit",
            "cellFontEnd",
            "cellFontOpenFontMemory",
            "cellFontOpenFontFile",
            "cellFontCloseFont",
            "cellFontCreateRenderer",
            "cellFontDestroyRenderer",
            "cellFontRenderCharGlyphImage",
            "cellFontGetHorizontalLayout",
        ];

        for func_name in &font_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "Font function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase5_font_ft_registration() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let font_ft_funcs = [
            "cellFontFTInit",
            "cellFontFTEnd",
            "cellFontFTOpenFontMemory",
            "cellFontFTLoadGlyph",
            "cellFontFTSetCharSize",
            "cellFontFTGetCharIndex",
        ];

        for func_name in &font_ft_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "FreeType function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase5_registration_count() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // Phase 5 adds 34 new functions (7 PNG + 6 JPG + 6 GIF + 9 Font + 6 FontFT)
        // Previous count was ~62 (through Phase 4), now ~96
        assert!(dispatcher.stub_map.len() >= 80,
            "Expected at least 80 registered functions, got {}", dispatcher.stub_map.len());
    }

    #[test]
    fn test_phase6_save_data_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let save_data_funcs = [
            "cellSaveDataListLoad2",
            "cellSaveDataListSave2",
            "cellSaveDataAutoLoad2",
            "cellSaveDataAutoSave2",
            "cellSaveDataFixedLoad2",
            "cellSaveDataFixedSave2",
            "cellSaveDataDelete2",
        ];

        for func_name in &save_data_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "SaveData function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase6_msg_dialog_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let msg_dialog_funcs = [
            "cellMsgDialogOpen2",
            "cellMsgDialogClose",
            "cellMsgDialogProgressBarSetMsg",
            "cellMsgDialogProgressBarInc",
        ];

        for func_name in &msg_dialog_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "MsgDialog function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase6_bgm_playback_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let bgm_funcs = [
            "cellSysutilGetBgmPlaybackStatus",
            "cellSysutilEnableBgmPlayback",
            "cellSysutilDisableBgmPlayback",
            "cellSysutilSetBgmPlaybackVolume",
        ];

        for func_name in &bgm_funcs {
            let found = dispatcher.stub_map.values().any(|entry| entry.name == *func_name);
            assert!(found, "BGM function '{}' should be registered", func_name);
        }
    }

    #[test]
    fn test_phase6_resc_set_convert_and_flip_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        let found = dispatcher.stub_map.values().any(|entry| entry.name == "cellRescSetConvertAndFlip");
        assert!(found, "cellRescSetConvertAndFlip should be registered");
    }

    #[test]
    fn test_phase6_registration_count() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);

        // Phase 6 adds: 7 SaveData + 4 MsgDialog + 3 BGM + 1 RescConvertAndFlip = 15
        // Previous count was ~96, now ~111
        assert!(dispatcher.stub_map.len() >= 100,
            "Expected at least 100 registered functions, got {}", dispatcher.stub_map.len());
    }
    
    // Phase 2 (todo.md) media module registration tests
    
    #[test]
    fn test_vdec_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let vdec_functions = [
            "cellVdecQueryAttr", "cellVdecOpen", "cellVdecClose",
            "cellVdecStartSeq", "cellVdecEndSeq", "cellVdecDecodeAu",
            "cellVdecGetPicture", "cellVdecGetPicItem", "cellVdecSetFrameRate",
        ];
        
        for func_name in &vdec_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellVdec function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_adec_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let adec_functions = [
            "cellAdecQueryAttr", "cellAdecOpen", "cellAdecClose",
            "cellAdecStartSeq", "cellAdecEndSeq", "cellAdecDecodeAu",
            "cellAdecGetPcm", "cellAdecGetPcmItem",
        ];
        
        for func_name in &adec_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellAdec function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_dmux_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let dmux_functions = [
            "cellDmuxQueryAttr", "cellDmuxOpen", "cellDmuxClose",
            "cellDmuxSetStream", "cellDmuxResetStream",
            "cellDmuxEnableEs", "cellDmuxDisableEs", "cellDmuxResetEs",
            "cellDmuxGetAu", "cellDmuxPeekAu", "cellDmuxReleaseAu",
        ];
        
        for func_name in &dmux_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellDmux function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_vpost_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let vpost_functions = [
            "cellVpostQueryAttr", "cellVpostOpen", "cellVpostClose", "cellVpostExec",
        ];
        
        for func_name in &vpost_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellVpost function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_media_module_total_count() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        // Phase 2 adds: 9 cellVdec + 8 cellAdec + 11 cellDmux + 4 cellVpost = 32
        // Previous count was ~111, now ~143
        assert!(dispatcher.stub_map.len() >= 140,
            "Expected at least 140 registered functions after media module wiring, got {}",
            dispatcher.stub_map.len());
    }
    
    // Phase 2 continued: network, HTTP, SSL, keyboard, mouse registration tests
    
    #[test]
    fn test_net_ctl_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let net_ctl_functions = [
            "cellNetCtlInit", "cellNetCtlTerm", "cellNetCtlGetState",
            "cellNetCtlGetInfo", "cellNetCtlNetStartDialogLoadAsync",
            "cellNetCtlNetStartDialogUnloadAsync", "cellNetCtlGetNatInfo",
            "cellNetCtlAddHandler", "cellNetCtlDelHandler",
        ];
        
        for func_name in &net_ctl_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellNetCtl function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_http_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let http_functions = [
            "cellHttpInit", "cellHttpEnd", "cellHttpCreateClient",
            "cellHttpDestroyClient", "cellHttpCreateTransaction",
            "cellHttpDestroyTransaction", "cellHttpSendRequest",
            "cellHttpRecvResponse", "cellHttpAddRequestHeader",
            "cellHttpGetStatusCode", "cellHttpGetResponseHeader",
            "cellHttpSetProxy",
        ];
        
        for func_name in &http_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellHttp function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_ssl_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let ssl_functions = [
            "cellSslInit", "cellSslEnd",
            "cellSslCertGetSerialNumber", "cellSslCertGetPublicKey",
            "cellSslCertGetRsaPublicKeyModulus", "cellSslCertGetRsaPublicKeyExponent",
            "cellSslCertGetNotBefore", "cellSslCertGetNotAfter",
            "cellSslCertGetSubjectName", "cellSslCertGetIssuerName",
            "cellSslCertUnload",
        ];
        
        for func_name in &ssl_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellSsl function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_kb_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let kb_functions = [
            "cellKbInit", "cellKbEnd", "cellKbGetInfo", "cellKbRead",
            "cellKbSetReadMode", "cellKbSetCodeType", "cellKbSetLedStatus",
            "cellKbClearBuf",
        ];
        
        for func_name in &kb_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellKb function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_mouse_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let mouse_functions = [
            "cellMouseInit", "cellMouseEnd", "cellMouseGetInfo",
            "cellMouseGetData", "cellMouseGetDataList",
            "cellMouseGetRawData", "cellMouseClearBuf",
        ];
        
        for func_name in &mouse_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "cellMouse function '{}' not registered", func_name);
        }
    }
    
    #[test]
    fn test_all_modules_total_count() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        // Previous ~190 + 13 SPURS + 6 libsre = ~209
        assert!(dispatcher.stub_map.len() >= 200,
            "Expected at least 200 registered functions after all module wiring, got {}",
            dispatcher.stub_map.len());
    }

    #[test]
    fn test_spurs_extended_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let spurs_functions = [
            "cellSpursDetachLv2EventQueue",
            "cellSpursAddPolicyModule",
            "cellSpursInitializeWithAttribute",
            "cellSpursAttributeInitialize",
            "cellSpursAttributeSetNamePrefix",
            "cellSpursGetInfo",
            "cellSpursWakeUp",
            "cellSpursRequestIdleSpu",
            "cellSpursGetSpuThreadGroupId",
            "cellSpursShutdownTaskset",
            "cellSpursJoinTaskset",
            "cellSpursCreateTasksetWithAttribute",
        ];
        for func_name in &spurs_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "SPURS function '{}' not registered", func_name);
        }
    }

    #[test]
    fn test_libsre_functions_registered() {
        let mut dispatcher = HleDispatcher::new();
        register_all_hle_functions(&mut dispatcher);
        
        let sre_functions = [
            "cellSreCompile",
            "cellSreFree",
            "cellSreMatch",
            "cellSreSearch",
            "cellSreReplace",
            "cellSreGetError",
        ];
        for func_name in &sre_functions {
            let found = dispatcher.stub_map.values().any(|info| info.name == *func_name);
            assert!(found, "libsre function '{}' not registered", func_name);
        }
    }
}
