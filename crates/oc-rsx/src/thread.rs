//! RSX thread (command processor)

use std::sync::Arc;
use oc_memory::MemoryManager;
use crate::state::RsxState;
use crate::fifo::CommandFifo;
use crate::methods::MethodHandler;
use crate::backend::{GraphicsBackend, null::NullBackend};

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
                self.clear_surface(data);
                return;
            }
            // NV4097_SET_BEGIN_END
            0x1808 => {
                if data == 0 {
                    // End primitive - flush vertices
                    self.flush_vertices();
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
        const DRAW_FIRST_MASK: u32 = 0xFFFFFF;
        const DRAW_COUNT_SHIFT: u32 = 24;
        const DRAW_COUNT_MASK: u32 = 0xFF;
        
        let first = data & DRAW_FIRST_MASK;
        let count = (data >> DRAW_COUNT_SHIFT) & DRAW_COUNT_MASK;
        
        tracing::trace!("Draw arrays: first={}, count={}", first, count);
        
        let primitive = self.convert_primitive_type();
        self.backend.draw_arrays(primitive, first, count);
    }

    /// Draw indexed command
    fn draw_indexed(&mut self, data: u32) {
        const DRAW_FIRST_MASK: u32 = 0xFFFFFF;
        const DRAW_COUNT_SHIFT: u32 = 24;
        const DRAW_COUNT_MASK: u32 = 0xFF;
        
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
        // Draw accumulated vertices
        tracing::trace!("Flush vertices");
        // TODO: Implement vertex buffer submission to backend
    }

    /// Get memory manager reference
    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
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
}
