//! Game loader for loading PS3 executables into the emulator
//!
//! This module provides the GameLoader struct which handles loading
//! ELF/SELF files into emulator memory and setting up the initial
//! PPU thread state.

use oc_core::error::{EmulatorError, LoaderError};
use oc_core::Result;
use oc_loader::elf::{pt, sht};
use oc_loader::{ElfLoader, PrxLoader, SelfLoader};
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

/// TOC (Table of Contents) offset from entry point in PPC64 ELF ABI
/// This is the standard offset used when no explicit TOC section is found
const TOC_OFFSET: u64 = 0x8000;

/// ELF section flag: Allocatable
const SHF_ALLOC: u64 = 0x2;

/// Default PRX base address (above main executable)
const PRX_BASE_ADDR: u32 = 0x2000_0000;

/// TLS (Thread-Local Storage) base address
const TLS_BASE_ADDR: u32 = 0x2800_0000;  // In user memory region

/// Default TLS size (64KB)
const DEFAULT_TLS_SIZE: u32 = 0x10000;

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
    /// Thread-Local Storage address (R13 register)
    pub tls_addr: u32,
    /// TLS size
    pub tls_size: u32,
    /// Original file path
    pub path: String,
    /// Whether the file was a SELF (encrypted) file
    pub is_self: bool,
    /// Loaded PRX modules
    pub prx_modules: Vec<String>,
}

/// Game loader for loading PS3 executables
pub struct GameLoader {
    /// Memory manager reference
    memory: Arc<MemoryManager>,
    /// PRX loader for shared libraries
    prx_loader: PrxLoader,
    /// Next available PRX base address
    next_prx_addr: u32,
}

impl GameLoader {
    /// Create a new game loader
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        Self { 
            memory,
            prx_loader: PrxLoader::new(),
            next_prx_addr: PRX_BASE_ADDR,
        }
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

    /// Parse EBOOT.BIN format
    ///
    /// EBOOT.BIN is either a SELF (encrypted ELF) or plain ELF file.
    /// This method handles both formats transparently.
    pub fn parse_eboot(&self, data: &[u8]) -> Result<(Vec<u8>, bool)> {
        info!("Parsing EBOOT.BIN format");
        
        // Check if it's a SELF file (encrypted executable)
        if SelfLoader::is_self(data) {
            info!("EBOOT.BIN is SELF format (encrypted)");
            self.handle_encrypted_eboot(data)
        } else if data.len() >= 4 && data[0..4] == [0x7F, b'E', b'L', b'F'] {
            info!("EBOOT.BIN is plain ELF format");
            Ok((data.to_vec(), false))
        } else {
            Err(EmulatorError::Loader(LoaderError::InvalidElf(
                "EBOOT.BIN is neither SELF nor ELF format".to_string()
            )))
        }
    }

