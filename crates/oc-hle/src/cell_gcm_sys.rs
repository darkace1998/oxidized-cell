//! cellGcmSys HLE - Graphics Command Management System
//!
//! This module provides HLE implementations for the PS3's RSX graphics system.
//! It manages display buffers, graphics memory, and the command FIFO.

use std::collections::HashMap;
use tracing::{debug, trace};

/// Maximum number of display buffers
pub const CELL_GCM_MAX_DISPLAY_BUFFERS: usize = 8;

/// GCM configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGcmConfig {
    /// Local memory address
    pub local_addr: u32,
    /// Local memory size
    pub local_size: u32,
    /// I/O memory address (main memory mapped for RSX)
    pub io_addr: u32,
    /// I/O memory size
    pub io_size: u32,
    /// Memory frequency (MHz)
    pub mem_frequency: u32,
    /// Core frequency (MHz)
    pub core_frequency: u32,
}

impl Default for CellGcmConfig {
    fn default() -> Self {
        Self {
            local_addr: 0xC0000000,  // RSX local memory base
            local_size: 256 * 1024 * 1024,  // 256 MB
            io_addr: 0,
            io_size: 0,
            mem_frequency: 650,  // MHz
            core_frequency: 500,  // MHz
        }
    }
}

/// GCM display buffer
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellGcmDisplayBuffer {
    /// Buffer address offset
    pub offset: u32,
    /// Pitch (bytes per line)
    pub pitch: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// GCM flip mode
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellGcmFlipMode {
    /// VSYNC mode (wait for vertical sync)
    Vsync = 1,
    /// HSYNC mode (wait for horizontal sync)
    Hsync = 2,
}

impl Default for CellGcmFlipMode {
    fn default() -> Self {
        CellGcmFlipMode::Vsync
    }
}

// ============================================================================
// RSX Backend Integration
// ============================================================================

/// RSX backend connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RsxConnectionState {
    /// Not connected to RSX backend
    #[default]
    Disconnected,
    /// Connected and ready
    Connected,
    /// Connection error occurred
    Error,
}

/// RSX backend interface trait
/// 
/// This trait defines the interface for connecting the GCM HLE to the actual RSX backend.
/// Implementations should provide the actual graphics rendering functionality.
pub trait RsxBackend: Send + Sync {
    /// Submit a command buffer to the RSX
    fn submit_commands(&mut self, commands: &[u32]) -> Result<(), RsxError>;
    
    /// Configure a display buffer
    fn configure_display_buffer(&mut self, buffer_id: u32, config: &CellGcmDisplayBuffer) -> Result<(), RsxError>;
    
    /// Queue a flip operation
    fn queue_flip(&mut self, buffer_id: u32) -> Result<(), RsxError>;
    
    /// Get the current RSX state
    fn get_state(&self) -> RsxConnectionState;
}

/// RSX operation error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsxError {
    /// RSX not initialized
    NotInitialized,
    /// Invalid parameter
    InvalidParameter,
    /// Command buffer overflow
    BufferOverflow,
    /// Backend error
    BackendError,
}

// ============================================================================
// Command Buffer Submission
// ============================================================================

/// Maximum number of commands in the command buffer
pub const CELL_GCM_MAX_COMMAND_BUFFER_SIZE: usize = 65536;

/// Command buffer entry
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GcmCommand {
    /// Command method (register offset)
    pub method: u32,
    /// Command data
    pub data: u32,
}

/// Command buffer for queuing RSX commands
#[derive(Debug)]
pub struct CommandBuffer {
    /// Commands in the buffer
    commands: Vec<GcmCommand>,
    /// Current write position
    write_pos: usize,
    /// Buffer capacity
    capacity: usize,
}

impl CommandBuffer {
    /// Create a new command buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
            write_pos: 0,
            capacity,
        }
    }

    /// Add a command to the buffer
    pub fn push(&mut self, method: u32, data: u32) -> Result<(), RsxError> {
        if self.commands.len() >= self.capacity {
            return Err(RsxError::BufferOverflow);
        }
        self.commands.push(GcmCommand { method, data });
        self.write_pos += 1;
        Ok(())
    }

    /// Get pending commands
    pub fn get_commands(&self) -> &[GcmCommand] {
        &self.commands
    }

    /// Clear the command buffer
    pub fn clear(&mut self) {
        self.commands.clear();
        self.write_pos = 0;
    }

    /// Get current command count
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for CommandBuffer {
    fn default() -> Self {
        Self::new(CELL_GCM_MAX_COMMAND_BUFFER_SIZE)
    }
}

// ============================================================================
// Texture Management
// ============================================================================

/// Maximum number of texture slots
pub const CELL_GCM_MAX_TEXTURES: usize = 16;

/// Texture format
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellGcmTextureFormat {
    /// 32-bit ARGB
    #[default]
    Argb8 = 0x00,
    /// 16-bit RGB565
    Rgb565 = 0x01,
    /// DXT1 compressed
    Dxt1 = 0x02,
    /// DXT3 compressed
    Dxt3 = 0x03,
    /// DXT5 compressed
    Dxt5 = 0x04,
    /// 16-bit depth
    Depth16 = 0x10,
    /// 24-bit depth
    Depth24 = 0x11,
}

