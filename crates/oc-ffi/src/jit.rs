//! JIT interface
//!
//! This module provides the FFI interface to the C++ JIT compilers for both PPU and SPU.
//! It includes support for JIT invocation, code cache management, and breakpoint integration.

use std::ptr;

/// Opaque handle to PPU JIT compiler
#[repr(C)]
pub struct PpuJit {
    _private: [u8; 0],
}

/// Opaque handle to SPU JIT compiler
#[repr(C)]
pub struct SpuJit {
    _private: [u8; 0],
}

// FFI declarations for PPU JIT
extern "C" {
    fn oc_ppu_jit_create() -> *mut PpuJit;
    fn oc_ppu_jit_destroy(jit: *mut PpuJit);
    fn oc_ppu_jit_compile(jit: *mut PpuJit, address: u32, code: *const u8, size: usize) -> i32;
    fn oc_ppu_jit_get_compiled(jit: *mut PpuJit, address: u32) -> *mut u8;
    fn oc_ppu_jit_invalidate(jit: *mut PpuJit, address: u32);
    fn oc_ppu_jit_clear_cache(jit: *mut PpuJit);
    fn oc_ppu_jit_add_breakpoint(jit: *mut PpuJit, address: u32);
    fn oc_ppu_jit_remove_breakpoint(jit: *mut PpuJit, address: u32);
    fn oc_ppu_jit_has_breakpoint(jit: *mut PpuJit, address: u32) -> i32;
}

// FFI declarations for SPU JIT
extern "C" {
    fn oc_spu_jit_create() -> *mut SpuJit;
    fn oc_spu_jit_destroy(jit: *mut SpuJit);
    fn oc_spu_jit_compile(jit: *mut SpuJit, address: u32, code: *const u8, size: usize) -> i32;
    fn oc_spu_jit_get_compiled(jit: *mut SpuJit, address: u32) -> *mut u8;
    fn oc_spu_jit_invalidate(jit: *mut SpuJit, address: u32);
    fn oc_spu_jit_clear_cache(jit: *mut SpuJit);
    fn oc_spu_jit_add_breakpoint(jit: *mut SpuJit, address: u32);
    fn oc_spu_jit_remove_breakpoint(jit: *mut SpuJit, address: u32);
    fn oc_spu_jit_has_breakpoint(jit: *mut SpuJit, address: u32) -> i32;
}

/// Safe wrapper for PPU JIT compiler
pub struct PpuJitCompiler {
    handle: *mut PpuJit,
}

impl PpuJitCompiler {
    /// Create a new PPU JIT compiler
    pub fn new() -> Option<Self> {
        let handle = unsafe { oc_ppu_jit_create() };
        if handle.is_null() {
            None
        } else {
            Some(Self { handle })
        }
    }

    /// Compile a PPU code block starting at the given address
    pub fn compile(&mut self, address: u32, code: &[u8]) -> Result<(), JitError> {
        let result = unsafe {
            oc_ppu_jit_compile(self.handle, address, code.as_ptr(), code.len())
        };
        
        match result {
            0 => Ok(()),
            -1 => Err(JitError::InvalidInput),
            -2 => Err(JitError::Disabled),
            _ => Err(JitError::CompilationFailed),
        }
    }

    /// Get compiled code for a given address
    pub fn get_compiled(&self, address: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_ppu_jit_get_compiled(self.handle, address) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    /// Invalidate compiled code at a specific address
    pub fn invalidate(&mut self, address: u32) {
        unsafe { oc_ppu_jit_invalidate(self.handle, address) }
    }

    /// Clear the entire code cache
    pub fn clear_cache(&mut self) {
        unsafe { oc_ppu_jit_clear_cache(self.handle) }
    }

    /// Add a breakpoint at the specified address
    pub fn add_breakpoint(&mut self, address: u32) {
        unsafe { oc_ppu_jit_add_breakpoint(self.handle, address) }
    }

    /// Remove a breakpoint at the specified address
    pub fn remove_breakpoint(&mut self, address: u32) {
        unsafe { oc_ppu_jit_remove_breakpoint(self.handle, address) }
    }

    /// Check if a breakpoint exists at the specified address
    pub fn has_breakpoint(&self, address: u32) -> bool {
        unsafe { oc_ppu_jit_has_breakpoint(self.handle, address) != 0 }
    }
}

impl Drop for PpuJitCompiler {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { oc_ppu_jit_destroy(self.handle) }
        }
    }
}

unsafe impl Send for PpuJitCompiler {}

/// Safe wrapper for SPU JIT compiler
pub struct SpuJitCompiler {
    handle: *mut SpuJit,
}

impl SpuJitCompiler {
    /// Create a new SPU JIT compiler
    pub fn new() -> Option<Self> {
        let handle = unsafe { oc_spu_jit_create() };
        if handle.is_null() {
            None
        } else {
            Some(Self { handle })
        }
    }