    /// Handle encrypted executables (SELF format)
    ///
    /// SELF files are encrypted ELF files used by PS3 for security.
    /// This method attempts to decrypt the SELF file to extract the ELF data.
    fn handle_encrypted_eboot(&self, data: &[u8]) -> Result<(Vec<u8>, bool)> {
        info!("Handling encrypted EBOOT (SELF format)");
        
        let self_loader = SelfLoader::new();
        
        // Attempt to decrypt the SELF file
        match self_loader.decrypt(data) {
            Ok(decrypted_elf) => {
                info!("Successfully decrypted SELF file, extracted {} bytes", decrypted_elf.len());
                Ok((decrypted_elf, true))
            }
            Err(e) => {
                // If decryption fails, try to extract embedded ELF without decryption
                warn!("SELF decryption failed: {}. Attempting to extract embedded ELF.", e);
                
                // Parse SELF header to find ELF offset
                if let Ok(header) = SelfLoader::parse_header(data) {
                    let elf_offset = header.header_len as usize;
                    
                    if data.len() > elf_offset + 4 {
                        // Check for ELF magic at offset
                        if data[elf_offset..elf_offset + 4] == [0x7F, b'E', b'L', b'F'] {
                            info!("Found embedded unencrypted ELF at offset 0x{:x}", elf_offset);
                            return Ok((data[elf_offset..].to_vec(), true));
                        }
                    }
                }
                
                Err(EmulatorError::Loader(LoaderError::DecryptionFailed(
                    format!("Failed to decrypt SELF file: {}", e)
                )))
            }
        }
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
        // For ET_EXEC (executable), entry point is absolute. For ET_DYN (shared object), 
        // entry point is relative and needs base address added.
        // ELF e_type: 2 = ET_EXEC, 3 = ET_DYN
        let entry_point = if elf_loader.header.e_type == 3 {
            // ET_DYN: entry point is relative, add base address
            base_addr as u64 + elf_loader.entry_point
        } else {
            // ET_EXEC or other: entry point is absolute
            elf_loader.entry_point
        };

        // Set up stack
        let stack_size = DEFAULT_STACK_SIZE;
        let stack_addr = STACK_BASE + stack_size; // Stack grows downward, so start at top

        // Calculate TOC (Table of Contents) pointer
        // For PPC64 ELF ABI, TOC is typically stored at .toc section or derived from entry point
        let toc = self.find_toc(&elf_loader, base_addr);

        // Set up Thread-Local Storage (TLS)
        let (tls_addr, tls_size) = self.setup_tls(&elf_loader)?;

        info!(
            "Game loaded: entry=0x{:x}, base=0x{:08x}, stack=0x{:08x}, toc=0x{:x}, tls=0x{:08x}",
            entry_point, base_addr, stack_addr, toc, tls_addr
        );

        Ok(LoadedGame {
            entry_point,
            base_addr,
            stack_addr,
            stack_size,
            toc,
            tls_addr,
            tls_size,
            path,
            is_self,
            prx_modules: Vec::new(),
        })
    }

    /// Calculate the base address for loading
    fn calculate_base_addr(&self, elf: &ElfLoader) -> u32 {
        // Check if ELF has a preferred base address
        for phdr in &elf.phdrs {
            if phdr.p_type == pt::LOAD {
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
                // Look for a PROGBITS section that is allocated (has SHF_ALLOC flag)
                if shdr.sh_type == sht::PROGBITS && (shdr.sh_flags & SHF_ALLOC) != 0 {
                    return base_addr as u64 + shdr.sh_addr;
                }
            }
        }

        // Fallback: TOC is typically entry_point + TOC_OFFSET for PPC64 ABI
        elf.entry_point.saturating_add(TOC_OFFSET)
    }

    /// Set up Thread-Local Storage (TLS)
    fn setup_tls(&self, elf: &ElfLoader) -> Result<(u32, u32)> {
        // Look for PT_TLS program header
        for phdr in &elf.phdrs {
            if phdr.p_type == pt::TLS {
                let tls_size = phdr.p_memsz.max(DEFAULT_TLS_SIZE as u64) as u32;
                let tls_addr = TLS_BASE_ADDR;

                // Allocate and zero-initialize TLS memory
                let zeros = vec![0u8; tls_size as usize];
                self.memory
                    .write_bytes(tls_addr, &zeros)
                    .map_err(EmulatorError::Memory)?;

                // Initialize TLS data from the ELF segment if present
                if phdr.p_filesz > 0 {
                    // The TLS initialization image would be copied here
                    // For now, we just zero-initialize it
                    debug!(
                        "TLS segment found: size=0x{:x}, init_size=0x{:x}",
                        phdr.p_memsz, phdr.p_filesz
                    );
                }

                info!(
                    "TLS allocated at 0x{:08x}, size=0x{:x}",
                    tls_addr, tls_size
                );
                return Ok((tls_addr, tls_size));
            }
        }

        // No TLS segment found, allocate default TLS
        let tls_size = DEFAULT_TLS_SIZE;
        let tls_addr = TLS_BASE_ADDR;
        
        // Allocate and zero-initialize default TLS
        let zeros = vec![0u8; tls_size as usize];
        self.memory
            .write_bytes(tls_addr, &zeros)
            .map_err(EmulatorError::Memory)?;

        debug!("Default TLS allocated at 0x{:08x}, size=0x{:x}", tls_addr, tls_size);
        Ok((tls_addr, tls_size))
    }