/// Texture descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellGcmTexture {
    /// Texture format
    pub format: u32,
    /// Mipmap levels
    pub mipmap: u8,
    /// Dimension (1D, 2D, 3D, Cube)
    pub dimension: u8,
    /// Cubemap flag
    pub cubemap: bool,
    /// Texture width
    pub width: u16,
    /// Texture height
    pub height: u16,
    /// Texture depth
    pub depth: u16,
    /// Pitch (bytes per row)
    pub pitch: u32,
    /// Texture data offset in video memory
    pub offset: u32,
    /// Location (local or main memory)
    pub location: u8,
}

/// Texture slot state
#[derive(Debug, Clone, Copy, Default)]
struct TextureSlot {
    /// Whether the slot is in use
    in_use: bool,
    /// Texture descriptor
    texture: CellGcmTexture,
}

// ============================================================================
// Render Target Configuration
// ============================================================================

/// Maximum number of render targets (MRT)
pub const CELL_GCM_MAX_RENDER_TARGETS: usize = 4;

/// Render target format
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellGcmSurfaceFormat {
    /// 32-bit ARGB8
    #[default]
    Argb8 = 0x00,
    /// 32-bit floating point
    Float32 = 0x01,
    /// 16-bit floating point RGBA
    HalfFloat4 = 0x02,
    /// 32-bit RGBA8
    Rgba8 = 0x03,
}

/// Depth buffer format
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellGcmDepthFormat {
    /// 16-bit depth
    #[default]
    Z16 = 0x00,
    /// 24-bit depth, 8-bit stencil
    Z24S8 = 0x01,
}

/// Render target configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellGcmSurface {
    /// Color format
    pub color_format: u32,
    /// Depth format
    pub depth_format: u32,
    /// Surface width
    pub width: u16,
    /// Surface height
    pub height: u16,
    /// Color buffer offsets (up to 4 MRT)
    pub color_offset: [u32; CELL_GCM_MAX_RENDER_TARGETS],
    /// Color buffer pitches
    pub color_pitch: [u32; CELL_GCM_MAX_RENDER_TARGETS],
    /// Depth buffer offset
    pub depth_offset: u32,
    /// Depth buffer pitch
    pub depth_pitch: u32,
    /// Color buffer location (local/main memory)
    pub color_location: [u8; CELL_GCM_MAX_RENDER_TARGETS],
    /// Depth buffer location
    pub depth_location: u8,
    /// Anti-aliasing mode
    pub antialias: u8,
    /// Render target type (linear/swizzle)
    pub target_type: u8,
}

// ============================================================================
// GCM Manager State
// ============================================================================

/// GCM manager state
pub struct GcmManager {
    /// Initialization flag
    initialized: bool,
    /// Current configuration
    config: CellGcmConfig,
    /// Display buffers
    display_buffers: [CellGcmDisplayBuffer; CELL_GCM_MAX_DISPLAY_BUFFERS],
    /// Current flip mode
    flip_mode: CellGcmFlipMode,
    /// Current display buffer
    current_buffer: u32,
    /// Command buffer context address
    context_addr: u32,
    /// Command buffer size
    context_size: u32,
    /// RSX connection state
    rsx_state: RsxConnectionState,
    /// Command buffer for queuing commands
    command_buffer: CommandBuffer,
    /// Texture slots
    texture_slots: [TextureSlot; CELL_GCM_MAX_TEXTURES],
    /// Current render target configuration
    render_target: CellGcmSurface,
    /// Texture reference counter (for generating unique IDs)
    texture_id_counter: u32,
    /// Active texture bindings (slot -> texture ID)
    texture_bindings: HashMap<u32, u32>,
}

