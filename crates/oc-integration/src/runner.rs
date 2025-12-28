//! Main emulator runner that integrates all subsystems
//!
//! This module provides the EmulatorRunner which ties together:
//! - Memory Manager
//! - PPU threads and interpreter
//! - SPU threads and interpreter
//! - RSX graphics thread
//! - LV2 kernel syscalls
//! - Thread scheduler

use crate::loader::{GameLoader, LoadedGame};
use oc_core::{Config, EmulatorError, Result, Scheduler, ThreadId, ThreadState};
use oc_memory::MemoryManager;
use oc_ppu::{PpuInterpreter, PpuThread};
use oc_spu::{SpuInterpreter, SpuThread};
use oc_rsx::RsxThread;
use oc_lv2::SyscallHandler;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Emulator runner state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerState {
    /// Runner is stopped
    Stopped,
    /// Runner is running
    Running,
    /// Runner is paused
    Paused,
}

/// Main emulator runner
pub struct EmulatorRunner {
    /// Configuration
    config: Config,
    /// Current state
    state: RunnerState,
    /// Shared memory manager
    memory: Arc<MemoryManager>,
    /// PPU threads
    ppu_threads: RwLock<Vec<Arc<RwLock<PpuThread>>>>,
    /// PPU interpreter
    ppu_interpreter: Arc<PpuInterpreter>,
    /// SPU threads
    spu_threads: RwLock<Vec<Arc<RwLock<SpuThread>>>>,
    /// SPU interpreter
    spu_interpreter: Arc<SpuInterpreter>,
    /// RSX thread
    rsx_thread: Arc<RwLock<RsxThread>>,
    /// LV2 syscall handler
    syscall_handler: Arc<SyscallHandler>,
    /// Thread scheduler
    scheduler: Arc<RwLock<Scheduler>>,
    /// Frame counter
    frame_count: u64,
    /// Total cycles executed
    total_cycles: u64,
    /// Last frame time
    last_frame_time: Instant,
    /// Target frame time (16.67ms for 60 FPS)
    target_frame_time: Duration,
}

impl EmulatorRunner {
    /// Create a new emulator runner
    pub fn new(config: Config) -> Result<Self> {
        tracing::info!("Initializing emulator runner");

        // Create memory manager
        let memory = MemoryManager::new()
            .map_err(|e| EmulatorError::Memory(e))?;

        // Create PPU interpreter
        let ppu_interpreter = Arc::new(PpuInterpreter::new(memory.clone()));

        // Create SPU interpreter
        let spu_interpreter = Arc::new(SpuInterpreter::new());

        // Create RSX thread
        let rsx_thread = Arc::new(RwLock::new(RsxThread::new(memory.clone())));

        // Create syscall handler
        let syscall_handler = Arc::new(SyscallHandler::new());

        // Create scheduler
        let scheduler = Arc::new(RwLock::new(Scheduler::new()));

        // Target 60 FPS
        let target_frame_time = Duration::from_micros(16667);

        Ok(Self {
            config,
            state: RunnerState::Stopped,
            memory,
            ppu_threads: RwLock::new(Vec::new()),
            ppu_interpreter,
            spu_threads: RwLock::new(Vec::new()),
            spu_interpreter,
            rsx_thread,
            syscall_handler,
            scheduler,
            frame_count: 0,
            total_cycles: 0,
            last_frame_time: Instant::now(),
            target_frame_time,
        })
    }

    /// Initialize the RSX graphics backend
    pub fn init_graphics(&mut self) -> Result<()> {
        let mut rsx = self.rsx_thread.write();
        rsx.init_backend()
            .map_err(|e| EmulatorError::Rsx(
                oc_core::error::RsxError::Vulkan(e)
            ))
    }
    
    /// Get the current framebuffer data for display
    pub fn get_framebuffer(&self) -> Option<oc_rsx::FramebufferData> {
        let rsx = self.rsx_thread.read();
        rsx.get_framebuffer()
    }
    
