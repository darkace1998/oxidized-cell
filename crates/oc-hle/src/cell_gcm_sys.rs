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
#[derive(Default)]
pub enum CellGcmFlipMode {
    /// VSYNC mode (wait for vertical sync)
    #[default]
    Vsync = 1,
    /// HSYNC mode (wait for horizontal sync)
    Hsync = 2,
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

// ============================================================================
// Shader Program Structures
// ============================================================================

/// Vertex program descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellGcmVertexProgram {
    /// Program size in bytes
    pub size: u32,
    /// Program data offset in memory
    pub offset: u32,
    /// Number of instructions
    pub num_instructions: u16,
    /// Number of input attributes
    pub num_inputs: u8,
    /// Number of output attributes
    pub num_outputs: u8,
    /// Input attribute mask
    pub input_mask: u32,
    /// Output attribute mask
    pub output_mask: u32,
}

/// Fragment program descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CellGcmFragmentProgram {
    /// Program size in bytes
    pub size: u32,
    /// Program data offset in memory
    pub offset: u32,
    /// Number of instructions
    pub num_instructions: u16,
    /// Number of texture samplers
    pub num_samplers: u8,
    /// Register count
    pub register_count: u8,
    /// Control register value
    pub control: u32,
}

/// Viewport configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGcmViewport {
    /// X origin
    pub x: u16,
    /// Y origin
    pub y: u16,
    /// Width
    pub width: u16,
    /// Height
    pub height: u16,
    /// Minimum depth (0.0 - 1.0 mapped to hardware range)
    pub z_min: f32,
    /// Maximum depth (0.0 - 1.0 mapped to hardware range)
    pub z_max: f32,
    /// Scale factors
    pub scale: [f32; 4],
    /// Offset factors
    pub offset: [f32; 4],
}

impl Default for CellGcmViewport {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            z_min: 0.0,
            z_max: 1.0,
            scale: [1920.0 / 2.0, 1080.0 / 2.0, 0.5, 0.0],
            offset: [1920.0 / 2.0, 1080.0 / 2.0, 0.5, 0.0],
        }
    }
}

/// Scissor rectangle configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellGcmScissor {
    /// X origin
    pub x: u16,
    /// Y origin
    pub y: u16,
    /// Width
    pub width: u16,
    /// Height
    pub height: u16,
}

impl Default for CellGcmScissor {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 4096,
            height: 4096,
        }
    }
}

/// Main memory mapping entry
#[derive(Debug, Clone, Copy)]
struct MemoryMapping {
    /// Main memory address
    main_addr: u32,
    /// Size in bytes
    size: u32,
    /// RSX offset
    offset: u32,
}

/// Flip status
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellGcmFlipStatus {
    /// Flip is not pending
    #[default]
    NotPending = 0,
    /// Flip is pending
    Pending = 1,
}

/// Primitive type for draw calls
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellGcmPrimitive {
    /// Points
    Points = 1,
    /// Lines
    Lines = 2,
    /// Line loop
    LineLoop = 3,
    /// Line strip
    LineStrip = 4,
    /// Triangles
    #[default]
    Triangles = 5,
    /// Triangle strip
    TriangleStrip = 6,
    /// Triangle fan
    TriangleFan = 7,
    /// Quads
    Quads = 8,
    /// Quad strip
    QuadStrip = 9,
    /// Polygon
    Polygon = 10,
}