impl GcmManager {
    /// Create a new GCM manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            config: CellGcmConfig::default(),
            display_buffers: [CellGcmDisplayBuffer::default(); CELL_GCM_MAX_DISPLAY_BUFFERS],
            flip_mode: CellGcmFlipMode::default(),
            current_buffer: 0,
            context_addr: 0,
            context_size: 0,
            rsx_state: RsxConnectionState::default(),
            command_buffer: CommandBuffer::default(),
            texture_slots: [TextureSlot::default(); CELL_GCM_MAX_TEXTURES],
            render_target: CellGcmSurface::default(),
            texture_id_counter: 0,
            texture_bindings: HashMap::new(),
        }
    }

    /// Initialize GCM system
    pub fn init(&mut self, context_addr: u32, context_size: u32) -> i32 {
        if self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        debug!(
            "GcmManager::init: context_addr=0x{:08X}, context_size=0x{:X}",
            context_addr, context_size
        );

        self.context_addr = context_addr;
        self.context_size = context_size;
        self.initialized = true;
        
        // Initialize RSX connection (simulated - actual connection would be to oc-rsx)
        self.rsx_state = RsxConnectionState::Connected;
        
        // Initialize command buffer
        self.command_buffer.clear();
        
        // Initialize texture slots
        for slot in &mut self.texture_slots {
            slot.in_use = false;
        }
        
        debug!("GcmManager initialized with RSX connection state: {:?}", self.rsx_state);

        0 // CELL_OK
    }

    /// Set flip mode
    pub fn set_flip_mode(&mut self, mode: CellGcmFlipMode) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        trace!("GcmManager::set_flip_mode: {:?}", mode);
        self.flip_mode = mode;
        
        // Submit flip mode configuration command to RSX
        let _ = self.submit_command(0x0002, mode as u32);

        0 // CELL_OK
    }

    /// Set flip (queue buffer swap)
    pub fn set_flip(&mut self, buffer_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }

        trace!("GcmManager::set_flip: buffer_id={}", buffer_id);
        self.current_buffer = buffer_id;
        
        // Queue flip command to RSX command buffer
        let _ = self.submit_command(0x0001, buffer_id);

        0 // CELL_OK
    }

    /// Set display buffer configuration
    pub fn set_display_buffer(
        &mut self,
        buffer_id: u32,
        offset: u32,
        pitch: u32,
        width: u32,
        height: u32,
    ) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }

        if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }

        // Validate buffer parameters
        if width == 0 || height == 0 || pitch == 0 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }

        debug!(
            "GcmManager::set_display_buffer: id={}, offset=0x{:X}, pitch={}, {}x{}",
            buffer_id, offset, pitch, width, height
        );

        // Store buffer configuration
        self.display_buffers[buffer_id as usize] = CellGcmDisplayBuffer {
            offset,
            pitch,
            width,
            height,
        };

        // TODO: Configure display buffer in RSX

        0 // CELL_OK
    }

    /// Get current configuration
    pub fn get_configuration(&self) -> CellGcmConfig {
        self.config
    }

    /// Convert address to RSX offset
    pub fn address_to_offset(&self, address: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410001u32 as i32); // CELL_GCM_ERROR_FAILURE
        }

        // Check if address is in RSX local memory
        if address >= self.config.local_addr
            && address < (self.config.local_addr + self.config.local_size)
        {
            Ok(address - self.config.local_addr)
        }
        // Check if address is in I/O memory (main memory mapped for RSX)
        else if self.config.io_size > 0
            && address >= self.config.io_addr
            && address < (self.config.io_addr + self.config.io_size)
        {
            Ok(address - self.config.io_addr + self.config.local_size)
        } else {
            Err(0x80410002u32 as i32) // CELL_GCM_ERROR_INVALID_VALUE
        }
    }

    /// Get display buffer info
    pub fn get_display_buffer(&self, buffer_id: u32) -> Option<&CellGcmDisplayBuffer> {
        if buffer_id < CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
            Some(&self.display_buffers[buffer_id as usize])
        } else {
            None
        }
    }

    // ========================================================================
    // RSX Backend Integration
    // ========================================================================

    /// Get the current RSX connection state
    pub fn get_rsx_state(&self) -> RsxConnectionState {
        self.rsx_state
    }

    /// Check if connected to RSX backend
    pub fn is_rsx_connected(&self) -> bool {
        self.rsx_state == RsxConnectionState::Connected
    }

    // ========================================================================
    // Command Buffer Submission
    // ========================================================================

    /// Submit a single command to the command buffer
    pub fn submit_command(&mut self, method: u32, data: u32) -> Result<(), RsxError> {
        if !self.initialized {
            return Err(RsxError::NotInitialized);
        }
        
        trace!("GcmManager::submit_command: method=0x{:04X}, data=0x{:08X}", method, data);
        self.command_buffer.push(method, data)
    }

    /// Submit multiple commands to the command buffer
    pub fn submit_commands(&mut self, commands: &[(u32, u32)]) -> Result<(), RsxError> {
        if !self.initialized {
            return Err(RsxError::NotInitialized);
        }
        
        for &(method, data) in commands {
            self.command_buffer.push(method, data)?;
        }
        
        debug!("GcmManager::submit_commands: submitted {} commands", commands.len());
        Ok(())
    }

    /// Flush the command buffer (submit to RSX for execution)
    pub fn flush_commands(&mut self) -> Result<usize, RsxError> {
        if !self.initialized {
            return Err(RsxError::NotInitialized);
        }
        
        if self.rsx_state != RsxConnectionState::Connected {
            return Err(RsxError::BackendError);
        }
        
        let command_count = self.command_buffer.len();
        
        // In a real implementation, this would send commands to oc-rsx
        // For now, we just clear the buffer to simulate processing
        debug!("GcmManager::flush_commands: flushing {} commands to RSX", command_count);
        
        self.command_buffer.clear();
        
        Ok(command_count)
    }

    /// Get the number of pending commands
    pub fn pending_command_count(&self) -> usize {
        self.command_buffer.len()
    }

    // ========================================================================
    // Texture Management
    // ========================================================================

    /// Set a texture in a slot
    pub fn set_texture(&mut self, slot: u32, texture: CellGcmTexture) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        if slot >= CELL_GCM_MAX_TEXTURES as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        debug!(
            "GcmManager::set_texture: slot={}, format={}, {}x{}",
            slot, texture.format, texture.width, texture.height
        );
        
        self.texture_slots[slot as usize] = TextureSlot {
            in_use: true,
            texture,
        };
        
        // Generate a texture ID and store the binding
        self.texture_id_counter += 1;
        self.texture_bindings.insert(slot, self.texture_id_counter);
        
        // Submit texture configuration command
        let _ = self.submit_command(0x1800 + slot, texture.offset);
        
        0 // CELL_OK
    }

    /// Invalidate (unbind) a texture from a slot
    pub fn invalidate_texture(&mut self, slot: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        if slot >= CELL_GCM_MAX_TEXTURES as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        trace!("GcmManager::invalidate_texture: slot={}", slot);
        
        self.texture_slots[slot as usize].in_use = false;
        self.texture_bindings.remove(&slot);
        
        0 // CELL_OK
    }

    /// Get texture info from a slot
    pub fn get_texture(&self, slot: u32) -> Option<&CellGcmTexture> {
        if slot >= CELL_GCM_MAX_TEXTURES as u32 {
            return None;
        }
        
        let texture_slot = &self.texture_slots[slot as usize];
        if texture_slot.in_use {
            Some(&texture_slot.texture)
        } else {
            None
        }
    }

    /// Get the number of active textures
    pub fn active_texture_count(&self) -> usize {
        self.texture_slots.iter().filter(|s| s.in_use).count()
    }

    // ========================================================================
    // Render Target Configuration
    // ========================================================================

    /// Set the render target configuration
    pub fn set_surface(&mut self, surface: CellGcmSurface) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        // Validate dimensions
        if surface.width == 0 || surface.height == 0 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        debug!(
            "GcmManager::set_surface: {}x{}, color_format={}, depth_format={}",
            surface.width, surface.height, surface.color_format, surface.depth_format
        );
        
        self.render_target = surface;
        
        // Submit render target configuration commands
        let _ = self.submit_command(0x0200, surface.color_format);
        let _ = self.submit_command(0x0204, surface.depth_format);
        let _ = self.submit_command(0x0208, (surface.width as u32) << 16 | (surface.height as u32));
        
        for i in 0..CELL_GCM_MAX_RENDER_TARGETS {
            let _ = self.submit_command(0x0210 + (i as u32 * 4), surface.color_offset[i]);
        }
        
        let _ = self.submit_command(0x0220, surface.depth_offset);
        
        0 // CELL_OK
    }

    /// Get the current render target configuration
    pub fn get_surface(&self) -> &CellGcmSurface {
        &self.render_target
    }

    /// Set individual color target
    pub fn set_color_target(&mut self, target: u32, offset: u32, pitch: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        if target >= CELL_GCM_MAX_RENDER_TARGETS as u32 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        trace!(
            "GcmManager::set_color_target: target={}, offset=0x{:X}, pitch={}",
            target, offset, pitch
        );
        
        self.render_target.color_offset[target as usize] = offset;
        self.render_target.color_pitch[target as usize] = pitch;
        
        // Submit color target command
        let _ = self.submit_command(0x0210 + target * 4, offset);
        let _ = self.submit_command(0x0230 + target * 4, pitch);
        
        0 // CELL_OK
    }

    /// Set depth target
    pub fn set_depth_target(&mut self, offset: u32, pitch: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        trace!(
            "GcmManager::set_depth_target: offset=0x{:X}, pitch={}",
            offset, pitch
        );
        
        self.render_target.depth_offset = offset;
        self.render_target.depth_pitch = pitch;
        
        // Submit depth target commands
        let _ = self.submit_command(0x0220, offset);
        let _ = self.submit_command(0x0240, pitch);
        
        0 // CELL_OK
    }
}