    /// Get the framebuffer dimensions
    pub fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        let rsx = self.rsx_thread.read();
        rsx.get_dimensions()
    }

    /// Get the current state
    pub fn state(&self) -> RunnerState {
        self.state
    }

    /// Start the emulator
    pub fn start(&mut self) -> Result<()> {
        if self.state == RunnerState::Running {
            return Ok(());
        }

        tracing::info!("Starting emulator");
        self.state = RunnerState::Running;
        self.last_frame_time = Instant::now();

        Ok(())
    }

    /// Pause the emulator
    pub fn pause(&mut self) -> Result<()> {
        if self.state == RunnerState::Running {
            tracing::info!("Pausing emulator");
            self.state = RunnerState::Paused;
        }
        Ok(())
    }

    /// Resume the emulator
    pub fn resume(&mut self) -> Result<()> {
        if self.state == RunnerState::Paused {
            tracing::info!("Resuming emulator");
            self.state = RunnerState::Running;
            self.last_frame_time = Instant::now();
        }
        Ok(())
    }

    /// Stop the emulator
    pub fn stop(&mut self) -> Result<()> {
        tracing::info!("Stopping emulator");
        self.state = RunnerState::Stopped;
        Ok(())
    }

    /// Check if the emulator is running
    pub fn is_running(&self) -> bool {
        self.state == RunnerState::Running
    }

    /// Check if the emulator is paused
    pub fn is_paused(&self) -> bool {
        self.state == RunnerState::Paused
    }

    /// Check if the emulator is stopped
    pub fn is_stopped(&self) -> bool {
        self.state == RunnerState::Stopped
    }

    /// Create a new PPU thread
    pub fn create_ppu_thread(&self, priority: u32) -> Result<u32> {
        let thread_id = {
            let threads = self.ppu_threads.read();
            threads.len() as u32
        };

        let thread = Arc::new(RwLock::new(PpuThread::new(thread_id, self.memory.clone())));
        
        // Add to scheduler
        self.scheduler.write().add_thread(ThreadId::Ppu(thread_id), priority);

        // Add to thread list
        self.ppu_threads.write().push(thread);

        tracing::debug!("Created PPU thread {} with priority {}", thread_id, priority);
        Ok(thread_id)
    }

    /// Create a new SPU thread
    pub fn create_spu_thread(&self, priority: u32) -> Result<u32> {
        let thread_id = {
            let threads = self.spu_threads.read();
            threads.len() as u32
        };

        let thread = Arc::new(RwLock::new(SpuThread::new(thread_id, self.memory.clone())));
        
        // Add to scheduler
        self.scheduler.write().add_thread(ThreadId::Spu(thread_id), priority);

        // Add to thread list
        self.spu_threads.write().push(thread);

        tracing::debug!("Created SPU thread {} with priority {}", thread_id, priority);
        Ok(thread_id)
    }

    /// Load a game from a file path
    ///
    /// This will:
    /// 1. Parse the ELF/SELF file
    /// 2. Load segments into emulator memory
    /// 3. Create the main PPU thread with the correct entry point
    /// 4. Set up initial register state (stack, TOC, etc.)
    ///
    /// After calling this method, call `start()` to begin execution.
    pub fn load_game<P: AsRef<Path>>(&self, path: P) -> Result<LoadedGame> {
        tracing::info!("Loading game: {}", path.as_ref().display());

        // Create game loader
        let loader = GameLoader::new(self.memory.clone());

        // Load the game
        let game = loader.load(path)?;

        // Create the main PPU thread
        let thread_id = self.create_ppu_thread_with_entry(&game)?;

        tracing::info!(
            "Game loaded successfully, main thread {} created at entry 0x{:x}",
            thread_id,
            game.entry_point
        );

        Ok(game)
    }

    /// Create a PPU thread with a specific entry point and initial state
    ///
    /// Note: Thread ID is currently derived from the thread count, which could lead to
    /// ID conflicts if threads are removed. A proper implementation would use a
    /// monotonically increasing counter.
    fn create_ppu_thread_with_entry(&self, game: &LoadedGame) -> Result<u32> {
        // TODO: Use a dedicated thread ID counter instead of thread count
        // to ensure unique IDs even after thread removal
        let thread_id = {
            let threads = self.ppu_threads.read();
            threads.len() as u32
        };

        let mut thread = PpuThread::new(thread_id, self.memory.clone());

        // PS3 uses function descriptors (OPD) for entry points in some cases.
        // The entry point address in the ELF may point to a descriptor containing:
        // - u32: actual code address
        // - u32: TOC value
        // 
        // However, many games have the entry point pointing directly to code.
        // We need to distinguish between:
        // 1. OPD: first word is a valid code address (typically 0x10000 - 0x3FFFFFFF)
        // 2. Direct code: first word is an instruction opcode
        //
        // PS3 user memory is typically in range 0x00010000 - 0x3FFFFFFF.
        // Instruction opcodes often have high bits set (e.g., 0xbc3be527 for stmw).
        let (real_entry, toc) = if let (Ok(first_word), Ok(second_word)) = (
            self.memory.read_be32(game.entry_point as u32),
            self.memory.read_be32(game.entry_point as u32 + 4),
        ) {
            // Check if first word looks like a valid code address in PS3 user memory range
            // Valid code addresses are typically between 0x10000 and 0x40000000
            // and are 4-byte aligned
            let is_valid_code_addr = first_word >= 0x10000 
                && first_word < 0x40000000 
                && (first_word & 3) == 0;
            
            // Also check if TOC looks reasonable (in user memory range)
            let is_valid_toc = second_word >= 0x10000 && second_word < 0x40000000;
            
            if is_valid_code_addr && is_valid_toc {
                tracing::info!(
                    "OPD at 0x{:x}: code_addr=0x{:x}, rtoc=0x{:x}",
                    game.entry_point, first_word, second_word
                );
                (first_word as u64, second_word as u64)
            } else {
                // Entry point is direct code, not OPD
                tracing::info!(
                    "Entry point 0x{:x} is direct code (first_word=0x{:x}), using TOC from ELF: 0x{:x}",
                    game.entry_point, first_word, game.toc
                );
                (game.entry_point, game.toc)
            }
        } else {
            // Fallback if can't read memory
            (game.entry_point, game.toc)
        };

        // Set up initial register state according to PS3 ABI
        // R1 = Stack pointer (pointing to top of stack, grows downward)
        // The stack pointer needs a small offset from the top for the initial stack frame
        const PPU_STACK_START_OFFSET: u64 = 0x70;
        thread.set_gpr(1, game.stack_addr as u64 - PPU_STACK_START_OFFSET);
        
        // R2 = TOC (Table of Contents) pointer for PPC64 ELF ABI
        // Use TOC from OPD if available, otherwise from ELF
        thread.set_gpr(2, toc);
        
        // R3 = argc (0 for now, could be set to actual argument count)
        thread.set_gpr(3, 0);
        
        // R4 = argv (null for now)
        thread.set_gpr(4, 0);
        
        // R5 = envp (null for now)
        thread.set_gpr(5, 0);

        // R13 = Thread-Local Storage (TLS) pointer
        thread.set_gpr(13, game.tls_addr as u64);

        // Set program counter to real entry point (from OPD if available)
        thread.set_pc(real_entry);

        // Set stack info
        thread.stack_addr = game.stack_addr;
        thread.stack_size = game.stack_size;

        // Set thread name
        thread.name = "main".to_string();

        // Start the thread in running state
        thread.start();

        let thread_arc = Arc::new(RwLock::new(thread));
        
        // Add to scheduler with high priority (main thread)
        self.scheduler.write().add_thread(ThreadId::Ppu(thread_id), 1000);

        // Add to thread list
        self.ppu_threads.write().push(thread_arc);

        tracing::debug!(
            "Created main PPU thread {}: entry=0x{:x} (OPD at 0x{:x}), stack=0x{:08x}, sp=0x{:08x}, toc=0x{:x}, tls=0x{:08x}",
            thread_id,
            real_entry,
            game.entry_point,
            game.stack_addr,
            game.stack_addr as u64 - PPU_STACK_START_OFFSET,
            toc,
            game.tls_addr
        );

        Ok(thread_id)
    }

    /// Execute a single frame
    pub fn run_frame(&mut self) -> Result<()> {
        if self.state != RunnerState::Running {
            return Ok(());
        }

        let frame_start = Instant::now();

        // Begin graphics frame
        {
            let mut rsx = self.rsx_thread.write();
            rsx.begin_frame();
        }

        // Run threads for this frame
        self.run_threads()?;

        // Process RSX commands
        self.process_rsx()?;

        // End graphics frame and present
        {
            let mut rsx = self.rsx_thread.write();
            rsx.end_frame();
        }

        // Update frame timing
        self.frame_count += 1;
        let frame_time = frame_start.elapsed();

        // Sleep to maintain target frame rate
        if frame_time < self.target_frame_time {
            std::thread::sleep(self.target_frame_time - frame_time);
        }

        self.last_frame_time = Instant::now();

        Ok(())
    }

    /// Run threads using the scheduler
    fn run_threads(&mut self) -> Result<()> {
        const MAX_CYCLES_PER_FRAME: u64 = 100000;
        let mut cycles = 0;

        while cycles < MAX_CYCLES_PER_FRAME {
            // Schedule next thread
            let thread_id = match self.scheduler.write().schedule() {
                Some(id) => id,
                None => break, // No ready threads
            };

            // Execute thread based on type
            match thread_id {
                ThreadId::Ppu(id) => {
                    self.execute_ppu_thread(id)?;
                    cycles += 1;
                }
                ThreadId::Spu(id) => {
                    self.execute_spu_thread(id)?;
                    cycles += 1;
                }
            }

            // Update time slice (1 cycle = 1us approximation)
            self.scheduler.write().update_time_slice(1);

            // Check if time slice expired
            if self.scheduler.read().time_slice_expired() {
                self.scheduler.write().yield_current();
            }
        }

        self.total_cycles += cycles;
        Ok(())
    }

    /// Execute a single PPU thread step
    fn execute_ppu_thread(&self, thread_id: u32) -> Result<()> {
        let threads = self.ppu_threads.read();
        let thread_arc = threads.get(thread_id as usize)
            .ok_or_else(|| EmulatorError::Ppu(
                oc_core::error::PpuError::ThreadError(format!("Invalid thread ID: {}", thread_id))
            ))?;
        let mut thread = thread_arc.write();

        // Check if thread is in running state
        if !thread.is_running() {
            return Ok(());
        }

        // Check if we're about to execute a syscall instruction
        let pc = thread.pc() as u32;
        let opcode = match self.memory.read_be32(pc) {
            Ok(op) => op,
            Err(e) => {
                tracing::error!("Failed to read instruction at 0x{:08x}: {}", pc, e);
                thread.stop();
                self.scheduler.write().set_thread_state(
                    ThreadId::Ppu(thread_id),
                    ThreadState::Stopped
                );
                return Err(EmulatorError::Memory(e));
            }
        };

        // Check if it's a syscall instruction (sc opcode = 0x44000002)
        if opcode == 0x44000002 {
            // Get syscall number from R11
            let syscall_num = thread.gpr(11);
            
            // Get syscall arguments from registers
            let mut args = [0u64; 8];
            for (i, arg) in args.iter_mut().enumerate() {
                *arg = thread.gpr(3 + i); // R3-R10 are argument registers
            }

            // Execute syscall
            match self.syscall_handler.handle(syscall_num, &args) {
                Ok(result) => {
                    // Store result in R3
                    thread.set_gpr(3, result as u64);
                    thread.advance_pc();
                }
                Err(e) => {
                    tracing::error!("Syscall {} failed: {}", syscall_num, e);
                    // Set error code in R3
                    thread.set_gpr(3, 0xFFFFFFFFFFFFFFFF);
                    thread.advance_pc();
                }
            }
            return Ok(());
        }

        // Execute one instruction normally
        match self.ppu_interpreter.step(&mut thread) {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::error!("PPU thread {} error: {}", thread_id, e);
                thread.stop();
                self.scheduler.write().set_thread_state(
                    ThreadId::Ppu(thread_id),
                    ThreadState::Stopped
                );
                Err(EmulatorError::Ppu(e))
            }
        }
    }

    /// Execute a single SPU thread step
    fn execute_spu_thread(&self, thread_id: u32) -> Result<()> {
        let threads = self.spu_threads.read();
        let thread_arc = threads.get(thread_id as usize)
            .ok_or_else(|| EmulatorError::Spu(
                oc_core::error::SpuError::InvalidSpuId(thread_id)
            ))?;
        let mut thread = thread_arc.write();

        // Check if thread is in running state
        if !thread.is_running() {
            return Ok(());
        }

        // Execute one instruction
        match self.spu_interpreter.step(&mut thread) {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::error!("SPU thread {} error: {}", thread_id, e);
                thread.stop();
                self.scheduler.write().set_thread_state(
                    ThreadId::Spu(thread_id),
                    ThreadState::Stopped
                );
                Err(EmulatorError::Spu(e))
            }
        }
    }

    /// Process RSX graphics commands
    fn process_rsx(&self) -> Result<()> {
        let mut rsx = self.rsx_thread.write();
        
        // Process any pending commands in the FIFO
        rsx.process_commands();
        
        Ok(())
    }

    /// Get memory manager reference
    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
    }

    /// Get syscall handler reference
    pub fn syscall_handler(&self) -> &Arc<SyscallHandler> {
        &self.syscall_handler
    }

    /// Get scheduler reference
    pub fn scheduler(&self) -> &Arc<RwLock<Scheduler>> {
        &self.scheduler
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get total cycles executed
    pub fn total_cycles(&self) -> u64 {
        self.total_cycles
    }

    /// Get FPS (frames per second)
    pub fn fps(&self) -> f64 {
        let elapsed = self.last_frame_time.elapsed();
        if elapsed.as_secs_f64() > 0.0 {
            1.0 / elapsed.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Get PPU thread count
    pub fn ppu_thread_count(&self) -> usize {
        self.ppu_threads.read().len()
    }

    /// Get SPU thread count
    pub fn spu_thread_count(&self) -> usize {
        self.spu_threads.read().len()
    }

    /// Get configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_creation() {
        let config = Config::default();
        let runner = EmulatorRunner::new(config).unwrap();
        
        assert_eq!(runner.state(), RunnerState::Stopped);
        assert_eq!(runner.frame_count(), 0);
        assert_eq!(runner.ppu_thread_count(), 0);
        assert_eq!(runner.spu_thread_count(), 0);
    }

    #[test]
    fn test_runner_state_transitions() {
        let config = Config::default();
        let mut runner = EmulatorRunner::new(config).unwrap();

        assert!(runner.is_stopped());

        runner.start().unwrap();
        assert!(runner.is_running());

        runner.pause().unwrap();
        assert!(runner.is_paused());

        runner.resume().unwrap();
        assert!(runner.is_running());

        runner.stop().unwrap();
        assert!(runner.is_stopped());
    }

    #[test]
    fn test_create_ppu_thread() {
        let config = Config::default();
        let runner = EmulatorRunner::new(config).unwrap();

        let thread_id = runner.create_ppu_thread(100).unwrap();
        assert_eq!(thread_id, 0);
        assert_eq!(runner.ppu_thread_count(), 1);

        let thread_id2 = runner.create_ppu_thread(200).unwrap();
        assert_eq!(thread_id2, 1);
        assert_eq!(runner.ppu_thread_count(), 2);
    }

    #[test]
    fn test_create_spu_thread() {
        let config = Config::default();
        let runner = EmulatorRunner::new(config).unwrap();

        let thread_id = runner.create_spu_thread(100).unwrap();
        assert_eq!(thread_id, 0);
        assert_eq!(runner.spu_thread_count(), 1);

        let thread_id2 = runner.create_spu_thread(200).unwrap();
        assert_eq!(thread_id2, 1);
        assert_eq!(runner.spu_thread_count(), 2);
    }
}
