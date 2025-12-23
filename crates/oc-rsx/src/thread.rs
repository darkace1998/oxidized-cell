//! RSX thread (command processor)

use std::sync::Arc;
use oc_memory::MemoryManager;
use crate::state::RsxState;
use crate::fifo::CommandFifo;
use crate::methods::MethodHandler;

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
}

impl RsxThread {
    /// Create a new RSX thread
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        Self {
            state: RsxThreadState::Stopped,
            gfx_state: RsxState::new(),
            fifo: CommandFifo::new(),
            memory,
        }
    }

    /// Process commands from FIFO
    pub fn process_commands(&mut self) {
        while let Some(cmd) = self.fifo.pop() {
            self.execute_command(cmd.method, cmd.data);
        }
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
            _ => {}
        }
        
        // Use the method handler for state updates
        MethodHandler::execute(method, data, &mut self.gfx_state);
    }

    /// Clear the surface
    fn clear_surface(&mut self, _mask: u32) {
        // Clear color/depth/stencil based on mask
        tracing::trace!("Clear surface");
    }

    /// Flush accumulated vertices
    fn flush_vertices(&mut self) {
        // Draw accumulated vertices
        tracing::trace!("Flush vertices");
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
}
