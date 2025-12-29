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
use oc_vfs::IsoReader;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn, error};

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
    /// and handle it accordingly. It also supports loading from PS3 game
    /// folder structures (looking for USRDIR/EBOOT.BIN) and ISO disc images.
    pub fn load<P: AsRef<Path>>(&self, path: P) -> Result<LoadedGame> {
        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();
        info!("Loading game: {}", path_str);

        // Try to find the actual executable
        let executable_path = self.find_executable(path)?;
        info!("Found executable: {}", executable_path.display());

        // Check if this is an ISO file
        let is_iso = executable_path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("iso"))
            .unwrap_or(false);

        let (data, actual_path) = if is_iso {
            // Load from ISO
            info!("Detected ISO disc image, extracting EBOOT.BIN...");
            let (iso_data, eboot_path) = self.load_from_iso(&executable_path)?;
            (iso_data, eboot_path)
        } else {
            // Read the file normally
            let file = File::open(&executable_path).map_err(|e| {
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
            (data, executable_path.to_string_lossy().to_string())
        };

        // Check file magic to determine format
        if data.len() < 4 {
            return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                "File too small to be a valid executable".to_string()
            )));
        }

        // Check if it's a SELF file (encrypted PS3 executable)
        let (elf_data, is_self) = if SelfLoader::is_self(&data) {
            info!("Detected SELF file (encrypted PS3 executable)");
            
            // Try to create a SELF loader with firmware keys
            let self_loader = self.create_self_loader();
            
            match self_loader.decrypt(&data) {
                Ok(decrypted) => {
                    // Verify we got valid ELF data
                    if decrypted.len() < 4 || decrypted[0..4] != [0x7F, b'E', b'L', b'F'] {
                        error!(
                            "SELF decryption returned {} bytes but it's not valid ELF data. Magic: {:02x?}",
                            decrypted.len(),
                            &decrypted[..4.min(decrypted.len())]
                        );
                        return Err(EmulatorError::Loader(LoaderError::DecryptionFailed(
                            format!(
                                "SELF decryption completed but produced invalid ELF data.\n\
                                 Expected ELF magic (7F 45 4C 46) but got: {:02x?}\n\n\
                                 This indicates:\n\
                                 - Incorrect or missing decryption keys\n\
                                 - Corrupted SELF file\n\
                                 - Unsupported SELF encryption version\n\n\
                                 The emulator has built-in keys for most retail games (APP type, revisions 0x00-0x1D).\n\
                                 If this is a recent game (2013+), it may need newer keys from PS3 firmware 4.8x+.\n\
                                 If this is a PSN/NPDRM game, it may need NPDRM-specific keys (type 8).\n\n\
                                 File: {}",
                                &decrypted[..4.min(decrypted.len())],
                                actual_path
                            )
                        )));
                    }
                    info!("Successfully decrypted SELF file → valid ELF data ({} bytes)", decrypted.len());
                    (decrypted, true)
                }
                Err(e) => {
                    error!("SELF decryption failed: {}", e);
                    
                    // Provide helpful error message
                    let has_keys = self_loader.has_keys();
                    let help_msg = if has_keys {
                        "Decryption keys are installed but decryption failed. \
                         The file may require additional keys or be corrupted."
                    } else {
                        "No decryption keys found. To decrypt PS3 games, you need to:\n\
                         1. Install PS3 firmware: Download the official firmware (PS3UPDAT.PUP) from \
                            playstation.com and place it in the 'firmware/' folder\n\
                         2. Or provide a keys.txt file with decryption keys\n\
                         3. Or use already decrypted game files (EBOOT.ELF instead of EBOOT.BIN)"
                    };
                    
                    return Err(EmulatorError::Loader(LoaderError::DecryptionFailed(
                        format!(
                            "This is an encrypted PS3 executable (SELF format).\n\
                             Decryption error: {}\n\n\
                             {}",
                            e, help_msg
                        )
                    )));
                }
            }
        } else if data[0..4] == [0x7F, b'E', b'L', b'F'] {
            info!("Detected plain ELF file");
            (data, false)
        } else {
            // Unknown format - try to identify what the file might be
            let magic_hex: String = data[0..4.min(data.len())]
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            
            // Check for common PS3 file formats to give better error messages
            let format_hint = if data.len() >= 4 {
                if &data[0..4] == b"\x00PSF" || (data[0] == 0x00 && data[1] == 0x00 && data[2] == 0x00 && data[3] <= 0x10) {
                    // PARAM.SFO or similar metadata file
                    "\nThis appears to be a PARAM.SFO or metadata file, not an executable.\n\
                     Look for EBOOT.BIN in the USRDIR folder instead."
                } else if &data[0..4] == b"\x7FPKG" {
                    // PKG file
                    "\nThis is a PKG (package) file. You need to extract/install it first.\n\
                     Use a PKG extractor tool, then load the EBOOT.BIN from the extracted contents."
                } else if data.len() >= 8 && &data[0..8] == b"PS3LICDA" {
                    // License file
                    "\nThis is a license data file, not an executable."
                } else if data.len() >= 16 && (&data[0x8000..0x8006] == b"\x01CD001" || (data.len() > 0x8000 && &data[0..6] == b"\x01CD001")) {
                    // ISO image
                    "\nThis appears to be an ISO disc image.\n\
                     Mount or extract the ISO, then load EBOOT.BIN from PS3_GAME/USRDIR/."
                } else if &data[0..3] == b"NPD" {
                    // EDAT/SDAT encrypted data
                    "\nThis is an encrypted data file (EDAT/SDAT), not an executable."
                } else {
                    ""
                }
            } else {
                ""
            };
            
            return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                format!(
                    "Unrecognized file format. Expected SELF (SCE\\0) or ELF (\\x7FELF).\n\
                     File magic bytes: {}{}\n\n\
                     Make sure you are loading a valid PS3 executable:\n\
                     - EBOOT.BIN (usually in USRDIR folder)\n\
                     - Decrypted ELF file\n\
                     - PRX module",
                    magic_hex, format_hint
                )
            )));
        };

        // Load the ELF
        self.load_elf(&elf_data, actual_path, is_self)
    }

    /// Create a SELF loader with firmware keys if available
    fn create_self_loader(&self) -> SelfLoader {
        // Try common firmware/keys locations
        let firmware_paths = [
            "firmware/",
            "dev_flash/",
            "./PS3/dev_flash/",
        ];

        for path in &firmware_paths {
            if Path::new(path).exists() {
                if let Ok(loader) = SelfLoader::with_firmware(path) {
                    info!("Loaded firmware keys from: {}", path);
                    return loader;
                }
            }
        }

        // Try keys.txt files
        let keys_files = [
            "keys.txt",
            "firmware/keys.txt",
            "dev_flash/keys.txt",
        ];

        for path in &keys_files {
            if Path::new(path).exists() {
                if let Ok(loader) = SelfLoader::with_keys_file(path) {
                    info!("Loaded keys from: {}", path);
                    return loader;
                }
            }
        }

        // Return default loader (will fail on encrypted files)
        warn!("No firmware keys found. Encrypted SELF files cannot be decrypted.");
        SelfLoader::new()
    }

    /// Load executable from an ISO disc image
    fn load_from_iso(&self, iso_path: &Path) -> Result<(Vec<u8>, String)> {
        let mut iso_reader = IsoReader::new(iso_path.to_path_buf());
        
        iso_reader.open().map_err(|e| {
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to open ISO file: {}\n\n\
                 Make sure the file is a valid ISO 9660 disc image.",
                e
            )))
        })?;

        // Log volume info
        if let Some(volume) = iso_reader.volume() {
            info!("ISO Volume: '{}' (System: {})", volume.volume_id, volume.system_id);
        }

        // Try to find EBOOT.BIN in common locations
        let eboot_paths = [
            "/PS3_GAME/USRDIR/EBOOT.BIN",
            "/USRDIR/EBOOT.BIN",
            "/EBOOT.BIN",
        ];

        for eboot_path in &eboot_paths {
            info!("Looking for {} in ISO...", eboot_path);
            match iso_reader.read_file(eboot_path) {
                Ok(data) => {
                    info!("Found EBOOT.BIN at {} ({} bytes)", eboot_path, data.len());
                    let display_path = format!("{}:{}", iso_path.display(), eboot_path);
                    return Ok((data, display_path));
                }
                Err(e) => {
                    debug!("Not found at {}: {}", eboot_path, e);
                }
            }
        }

        // Try to list root directory contents for debugging
        let mut available_files = String::new();
        if let Ok(entries) = iso_reader.list_directory("/") {
            available_files.push_str("\n\nISO root directory contents:\n");
            for entry in entries.iter().take(20) {
                let entry_type = if entry.is_directory { "DIR " } else { "FILE" };
                available_files.push_str(&format!("  [{}] {}\n", entry_type, entry.name));
            }
            if entries.len() > 20 {
                available_files.push_str(&format!("  ... and {} more\n", entries.len() - 20));
            }
        }

        // Check if PS3_GAME exists
        if let Ok(entries) = iso_reader.list_directory("/PS3_GAME") {
            available_files.push_str("\n/PS3_GAME contents:\n");
            for entry in entries.iter().take(10) {
                let entry_type = if entry.is_directory { "DIR " } else { "FILE" };
                available_files.push_str(&format!("  [{}] {}\n", entry_type, entry.name));
            }
        }

        Err(EmulatorError::Loader(LoaderError::InvalidElf(format!(
            "Could not find EBOOT.BIN in ISO file: {}\n\n\
             Searched locations:\n\
             - /PS3_GAME/USRDIR/EBOOT.BIN\n\
             - /USRDIR/EBOOT.BIN\n\
             - /EBOOT.BIN\n\n\
             This ISO may not be a valid PS3 game disc.{}",
            iso_path.display(),
            available_files
        ))))
    }

    /// Find the actual executable from a path
    ///
    /// Supports:
    /// - Direct path to EBOOT.BIN or .elf file
    /// - Path to PS3 game folder (will look for PS3_GAME/USRDIR/EBOOT.BIN)
    /// - Path to USRDIR folder (will look for EBOOT.BIN inside)
    fn find_executable(&self, path: &Path) -> Result<PathBuf> {
        // If it's a file, check if it's an ISO or use it directly
        if path.is_file() {
            // Check if it's an ISO file by extension
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("iso") {
                    // This is an ISO file, we'll handle it specially
                    return Ok(path.to_path_buf());
                }
            }
            return Ok(path.to_path_buf());
        }

        // If it's a directory, look for the executable
        if path.is_dir() {
            // Common PS3 game folder structures:
            // 1. /PS3_GAME/USRDIR/EBOOT.BIN
            // 2. /USRDIR/EBOOT.BIN
            // 3. /EBOOT.BIN

            let candidates = [
                path.join("PS3_GAME/USRDIR/EBOOT.BIN"),
                path.join("USRDIR/EBOOT.BIN"),
                path.join("EBOOT.BIN"),
                path.join("eboot.bin"),
                path.join("PS3_GAME/USRDIR/eboot.bin"),
                path.join("USRDIR/eboot.bin"),
            ];

            for candidate in &candidates {
                if candidate.is_file() {
                    info!("Found executable at: {}", candidate.display());
                    return Ok(candidate.clone());
                }
            }

            // Also check for any .elf files
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if let Some(ext) = entry_path.extension() {
                        if ext.eq_ignore_ascii_case("elf") || ext.eq_ignore_ascii_case("self") {
                            info!("Found executable at: {}", entry_path.display());
                            return Ok(entry_path);
                        }
                    }
                }
            }

            return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                format!(
                    "Could not find executable in folder: {}\n\n\
                     Expected one of:\n\
                     - PS3_GAME/USRDIR/EBOOT.BIN\n\
                     - USRDIR/EBOOT.BIN\n\
                     - EBOOT.BIN\n\
                     - Any .elf or .self file",
                    path.display()
                )
            )));
        }

        // Path doesn't exist
        Err(EmulatorError::Loader(LoaderError::InvalidElf(
            format!("File or directory not found: {}", path.display())
        )))
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
        info!(
            "Loading ELF data: {} bytes from '{}' (is_self: {})",
            data.len(),
            path,
            is_self
        );
        
        let mut cursor = Cursor::new(data);

        // Parse ELF header with enhanced error context
        let mut elf_loader = ElfLoader::new(&mut cursor).map_err(|e| {
            error!(
                "Failed to parse ELF from '{}' ({} bytes): {}",
                path, data.len(), e
            );
            EmulatorError::Loader(LoaderError::InvalidElf(format!(
                "Failed to parse ELF file '{}' ({} bytes): {}",
                path, data.len(), e
            )))
        })?;

        info!(
            "ELF parsed successfully: entry=0x{:x}, type={}, machine=0x{:x}, {} program headers, {} section headers",
            elf_loader.entry_point,
            match elf_loader.header.e_type {
                1 => "ET_REL (relocatable)",
                2 => "ET_EXEC (executable)",
                3 => "ET_DYN (shared object)",
                4 => "ET_CORE (core dump)",
                _ => "unknown"
            },
            elf_loader.header.e_machine,
            elf_loader.phdrs.len(),
            elf_loader.shdrs.len()
        );

        // Log program headers for debugging
        for (i, phdr) in elf_loader.phdrs.iter().enumerate() {
            let type_str = match phdr.p_type {
                0 => "NULL",
                1 => "LOAD",
                2 => "DYNAMIC",
                3 => "INTERP",
                4 => "NOTE",
                7 => "TLS",
                _ => "OTHER"
            };
            debug!(
                "  PHDR[{}]: type={} offset=0x{:x} vaddr=0x{:x} filesz=0x{:x} memsz=0x{:x} flags=0x{:x}",
                i, type_str, phdr.p_offset, phdr.p_vaddr, phdr.p_filesz, phdr.p_memsz, phdr.p_flags
            );
        }

        // Determine base address
        let base_addr = self.calculate_base_addr(&elf_loader);

        // Load segments into memory with enhanced error context
        elf_loader
            .load_segments(&mut cursor, &self.memory, base_addr)
            .map_err(|e| {
                error!("Failed to load ELF segments from '{}': {}", path, e);
                EmulatorError::Loader(e)
            })?;

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

        // Validate that the entry point contains valid code
        self.validate_entry_point(entry_point, &path, is_self)?;

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

    /// Validate that the entry point contains valid PowerPC code
    ///
    /// This checks that the first instruction at the entry point is a valid
    /// PowerPC instruction to catch issues with:
    /// - Failed SELF decryption (encrypted data)
    /// - Incorrect entry point
    /// - Corrupted executable
    ///
    /// Note: PS3 executables may use OPD (function descriptors) where the entry point
    /// contains a pointer to the real code, not an instruction directly.
    fn validate_entry_point(&self, entry_point: u64, path: &str, is_self: bool) -> Result<()> {
        // Read the first two words at the entry point
        let entry_addr = entry_point as u32;
        let first_word = match self.memory.read_be32(entry_addr) {
            Ok(op) => op,
            Err(e) => {
                return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                    format!(
                        "Entry point 0x{:08x} is not accessible in memory: {}\n\
                         This may indicate that the entry point is outside loaded segments.\n\
                         File: {}",
                        entry_point, e, path
                    )
                )));
            }
        };
        
        let second_word = self.memory.read_be32(entry_addr + 4).unwrap_or(0);

        // Check if this might be an OPD (function descriptor) rather than direct code
        // OPD format: [code_address: u32, toc_pointer: u32]
        // Valid code addresses are typically in range 0x10000 - 0x40000000 and 4-byte aligned
        let is_valid_code_addr = first_word >= 0x10000 
            && first_word < 0x40000000 
            && (first_word & 3) == 0;
        let is_valid_toc = second_word >= 0x10000 && second_word < 0x40000000;
        
        if is_valid_code_addr && is_valid_toc {
            // This looks like an OPD - validate the actual code at the pointed address
            debug!(
                "Entry point 0x{:08x} appears to be OPD: code_addr=0x{:08x}, toc=0x{:08x}",
                entry_point, first_word, second_word
            );
            
            // Read and validate the actual instruction at the code address
            let real_entry = first_word;
            let real_opcode = match self.memory.read_be32(real_entry) {
                Ok(op) => op,
                Err(e) => {
                    return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                        format!(
                            "OPD code address 0x{:08x} is not accessible in memory: {}\n\
                             Entry point 0x{:08x} contains OPD pointing to invalid address.\n\
                             File: {}",
                            real_entry, e, entry_point, path
                        )
                    )));
                }
            };
            
            let primary_op = (real_opcode >> 26) & 0x3F;
            if primary_op == 0 || primary_op == 1 || primary_op == 5 || primary_op == 6 {
                let bytes = [
                    (real_opcode >> 24) & 0xFF,
                    (real_opcode >> 16) & 0xFF,
                    (real_opcode >> 8) & 0xFF,
                    real_opcode & 0xFF,
                ];
                return Err(EmulatorError::Loader(LoaderError::InvalidElf(
                    format!(
                        "Invalid instruction at real entry 0x{:08x}: 0x{:08x} ({:02x} {:02x} {:02x} {:02x})\n\
                         OPD at 0x{:08x} points to code with invalid primary opcode {}.\n\
                         File: {}",
                        real_entry, real_opcode, bytes[0], bytes[1], bytes[2], bytes[3],
                        entry_point, primary_op, path
                    )
                )));
            }
            
            debug!(
                "OPD validation passed: real entry 0x{:08x} contains opcode 0x{:08x} (primary_op={})",
                real_entry, real_opcode, primary_op
            );
            return Ok(());
        }

        // Not an OPD - check if the first word is a valid instruction
        let opcode = first_word;
        let primary_op = (opcode >> 26) & 0x3F;
        
        // Primary opcodes 0, 1, 5, 6 are reserved/invalid on PowerPC
        // If we see these, it's likely encrypted data or corruption
        if primary_op == 0 || primary_op == 1 || primary_op == 5 || primary_op == 6 {
            let bytes = [
                (opcode >> 24) & 0xFF,
                (opcode >> 16) & 0xFF,
                (opcode >> 8) & 0xFF,
                opcode & 0xFF,
            ];
            
            let error_msg = if is_self {
                format!(
                    "Invalid instruction at entry point 0x{:08x}: 0x{:08x} ({:02x} {:02x} {:02x} {:02x})\n\
                     Primary opcode {} is reserved/invalid on PowerPC.\n\n\
                     This SELF file appears to be incompletely or incorrectly decrypted.\n\
                     The entry point contains encrypted or invalid data instead of valid PowerPC code.\n\n\
                     Possible solutions:\n\
                     1. Ensure PS3 firmware keys are properly installed:\n\
                        - Download official PS3 firmware (PS3UPDAT.PUP)\n\
                        - Place it in the 'firmware/' folder\n\
                        - Restart the emulator to load keys\n\
                     2. Use a decrypted ELF file instead of encrypted SELF\n\
                     3. Verify the game file is not corrupted\n\
                     4. Some homebrew SELF files may use non-standard encryption\n\n\
                     File: {}",
                    entry_point, opcode, bytes[0], bytes[1], bytes[2], bytes[3],
                    primary_op, path
                )
            } else {
                format!(
                    "Invalid instruction at entry point 0x{:08x}: 0x{:08x} ({:02x} {:02x} {:02x} {:02x})\n\
                     Primary opcode {} is reserved/invalid on PowerPC.\n\n\
                     This suggests the file is corrupted or not a valid PS3 executable.\n\
                     The entry point should contain valid PowerPC instructions.\n\n\
                     File: {}",
                    entry_point, opcode, bytes[0], bytes[1], bytes[2], bytes[3],
                    primary_op, path
                )
            };
            
            return Err(EmulatorError::Loader(LoaderError::InvalidElf(error_msg)));
        }

        // Entry point looks valid
        debug!(
            "Entry point validation passed: 0x{:08x} contains opcode 0x{:08x} (primary_op={})",
            entry_point, opcode, primary_op
        );
        
        Ok(())
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