impl Default for GcmManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellGcmInit - Initialize the graphics system
///
/// # Arguments
/// * `context_addr` - Address for command buffer context
/// * `context_size` - Size of command buffer
/// * `config` - Configuration structure
///
/// # Returns
/// * 0 on success
/// * Error code on failure
pub fn cell_gcm_init(context_addr: u32, context_size: u32, _config_addr: u32) -> i32 {
    debug!(
        "cellGcmInit(context_addr=0x{:08X}, context_size=0x{:X})",
        context_addr, context_size
    );

    // Validate parameters
    if context_size < 1024 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    crate::context::get_hle_context_mut().gcm.init(context_addr, context_size)
}

/// cellGcmSetFlipMode - Set display flip mode
///
/// # Arguments
/// * `mode` - Flip mode (VSYNC or HSYNC)
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_flip_mode(mode: u32) -> i32 {
    let flip_mode = if mode == 1 {
        CellGcmFlipMode::Vsync
    } else {
        CellGcmFlipMode::Hsync
    };
    trace!("cellGcmSetFlipMode(mode={:?})", flip_mode);

    crate::context::get_hle_context_mut().gcm.set_flip_mode(flip_mode)
}

/// cellGcmSetFlip - Flip display buffer
///
/// # Arguments
/// * `buffer_id` - Buffer ID to flip to
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_flip(buffer_id: u32) -> i32 {
    trace!("cellGcmSetFlip(buffer_id={})", buffer_id);

    if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    crate::context::get_hle_context_mut().gcm.set_flip(buffer_id)
}