/// Index type for indexed draw calls
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellGcmIndexType {
    /// 16-bit indices
    #[default]
    Index16 = 0,
    /// 32-bit indices
    Index32 = 1,
}

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
    /// Current vertex program
    vertex_program: Option<CellGcmVertexProgram>,
    /// Current fragment program
    fragment_program: Option<CellGcmFragmentProgram>,
    /// Current viewport
    viewport: CellGcmViewport,
    /// Current scissor rectangle
    scissor: CellGcmScissor,
    /// Main memory mappings
    memory_mappings: Vec<MemoryMapping>,
    /// Next memory mapping offset
    next_io_offset: u32,
    /// Flip status
    flip_status: CellGcmFlipStatus,
    /// Draw call counter (for statistics)
    draw_call_count: u64,
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
            vertex_program: None,
            fragment_program: None,
            viewport: CellGcmViewport::default(),
            scissor: CellGcmScissor::default(),
            memory_mappings: Vec::new(),
            next_io_offset: 0,
            flip_status: CellGcmFlipStatus::default(),
            draw_call_count: 0,
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
        self.flip_status = CellGcmFlipStatus::Pending;
        
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

    // ========================================================================
    // Shader Program Management
    // ========================================================================

    /// Set vertex program (shader)
    pub fn set_vertex_program(&mut self, program: CellGcmVertexProgram) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        debug!(
            "GcmManager::set_vertex_program: size={}, offset=0x{:X}, instructions={}",
            program.size, program.offset, program.num_instructions
        );
        
        self.vertex_program = Some(program);
        
        // Submit vertex program configuration commands
        // NV4097_SET_TRANSFORM_PROGRAM_LOAD
        let _ = self.submit_command(0x1E94, program.offset);
        let _ = self.submit_command(0x1E98, program.num_instructions as u32);
        
        0 // CELL_OK
    }

    /// Get the current vertex program
    pub fn get_vertex_program(&self) -> Option<&CellGcmVertexProgram> {
        self.vertex_program.as_ref()
    }

    /// Set fragment program (shader)
    pub fn set_fragment_program(&mut self, program: CellGcmFragmentProgram) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        debug!(
            "GcmManager::set_fragment_program: size={}, offset=0x{:X}, instructions={}",
            program.size, program.offset, program.num_instructions
        );
        
        self.fragment_program = Some(program);
        
        // Submit fragment program configuration commands
        // NV4097_SET_SHADER_PROGRAM
        let _ = self.submit_command(0x08E4, program.offset);
        let _ = self.submit_command(0x1D60, program.control);
        
        0 // CELL_OK
    }

    /// Get the current fragment program
    pub fn get_fragment_program(&self) -> Option<&CellGcmFragmentProgram> {
        self.fragment_program.as_ref()
    }

    /// Invalidate current shader programs
    pub fn invalidate_programs(&mut self) {
        self.vertex_program = None;
        self.fragment_program = None;
    }

    // ========================================================================
    // Viewport and Scissor
    // ========================================================================

    /// Set viewport
    pub fn set_viewport(&mut self, x: u16, y: u16, width: u16, height: u16, z_min: f32, z_max: f32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        // Validate dimensions
        if width == 0 || height == 0 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        debug!(
            "GcmManager::set_viewport: x={}, y={}, {}x{}, z={:.2}-{:.2}",
            x, y, width, height, z_min, z_max
        );
        
        // Calculate scale and offset factors
        let scale_x = width as f32 / 2.0;
        let scale_y = height as f32 / 2.0;
        let scale_z = (z_max - z_min) / 2.0;
        let offset_x = x as f32 + scale_x;
        let offset_y = y as f32 + scale_y;
        let offset_z = (z_max + z_min) / 2.0;
        
        self.viewport = CellGcmViewport {
            x,
            y,
            width,
            height,
            z_min,
            z_max,
            scale: [scale_x, scale_y, scale_z, 0.0],
            offset: [offset_x, offset_y, offset_z, 0.0],
        };
        
        // Submit viewport commands
        // NV4097_SET_VIEWPORT_HORIZONTAL
        let _ = self.submit_command(0x0A00, ((width as u32) << 16) | (x as u32));
        // NV4097_SET_VIEWPORT_VERTICAL
        let _ = self.submit_command(0x0A04, ((height as u32) << 16) | (y as u32));
        
        // NV4097_SET_VIEWPORT_SCALE (using bit conversion for floats)
        let _ = self.submit_command(0x0A20, scale_x.to_bits());
        let _ = self.submit_command(0x0A24, scale_y.to_bits());
        let _ = self.submit_command(0x0A28, scale_z.to_bits());
        let _ = self.submit_command(0x0A2C, 0); // w scale
        
        // NV4097_SET_VIEWPORT_OFFSET
        let _ = self.submit_command(0x0A30, offset_x.to_bits());
        let _ = self.submit_command(0x0A34, offset_y.to_bits());
        let _ = self.submit_command(0x0A38, offset_z.to_bits());
        let _ = self.submit_command(0x0A3C, 0); // w offset
        
        0 // CELL_OK
    }

    /// Get the current viewport
    pub fn get_viewport(&self) -> &CellGcmViewport {
        &self.viewport
    }

    /// Set scissor rectangle
    pub fn set_scissor(&mut self, x: u16, y: u16, width: u16, height: u16) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        // Validate dimensions
        if width == 0 || height == 0 {
            return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        trace!(
            "GcmManager::set_scissor: x={}, y={}, {}x{}",
            x, y, width, height
        );
        
        self.scissor = CellGcmScissor {
            x,
            y,
            width,
            height,
        };
        
        // Submit scissor commands
        // NV4097_SET_SCISSOR_HORIZONTAL
        let _ = self.submit_command(0x08C0, ((width as u32) << 16) | (x as u32));
        // NV4097_SET_SCISSOR_VERTICAL
        let _ = self.submit_command(0x08C4, ((height as u32) << 16) | (y as u32));
        
        0 // CELL_OK
    }

    /// Get the current scissor rectangle
    pub fn get_scissor(&self) -> &CellGcmScissor {
        &self.scissor
    }

    // ========================================================================
    // Draw Calls
    // ========================================================================

    /// Draw non-indexed primitives
    pub fn draw_arrays(&mut self, primitive: CellGcmPrimitive, first: u32, count: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        if count == 0 {
            return 0; // Nothing to draw, but not an error
        }
        
        debug!(
            "GcmManager::draw_arrays: primitive={:?}, first={}, count={}",
            primitive, first, count
        );
        
        self.draw_call_count += 1;
        
        // Submit draw call commands
        // NV4097_SET_BEGIN_END
        let _ = self.submit_command(0x1808, primitive as u32);
        // NV4097_DRAW_ARRAYS
        let _ = self.submit_command(0x1814, (first << 8) | (count - 1));
        // NV4097_SET_BEGIN_END (end)
        let _ = self.submit_command(0x1808, 0);
        
        0 // CELL_OK
    }

    /// Draw indexed primitives
    pub fn draw_index_array(&mut self, primitive: CellGcmPrimitive, index_offset: u32, count: u32, index_type: CellGcmIndexType, location: u8) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        if count == 0 {
            return 0; // Nothing to draw, but not an error
        }
        
        debug!(
            "GcmManager::draw_index_array: primitive={:?}, offset=0x{:X}, count={}, type={:?}",
            primitive, index_offset, count, index_type
        );
        
        self.draw_call_count += 1;
        
        // Submit indexed draw call commands
        // NV4097_SET_INDEX_ARRAY_ADDRESS
        let type_value = match index_type {
            CellGcmIndexType::Index16 => 0,
            CellGcmIndexType::Index32 => 1,
        };
        let _ = self.submit_command(0x181C, index_offset);
        let _ = self.submit_command(0x1820, (type_value << 4) | (location as u32));
        
        // NV4097_SET_BEGIN_END
        let _ = self.submit_command(0x1808, primitive as u32);
        // NV4097_DRAW_INDEX_ARRAY
        let _ = self.submit_command(0x1824, count - 1);
        // NV4097_SET_BEGIN_END (end)
        let _ = self.submit_command(0x1808, 0);
        
        0 // CELL_OK
    }

    /// Get the total draw call count (for statistics)
    pub fn get_draw_call_count(&self) -> u64 {
        self.draw_call_count
    }

    /// Reset draw call counter
    pub fn reset_draw_call_count(&mut self) {
        self.draw_call_count = 0;
    }

    // ========================================================================
    // Main Memory Mapping
    // ========================================================================

    /// Map main memory for RSX access
    pub fn map_main_memory(&mut self, address: u32, size: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410001u32 as i32); // CELL_GCM_ERROR_FAILURE
        }
        
        // Validate parameters
        if size == 0 {
            return Err(0x80410002u32 as i32); // CELL_GCM_ERROR_INVALID_VALUE
        }
        
        // Align size to 1MB boundary (RSX memory mapping granularity)
        let aligned_size = (size + 0xFFFFF) & !0xFFFFF;
        
        debug!(
            "GcmManager::map_main_memory: address=0x{:08X}, size=0x{:X} (aligned=0x{:X})",
            address, size, aligned_size
        );
        
        // Calculate offset in I/O space
        let offset = self.next_io_offset;
        self.next_io_offset += aligned_size;
        
        // Store mapping
        self.memory_mappings.push(MemoryMapping {
            main_addr: address,
            size: aligned_size,
            offset,
        });
        
        // Update I/O memory configuration
        if self.config.io_addr == 0 {
            self.config.io_addr = address;
        }
        self.config.io_size += aligned_size;
        
        Ok(offset)
    }

    /// Unmap main memory from RSX
    pub fn unmap_main_memory(&mut self, offset: u32) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        // Find and remove mapping
        if let Some(pos) = self.memory_mappings.iter().position(|m| m.offset == offset) {
            let mapping = self.memory_mappings.remove(pos);
            debug!(
                "GcmManager::unmap_main_memory: offset=0x{:X}, size=0x{:X}",
                offset, mapping.size
            );
            
            // Update I/O size
            self.config.io_size = self.config.io_size.saturating_sub(mapping.size);
            
            0 // CELL_OK
        } else {
            0x80410002u32 as i32 // CELL_GCM_ERROR_INVALID_VALUE
        }
    }

    /// Get the number of memory mappings
    pub fn get_memory_mapping_count(&self) -> usize {
        self.memory_mappings.len()
    }

    // ========================================================================
    // Flip Status
    // ========================================================================

    /// Reset flip status to not pending
    pub fn reset_flip_status(&mut self) -> i32 {
        if !self.initialized {
            return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
        }
        
        trace!("GcmManager::reset_flip_status");
        self.flip_status = CellGcmFlipStatus::NotPending;
        
        0 // CELL_OK
    }

    /// Get current flip status
    pub fn get_flip_status(&self) -> CellGcmFlipStatus {
        self.flip_status
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
pub fn cell_gcm_get_configuration(config_addr: u32) -> i32 {
    trace!("cellGcmGetConfiguration(config_addr=0x{:08X})", config_addr);

    // Validate address
    if config_addr == 0 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    let config = crate::context::get_hle_context().gcm.get_configuration();
    
    // Write CellGcmConfig structure to memory (24 bytes)
    // Structure layout:
    //   local_addr: u32 (offset 0)
    //   local_size: u32 (offset 4)
    //   io_addr: u32 (offset 8)
    //   io_size: u32 (offset 12)
    //   mem_frequency: u32 (offset 16)
    //   core_frequency: u32 (offset 20)
    if let Err(_) = crate::memory::write_be32(config_addr, config.local_addr) {
        return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
    }
    if let Err(_) = crate::memory::write_be32(config_addr + 4, config.local_size) {
        return 0x80410001u32 as i32;
    }
    if let Err(_) = crate::memory::write_be32(config_addr + 8, config.io_addr) {
        return 0x80410001u32 as i32;
    }
    if let Err(_) = crate::memory::write_be32(config_addr + 12, config.io_size) {
        return 0x80410001u32 as i32;
    }
    if let Err(_) = crate::memory::write_be32(config_addr + 16, config.mem_frequency) {
        return 0x80410001u32 as i32;
    }
    if let Err(_) = crate::memory::write_be32(config_addr + 20, config.core_frequency) {
        return 0x80410001u32 as i32;
    }

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
pub fn cell_gcm_address_to_offset(address: u32, offset_addr: u32) -> i32 {
    trace!("cellGcmAddressToOffset(address=0x{:08X}, offset_addr=0x{:08X})", address, offset_addr);

    // Validate output address
    if offset_addr == 0 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }

    match crate::context::get_hle_context().gcm.address_to_offset(address) {
        Ok(offset) => {
            // Write offset to memory
            if let Err(_) = crate::memory::write_be32(offset_addr, offset) {
                return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
            }
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

// ============================================================================
// Shader Program Functions
// ============================================================================

/// cellGcmSetVertexProgram - Set vertex shader program
///
/// Reads the vertex program descriptor from memory and configures the RSX
/// to use the specified vertex shader.
///
/// # Arguments
/// * `program_addr` - Address of vertex program descriptor
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_vertex_program(program_addr: u32) -> i32 {
    debug!("cellGcmSetVertexProgram(program_addr=0x{:08X})", program_addr);
    
    // Read CellGcmVertexProgram from memory
    // Structure layout:
    //   size: u32 (offset 0)
    //   offset: u32 (offset 4)
    //   num_instructions: u16 (offset 8)
    //   num_inputs: u8 (offset 10)
    //   num_outputs: u8 (offset 11)
    //   input_mask: u32 (offset 12)
    //   output_mask: u32 (offset 16)
    let program = if program_addr != 0 && crate::memory::is_hle_memory_initialized() {
        let size = crate::memory::read_be32(program_addr).unwrap_or(0);
        let offset = crate::memory::read_be32(program_addr + 4).unwrap_or(0);
        let num_instructions = crate::memory::read_be16(program_addr + 8).unwrap_or(0);
        let num_inputs = crate::memory::read_u8(program_addr + 10).unwrap_or(0);
        let num_outputs = crate::memory::read_u8(program_addr + 11).unwrap_or(0);
        let input_mask = crate::memory::read_be32(program_addr + 12).unwrap_or(0xFFFF);
        let output_mask = crate::memory::read_be32(program_addr + 16).unwrap_or(0xFFFF);
        
        CellGcmVertexProgram {
            size,
            offset,
            num_instructions,
            num_inputs,
            num_outputs,
            input_mask,
            output_mask,
        }
    } else {
        // Fallback to default when memory not available (for testing/HLE stub mode)
        CellGcmVertexProgram {
            size: 0,
            offset: 0,
            num_instructions: 0,
            num_inputs: 0,
            num_outputs: 0,
            input_mask: 0xFFFF,
            output_mask: 0xFFFF,
        }
    };
    
    crate::context::get_hle_context_mut().gcm.set_vertex_program(program)
}

/// cellGcmSetFragmentProgram - Set fragment shader program
///
/// Reads the fragment program descriptor from memory and configures the RSX
/// to use the specified fragment shader.
///
/// # Arguments
/// * `program_addr` - Address of fragment program descriptor
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_fragment_program(program_addr: u32) -> i32 {
    debug!("cellGcmSetFragmentProgram(program_addr=0x{:08X})", program_addr);
    
    // Read CellGcmFragmentProgram from memory
    // Structure layout:
    //   size: u32 (offset 0)
    //   offset: u32 (offset 4)
    //   num_instructions: u16 (offset 8)
    //   num_samplers: u8 (offset 10)
    //   register_count: u8 (offset 11)
    //   control: u32 (offset 12)
    let program = if program_addr != 0 && crate::memory::is_hle_memory_initialized() {
        let size = crate::memory::read_be32(program_addr).unwrap_or(0);
        let offset = crate::memory::read_be32(program_addr + 4).unwrap_or(0);
        let num_instructions = crate::memory::read_be16(program_addr + 8).unwrap_or(0);
        let num_samplers = crate::memory::read_u8(program_addr + 10).unwrap_or(0);
        let register_count = crate::memory::read_u8(program_addr + 11).unwrap_or(0);
        let control = crate::memory::read_be32(program_addr + 12).unwrap_or(0);
        
        CellGcmFragmentProgram {
            size,
            offset,
            num_instructions,
            num_samplers,
            register_count,
            control,
        }
    } else {
        // Fallback to default when memory not available (for testing/HLE stub mode)
        CellGcmFragmentProgram {
            size: 0,
            offset: 0,
            num_instructions: 0,
            num_samplers: 0,
            register_count: 0,
            control: 0,
        }
    };
    
    crate::context::get_hle_context_mut().gcm.set_fragment_program(program)
}

// ============================================================================
// Draw Call Functions
// ============================================================================

/// cellGcmSetDrawArrays - Draw non-indexed primitives
///
/// # Arguments
/// * `primitive` - Primitive type
/// * `first` - First vertex index
/// * `count` - Number of vertices to draw
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_draw_arrays(primitive: u32, first: u32, count: u32) -> i32 {
    trace!("cellGcmSetDrawArrays(primitive={}, first={}, count={})", primitive, first, count);
    
    // Convert primitive type
    let prim = match primitive {
        1 => CellGcmPrimitive::Points,
        2 => CellGcmPrimitive::Lines,
        3 => CellGcmPrimitive::LineLoop,
        4 => CellGcmPrimitive::LineStrip,
        5 => CellGcmPrimitive::Triangles,
        6 => CellGcmPrimitive::TriangleStrip,
        7 => CellGcmPrimitive::TriangleFan,
        8 => CellGcmPrimitive::Quads,
        9 => CellGcmPrimitive::QuadStrip,
        10 => CellGcmPrimitive::Polygon,
        _ => return 0x80410002u32 as i32, // CELL_GCM_ERROR_INVALID_VALUE
    };
    
    crate::context::get_hle_context_mut().gcm.draw_arrays(prim, first, count)
}

/// cellGcmSetDrawIndexArray - Draw indexed primitives
///
/// # Arguments
/// * `primitive` - Primitive type
/// * `count` - Number of indices to draw
/// * `index_type` - Index type (16-bit or 32-bit)
/// * `location` - Memory location (local or main)
/// * `index_offset` - Offset to index data
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_draw_index_array(
    primitive: u32,
    count: u32,
    index_type: u32,
    location: u8,
    index_offset: u32,
) -> i32 {
    trace!(
        "cellGcmSetDrawIndexArray(primitive={}, count={}, type={}, location={}, offset=0x{:X})",
        primitive, count, index_type, location, index_offset
    );
    
    // Convert primitive type
    let prim = match primitive {
        1 => CellGcmPrimitive::Points,
        2 => CellGcmPrimitive::Lines,
        3 => CellGcmPrimitive::LineLoop,
        4 => CellGcmPrimitive::LineStrip,
        5 => CellGcmPrimitive::Triangles,
        6 => CellGcmPrimitive::TriangleStrip,
        7 => CellGcmPrimitive::TriangleFan,
        8 => CellGcmPrimitive::Quads,
        9 => CellGcmPrimitive::QuadStrip,
        10 => CellGcmPrimitive::Polygon,
        _ => return 0x80410002u32 as i32, // CELL_GCM_ERROR_INVALID_VALUE
    };
    
    // Convert index type
    let idx_type = match index_type {
        0 => CellGcmIndexType::Index16,
        1 => CellGcmIndexType::Index32,
        _ => return 0x80410002u32 as i32, // CELL_GCM_ERROR_INVALID_VALUE
    };
    
    crate::context::get_hle_context_mut().gcm.draw_index_array(prim, index_offset, count, idx_type, location)
}

// ============================================================================
// Viewport and Scissor Functions
// ============================================================================

/// cellGcmSetViewport - Set viewport transformation
///
/// # Arguments
/// * `x` - X origin
/// * `y` - Y origin
/// * `width` - Width
/// * `height` - Height
/// * `z_min` - Minimum Z value
/// * `z_max` - Maximum Z value
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_viewport(x: u16, y: u16, width: u16, height: u16, z_min: f32, z_max: f32) -> i32 {
    debug!(
        "cellGcmSetViewport(x={}, y={}, {}x{}, z={:.2}-{:.2})",
        x, y, width, height, z_min, z_max
    );
    
    crate::context::get_hle_context_mut().gcm.set_viewport(x, y, width, height, z_min, z_max)
}

/// cellGcmSetScissor - Set scissor test rectangle
///
/// # Arguments
/// * `x` - X origin
/// * `y` - Y origin
/// * `width` - Width
/// * `height` - Height
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_set_scissor(x: u16, y: u16, width: u16, height: u16) -> i32 {
    trace!("cellGcmSetScissor(x={}, y={}, {}x{})", x, y, width, height);
    
    crate::context::get_hle_context_mut().gcm.set_scissor(x, y, width, height)
}

// ============================================================================
// Memory Mapping Functions
// ============================================================================

/// cellGcmMapMainMemory - Map main memory for RSX access
///
/// Maps a region of main memory so it can be accessed by the RSX.
/// The resulting offset is written to `offset_addr` and can be used
/// in RSX commands that reference main memory.
///
/// # Arguments
/// * `address` - Main memory address to map
/// * `size` - Size to map (bytes, will be aligned to 1MB boundary)
/// * `offset_addr` - Address to write resulting RSX offset
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_map_main_memory(address: u32, size: u32, offset_addr: u32) -> i32 {
    debug!("cellGcmMapMainMemory(address=0x{:08X}, size=0x{:X}, offset_addr=0x{:08X})", address, size, offset_addr);
    
    // Validate output address
    if offset_addr == 0 {
        return 0x80410002u32 as i32; // CELL_GCM_ERROR_INVALID_VALUE
    }
    
    match crate::context::get_hle_context_mut().gcm.map_main_memory(address, size) {
        Ok(offset) => {
            // Write offset to memory
            if let Err(_) = crate::memory::write_be32(offset_addr, offset) {
                return 0x80410001u32 as i32; // CELL_GCM_ERROR_FAILURE
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellGcmUnmapIoAddress - Unmap previously mapped I/O memory
///
/// # Arguments
/// * `offset` - RSX offset to unmap
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_unmap_io_address(offset: u32) -> i32 {
    debug!("cellGcmUnmapIoAddress(offset=0x{:X})", offset);
    
    crate::context::get_hle_context_mut().gcm.unmap_main_memory(offset)
}

// ============================================================================
// Flip Status Functions
// ============================================================================

/// cellGcmResetFlipStatus - Reset flip status to not pending
///
/// # Returns
/// * 0 on success
pub fn cell_gcm_reset_flip_status() -> i32 {
    trace!("cellGcmResetFlipStatus()");
    
    crate::context::get_hle_context_mut().gcm.reset_flip_status()
}

/// cellGcmGetFlipStatus - Get current flip status
///
/// # Returns
/// * 0 if flip is not pending
/// * 1 if flip is pending
pub fn cell_gcm_get_flip_status() -> u32 {
    trace!("cellGcmGetFlipStatus()");
    
    crate::context::get_hle_context().gcm.get_flip_status() as u32
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

    // ========================================================================
    // Shader Program Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_vertex_program() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        let program = CellGcmVertexProgram {
            size: 256,
            offset: 0x1000,
            num_instructions: 32,
            num_inputs: 4,
            num_outputs: 4,
            input_mask: 0x000F,
            output_mask: 0x000F,
        };
        
        assert_eq!(manager.set_vertex_program(program), 0);
        
        let vp = manager.get_vertex_program();
        assert!(vp.is_some());
        assert_eq!(vp.unwrap().size, 256);
        assert_eq!(vp.unwrap().num_instructions, 32);
    }

    #[test]
    fn test_gcm_manager_fragment_program() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        let program = CellGcmFragmentProgram {
            size: 512,
            offset: 0x2000,
            num_instructions: 64,
            num_samplers: 4,
            register_count: 16,
            control: 0x12345678,
        };
        
        assert_eq!(manager.set_fragment_program(program), 0);
        
        let fp = manager.get_fragment_program();
        assert!(fp.is_some());
        assert_eq!(fp.unwrap().size, 512);
        assert_eq!(fp.unwrap().num_samplers, 4);
    }

    #[test]
    fn test_gcm_manager_invalidate_programs() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        manager.set_vertex_program(CellGcmVertexProgram::default());
        manager.set_fragment_program(CellGcmFragmentProgram::default());
        
        assert!(manager.get_vertex_program().is_some());
        assert!(manager.get_fragment_program().is_some());
        
        manager.invalidate_programs();
        
        assert!(manager.get_vertex_program().is_none());
        assert!(manager.get_fragment_program().is_none());
    }

    #[test]
    fn test_gcm_set_vertex_program_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_vertex_program(0x10000), 0);
    }

    #[test]
    fn test_gcm_set_fragment_program_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_fragment_program(0x10000), 0);
    }

    // ========================================================================
    // Viewport and Scissor Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_viewport() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        assert_eq!(manager.set_viewport(0, 0, 1920, 1080, 0.0, 1.0), 0);
        
        let vp = manager.get_viewport();
        assert_eq!(vp.x, 0);
        assert_eq!(vp.y, 0);
        assert_eq!(vp.width, 1920);
        assert_eq!(vp.height, 1080);
    }

    #[test]
    fn test_gcm_manager_viewport_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Invalid dimensions
        assert!(manager.set_viewport(0, 0, 0, 0, 0.0, 1.0) != 0);
    }

    #[test]
    fn test_gcm_manager_scissor() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        assert_eq!(manager.set_scissor(100, 100, 800, 600), 0);
        
        let scissor = manager.get_scissor();
        assert_eq!(scissor.x, 100);
        assert_eq!(scissor.y, 100);
        assert_eq!(scissor.width, 800);
        assert_eq!(scissor.height, 600);
    }

    #[test]
    fn test_gcm_manager_scissor_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Invalid dimensions
        assert!(manager.set_scissor(0, 0, 0, 0) != 0);
    }

    #[test]
    fn test_gcm_set_viewport_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_viewport(0, 0, 1920, 1080, 0.0, 1.0), 0);
    }

    #[test]
    fn test_gcm_set_scissor_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_set_scissor(0, 0, 1920, 1080), 0);
    }

    // ========================================================================
    // Draw Call Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_draw_arrays() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        assert_eq!(manager.draw_arrays(CellGcmPrimitive::Triangles, 0, 36), 0);
        assert_eq!(manager.get_draw_call_count(), 1);
        
        // Empty draw is okay
        assert_eq!(manager.draw_arrays(CellGcmPrimitive::Triangles, 0, 0), 0);
        assert_eq!(manager.get_draw_call_count(), 1); // Not incremented for empty draw
    }

    #[test]
    fn test_gcm_manager_draw_index_array() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        assert_eq!(manager.draw_index_array(CellGcmPrimitive::Triangles, 0x1000, 36, CellGcmIndexType::Index16, 0), 0);
        assert_eq!(manager.get_draw_call_count(), 1);
        
        assert_eq!(manager.draw_index_array(CellGcmPrimitive::TriangleStrip, 0x2000, 100, CellGcmIndexType::Index32, 0), 0);
        assert_eq!(manager.get_draw_call_count(), 2);
    }

    #[test]
    fn test_gcm_manager_draw_call_count() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        assert_eq!(manager.get_draw_call_count(), 0);
        
        manager.draw_arrays(CellGcmPrimitive::Triangles, 0, 3);
        manager.draw_arrays(CellGcmPrimitive::Triangles, 3, 3);
        manager.draw_index_array(CellGcmPrimitive::Lines, 0, 10, CellGcmIndexType::Index16, 0);
        
        assert_eq!(manager.get_draw_call_count(), 3);
        
        manager.reset_draw_call_count();
        assert_eq!(manager.get_draw_call_count(), 0);
    }

    #[test]
    fn test_gcm_set_draw_arrays_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        // Valid primitive types
        assert_eq!(cell_gcm_set_draw_arrays(5, 0, 36), 0); // Triangles
        assert_eq!(cell_gcm_set_draw_arrays(6, 0, 4), 0);  // Triangle strip
        
        // Invalid primitive type
        assert!(cell_gcm_set_draw_arrays(0, 0, 36) != 0);
        assert!(cell_gcm_set_draw_arrays(100, 0, 36) != 0);
    }

    #[test]
    fn test_gcm_set_draw_index_array_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        // Valid call
        assert_eq!(cell_gcm_set_draw_index_array(5, 36, 0, 0, 0x1000), 0);
        
        // Invalid primitive type
        assert!(cell_gcm_set_draw_index_array(0, 36, 0, 0, 0x1000) != 0);
        
        // Invalid index type
        assert!(cell_gcm_set_draw_index_array(5, 36, 99, 0, 0x1000) != 0);
    }

    // ========================================================================
    // Memory Mapping Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_map_main_memory() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Map memory
        let result = manager.map_main_memory(0x30000000, 0x100000);
        assert!(result.is_ok());
        let offset1 = result.unwrap();
        
        // Map more memory
        let result = manager.map_main_memory(0x30200000, 0x200000);
        assert!(result.is_ok());
        let offset2 = result.unwrap();
        
        assert_ne!(offset1, offset2);
        assert_eq!(manager.get_memory_mapping_count(), 2);
    }

    #[test]
    fn test_gcm_manager_unmap_main_memory() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        let offset = manager.map_main_memory(0x30000000, 0x100000).unwrap();
        assert_eq!(manager.get_memory_mapping_count(), 1);
        
        assert_eq!(manager.unmap_main_memory(offset), 0);
        assert_eq!(manager.get_memory_mapping_count(), 0);
        
        // Double unmap should fail
        assert!(manager.unmap_main_memory(offset) != 0);
    }

    #[test]
    fn test_gcm_manager_map_memory_validation() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Zero size should fail
        assert!(manager.map_main_memory(0x30000000, 0).is_err());
    }

    #[test]
    fn test_gcm_map_main_memory_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        // Test that null address is rejected
        assert!(cell_gcm_map_main_memory(0x30000000, 0x100000, 0) != 0);
        
        // When memory subsystem is initialized, the function would succeed
        // with a valid address. Without memory, we can only test validation.
    }

    // ========================================================================
    // Flip Status Tests
    // ========================================================================

    #[test]
    fn test_gcm_manager_flip_status() {
        let mut manager = GcmManager::new();
        manager.init(0x10000000, 1024 * 1024);
        
        // Initially not pending
        assert_eq!(manager.get_flip_status(), CellGcmFlipStatus::NotPending);
        
        // After set_flip, should be pending
        manager.set_display_buffer(0, 0, 1920 * 4, 1920, 1080);
        manager.set_flip(0);
        assert_eq!(manager.get_flip_status(), CellGcmFlipStatus::Pending);
        
        // After reset, should be not pending
        manager.reset_flip_status();
        assert_eq!(manager.get_flip_status(), CellGcmFlipStatus::NotPending);
    }

    #[test]
    fn test_gcm_reset_flip_status_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_reset_flip_status(), 0);
    }

    #[test]
    fn test_gcm_get_flip_status_api() {
        crate::context::reset_hle_context();
        crate::context::get_hle_context_mut().gcm.init(0x10000000, 1024 * 1024);
        
        assert_eq!(cell_gcm_get_flip_status(), 0); // Not pending
        
        // Set flip to make it pending
        crate::context::get_hle_context_mut().gcm.set_display_buffer(0, 0, 1920 * 4, 1920, 1080);
        crate::context::get_hle_context_mut().gcm.set_flip(0);
        
        assert_eq!(cell_gcm_get_flip_status(), 1); // Pending
    }

    // ========================================================================
    // Primitive Type Enum Tests
    // ========================================================================

    #[test]
    fn test_primitive_type_enum() {
        assert_eq!(CellGcmPrimitive::Points as u32, 1);
        assert_eq!(CellGcmPrimitive::Lines as u32, 2);
        assert_eq!(CellGcmPrimitive::Triangles as u32, 5);
        assert_eq!(CellGcmPrimitive::TriangleStrip as u32, 6);
        assert_eq!(CellGcmPrimitive::Quads as u32, 8);
    }

    #[test]
    fn test_index_type_enum() {
        assert_eq!(CellGcmIndexType::Index16 as u32, 0);
        assert_eq!(CellGcmIndexType::Index32 as u32, 1);
    }

    #[test]
    fn test_flip_status_enum() {
        assert_eq!(CellGcmFlipStatus::NotPending as u32, 0);
        assert_eq!(CellGcmFlipStatus::Pending as u32, 1);
    }

    #[test]
    fn test_viewport_default() {
        let vp = CellGcmViewport::default();
        assert_eq!(vp.x, 0);
        assert_eq!(vp.y, 0);
        assert_eq!(vp.width, 1920);
        assert_eq!(vp.height, 1080);
    }

    #[test]
    fn test_scissor_default() {
        let scissor = CellGcmScissor::default();
        assert_eq!(scissor.x, 0);
        assert_eq!(scissor.y, 0);
        assert_eq!(scissor.width, 4096);
        assert_eq!(scissor.height, 4096);
    }
}