    /// Compile an SPU code block starting at the given address
    pub fn compile(&mut self, address: u32, code: &[u8]) -> Result<(), JitError> {
        let result = unsafe {
            oc_spu_jit_compile(self.handle, address, code.as_ptr(), code.len())
        };
        
        match result {
            0 => Ok(()),
            -1 => Err(JitError::InvalidInput),
            -2 => Err(JitError::Disabled),
            _ => Err(JitError::CompilationFailed),
        }
    }

    /// Get compiled code for a given address
    pub fn get_compiled(&self, address: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_spu_jit_get_compiled(self.handle, address) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    /// Invalidate compiled code at a specific address
    pub fn invalidate(&mut self, address: u32) {
        unsafe { oc_spu_jit_invalidate(self.handle, address) }
    }

    /// Clear the entire code cache
    pub fn clear_cache(&mut self) {
        unsafe { oc_spu_jit_clear_cache(self.handle) }
    }

    /// Add a breakpoint at the specified address
    pub fn add_breakpoint(&mut self, address: u32) {
        unsafe { oc_spu_jit_add_breakpoint(self.handle, address) }
    }

    /// Remove a breakpoint at the specified address
    pub fn remove_breakpoint(&mut self, address: u32) {
        unsafe { oc_spu_jit_remove_breakpoint(self.handle, address) }
    }

    /// Check if a breakpoint exists at the specified address
    pub fn has_breakpoint(&self, address: u32) -> bool {
        unsafe { oc_spu_jit_has_breakpoint(self.handle, address) != 0 }
    }
}

impl Drop for SpuJitCompiler {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { oc_spu_jit_destroy(self.handle) }
        }
    }
}

unsafe impl Send for SpuJitCompiler {}

/// JIT compilation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitError {
    /// Invalid input parameters
    InvalidInput,
    /// JIT compiler is disabled
    Disabled,
    /// Compilation failed
    CompilationFailed,
}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JitError::InvalidInput => write!(f, "Invalid input parameters"),
            JitError::Disabled => write!(f, "JIT compiler is disabled"),
            JitError::CompilationFailed => write!(f, "JIT compilation failed"),
        }
    }
}

impl std::error::Error for JitError {}

/// JIT compiler handle (legacy, for backwards compatibility)
pub struct JitCompiler {
    _private: (),
}

impl JitCompiler {
    /// Create a new JIT compiler (placeholder for backwards compatibility)
    pub fn new() -> Option<Self> {
        Some(Self { _private: () })
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self { _private: () }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_jit_creation() {
        let jit = PpuJitCompiler::new();
        assert!(jit.is_some(), "PPU JIT should be created successfully");
    }

    #[test]
    fn test_spu_jit_creation() {
        let jit = SpuJitCompiler::new();
        assert!(jit.is_some(), "SPU JIT should be created successfully");
    }

    #[test]
    fn test_ppu_jit_compile() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        // Simple PPU instruction: nop (ori r0, r0, 0) = 0x60000000
        let code = [0x60, 0x00, 0x00, 0x00];
        let result = jit.compile(0x1000, &code);
        assert!(result.is_ok(), "Compilation should succeed");
    }

    #[test]
    fn test_spu_jit_compile() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        // Simple SPU instruction: nop = 0x40200000
        let code = [0x40, 0x20, 0x00, 0x00];
        let result = jit.compile(0x1000, &code);
        assert!(result.is_ok(), "Compilation should succeed");
    }

    #[test]
    fn test_ppu_breakpoint() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        let address = 0x1000;
        
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint initially");
        
        jit.add_breakpoint(address);
        assert!(jit.has_breakpoint(address), "Should have breakpoint after adding");
        
        jit.remove_breakpoint(address);
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint after removing");
    }

    #[test]
    fn test_spu_breakpoint() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        let address = 0x1000;
        
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint initially");
        
        jit.add_breakpoint(address);
        assert!(jit.has_breakpoint(address), "Should have breakpoint after adding");
        
        jit.remove_breakpoint(address);
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint after removing");
    }

    #[test]
    fn test_ppu_cache_operations() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        let code = [0x60, 0x00, 0x00, 0x00];
        let address = 0x1000;
        
        jit.compile(address, &code).expect("Compilation failed");
        
        // Should have compiled code
        let compiled = jit.get_compiled(address);
        assert!(compiled.is_some(), "Should have compiled code");
        
        // Invalidate it
        jit.invalidate(address);
        
        // Clear entire cache
        jit.clear_cache();
    }

    #[test]
    fn test_spu_cache_operations() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        let code = [0x40, 0x20, 0x00, 0x00];
        let address = 0x1000;
        
        jit.compile(address, &code).expect("Compilation failed");
        
        // Should have compiled code
        let compiled = jit.get_compiled(address);
        assert!(compiled.is_some(), "Should have compiled code");
        
        // Invalidate it
        jit.invalidate(address);
        
        // Clear entire cache
        jit.clear_cache();
    }
}