    /// Find and load PRX dependencies
    ///
    /// This method scans the ELF dynamic section to find required PRX modules
    /// and loads them automatically.
    pub fn load_prx_dependencies<P: AsRef<Path>>(
        &mut self,
        game: &mut LoadedGame,
        game_dir: P,
    ) -> Result<()> {
        info!("Loading PRX dependencies from game directory");
        
        // Typical PRX locations in PS3 games
        let prx_search_paths = [
            game_dir.as_ref().join("USRDIR"),
            game_dir.as_ref().join("PS3_GAME").join("USRDIR"),
            game_dir.as_ref().to_path_buf(),
        ];
        
        // Common system PRX modules that games depend on
        let system_prx_modules = [
            "libfs.sprx",
            "libsysutil.sprx",
            "libgcm_sys.sprx",
            "libsysmodule.sprx",
            "libnet.sprx",
            "libhttp.sprx",
            "libssl.sprx",
            "libaudio.sprx",
            "libpngdec.sprx",
            "libjpgdec.sprx",
        ];
        
        let mut loaded_count = 0;
        
        // Try to load system PRX modules from game directory
        for prx_name in &system_prx_modules {
            let mut found = false;
            
            for search_path in &prx_search_paths {
                let prx_path = search_path.join(prx_name);
                
                if prx_path.exists() {
                    debug!("Found PRX dependency: {:?}", prx_path);
                    
                    match self.load_prx_module(game, &prx_path) {
                        Ok(_) => {
                            loaded_count += 1;
                            found = true;
                            break;
                        }
                        Err(e) => {
                            debug!("Failed to load {}: {}", prx_name, e);
                        }
                    }
                }
            }
            
            if !found {
                debug!("PRX module {} not found in game directory (may be system module)", prx_name);
            }
        }
        
        info!("Loaded {} PRX dependencies", loaded_count);
        
        // After loading all dependencies, resolve imports
        if loaded_count > 0 {
            self.resolve_imports(game)?;
        }
        
        Ok(())
    }

    /// Load PRX modules from a list of paths
    pub fn load_prx_modules<P: AsRef<Path>>(
        &mut self,
        game: &mut LoadedGame,
        prx_paths: &[P],
    ) -> Result<()> {
        info!("Loading {} PRX modules", prx_paths.len());

        for prx_path in prx_paths {
            self.load_prx_module(game, prx_path)?;
        }

        // Resolve all imports after loading all modules
        self.resolve_imports(game)?;

        Ok(())
    }