/// cellGcmSetDisplayBuffer - Configure display buffer
///
/// # Arguments
/// * `buffer_id` - Buffer ID (0 or 1 for double buffering)
/// * `offset` - Memory offset of buffer
/// * `pitch` - Pitch (bytes per line)
/// * `width` - Width in pixels
/// * `height` - Height in pixels
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_display_buffer(
    buffer_id: u32,
    offset: u32,
    pitch: u32,
    width: u32,
    height: u32,
) -> i32 {
    debug!(
        "cellGcmSetDisplayBuffer(id={}, offset=0x{:X}, pitch={}, {}x{})",
        buffer_id, offset, pitch, width, height
    );

    // Validate parameters
    if buffer_id >= CELL_GCM_MAX_DISPLAY_BUFFERS as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    if width == 0 || height == 0 || pitch == 0 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    crate::context::get_hle_context_mut().gcm.set_display_buffer(buffer_id, offset, pitch, width, height)
}

/// cellGcmGetConfiguration - Get current GCM configuration
///
/// # Arguments
/// * `config_addr` - Address to write configuration to
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_get_configuration(_config_addr: u32) -> i32 {
    trace!("cellGcmGetConfiguration()");

    let _config = crate::context::get_hle_context().gcm.get_configuration();
    // TODO: Write configuration to memory at _config_addr

    0 // CELL_OK
}

/// cellGcmAddressToOffset - Convert memory address to RSX offset
///
/// # Arguments
/// * `address` - Memory address
/// * `offset_addr` - Address to write offset to
///
/// # Returns
/// * 0 on success
/// * Error code if address is invalid
pub fn cell_gcm_address_to_offset(address: u32, _offset_addr: u32) -> i32 {
    trace!("cellGcmAddressToOffset(address=0x{:08X})", address);

    match crate::context::get_hle_context().gcm.address_to_offset(address) {
        Ok(_offset) => {
            // TODO: Write offset to memory at _offset_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellGcmGetTiledPitchSize - Calculate pitch size for tiled memory
///
/// # Arguments
/// * `pitch` - Pitch in pixels
///
/// # Returns
/// * Aligned pitch size
pub fn cell_gcm_get_tiled_pitch_size(pitch: u32) -> u32 {
    trace!("cellGcmGetTiledPitchSize(pitch={})", pitch);

    // Align to 64 bytes (minimum tile granularity)
    (pitch + 63) & !63
}

// ============================================================================
// Command Buffer Submission Functions
// ============================================================================

/// cellGcmFlush - Flush command buffer to RSX
///
/// # Returns
/// * 0 on success
/// * Error code on failure
pub fn cell_gcm_flush() -> i32 {
    trace!("cellGcmFlush()");
    
    match crate::context::get_hle_context_mut().gcm.flush_commands() {
        Ok(count) => {
            trace!("Flushed {} commands to RSX", count);
            0 // CELL_OK
        }
        Err(_) => 0x80410001u32 as i32, // CELL_GCM_ERROR_FAILURE
    }
}

/// cellGcmFinish - Wait for RSX to finish processing commands
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_finish() -> i32 {
    debug!("cellGcmFinish()");
    
    // First flush any pending commands
    let _ = crate::context::get_hle_context_mut().gcm.flush_commands();
    
    // In a real implementation, this would wait for RSX to complete
    // For now, we just return success
    0 // CELL_OK
}

// ============================================================================
// Texture Management Functions
// ============================================================================

/// cellGcmSetTexture - Set texture in a slot
///
/// # Arguments
/// * `slot` - Texture slot (0-15)
/// * `texture_addr` - Address of texture descriptor
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_texture(slot: u32, _texture_addr: u32) -> i32 {
    debug!("cellGcmSetTexture(slot={})", slot);
    
    if slot >= CELL_GCM_MAX_TEXTURES as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }
    
    // Default texture dimensions for HLE stub
    const DEFAULT_TEXTURE_WIDTH: u16 = 256;
    const DEFAULT_TEXTURE_HEIGHT: u16 = 256;
    const BYTES_PER_PIXEL: u32 = 4; // ARGB8 format
    
    // Create a default texture descriptor (in real implementation, would read from memory)
    let texture = CellGcmTexture {
        format: CellGcmTextureFormat::Argb8 as u32,
        mipmap: 1,
        dimension: 2, // 2D texture
        cubemap: false,
        width: DEFAULT_TEXTURE_WIDTH,
        height: DEFAULT_TEXTURE_HEIGHT,
        depth: 1,
        pitch: DEFAULT_TEXTURE_WIDTH as u32 * BYTES_PER_PIXEL,
        offset: 0,
        location: 0, // Local memory
    };
    
    crate::context::get_hle_context_mut().gcm.set_texture(slot, texture)
}

/// cellGcmSetTextureAddress - Set texture addressing mode
///
/// # Arguments
/// * `slot` - Texture slot
/// * `wrap_s` - S (U) wrap mode
/// * `wrap_t` - T (V) wrap mode
/// * `wrap_r` - R (W) wrap mode
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_texture_address(slot: u32, _wrap_s: u32, _wrap_t: u32, _wrap_r: u32) -> i32 {
    trace!("cellGcmSetTextureAddress(slot={})", slot);
    
    if slot >= CELL_GCM_MAX_TEXTURES as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }
    
    // Submit texture address configuration command
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x1840 + slot, _wrap_s);
    
    0 // CELL_OK
}

