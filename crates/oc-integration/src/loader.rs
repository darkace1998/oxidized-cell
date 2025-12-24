//! Game loader for loading PS3 executables into the emulator
//!
//! This module provides the GameLoader struct which handles loading
//! ELF/SELF files into emulator memory and setting up the initial
//! PPU thread state.

use oc_core::error::{EmulatorError, LoaderError};
use oc_core::Result;
use oc_loader::{ElfLoader, SelfLoader};
use oc_memory::MemoryManager;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Default stack size for PPU threads (1 MB)
const DEFAULT_STACK_SIZE: u32 = 0x0010_0000;

/// Default base address for game loading
const DEFAULT_BASE_ADDR: u32 = 0x1000_0000;

/// PS3 default stack base address
const STACK_BASE: u32 = 0xD000_0000;

/// Loaded game information
#[derive(Debug, Clone)]
pub struct LoadedGame {
    /// Entry point address
    pub entry_point: u64,
    /// Base address where the executable was loaded
    pub base_addr: u32,
    /// Stack address
    pub stack_addr: u32,
    /// Stack size
    pub stack_size: u32,
    /// Table of Contents (TOC) pointer for PPC64 ABI
    pub toc: u64,
    /// Original file path
    pub path: String,
    /// Whether the file was a SELF (encrypted) file
    pub is_self: bool,
}

/// Game loader for loading PS3 executables
pub struct GameLoader {
    /// Memory manager reference
    memory: Arc<MemoryManager>,
}

impl GameLoader {
    /// Create a new game loader
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        Self { memory }
    }

    /// Load a game from a file path
    ///
    /// This will automatically detect whether the file is an ELF or SELF file
    /// and handle it accordingly.
    pub fn load<P: AsRef<Path>>(&self, path: P) -> Result<LoadedGame> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        info!("Loading game: {}", path_str);

        // Read the file
        let file = File::open(&path).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to open file: {}",
                e
            )))
        })?;

        let mut reader = BufReader::new(file);
        let mut data = Vec::new();
        reader.read_to_end(&mut data).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!("Failed to read file: {}", e)))
        })?;

        // Check if it's a SELF file
        let (elf_data, is_self) = if SelfLoader::is_self(&data) {
            info!("Detected SELF file, attempting to decrypt/extract ELF");
            let self_loader = SelfLoader::new();
            match self_loader.decrypt(&data) {
                Ok(decrypted) => (decrypted, true),
                Err(e) => {
                    warn!("SELF decryption failed: {}. Trying as raw ELF.", e);
                    (data, false)
                }
            }
        } else {
            (data, false)
        };

        // Load the ELF
        self.load_elf(&elf_data, path_str, is_self)
    }

    /// Load an ELF file from bytes
    fn load_elf(&self, data: &[u8], path: String, is_self: bool) -> Result<LoadedGame> {
        let mut cursor = Cursor::new(data);

        // Parse ELF header
        let mut elf_loader = ElfLoader::new(&mut cursor).map_err(EmulatorError::Loader)?;

        info!(
            "ELF entry point: 0x{:x}, {} program headers",
            elf_loader.entry_point,
            elf_loader.phdrs.len()
        );

        // Determine base address
        let base_addr = self.calculate_base_addr(&elf_loader);

        // Load segments into memory
        elf_loader
            .load_segments(&mut cursor, &self.memory, base_addr)
            .map_err(EmulatorError::Loader)?;

        // Parse symbols for debugging
        if let Err(e) = elf_loader.parse_symbols(&mut cursor) {
            debug!("Failed to parse symbols (non-fatal): {}", e);
        }

        // Process relocations
        if let Err(e) = elf_loader.process_relocations(&mut cursor, &self.memory, base_addr) {
            debug!("Failed to process relocations (non-fatal): {}", e);
        }

        // Calculate the actual entry point address
        let entry_point = if elf_loader.entry_point < 0x1_0000_0000 {
            // Entry point is already an absolute address
            elf_loader.entry_point
        } else {
            // Entry point needs base address adjustment
            base_addr as u64 + elf_loader.entry_point
        };

        // Set up stack
        let stack_size = DEFAULT_STACK_SIZE;
        let stack_addr = STACK_BASE + stack_size; // Stack grows downward, so start at top

        // Calculate TOC (Table of Contents) pointer
        // For PPC64 ELF ABI, TOC is typically stored at .toc section or derived from entry point
        let toc = self.find_toc(&elf_loader, base_addr);

        info!(
            "Game loaded: entry=0x{:x}, base=0x{:08x}, stack=0x{:08x}, toc=0x{:x}",
            entry_point, base_addr, stack_addr, toc
        );

        Ok(LoadedGame {
            entry_point,
            base_addr,
            stack_addr,
            stack_size,
            toc,
            path,
            is_self,
        })
    }

    /// Calculate the base address for loading
    fn calculate_base_addr(&self, elf: &ElfLoader) -> u32 {
        // Check if ELF has a preferred base address
        for phdr in &elf.phdrs {
            if phdr.p_type == 1 {
                // PT_LOAD
                if phdr.p_vaddr > 0 && phdr.p_vaddr < 0x1_0000_0000 {
                    // Use the virtual address from the ELF
                    return 0; // No adjustment needed, use vaddr as-is
                }
            }
        }

        // Use default base address
        DEFAULT_BASE_ADDR
    }

    /// Find the TOC (Table of Contents) pointer
    fn find_toc(&self, elf: &ElfLoader, base_addr: u32) -> u64 {
        // Try to find .toc section
        for shdr in &elf.shdrs {
            if shdr.sh_addr > 0 {
                // TOC is often at .toc section address
                // For now, use a simple heuristic
                if shdr.sh_type == 1 && shdr.sh_flags & 0x2 != 0 {
                    // SHT_PROGBITS, writable
                    return base_addr as u64 + shdr.sh_addr;
                }
            }
        }

        // Fallback: TOC is typically entry_point + 0x8000 for PPC64
        elf.entry_point.saturating_add(0x8000)
    }

    /// Get memory manager reference
    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_memory() -> Arc<MemoryManager> {
        MemoryManager::new().unwrap()
    }

    #[test]
    fn test_game_loader_creation() {
        let memory = create_test_memory();
        let _loader = GameLoader::new(memory);
    }

    #[test]
    fn test_loaded_game_struct() {
        let game = LoadedGame {
            entry_point: 0x10000,
            base_addr: 0x10000000,
            stack_addr: 0xD0100000,
            stack_size: 0x100000,
            toc: 0x18000,
            path: "/test/game.elf".to_string(),
            is_self: false,
        };

        assert_eq!(game.entry_point, 0x10000);
        assert_eq!(game.base_addr, 0x10000000);
        assert!(!game.is_self);
    }

    #[test]
    fn test_calculate_base_addr_default() {
        let memory = create_test_memory();
        let _loader = GameLoader::new(memory);

        // Create a minimal ELF loader mock with no segments
        // Since we can't easily create an ElfLoader without valid data,
        // we'll just verify the default constant is correct
        assert_eq!(DEFAULT_BASE_ADDR, 0x10000000);
    }
}