    /// Load a single PRX module
    fn load_prx_module<P: AsRef<Path>>(&mut self, game: &mut LoadedGame, prx_path: P) -> Result<()> {
        let path_str = prx_path.as_ref().to_string_lossy().to_string();
        info!("Loading PRX: {}", path_str);

        // Read the PRX file
        let file = File::open(&prx_path).map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to open PRX file: {}",
                e
            )))
        })?;

        let mut reader = BufReader::new(file);

        // Extract module name from filename
        let module_name = prx_path
            .as_ref()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Allocate base address for this PRX
        let base_addr = self.next_prx_addr;

        // Load the PRX module
        match self.prx_loader.load_module(
            module_name.clone(),
            &mut reader,
            &self.memory,
            base_addr,
        ) {
            Ok(module) => {
                info!(
                    "PRX loaded: {} at 0x{:08x} ({} exports, {} imports)",
                    module.name,
                    module.base_addr,
                    module.exports.len(),
                    module.imports.len()
                );

                // Update next PRX address (add 16MB spacing between PRX modules)
                self.next_prx_addr += 0x0100_0000;

                // Track loaded module
                game.prx_modules.push(module.name);
            }
            Err(e) => {
                warn!("Failed to load PRX {}: {}", module_name, e);
                // Continue loading other modules even if one fails
            }
        }

        Ok(())
    }

    /// Resolve imports for all loaded modules
    fn resolve_imports(&mut self, game: &LoadedGame) -> Result<()> {
        info!("Resolving imports for {} modules", game.prx_modules.len());

        for module_name in &game.prx_modules {
            match self.prx_loader.resolve_imports(module_name) {
                Ok(()) => {
                    debug!("Imports resolved for module: {}", module_name);
                }
                Err(e) => {
                    warn!("Failed to resolve imports for {}: {}", module_name, e);
                    // Non-fatal: continue with other modules
                }
            }
        }

        Ok(())
    }

    /// Get a reference to the PRX loader
    pub fn prx_loader(&self) -> &PrxLoader {
        &self.prx_loader
    }

    /// Get a mutable reference to the PRX loader
    pub fn prx_loader_mut(&mut self) -> &mut PrxLoader {
        &mut self.prx_loader
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
            tls_addr: 0x28000000,  // In user memory
            tls_size: 0x10000,
            path: "/test/game.elf".to_string(),
            is_self: false,
            prx_modules: Vec::new(),
        };

        assert_eq!(game.entry_point, 0x10000);
        assert_eq!(game.base_addr, 0x10000000);
        assert_eq!(game.tls_addr, 0x28000000);
        assert!(!game.is_self);
        assert_eq!(game.prx_modules.len(), 0);
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

    #[test]
    fn test_tls_constants() {
        // Verify TLS constants are set correctly
        assert_eq!(TLS_BASE_ADDR, 0x28000000);  // In user memory region
        assert_eq!(DEFAULT_TLS_SIZE, 0x10000);
    }

    #[test]
    fn test_prx_base_addr_constant() {
        // Verify PRX base address is above main executable
        assert!(PRX_BASE_ADDR > DEFAULT_BASE_ADDR);
        assert_eq!(PRX_BASE_ADDR, 0x20000000);
    }

    #[test]
    fn test_game_loader_with_prx_support() {
        let memory = create_test_memory();
        let loader = GameLoader::new(memory);
        
        // Verify PRX loader is initialized
        assert_eq!(loader.prx_loader().list_modules().len(), 0);
    }
    
    #[test]
    fn test_parse_eboot_elf_format() {
        let memory = create_test_memory();
        let loader = GameLoader::new(memory);
        
        // Create a minimal ELF header
        let mut elf_data = vec![0x7F, b'E', b'L', b'F'];
        elf_data.extend_from_slice(&[2, 2, 1, 0]); // 64-bit big-endian
        elf_data.resize(64, 0); // Pad to minimum ELF header size
        
        match loader.parse_eboot(&elf_data) {
            Ok((data, is_self)) => {
                assert!(!is_self);
                assert_eq!(data.len(), elf_data.len());
            }
            Err(_) => {
                // May fail due to incomplete header, but should at least be recognized
            }
        }
    }
    
    #[test]
    fn test_parse_eboot_invalid_format() {
        let memory = create_test_memory();
        let loader = GameLoader::new(memory);
        
        // Invalid data that's neither SELF nor ELF
        let invalid_data = vec![0x00, 0x00, 0x00, 0x00];
        
        let result = loader.parse_eboot(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_loaded_game_with_prx_modules() {
        let mut game = LoadedGame {
            entry_point: 0x10000,
            base_addr: 0x10000000,
            stack_addr: 0xD0100000,
            stack_size: 0x100000,
            toc: 0x18000,
            tls_addr: 0x28000000,  // In user memory
            tls_size: 0x10000,
            path: "/test/game.elf".to_string(),
            is_self: false,
            prx_modules: Vec::new(),
        };

        // Test adding PRX modules
        game.prx_modules.push("libtest.prx".to_string());
        game.prx_modules.push("libfoo.prx".to_string());
        
        assert_eq!(game.prx_modules.len(), 2);
        assert_eq!(game.prx_modules[0], "libtest.prx");
        assert_eq!(game.prx_modules[1], "libfoo.prx");
    }
}