/// cellGcmSetTextureFilter - Set texture filtering mode
///
/// # Arguments
/// * `slot` - Texture slot
/// * `min_filter` - Minification filter
/// * `mag_filter` - Magnification filter
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_texture_filter(slot: u32, _min_filter: u32, _mag_filter: u32) -> i32 {
    trace!("cellGcmSetTextureFilter(slot={})", slot);
    
    if slot >= CELL_GCM_MAX_TEXTURES as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }
    
    // Submit texture filter configuration command
    let filter_value = (_min_filter << 16) | _mag_filter;
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x1850 + slot, filter_value);
    
    0 // CELL_OK
}

/// cellGcmSetTextureControl - Set texture control parameters
///
/// # Arguments
/// * `slot` - Texture slot
/// * `enable` - Enable/disable texture
/// * `min_lod` - Minimum LOD level
/// * `max_lod` - Maximum LOD level
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_texture_control(slot: u32, enable: bool, _min_lod: u32, _max_lod: u32) -> i32 {
    trace!("cellGcmSetTextureControl(slot={}, enable={})", slot, enable);
    
    if slot >= CELL_GCM_MAX_TEXTURES as u32 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }
    
    if !enable {
        return crate::context::get_hle_context_mut().gcm.invalidate_texture(slot);
    }
    
    0 // CELL_OK
}

/// cellGcmInvalidateTextureCache - Invalidate texture cache
///
/// # Arguments
/// * `type` - Cache type to invalidate
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_invalidate_texture_cache(_cache_type: u32) -> i32 {
    debug!("cellGcmInvalidateTextureCache()");
    
    // Submit cache invalidation command
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x1FD8, 0);
    
    0 // CELL_OK
}

// ============================================================================
// Render Target Configuration Functions
// ============================================================================

/// cellGcmSetSurface - Set render target surface
///
/// # Arguments
/// * `surface_addr` - Address of surface descriptor
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_surface(_surface_addr: u32) -> i32 {
    debug!("cellGcmSetSurface()");
    
    // Create a default surface configuration (in real implementation, would read from memory)
    let surface = CellGcmSurface {
        color_format: CellGcmSurfaceFormat::Argb8 as u32,
        depth_format: CellGcmDepthFormat::Z24S8 as u32,
        width: 1920,
        height: 1080,
        color_offset: [0; CELL_GCM_MAX_RENDER_TARGETS],
        color_pitch: [1920 * 4; CELL_GCM_MAX_RENDER_TARGETS],
        depth_offset: 0,
        depth_pitch: 1920 * 4,
        color_location: [0; CELL_GCM_MAX_RENDER_TARGETS],
        depth_location: 0,
        antialias: 0,
        target_type: 0,
    };
    
    crate::context::get_hle_context_mut().gcm.set_surface(surface)
}

/// cellGcmSetColorMask - Set color write mask
///
/// # Arguments
/// * `mask` - Color component mask (RGBA)
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_color_mask(mask: u32) -> i32 {
    trace!("cellGcmSetColorMask(mask=0x{:X})", mask);
    
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x0300, mask);
    0 // CELL_OK
}

/// cellGcmSetDepthMask - Set depth write enable
///
/// # Arguments
/// * `enable` - Enable/disable depth writes
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_depth_mask(enable: bool) -> i32 {
    trace!("cellGcmSetDepthMask(enable={})", enable);
    
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x0304, enable as u32);
    0 // CELL_OK
}

/// cellGcmSetDepthFunc - Set depth comparison function
///
/// # Arguments
/// * `func` - Comparison function
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_depth_func(func: u32) -> i32 {
    trace!("cellGcmSetDepthFunc(func={})", func);
    
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x0308, func);
    0 // CELL_OK
}

/// cellGcmSetClearColor - Set clear color
///
/// # Arguments
/// * `color` - ARGB color value
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_clear_color(color: u32) -> i32 {
    trace!("cellGcmSetClearColor(color=0x{:08X})", color);
    
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x0310, color);
    0 // CELL_OK
}

