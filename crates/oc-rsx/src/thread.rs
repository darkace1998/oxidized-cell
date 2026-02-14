//! RSX thread (command processor)

use std::sync::Arc;
use oc_memory::MemoryManager;
use oc_core::{RsxBridgeReceiver, BridgeMessage, BridgeCommand, BridgeDisplayBuffer};
use crate::state::RsxState;
use crate::fifo::{CommandFifo, RsxCommand};
use crate::methods::MethodHandler;
use crate::backend::{GraphicsBackend, null::NullBackend};

// Draw command data extraction constants
const DRAW_FIRST_MASK: u32 = 0xFFFFFF;
const DRAW_COUNT_SHIFT: u32 = 24;
const DRAW_COUNT_MASK: u32 = 0xFF;

/// Display buffer configuration received from GCM
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayBuffer {
    /// Buffer offset in memory
    pub offset: u32,
    /// Pitch (bytes per line)
    pub pitch: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Whether this buffer is configured
    pub configured: bool,
}

/// Maximum display buffers
pub const MAX_DISPLAY_BUFFERS: usize = 8;

/// RSX thread state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsxThreadState {
    Stopped,
    Running,
    Idle,
}

/// RSX command processor thread
pub struct RsxThread {
    /// Thread state
    pub state: RsxThreadState,
    /// Graphics state
    pub gfx_state: RsxState,
    /// Command FIFO
    pub fifo: CommandFifo,
    /// Memory manager reference
    memory: Arc<MemoryManager>,
    /// Graphics backend
    backend: Box<dyn GraphicsBackend>,
    /// Bridge receiver for commands from GCM HLE
    bridge_receiver: Option<RsxBridgeReceiver>,
    /// Display buffers configured by GCM
    display_buffers: [DisplayBuffer; MAX_DISPLAY_BUFFERS],
    /// Current display buffer index
    current_display_buffer: u32,
    /// Flip pending flag
    flip_pending: bool,
    /// Pending flip buffer id
    pending_flip_buffer: u32,
}