/// cellGcmSetClearDepthStencil - Set clear depth and stencil values
///
/// # Arguments
/// * `depth` - Depth clear value
/// * `stencil` - Stencil clear value
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_clear_depth_stencil(depth: u32, stencil: u8) -> i32 {
    trace!("cellGcmSetClearDepthStencil(depth={}, stencil={})", depth, stencil);
    
    let value = (depth & 0xFFFFFF00) | (stencil as u32);
    let _ = crate::context::get_hle_context_mut().gcm.submit_command(0x0314, value);
    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcm_manager() {
        let mut manager = GcmManager::new();
        assert_eq!(manager.init(0x10000000, 1024 * 1024), 0);
        
        // Test display buffer configuration
        assert_eq!(manager.set_display_buffer(0, 0x1000, 1920 * 4, 1920, 1080), 0);
        
        // Test flip mode
        assert_eq!(manager.set_flip_mode(CellGcmFlipMode::Vsync), 0);
        
        // Test flip
        assert_eq!(manager.set_flip(0), 0);
        
        // Test configuration retrieval
        let config = manager.get_configuration();
        assert_eq!(config.local_addr, 0xC0000000);
    }

    #[test]
    fn test_gcm_manager_address_conversion() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Test valid local memory address
        let offset = manager.address_to_offset(0xC0001000);
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 0x1000);
        
        // Test invalid address
        let invalid = manager.address_to_offset(0x12345678);
        assert!(invalid.is_err());
    }

    #[test]
    fn test_gcm_manager_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Test invalid buffer ID
        assert!(manager.set_display_buffer(99, 0, 1920, 1920, 1080) != 0);
        
        // Test invalid dimensions
        assert!(manager.set_display_buffer(0, 0, 0, 0, 0) != 0);
        
        // Test invalid flip buffer
        assert!(manager.set_flip(99) != 0);
    }

    #[test]
    fn test_gcm_init() {
        // Reset context first to ensure clean state
        crate::context::reset_hle_context();
        
        let result = cell_gcm_init(0x10000000, 1024 * 1024, 0);
        assert_eq!(result, 0);
        
        // Reset context to test invalid size
        crate::context::reset_hle_context();
        
        // Test invalid context size
        let result = cell_gcm_init(0x10000000, 512, 0);
        assert!(result != 0);
    }

    #[test]
    fn test_gcm_config_default() {
        let config = CellGcmConfig::default();
        assert_eq!(config.local_addr, 0xC0000000);
        assert_eq!(config.local_size, 256 * 1024 * 1024);
    }

    #[test]
    fn test_set_flip_mode() {
        // Reset context and initialize GCM
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        let result = cell_gcm_set_flip_mode(1);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_display_buffer_validation() {
        // Reset context and initialize GCM
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        // Valid call
        assert_eq!(cell_gcm_set_display_buffer(0, 0x1000, 1920 * 4, 1920, 1080), 0);
        
        // Invalid buffer ID
        assert!(cell_gcm_set_display_buffer(99, 0x1000, 1920 * 4, 1920, 1080) != 0);
        
        // Invalid dimensions
        assert!(cell_gcm_set_display_buffer(0, 0x1000, 0, 0, 0) != 0);
    }

    #[test]
    fn test_tiled_pitch_size() {
        assert_eq!(cell_gcm_get_tiled_pitch_size(100), 128);
        assert_eq!(cell_gcm_get_tiled_pitch_size(64), 64);
        assert_eq!(cell_gcm_get_tiled_pitch_size(65), 128);
    }

    // ========================================================================
    // RSX Backend Integration Tests
    // ========================================================================

    #[test]
    fn test_rsx_connection_state() {
        let mut manager = GcmManager::new();
        
        // Before init, should be disconnected
        assert_eq!(manager.get_rsx_state(), RsxConnectionState::Disconnected);
        assert!(!manager.is_rsx_connected());
        
        // After init, should be connected
        manager.init(0x10000000, 1024 * 1024);
        assert_eq!(manager.get_rsx_state(), RsxConnectionState::Connected);
        assert!(manager.is_rsx_connected());
    }

    // ========================================================================
    // Command Buffer Submission Tests
    // ========================================================================

    #[test]
    fn test_command_buffer() {
        let mut buffer = CommandBuffer::new(100);
        
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        
        // Add commands
        assert!(buffer.push(0x0001, 0x12345678).is_ok());
        assert!(buffer.push(0x0002, 0xABCDEF00).is_ok());
        
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 2);
        
        // Check commands
        let commands = buffer.get_commands();
        assert_eq!(commands[0].method, 0x0001);
        assert_eq!(commands[0].data, 0x12345678);
        assert_eq!(commands[1].method, 0x0002);
        assert_eq!(commands[1].data, 0xABCDEF00);
        
        // Clear
        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_command_buffer_overflow() {
        let mut buffer = CommandBuffer::new(2);
        
        assert!(buffer.push(0x0001, 0).is_ok());
        assert!(buffer.push(0x0002, 0).is_ok());
        assert!(buffer.push(0x0003, 0).is_err()); // Should overflow
    }

    #[test]
    fn test_gcm_manager_commands() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Submit commands
        assert!(manager.submit_command(0x0001, 0x12345678).is_ok());
        assert!(manager.submit_commands(&[(0x0002, 0x1111), (0x0003, 0x2222)]).is_ok());
        
        assert_eq!(manager.pending_command_count(), 3);
        
        // Flush commands
        let result = manager.flush_commands();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
        
        assert_eq!(manager.pending_command_count(), 0);
    }

    #[test]
    fn test_gcm_manager_commands_not_initialized() {
        let mut manager = GcmManager::new();
        
        // Should fail when not initialized
        assert!(manager.submit_command(0x0001, 0).is_err());
        assert!(manager.flush_commands().is_err());
    }

    // ========================================================================
    // Texture Management Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_textures() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Set texture
        let texture = CellGcmTexture {
            format: CellGcmTextureFormat::Argb8 as u32,
            mipmap: 1,
            dimension: 2,
            cubemap: false,
            width: 256,
            height: 256,
            depth: 1,
            pitch: 256 * 4,
            offset: 0x1000,
            location: 0,
        };
        
        assert_eq!(manager.set_texture(0, texture), 0);
        assert_eq!(manager.active_texture_count(), 1);
        
        // Get texture
        let tex = manager.get_texture(0);
        assert!(tex.is_some());
        assert_eq!(tex.unwrap().width, 256);
        
        // Invalidate texture
        assert_eq!(manager.invalidate_texture(0), 0);
        assert_eq!(manager.active_texture_count(), 0);
        assert!(manager.get_texture(0).is_none());
    }

    #[test]
    fn test_gcm_manager_texture_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Invalid slot
        assert!(manager.set_texture(99, CellGcmTexture::default()) != 0);
        assert!(manager.invalidate_texture(99) != 0);
        assert!(manager.get_texture(99).is_none());
    }

    // ========================================================================
    // Render Target Configuration Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_render_target() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Set surface
        let surface = CellGcmSurface {
            color_format: CellGcmSurfaceFormat::Argb8 as u32,
            depth_format: CellGcmDepthFormat::Z24S8 as u32,
            width: 1920,
            height: 1080,
            color_offset: [0x0000, 0x1000, 0x2000, 0x3000],
            color_pitch: [1920 * 4; CELL_GCM_MAX_RENDER_TARGETS],
            depth_offset: 0x10000,
            depth_pitch: 1920 * 4,
            color_location: [0; CELL_GCM_MAX_RENDER_TARGETS],
            depth_location: 0,
            antialias: 0,
            target_type: 0,
        };
        
        assert_eq!(manager.set_surface(surface), 0);
        
        let rt = manager.get_surface();
        assert_eq!(rt.width, 1920);
        assert_eq!(rt.height, 1080);
    }

    #[test]
    fn test_gcm_manager_render_target_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Invalid dimensions
        let surface = CellGcmSurface {
            width: 0,
            height: 0,
            ..Default::default()
        };
        
        assert!(manager.set_surface(surface) != 0);
    }

    #[test]
    fn test_gcm_manager_color_depth_targets() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Set color target
        assert_eq!(manager.set_color_target(0, 0x1000, 1920 * 4), 0);
        assert_eq!(manager.set_color_target(1, 0x2000, 1920 * 4), 0);
        
        // Invalid target
        assert!(manager.set_color_target(99, 0, 0) != 0);
        
        // Set depth target
        assert_eq!(manager.set_depth_target(0x10000, 1920 * 4), 0);
    }

    // ========================================================================
    // Public API Tests
    // ========================================================================

    #[test]
    fn test_gcm_flush() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_flush(), 0);
    }

    #[test]
    fn test_gcm_finish() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_finish(), 0);
    }

    #[test]
    fn test_gcm_set_texture() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_texture(0, 0x10000), 0);
        assert!(cell_gcm_set_texture(99, 0x10000) != 0);
    }

    #[test]
    fn test_gcm_texture_functions() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_texture_address(0, 0, 0, 0), 0);
        assert_eq!(cell_gcm_set_texture_filter(0, 0, 0), 0);
        assert_eq!(cell_gcm_set_texture_control(0, true, 0, 10), 0);
        assert_eq!(cell_gcm_invalidate_texture_cache(0), 0);
    }

    #[test]
    fn test_gcm_render_target_functions() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_surface(0x10000), 0);
        assert_eq!(cell_gcm_set_color_mask(0xFFFFFFFF), 0);
        assert_eq!(cell_gcm_set_depth_mask(true), 0);
        assert_eq!(cell_gcm_set_depth_func(1), 0);
        assert_eq!(cell_gcm_set_clear_color(0xFF000000), 0);
        assert_eq!(cell_gcm_set_clear_depth_stencil(0xFFFFFF00, 0), 0);
    }

    #[test]
    fn test_texture_format_enum() {
        assert_eq!(CellGcmTextureFormat::Argb8 as u32, 0x00);
        assert_eq!(CellGcmTextureFormat::Rgb565 as u32, 0x01);
        assert_eq!(CellGcmTextureFormat::Dxt1 as u32, 0x02);
    }

    #[test]
    fn test_surface_format_enum() {
        assert_eq!(CellGcmSurfaceFormat::Argb8 as u32, 0x00);
        assert_eq!(CellGcmSurfaceFormat::Float32 as u32, 0x01);
    }

    #[test]
    fn test_depth_format_enum() {
        assert_eq!(CellGcmDepthFormat::Z16 as u32, 0x00);
        assert_eq!(CellGcmDepthFormat::Z24S8 as u32, 0x01);
    }
}