impl RsxThread {
    /// Create a new RSX thread with default (null) backend
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        Self::with_backend(memory, Box::new(NullBackend::new()))
    }

    /// Create a new RSX thread with specified backend
    pub fn with_backend(memory: Arc<MemoryManager>, backend: Box<dyn GraphicsBackend>) -> Self {
        Self {
            state: RsxThreadState::Stopped,
            gfx_state: RsxState::new(),
            fifo: CommandFifo::new(),
            memory,
            backend,
            bridge_receiver: None,
            display_buffers: [DisplayBuffer::default(); MAX_DISPLAY_BUFFERS],
            current_display_buffer: 0,
            flip_pending: false,
            pending_flip_buffer: 0,
        }
    }
    
    /// Set the bridge receiver for receiving commands from GCM HLE
    pub fn set_bridge_receiver(&mut self, receiver: RsxBridgeReceiver) {
        tracing::info!("RsxThread: Bridge receiver connected");
        receiver.connect();
        self.bridge_receiver = Some(receiver);
    }
    
    /// Check if bridge is connected
    pub fn has_bridge(&self) -> bool {
        self.bridge_receiver.is_some()
    }
    
    /// Process messages from the GCM bridge
    pub fn process_bridge_messages(&mut self) {
        let receiver = match &self.bridge_receiver {
            Some(r) => r,
            None => return,
        };
        
        // Drain all pending messages
        let messages = receiver.drain();
        
        for message in messages {
            match message {
                BridgeMessage::Commands(commands) => {
                    self.process_bridge_commands(commands);
                }
                BridgeMessage::ConfigureDisplayBuffer(buffer) => {
                    self.configure_display_buffer(buffer);
                }
                BridgeMessage::Flip(request) => {
                    self.handle_flip_request(request.buffer_id);
                }
                BridgeMessage::Finish => {
                    self.handle_finish();
                }
            }
        }
    }
    
    /// Process commands received from bridge
    fn process_bridge_commands(&mut self, commands: Vec<BridgeCommand>) {
        tracing::debug!("RsxThread: Processing {} commands from bridge", commands.len());
        
        for cmd in commands {
            // Add to FIFO for processing
            self.fifo.push(RsxCommand {
                method: cmd.method,
                data: cmd.data,
            });
        }
        
        // Process the commands immediately
        self.process_commands();
    }
    
    /// Configure a display buffer
    fn configure_display_buffer(&mut self, buffer: BridgeDisplayBuffer) {
        if buffer.id as usize >= MAX_DISPLAY_BUFFERS {
            tracing::warn!("RsxThread: Invalid display buffer id {}", buffer.id);
            return;
        }
        
        tracing::debug!(
            "RsxThread: Configuring display buffer {}: offset=0x{:X}, pitch={}, {}x{}",
            buffer.id, buffer.offset, buffer.pitch, buffer.width, buffer.height
        );
        
        self.display_buffers[buffer.id as usize] = DisplayBuffer {
            offset: buffer.offset,
            pitch: buffer.pitch,
            width: buffer.width,
            height: buffer.height,
            configured: true,
        };
    }
    
    /// Handle a flip request from GCM
    fn handle_flip_request(&mut self, buffer_id: u32) {
        tracing::debug!("RsxThread: Flip requested to buffer {}", buffer_id);
        
        if buffer_id as usize >= MAX_DISPLAY_BUFFERS {
            tracing::warn!("RsxThread: Invalid flip buffer id {}", buffer_id);
            return;
        }
        
        self.flip_pending = true;
        self.pending_flip_buffer = buffer_id;
        
        // Perform the flip at end of frame
        self.perform_flip();
    }
    
    /// Perform the actual flip operation
    fn perform_flip(&mut self) {
        if !self.flip_pending {
            return;
        }
        
        let buffer_id = self.pending_flip_buffer;
        
        if !self.display_buffers[buffer_id as usize].configured {
            tracing::warn!("RsxThread: Flip to unconfigured buffer {}", buffer_id);
        }
        
        // End current frame and present
        self.end_frame();
        
        // Update current buffer
        self.current_display_buffer = buffer_id;
        self.flip_pending = false;
        
        // Signal flip complete to GCM
        if let Some(ref receiver) = self.bridge_receiver {
            receiver.signal_flip_complete(buffer_id);
        }
        
        // Begin next frame
        self.begin_frame();
        
        tracing::trace!("RsxThread: Flip complete to buffer {}", buffer_id);
    }
    
    /// Handle finish/sync request
    fn handle_finish(&mut self) {
        tracing::debug!("RsxThread: Finish requested, processing remaining commands");
        
        // Process any remaining commands in FIFO
        self.process_commands();
        
        // Signal finish complete
        if let Some(ref receiver) = self.bridge_receiver {
            receiver.signal_finish_complete();
        }
    }
    
    /// Get current display buffer index
    pub fn current_display_buffer(&self) -> u32 {
        self.current_display_buffer
    }
    
    /// Get display buffer info
    pub fn get_display_buffer(&self, id: u32) -> Option<&DisplayBuffer> {
        if id as usize >= MAX_DISPLAY_BUFFERS {
            return None;
        }
        let buf = &self.display_buffers[id as usize];
        if buf.configured {
            Some(buf)
        } else {
            None
        }
    }

    /// Initialize the graphics backend
    pub fn init_backend(&mut self) -> Result<(), String> {
        self.backend.init()
    }

    /// Process commands from FIFO
    pub fn process_commands(&mut self) {
        while let Some(cmd) = self.fifo.pop() {
            self.execute_command(cmd.method, cmd.data);
        }
    }

    /// Begin a frame
    pub fn begin_frame(&mut self) {
        self.backend.begin_frame();
    }

    /// End a frame
    pub fn end_frame(&mut self) {
        self.backend.end_frame();
    }

    /// Execute a single RSX command
    fn execute_command(&mut self, method: u32, data: u32) {
        tracing::trace!("RSX method 0x{:04x} = 0x{:08x}", method, data);
        
        // Handle special commands that need more than just state updates
        match method {
            // NV4097_CLEAR_SURFACE
            0x1D94 => {
                // Let the method handler update state first
                MethodHandler::execute(method, data, &mut self.gfx_state);
                self.clear_surface(data);
                return;
            }
            // NV4097_SET_BEGIN_END
            0x1808 => {
                if data == 0 {
                    // End primitive - flush vertices
                    self.flush_vertices();
                } else {
                    // Begin primitive — apply current viewport, scissor, and texture state
                    MethodHandler::execute(method, data, &mut self.gfx_state);
                    self.apply_render_state();
                    return;
                }
            }
            // NV4097_DRAW_ARRAYS
            0x1810 => {
                self.draw_arrays(data);
                return;
            }
            // NV4097_DRAW_INDEX_ARRAY
            0x1814 => {
                self.draw_indexed(data);
                return;
            }
            _ => {}
        }
        
        // Use the method handler for state updates
        MethodHandler::execute(method, data, &mut self.gfx_state);
    }
    
    /// Apply current render state (viewport, scissor, textures, vertex attributes) to the backend
    fn apply_render_state(&mut self) {
        // Forward viewport state
        self.backend.set_viewport(
            self.gfx_state.viewport_x,
            self.gfx_state.viewport_y,
            self.gfx_state.viewport_width,
            self.gfx_state.viewport_height,
            self.gfx_state.depth_min,
            self.gfx_state.depth_max,
        );
        
        // Forward scissor state
        self.backend.set_scissor(
            self.gfx_state.scissor_x as u32,
            self.gfx_state.scissor_y as u32,
            self.gfx_state.scissor_width as u32,
            self.gfx_state.scissor_height as u32,
        );
        
        // Forward texture bindings for enabled texture units
        for i in 0..16u32 {
            let control = self.gfx_state.texture_control[i as usize];
            // Bit 31 of control0 is the enable flag
            if (control & 0x8000_0000) != 0 {
                let offset = self.gfx_state.texture_offset[i as usize];
                if offset != 0 {
                    self.backend.bind_texture(i, offset);
                }
            }
        }
        
        // Build and forward vertex attribute descriptions from RSX state
        self.apply_vertex_attributes();
    }
    
    /// Build vertex attribute descriptors from RSX state and forward to the backend
    fn apply_vertex_attributes(&mut self) {
        use crate::vertex::{VertexAttribute, VertexAttributeType};
        
        let input_mask = self.gfx_state.vertex_attrib_input_mask;
        let mut attrs = Vec::new();
        
        for i in 0..16u32 {
            if (input_mask & (1 << i)) == 0 {
                continue;
            }
            
            let format = self.gfx_state.vertex_attrib_format[i as usize];
            if format == 0 {
                continue;
            }
            
            let type_bits = format & 0xF;
            let size = ((format >> 4) & 0xF) as u8;
            let stride = ((format >> 8) & 0xFF) as u16;
            
            let attr_type = match type_bits {
                1 => VertexAttributeType::FLOAT,
                2 => VertexAttributeType::HALF_FLOAT,
                4 => VertexAttributeType::BYTE,
                5 => VertexAttributeType::SHORT,
                6 => VertexAttributeType::COMPRESSED,
                7 => VertexAttributeType::BYTE,
                _ => VertexAttributeType::FLOAT,
            };
            
            let normalized = matches!(type_bits, 4 | 6); // u8n (4) and compressed (6) are normalized; u8 (7) is raw
            
            if size == 0 {
                tracing::warn!("Vertex attribute {} has size=0, clamping to 1", i);
            }
            let size = size.max(1);
            
            attrs.push(VertexAttribute {
                index: i as u8,
                size,
                type_: attr_type,
                stride,
                offset: 0,
                normalized,
            });
        }
        
        if !attrs.is_empty() {
            self.backend.set_vertex_attributes(&attrs);
        }
    }

    /// Clear the surface
    fn clear_surface(&mut self, mask: u32) {
        tracing::trace!("Clear surface with mask 0x{:08x}", mask);
        
        // Extract clear color from state
        let color_u32 = self.gfx_state.clear_color;
        let r = ((color_u32 >> 24) & 0xFF) as f32 / 255.0;
        let g = ((color_u32 >> 16) & 0xFF) as f32 / 255.0;
        let b = ((color_u32 >> 8) & 0xFF) as f32 / 255.0;
        let a = (color_u32 & 0xFF) as f32 / 255.0;
        
        // Call backend clear
        self.backend.clear(
            [r, g, b, a],
            self.gfx_state.clear_depth,
            self.gfx_state.clear_stencil,
        );
    }

    /// Draw arrays command
    fn draw_arrays(&mut self, data: u32) {
        let first = data & DRAW_FIRST_MASK;
        let count = (data >> DRAW_COUNT_SHIFT) & DRAW_COUNT_MASK;
        
        tracing::trace!("Draw arrays: first={}, count={}", first, count);
        
        let primitive = self.convert_primitive_type();
        self.backend.draw_arrays(primitive, first, count);
    }

    /// Draw indexed command
    fn draw_indexed(&mut self, data: u32) {
        let first = data & DRAW_FIRST_MASK;
        let count = (data >> DRAW_COUNT_SHIFT) & DRAW_COUNT_MASK;
        
        tracing::trace!("Draw indexed: first={}, count={}", first, count);
        
        let primitive = self.convert_primitive_type();
        self.backend.draw_indexed(primitive, first, count);
    }

    /// Convert RSX primitive type to backend format
    fn convert_primitive_type(&self) -> crate::backend::PrimitiveType {
        use crate::backend::PrimitiveType;
        match self.gfx_state.primitive_type {
            1 => PrimitiveType::Points,
            2 => PrimitiveType::Lines,
            3 => PrimitiveType::LineLoop,
            4 => PrimitiveType::LineStrip,
            5 => PrimitiveType::Triangles,
            6 => PrimitiveType::TriangleStrip,
            7 => PrimitiveType::TriangleFan,
            8 => PrimitiveType::Quads,
            _ => PrimitiveType::Triangles, // Default
        }
    }

    /// Flush accumulated vertices
    fn flush_vertices(&mut self) {
        tracing::trace!("Flush vertices");
        
        // Read vertex data from memory and submit to the backend
        // This reads vertex attributes configured via NV4097_SET_VERTEX_DATA_ARRAY_FORMAT/OFFSET
        // and submits them as vertex buffers to the Vulkan backend
        
        let input_mask = self.gfx_state.vertex_attrib_input_mask;
        
        // Process each enabled vertex attribute
        for i in 0..16u32 {
            // Check if this attribute is enabled in the input mask
            if (input_mask & (1 << i)) == 0 {
                continue;
            }
            
            let format = self.gfx_state.vertex_attrib_format[i as usize];
            let offset = self.gfx_state.vertex_attrib_offset[i as usize];
            
            // Skip if format is 0 (not configured)
            if format == 0 {
                continue;
            }
            
            // Parse the vertex attribute format
            // Format bits:
            // [3:0]   - type (1=f32, 2=f16, 3=fixed16.16, 4=u8n, 5=s16, 6=cmp, 7=u8)
            // [7:4]   - size (1-4 components)
            // [15:8]  - stride
            let type_bits = format & 0xF;
            let size = ((format >> 4) & 0xF) as u8;
            let stride = ((format >> 8) & 0xFF) as u16;
            
            // Calculate the byte size for this attribute type
            let type_byte_size = match type_bits {
                1 => 4u32, // f32
                2 => 2,    // f16
                3 => 4,    // fixed16.16
                4 => 1,    // u8 normalized
                5 => 2,    // s16
                6 => 4,    // compressed
                7 => 1,    // u8
                _ => 4,    // default to f32
            };
            
            let attr_size = (size as u32).max(1) * type_byte_size;
            let effective_stride = if stride == 0 { attr_size as u16 } else { stride };
            
            // Read vertex data from RSX local memory
            // The offset is relative to the RSX local memory base
            // We read enough data for a reasonable number of vertices
            // (typically the draw call count, but we use 256 vertices as a reasonable max)
            const MAX_VERTEX_COUNT: u32 = 256;
            
            let data_size = (effective_stride as u32) * MAX_VERTEX_COUNT;
            let max_size = 4096u32; // 4KB max per attribute
            let read_size = data_size.min(max_size);
            
            if offset != 0 {
                // Try to read vertex data from RSX local memory
                match self.memory.read_rsx(offset, read_size) {
                    Ok(vertex_data) => {
                        // Submit vertex buffer to the backend
                        self.backend.submit_vertex_buffer(i, &vertex_data, effective_stride as u32);
                        
                        tracing::trace!(
                            "Submitted vertex buffer: attr={}, offset=0x{:08x}, stride={}, size={}",
                            i, offset, effective_stride, vertex_data.len()
                        );
                    }
                    Err(e) => {
                        tracing::trace!(
                            "Could not read vertex data for attr {} at offset 0x{:08x}: {:?}",
                            i, offset, e
                        );
                    }
                }
            }
        }
        
        // Submit index buffer if index array is configured
        let index_addr = self.gfx_state.index_array_address;
        if index_addr != 0 {
            let index_type = self.gfx_state.draw_index_type;
            let index_type_size = match index_type {
                1 => 4u32, // u32 indices
                0 => 2u32, // u16 indices
                _ => {
                    tracing::warn!("Unknown index type {}, defaulting to u16", index_type);
                    2u32
                }
            };
            let index_count = self.gfx_state.draw_count.max(1);
            let index_data_size = index_count * index_type_size;
            let max_index_size = 4096u32;
            let read_size = index_data_size.min(max_index_size);
            
            match self.memory.read_rsx(index_addr, read_size) {
                Ok(index_data) => {
                    self.backend.submit_index_buffer(&index_data, index_type_size);
                    tracing::trace!(
                        "Submitted index buffer: addr=0x{:08x}, type_size={}, count={}",
                        index_addr, index_type_size, index_count
                    );
                }
                Err(e) => {
                    tracing::trace!(
                        "Could not read index data at 0x{:08x}: {:?}",
                        index_addr, e
                    );
                }
            }
        }
        
        tracing::trace!("Vertex flush complete");
    }

    /// Get memory manager reference
    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
    }
    
    /// Get the current framebuffer contents for display
    pub fn get_framebuffer(&self) -> Option<crate::backend::FramebufferData> {
        self.backend.get_framebuffer()
    }
    
    /// Get the framebuffer dimensions
    pub fn get_dimensions(&self) -> (u32, u32) {
        self.backend.get_dimensions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsx_thread_creation() {
        let memory = MemoryManager::new().unwrap();
        let thread = RsxThread::new(memory);
        assert_eq!(thread.state, RsxThreadState::Stopped);
    }

    #[test]
    fn test_rsx_thread_init_backend() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        // Null backend should always init successfully
        assert!(thread.init_backend().is_ok());
    }
    
    #[test]
    fn test_execute_clear_surface() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // Set clear color via NV4097_SET_COLOR_CLEAR_VALUE (0x0304)
        thread.execute_command(0x0304, 0xFF0000FF); // Red with alpha
        // Execute NV4097_CLEAR_SURFACE (0x1D94) with color+depth clear flags
        thread.execute_command(0x1D94, 0xF3); // Clear Z + S + R + G + B + A
        
        // If we got here without panic, the clear was forwarded to the backend
    }
    
    #[test]
    fn test_execute_viewport_and_scissor() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // Set viewport via NV4097 commands
        // NV4097_SET_VIEWPORT_HORIZONTAL (0x0A00): x=0, width=1280
        thread.execute_command(0x0A00, (1280 << 16) | 0);
        // NV4097_SET_VIEWPORT_VERTICAL (0x0A04): y=0, height=720
        thread.execute_command(0x0A04, (720 << 16) | 0);
        
        // Verify state was updated
        assert_eq!(thread.gfx_state.viewport_width, 1280.0);
        assert_eq!(thread.gfx_state.viewport_height, 720.0);
        
        // Set scissor
        // NV4097_SET_SCISSOR_HORIZONTAL (0x08C0): x=0, width=1280
        thread.execute_command(0x08C0, (1280 << 16) | 0);
        // NV4097_SET_SCISSOR_VERTICAL (0x08C4): y=0, height=720
        thread.execute_command(0x08C4, (720 << 16) | 0);
        
        assert_eq!(thread.gfx_state.scissor_width, 1280);
        assert_eq!(thread.gfx_state.scissor_height, 720);
    }
    
    #[test]
    fn test_begin_end_forwards_render_state() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // Set viewport
        thread.execute_command(0x0A00, (1280 << 16) | 0);
        thread.execute_command(0x0A04, (720 << 16) | 0);
        
        // Set scissor
        thread.execute_command(0x08C0, (1280 << 16) | 0);
        thread.execute_command(0x08C4, (720 << 16) | 0);
        
        // NV4097_SET_BEGIN_END with data != 0 should forward state to backend
        // Primitive type 5 = triangles
        thread.execute_command(0x1808, 5);
        
        // End primitive (data == 0)
        thread.execute_command(0x1808, 0);
    }
    
    #[test]
    fn test_texture_bind_state() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // Set texture offset for unit 0: NV4097_SET_TEXTURE_OFFSET (0x1A00)
        thread.execute_command(0x1A00, 0x0010_0000);
        assert_eq!(thread.gfx_state.texture_offset[0], 0x0010_0000);
        
        // Set texture control for unit 0: NV4097_SET_TEXTURE_CONTROL0 (0x1A08)
        // Enable bit is bit 31
        thread.execute_command(0x1A08, 0x8000_0000);
        assert_eq!(thread.gfx_state.texture_control[0], 0x8000_0000);
    }
    
    #[test]
    fn test_display_buffer_configuration() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        
        let buf = BridgeDisplayBuffer {
            id: 0,
            offset: 0x100000,
            pitch: 5120,
            width: 1280,
            height: 720,
        };
        thread.configure_display_buffer(buf);
        
        let db = thread.get_display_buffer(0).unwrap();
        assert_eq!(db.width, 1280);
        assert_eq!(db.height, 720);
        assert_eq!(db.pitch, 5120);
        assert!(db.configured);
    }
    
    #[test]
    fn test_null_backend_framebuffer_after_commands() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        thread.begin_frame();
        // Set clear color to opaque red (RGBA: R=255, G=0, B=0, A=255)
        thread.execute_command(0x0304, 0xFF0000FF);
        thread.execute_command(0x1D94, 0xF3); // Clear all
        thread.end_frame();
        
        let fb = thread.get_framebuffer();
        assert!(fb.is_some());
        let fb = fb.unwrap();
        assert_eq!(fb.width, 1280);
        assert_eq!(fb.height, 720);
        assert!(!fb.pixels.is_empty());
    }
    
    #[test]
    fn test_vertex_attributes_forwarded_on_begin() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // Enable vertex attribute 0 in the input mask
        thread.gfx_state.vertex_attrib_input_mask = 0x1; // Attr 0 enabled
        // Configure format: type=1 (f32), size=4 components, stride=16
        thread.gfx_state.vertex_attrib_format[0] = (16 << 8) | (4 << 4) | 1;
        
        // SET_BEGIN_END with primitive type triggers apply_render_state
        // which should now call set_vertex_attributes
        thread.execute_command(0x1808, 5); // Begin triangles
        thread.execute_command(0x1808, 0); // End primitive
        
        // The apply_vertex_attributes should have been called via apply_render_state
        // If we got here without panic, the path is connected
    }
    
    #[test]
    fn test_multiple_vertex_attributes_forwarded() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // Enable vertex attributes 0, 1, and 2
        thread.gfx_state.vertex_attrib_input_mask = 0x7; // Attrs 0-2
        // Attr 0: position (4x f32, stride 32)
        thread.gfx_state.vertex_attrib_format[0] = (32 << 8) | (4 << 4) | 1;
        // Attr 1: normal (3x f32, stride 32)
        thread.gfx_state.vertex_attrib_format[1] = (32 << 8) | (3 << 4) | 1;
        // Attr 2: texcoord (2x f32, stride 32)
        thread.gfx_state.vertex_attrib_format[2] = (32 << 8) | (2 << 4) | 1;
        
        thread.execute_command(0x1808, 5); // Begin triangles
        thread.execute_command(0x1808, 0); // End primitive
    }
    
    #[test]
    fn test_draw_arrays_runs_full_pipeline() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        thread.begin_frame();
        // Clear first
        thread.execute_command(0x0304, 0x000000FF);
        thread.execute_command(0x1D94, 0xF3);
        
        // Set up viewport
        thread.execute_command(0x0A00, (1280 << 16) | 0);
        thread.execute_command(0x0A04, (720 << 16) | 0);
        
        // Begin primitive
        thread.execute_command(0x1808, 5); // Triangles
        
        // Draw arrays: first=0, count=3
        let draw_data = (3 << 24) | 0; // count=3, first=0
        thread.execute_command(0x1810, draw_data);
        
        // End primitive
        thread.execute_command(0x1808, 0);
        
        thread.end_frame();
    }
    
    #[test]
    fn test_draw_indexed_runs_full_pipeline() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        thread.begin_frame();
        thread.execute_command(0x0304, 0x000000FF);
        thread.execute_command(0x1D94, 0xF3);
        
        // Begin primitive
        thread.execute_command(0x1808, 5);
        
        // Draw indexed: first=0, count=6
        let draw_data = (6 << 24) | 0;
        thread.execute_command(0x1814, draw_data);
        
        thread.execute_command(0x1808, 0);
        thread.end_frame();
    }
    
    #[test]
    fn test_surface_configuration_stored() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        // SET_SURFACE_FORMAT (0x0180)
        thread.execute_command(0x0180, 0x121);
        assert_eq!(thread.gfx_state.surface_format, 0x121);
        
        // SET_SURFACE_COLOR_TARGET (0x0200)
        thread.execute_command(0x0200, 1);
        assert_eq!(thread.gfx_state.surface_color_target, 1);
        
        // SET_SURFACE_COLOR_AOFFSET (0x0194)
        thread.execute_command(0x0194, 0x00100000);
        assert_eq!(thread.gfx_state.surface_offset_color[0], 0x00100000);
        
        // SET_SURFACE_ZETA_OFFSET (0x01B8)
        thread.execute_command(0x01B8, 0x00200000);
        assert_eq!(thread.gfx_state.surface_offset_depth, 0x00200000);
        
        // SET_SURFACE_PITCH_A (0x01A4)
        thread.execute_command(0x01A4, 5120);
        assert_eq!(thread.gfx_state.surface_pitch[0], 5120);
        
        // SET_SURFACE_CLIP (0x02BC, 0x02C0)
        thread.execute_command(0x02BC, (1280 << 16) | 0); // x=0, width=1280
        thread.execute_command(0x02C0, (720 << 16) | 0);  // y=0, height=720
        assert_eq!(thread.gfx_state.surface_clip_width, 1280);
        assert_eq!(thread.gfx_state.surface_clip_height, 720);
    }
    
    #[test]
    fn test_multiple_clears_in_frame() {
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        thread.begin_frame();
        
        // First clear: red
        thread.execute_command(0x0304, 0xFF0000FF);
        thread.execute_command(0x1D94, 0xF3);
        
        // Second clear: blue (in-render-pass clear path)
        thread.execute_command(0x0304, 0x0000FFFF);
        thread.execute_command(0x1D94, 0xF3);
        
        thread.end_frame();
    }
    
    #[test]
    fn test_full_draw_pipeline_sequence() {
        // Simulates a typical game frame: clear → set state → begin → draw → end
        let memory = MemoryManager::new().unwrap();
        let mut thread = RsxThread::new(memory);
        thread.init_backend().unwrap();
        
        thread.begin_frame();
        
        // 1. Surface setup
        thread.execute_command(0x0180, 0x121);  // SET_SURFACE_FORMAT
        thread.execute_command(0x0200, 1);       // SET_SURFACE_COLOR_TARGET
        
        // 2. Clear
        thread.execute_command(0x0304, 0x000000FF);
        thread.execute_command(0x1D94, 0xF3);
        
        // 3. Set viewport + scissor
        thread.execute_command(0x0A00, (1280 << 16));
        thread.execute_command(0x0A04, (720 << 16));
        thread.execute_command(0x08C0, (1280 << 16));
        thread.execute_command(0x08C4, (720 << 16));
        
        // 4. Configure vertex attributes
        thread.gfx_state.vertex_attrib_input_mask = 0x3;
        thread.gfx_state.vertex_attrib_format[0] = (32 << 8) | (4 << 4) | 1;
        thread.gfx_state.vertex_attrib_format[1] = (32 << 8) | (2 << 4) | 1;
        
        // 5. Begin primitive
        thread.execute_command(0x1808, 5);
        
        // 6. Draw
        thread.execute_command(0x1810, (3 << 24));
        
        // 7. End primitive
        thread.execute_command(0x1808, 0);
        
        // 8. End frame
        thread.end_frame();
        
        // Verify framebuffer is valid
        let fb = thread.get_framebuffer();
        assert!(fb.is_some());
    }
}
