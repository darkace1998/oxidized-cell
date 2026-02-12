//! SPU Runtime Environment (libsre) HLE
//!
//! This module provides HLE implementations for the PS3's SPU Runtime Environment.
//! The SPU Runtime provides module loading and execution support for SPU programs.

use std::collections::HashMap;
use tracing::{debug, trace};

/// Error codes
pub const SPU_RUNTIME_ERROR_NOT_INITIALIZED: i32 = 0x80410a01u32 as i32;
pub const SPU_RUNTIME_ERROR_INVALID_ARGUMENT: i32 = 0x80410a02u32 as i32;
pub const SPU_RUNTIME_ERROR_NO_MEMORY: i32 = 0x80410a03u32 as i32;
pub const SPU_RUNTIME_ERROR_MODULE_NOT_FOUND: i32 = 0x80410a04u32 as i32;
pub const SPU_RUNTIME_ERROR_ALREADY_LOADED: i32 = 0x80410a05u32 as i32;

/// Maximum number of SPU modules
pub const SPU_MAX_MODULES: usize = 64;

/// SPU module state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpuModuleState {
    /// Module not loaded
    #[default]
    NotLoaded = 0,
    /// Module is loading
    Loading = 1,
    /// Module is loaded and ready
    Loaded = 2,
    /// Module is running
    Running = 3,
    /// Module is stopped
    Stopped = 4,
    /// Module loading failed
    Failed = 5,
}

/// SPU module type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpuModuleType {
    /// Unknown type
    #[default]
    Unknown = 0,
    /// ELF executable
    Elf = 1,
    /// SPU module (SPRX)
    Sprx = 2,
}

/// SPU module information
#[derive(Debug, Clone)]
pub struct SpuModuleInfo {
    /// Module ID
    pub id: u32,
    /// Module name
    pub name: String,
    /// Module path
    pub path: String,
    /// Module type
    pub module_type: SpuModuleType,
    /// Module state
    pub state: SpuModuleState,
    /// Entry point address
    pub entry_point: u32,
    /// Code size
    pub code_size: u32,
    /// Data size
    pub data_size: u32,
    /// Load address in SPU local store
    pub load_address: u32,
    /// Parsed ELF header
    pub elf_header: Option<SpuElfHeader>,
    /// Relocations
    pub relocations: Vec<SpuRelocation>,
    /// Symbol table
    pub symbols: Vec<SpuSymbol>,
    /// Local store image (loaded segments)
    pub local_store: Option<SpuLocalStore>,
}

impl Default for SpuModuleInfo {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            path: String::new(),
            module_type: SpuModuleType::Unknown,
            state: SpuModuleState::NotLoaded,
            entry_point: 0,
            code_size: 0,
            data_size: 0,
            load_address: 0,
            elf_header: None,
            relocations: Vec::new(),
            symbols: Vec::new(),
            local_store: None,
        }
    }
}

/// SPU segment information
#[derive(Debug, Clone, Default)]
pub struct SpuSegment {
    /// Segment type (1=Load, etc.)
    pub seg_type: u32,
    /// Virtual address
    pub vaddr: u32,
    /// File size
    pub file_size: u32,
    /// Memory size
    pub mem_size: u32,
    /// Segment flags
    pub flags: u32,
    /// File offset
    pub offset: u32,
}

/// SPU ELF header (simplified)
#[derive(Debug, Clone, Default)]
pub struct SpuElfHeader {
    /// ELF magic valid
    pub valid: bool,
    /// Entry point
    pub entry: u32,
    /// Program header offset
    pub phoff: u32,
    /// Number of program headers
    pub phnum: u16,
    /// Section header offset
    pub shoff: u32,
    /// Number of section headers
    pub shnum: u16,
    /// Section header string table index
    pub shstrndx: u16,
    /// Segments
    pub segments: Vec<SpuSegment>,
}

/// SPU relocation type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuRelocationType {
    /// No relocation
    None = 0,
    /// 32-bit absolute address: S + A
    Addr32 = 1,
    /// 16-bit absolute address (for SPU form 1 instructions): (S + A) >> 2
    Addr16 = 2,
    /// Relative 32-bit: S + A - P
    Rel32 = 10,
}

/// SPU relocation entry
#[derive(Debug, Clone)]
pub struct SpuRelocation {
    /// Offset in section where relocation applies
    pub offset: u32,
    /// Symbol index
    pub sym_index: u32,
    /// Relocation type
    pub rel_type: SpuRelocationType,
    /// Addend
    pub addend: i32,
}

/// SPU symbol entry
#[derive(Debug, Clone)]
pub struct SpuSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol value (address)
    pub value: u32,
    /// Symbol size
    pub size: u32,
    /// Section index (0 = undefined/import)
    pub section_index: u16,
    /// Is this an imported symbol?
    pub is_import: bool,
    /// Resolved address (after relocation)
    pub resolved_address: Option<u32>,
}

/// SPU local store contents for a loaded module
#[derive(Debug, Clone)]
pub struct SpuLocalStore {
    /// Local store memory (256KB)
    pub memory: Vec<u8>,
    /// Total code bytes loaded
    pub code_size: u32,
    /// Total data bytes loaded
    pub data_size: u32,
}

/// SPU Runtime manager
pub struct SpuRuntimeManager {
    /// Initialization flag
    initialized: bool,
    /// Loaded modules
    modules: HashMap<u32, SpuModuleInfo>,
    /// Next module ID
    next_module_id: u32,
    /// SPU local store size (256KB per SPU)
    local_store_size: u32,
    /// Number of available SPUs
    num_spus: u32,
}

impl SpuRuntimeManager {
    /// Create a new SPU Runtime manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            modules: HashMap::new(),
            next_module_id: 1,
            local_store_size: 256 * 1024, // 256KB
            num_spus: 6,
        }
    }

    /// Initialize the SPU Runtime
    pub fn init(&mut self, num_spus: u32) -> i32 {
        if self.initialized {
            return SPU_RUNTIME_ERROR_ALREADY_LOADED;
        }

        debug!("SpuRuntimeManager::init: num_spus={}", num_spus);

        self.num_spus = num_spus.min(8);
        self.initialized = true;

        0 // CELL_OK
    }

    /// Finalize the SPU Runtime
    pub fn finalize(&mut self) -> i32 {
        if !self.initialized {
            return SPU_RUNTIME_ERROR_NOT_INITIALIZED;
        }

        debug!("SpuRuntimeManager::finalize");

        // Unload all modules
        self.modules.clear();
        self.initialized = false;

        0 // CELL_OK
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    // ========================================================================
    // Module Loading
    // ========================================================================

    /// Load an SPU module from path
    pub fn load_module(&mut self, path: &str, name: &str) -> Result<u32, i32> {
        if !self.initialized {
            return Err(SPU_RUNTIME_ERROR_NOT_INITIALIZED);
        }

        if self.modules.len() >= SPU_MAX_MODULES {
            return Err(SPU_RUNTIME_ERROR_NO_MEMORY);
        }

        // Check if already loaded
        if self.modules.values().any(|m| m.path == path) {
            return Err(SPU_RUNTIME_ERROR_ALREADY_LOADED);
        }

        let module_id = self.next_module_id;
        self.next_module_id += 1;

        debug!(
            "SpuRuntimeManager::load_module: id={}, path={}, name={}",
            module_id, path, name
        );

        let module = SpuModuleInfo {
            id: module_id,
            name: name.to_string(),
            path: path.to_string(),
            module_type: SpuModuleType::Elf,
            state: SpuModuleState::Loading,
            entry_point: 0,
            code_size: 0,
            data_size: 0,
            load_address: 0,
            elf_header: None,
            relocations: Vec::new(),
            symbols: Vec::new(),
            local_store: None,
        };

        self.modules.insert(module_id, module);

        // Simulate successful load
        if let Some(module) = self.modules.get_mut(&module_id) {
            module.state = SpuModuleState::Loaded;
        }

        Ok(module_id)
    }

    /// Load an SPU module from binary data
    pub fn load_module_data(&mut self, name: &str, data: &[u8]) -> Result<u32, i32> {
        if !self.initialized {
            return Err(SPU_RUNTIME_ERROR_NOT_INITIALIZED);
        }

        if self.modules.len() >= SPU_MAX_MODULES {
            return Err(SPU_RUNTIME_ERROR_NO_MEMORY);
        }

        let module_id = self.next_module_id;
        self.next_module_id += 1;

        debug!(
            "SpuRuntimeManager::load_module_data: id={}, name={}, size={}",
            module_id, name, data.len()
        );

        // Parse ELF header (basic validation)
        let header = self.parse_elf_header(data);
        if !header.valid {
            return Err(SPU_RUNTIME_ERROR_INVALID_ARGUMENT);
        }

        // Load PT_LOAD segments into local store image
        let local_store = self.load_segments_to_local_store(data, &header)?;

        // Parse relocations from ELF section headers
        let relocations = self.parse_relocations(data, &header);

        // Parse symbol table
        let symbols = self.parse_symbols(data, &header);

        let module = SpuModuleInfo {
            id: module_id,
            name: name.to_string(),
            path: String::new(),
            module_type: SpuModuleType::Elf,
            state: SpuModuleState::Loaded,
            entry_point: header.entry,
            code_size: local_store.code_size,
            data_size: local_store.data_size,
            load_address: 0,
            elf_header: Some(header),
            relocations,
            symbols,
            local_store: Some(local_store),
        };

        self.modules.insert(module_id, module);

        Ok(module_id)
    }

    /// Parse SPU ELF header and program headers
    fn parse_elf_header(&self, data: &[u8]) -> SpuElfHeader {
        let mut header = SpuElfHeader::default();

        // Minimum ELF header size
        if data.len() < 52 {
            return header;
        }

        // Check ELF magic: 0x7F 'E' 'L' 'F'
        if data[0..4] != [0x7F, b'E', b'L', b'F'] {
            return header;
        }

        // Check for SPU class (32-bit)
        if data[4] != 1 {
            return header;
        }

        header.valid = true;

        // Entry point (big-endian for Cell/BE SPU)
        header.entry = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);

        // Program header offset
        header.phoff = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);

        // Section header offset
        header.shoff = u32::from_be_bytes([data[32], data[33], data[34], data[35]]);

        // Number of program headers (at offset 44)
        header.phnum = u16::from_be_bytes([data[44], data[45]]);

        // Section header entry size at 46, section count at 48, shstrndx at 50
        if data.len() >= 52 {
            header.shnum = u16::from_be_bytes([data[48], data[49]]);
            header.shstrndx = u16::from_be_bytes([data[50], data[51]]);
        }

        // Parse program headers (each is 32 bytes for 32-bit ELF)
        let ph_entry_size = 32usize;
        let ph_start = header.phoff as usize;
        for i in 0..header.phnum as usize {
            let off = ph_start + i * ph_entry_size;
            if off + ph_entry_size > data.len() {
                break;
            }

            let seg = SpuSegment {
                seg_type: u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]),
                offset: u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]),
                vaddr: u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]),
                // paddr at off+12, skip
                file_size: u32::from_be_bytes([data[off + 16], data[off + 17], data[off + 18], data[off + 19]]),
                mem_size: u32::from_be_bytes([data[off + 20], data[off + 21], data[off + 22], data[off + 23]]),
                flags: u32::from_be_bytes([data[off + 24], data[off + 25], data[off + 26], data[off + 27]]),
            };

            trace!(
                "SpuRuntimeManager: segment[{}] type={} vaddr=0x{:08X} filesz={} memsz={} flags=0x{:X}",
                i, seg.seg_type, seg.vaddr, seg.file_size, seg.mem_size, seg.flags
            );

            header.segments.push(seg);
        }

        trace!(
            "SpuRuntimeManager: ELF entry=0x{:08X}, phoff={}, phnum={}, shoff={}, shnum={}",
            header.entry, header.phoff, header.phnum, header.shoff, header.shnum
        );

        header
    }

    // ========================================================================
    // Segment Loading
    // ========================================================================

    /// Load PT_LOAD segments from ELF data into a local store image
    fn load_segments_to_local_store(&self, data: &[u8], header: &SpuElfHeader) -> Result<SpuLocalStore, i32> {
        let mut ls = SpuLocalStore {
            memory: vec![0u8; self.local_store_size as usize],
            code_size: 0,
            data_size: 0,
        };

        const PT_LOAD: u32 = 1;
        // SPU segment flags: PF_X = 1, PF_W = 2, PF_R = 4
        const PF_X: u32 = 1;

        for seg in &header.segments {
            if seg.seg_type != PT_LOAD {
                continue;
            }

            let dst_start = seg.vaddr as usize;
            let dst_end = dst_start + seg.mem_size as usize;

            // Validate fits in local store
            if dst_end > ls.memory.len() {
                debug!(
                    "SpuRuntimeManager: segment vaddr=0x{:08X} size={} exceeds local store",
                    seg.vaddr, seg.mem_size
                );
                return Err(SPU_RUNTIME_ERROR_NO_MEMORY);
            }

            // Copy file data
            let src_start = seg.offset as usize;
            let copy_len = seg.file_size as usize;
            if src_start + copy_len <= data.len() {
                ls.memory[dst_start..dst_start + copy_len]
                    .copy_from_slice(&data[src_start..src_start + copy_len]);
            }

            // Zero-fill BSS (mem_size > file_size)
            if seg.mem_size > seg.file_size {
                let bss_start = dst_start + copy_len;
                let bss_end = dst_start + seg.mem_size as usize;
                for b in &mut ls.memory[bss_start..bss_end] {
                    *b = 0;
                }
            }

            // Track code vs data size
            if seg.flags & PF_X != 0 {
                ls.code_size += seg.mem_size;
            } else {
                ls.data_size += seg.mem_size;
            }

            debug!(
                "SpuRuntimeManager: loaded segment vaddr=0x{:08X} filesz={} memsz={} flags=0x{:X}",
                seg.vaddr, seg.file_size, seg.mem_size, seg.flags
            );
        }

        Ok(ls)
    }

    // ========================================================================
    // Relocation Handling
    // ========================================================================

    /// Parse relocation entries from ELF section data (SHT_RELA = 4)
    pub fn parse_relocations(&self, data: &[u8], header: &SpuElfHeader) -> Vec<SpuRelocation> {
        let mut relocs = Vec::new();

        // Parse section headers to find SHT_RELA sections
        const SH_ENTRY_SIZE: usize = 40; // 32-bit ELF section header is 40 bytes
        const SHT_RELA: u32 = 4;

        let sh_start = header.shoff as usize;
        for i in 0..header.shnum as usize {
            let off = sh_start + i * SH_ENTRY_SIZE;
            if off + SH_ENTRY_SIZE > data.len() {
                break;
            }

            let sh_type = u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            if sh_type != SHT_RELA {
                continue;
            }

            let sh_offset = u32::from_be_bytes([data[off + 16], data[off + 17], data[off + 18], data[off + 19]]);
            let sh_size = u32::from_be_bytes([data[off + 20], data[off + 21], data[off + 22], data[off + 23]]);

            // Each RELA entry is 12 bytes: offset(4) + info(4) + addend(4)
            let rela_start = sh_offset as usize;
            let num_entries = sh_size as usize / 12;

            for j in 0..num_entries {
                let roff = rela_start + j * 12;
                if roff + 12 > data.len() {
                    break;
                }

                let r_offset = u32::from_be_bytes([data[roff], data[roff + 1], data[roff + 2], data[roff + 3]]);
                let r_info = u32::from_be_bytes([data[roff + 4], data[roff + 5], data[roff + 6], data[roff + 7]]);
                let r_addend = i32::from_be_bytes([data[roff + 8], data[roff + 9], data[roff + 10], data[roff + 11]]);

                let sym_index = r_info >> 8;
                let rel_type_val = r_info & 0xFF;

                let rel_type = match rel_type_val {
                    1 => SpuRelocationType::Addr32,
                    2 => SpuRelocationType::Addr16,
                    10 => SpuRelocationType::Rel32,
                    _ => SpuRelocationType::None,
                };

                relocs.push(SpuRelocation {
                    offset: r_offset,
                    sym_index,
                    rel_type,
                    addend: r_addend,
                });
            }
        }

        trace!("SpuRuntimeManager: parsed {} relocations", relocs.len());
        relocs
    }

    /// Apply relocations to a local store image
    pub fn apply_relocations(
        local_store: &mut SpuLocalStore,
        relocations: &[SpuRelocation],
        symbols: &[SpuSymbol],
        base_address: u32,
    ) -> i32 {
        let mut applied = 0;

        for reloc in relocations {
            // Get symbol value (S)
            let sym_value = if (reloc.sym_index as usize) < symbols.len() {
                symbols[reloc.sym_index as usize]
                    .resolved_address
                    .unwrap_or(symbols[reloc.sym_index as usize].value)
            } else {
                0
            };

            let offset = reloc.offset as usize;
            let a = reloc.addend;
            let p = base_address.wrapping_add(reloc.offset);

            match reloc.rel_type {
                SpuRelocationType::Addr32 => {
                    // S + A
                    let val = sym_value.wrapping_add(a as u32);
                    if offset + 4 <= local_store.memory.len() {
                        local_store.memory[offset..offset + 4]
                            .copy_from_slice(&val.to_be_bytes());
                        applied += 1;
                    }
                }
                SpuRelocationType::Addr16 => {
                    // (S + A) >> 2, used for SPU branch targets (word-addressed)
                    let val = sym_value.wrapping_add(a as u32) >> 2;
                    if offset + 2 <= local_store.memory.len() {
                        let existing = u32::from_be_bytes([
                            local_store.memory[offset.saturating_sub(2).min(offset)],
                            local_store.memory[offset.saturating_sub(1).min(offset)],
                            local_store.memory[offset],
                            local_store.memory[(offset + 1).min(local_store.memory.len() - 1)],
                        ]);
                        // Patch the 16-bit immediate field (bits 15:0 of the instruction word)
                        let patched = (existing & 0xFFFF0000) | (val & 0xFFFF);
                        let bytes = patched.to_be_bytes();
                        if offset >= 2 && offset + 2 <= local_store.memory.len() {
                            local_store.memory[offset - 2..offset + 2].copy_from_slice(&bytes);
                        }
                        applied += 1;
                    }
                }
                SpuRelocationType::Rel32 => {
                    // S + A - P
                    let val = sym_value.wrapping_add(a as u32).wrapping_sub(p);
                    if offset + 4 <= local_store.memory.len() {
                        local_store.memory[offset..offset + 4]
                            .copy_from_slice(&val.to_be_bytes());
                        applied += 1;
                    }
                }
                SpuRelocationType::None => {}
            }
        }

        debug!("SpuRuntimeManager: applied {}/{} relocations", applied, relocations.len());
        0 // CELL_OK
    }

    // ========================================================================
    // Symbol Resolution
    // ========================================================================

    /// Parse symbol table from ELF section data (SHT_SYMTAB = 2)
    pub fn parse_symbols(&self, data: &[u8], header: &SpuElfHeader) -> Vec<SpuSymbol> {
        let mut symbols = Vec::new();

        const SH_ENTRY_SIZE: usize = 40;
        const SHT_SYMTAB: u32 = 2;
        const SHT_STRTAB: u32 = 3;

        let sh_start = header.shoff as usize;

        // First find the string table for symbol names
        let mut strtab_offset = 0usize;
        let mut strtab_size = 0usize;
        let mut symtab_offset = 0usize;
        let mut symtab_size = 0usize;
        let mut symtab_link = 0u32;

        for i in 0..header.shnum as usize {
            let off = sh_start + i * SH_ENTRY_SIZE;
            if off + SH_ENTRY_SIZE > data.len() {
                break;
            }

            let sh_type = u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);

            if sh_type == SHT_SYMTAB {
                symtab_offset = u32::from_be_bytes([data[off + 16], data[off + 17], data[off + 18], data[off + 19]]) as usize;
                symtab_size = u32::from_be_bytes([data[off + 20], data[off + 21], data[off + 22], data[off + 23]]) as usize;
                symtab_link = u32::from_be_bytes([data[off + 24], data[off + 25], data[off + 26], data[off + 27]]);
            }
        }

        // Find the linked string table
        if symtab_size > 0 {
            let link_idx = symtab_link as usize;
            let link_off = sh_start + link_idx * SH_ENTRY_SIZE;
            if link_off + SH_ENTRY_SIZE <= data.len() {
                let sh_type = u32::from_be_bytes([data[link_off + 4], data[link_off + 5], data[link_off + 6], data[link_off + 7]]);
                if sh_type == SHT_STRTAB {
                    strtab_offset = u32::from_be_bytes([data[link_off + 16], data[link_off + 17], data[link_off + 18], data[link_off + 19]]) as usize;
                    strtab_size = u32::from_be_bytes([data[link_off + 20], data[link_off + 21], data[link_off + 22], data[link_off + 23]]) as usize;
                }
            }
        }

        // Each 32-bit ELF symbol entry is 16 bytes: name(4) + value(4) + size(4) + info(1) + other(1) + shndx(2)
        let num_symbols = symtab_size / 16;
        for i in 0..num_symbols {
            let soff = symtab_offset + i * 16;
            if soff + 16 > data.len() {
                break;
            }

            let name_idx = u32::from_be_bytes([data[soff], data[soff + 1], data[soff + 2], data[soff + 3]]) as usize;
            let value = u32::from_be_bytes([data[soff + 4], data[soff + 5], data[soff + 6], data[soff + 7]]);
            let size = u32::from_be_bytes([data[soff + 8], data[soff + 9], data[soff + 10], data[soff + 11]]);
            let shndx = u16::from_be_bytes([data[soff + 14], data[soff + 15]]);

            // Read name from string table
            let name = if name_idx > 0 && strtab_offset + name_idx < strtab_offset + strtab_size && strtab_offset + name_idx < data.len() {
                let start = strtab_offset + name_idx;
                let end = data[start..].iter().position(|&b| b == 0).map(|p| start + p).unwrap_or(start);
                String::from_utf8_lossy(&data[start..end]).to_string()
            } else {
                String::new()
            };

            // SHN_UNDEF = 0 means the symbol is imported (undefined)
            let is_import = shndx == 0 && !name.is_empty();

            symbols.push(SpuSymbol {
                name,
                value,
                size,
                section_index: shndx,
                is_import,
                resolved_address: if !is_import { Some(value) } else { None },
            });
        }

        trace!("SpuRuntimeManager: parsed {} symbols ({} imports)",
            symbols.len(),
            symbols.iter().filter(|s| s.is_import).count()
        );

        symbols
    }

    /// Resolve imported symbols against a set of exported symbols
    /// Returns the number of symbols successfully resolved
    pub fn resolve_symbols(
        symbols: &mut [SpuSymbol],
        exports: &HashMap<String, u32>,
    ) -> usize {
        let mut resolved_count = 0;

        for sym in symbols.iter_mut() {
            if sym.is_import && sym.resolved_address.is_none() {
                if let Some(&addr) = exports.get(&sym.name) {
                    sym.resolved_address = Some(addr);
                    resolved_count += 1;
                    trace!("SpuRuntimeManager: resolved import '{}' → 0x{:08X}", sym.name, addr);
                } else {
                    debug!("SpuRuntimeManager: unresolved import '{}'", sym.name);
                }
            }
        }

        resolved_count
    }

    /// Unload an SPU module
    pub fn unload_module(&mut self, module_id: u32) -> i32 {
        if !self.initialized {
            return SPU_RUNTIME_ERROR_NOT_INITIALIZED;
        }

        if let Some(module) = self.modules.remove(&module_id) {
            debug!("SpuRuntimeManager::unload_module: id={}, name={}", module_id, module.name);
            0 // CELL_OK
        } else {
            SPU_RUNTIME_ERROR_MODULE_NOT_FOUND
        }
    }

    /// Get module info
    pub fn get_module_info(&self, module_id: u32) -> Option<&SpuModuleInfo> {
        self.modules.get(&module_id)
    }

    /// Get module by name
    pub fn get_module_by_name(&self, name: &str) -> Option<&SpuModuleInfo> {
        self.modules.values().find(|m| m.name == name)
    }

    /// Get loaded module count
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// List all loaded modules
    pub fn list_modules(&self) -> Vec<&SpuModuleInfo> {
        self.modules.values().collect()
    }

    // ========================================================================
    // Module Execution
    // ========================================================================

    /// Start execution of a module
    pub fn start_module(&mut self, module_id: u32, spu_id: u32, arg: u64) -> i32 {
        if !self.initialized {
            return SPU_RUNTIME_ERROR_NOT_INITIALIZED;
        }

        if spu_id >= self.num_spus {
            return SPU_RUNTIME_ERROR_INVALID_ARGUMENT;
        }

        let module = match self.modules.get_mut(&module_id) {
            Some(m) => m,
            None => return SPU_RUNTIME_ERROR_MODULE_NOT_FOUND,
        };

        if module.state != SpuModuleState::Loaded && module.state != SpuModuleState::Stopped {
            return SPU_RUNTIME_ERROR_INVALID_ARGUMENT;
        }

        debug!(
            "SpuRuntimeManager::start_module: id={}, spu={}, arg=0x{:016X}",
            module_id, spu_id, arg
        );

        // In a real implementation, this would:
        // 1. DMA module code to SPU local store
        // 2. Set up SPU argument registers
        // 3. Start SPU execution

        module.state = SpuModuleState::Running;

        0 // CELL_OK
    }

    /// Stop execution of a module
    pub fn stop_module(&mut self, module_id: u32) -> i32 {
        if !self.initialized {
            return SPU_RUNTIME_ERROR_NOT_INITIALIZED;
        }

        let module = match self.modules.get_mut(&module_id) {
            Some(m) => m,
            None => return SPU_RUNTIME_ERROR_MODULE_NOT_FOUND,
        };

        if module.state != SpuModuleState::Running {
            return SPU_RUNTIME_ERROR_INVALID_ARGUMENT;
        }

        debug!("SpuRuntimeManager::stop_module: id={}", module_id);

        module.state = SpuModuleState::Stopped;

        0 // CELL_OK
    }

    /// Check if module is running
    pub fn is_module_running(&self, module_id: u32) -> bool {
        self.modules.get(&module_id)
            .map(|m| m.state == SpuModuleState::Running)
            .unwrap_or(false)
    }

    /// Get module state
    pub fn get_module_state(&self, module_id: u32) -> Option<SpuModuleState> {
        self.modules.get(&module_id).map(|m| m.state)
    }

    // ========================================================================
    // Module Communication
    // ========================================================================

    /// Send data to SPU module (via mailbox)
    pub fn send_to_module(&self, module_id: u32, data: u32) -> i32 {
        if !self.initialized {
            return SPU_RUNTIME_ERROR_NOT_INITIALIZED;
        }

        if !self.is_module_running(module_id) {
            return SPU_RUNTIME_ERROR_INVALID_ARGUMENT;
        }

        trace!("SpuRuntimeManager::send_to_module: id={}, data=0x{:08X}", module_id, data);

        // In a real implementation, this would write to SPU mailbox

        0 // CELL_OK
    }

    /// Receive data from SPU module (via mailbox)
    pub fn receive_from_module(&self, module_id: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(SPU_RUNTIME_ERROR_NOT_INITIALIZED);
        }

        if !self.is_module_running(module_id) {
            return Err(SPU_RUNTIME_ERROR_INVALID_ARGUMENT);
        }

        trace!("SpuRuntimeManager::receive_from_module: id={}", module_id);

        // In a real implementation, this would read from SPU mailbox
        // For HLE, return 0
        Ok(0)
    }

    /// Signal interrupt to SPU module
    pub fn signal_module(&self, module_id: u32, signal: u32) -> i32 {
        if !self.initialized {
            return SPU_RUNTIME_ERROR_NOT_INITIALIZED;
        }

        if !self.modules.contains_key(&module_id) {
            return SPU_RUNTIME_ERROR_MODULE_NOT_FOUND;
        }

        debug!("SpuRuntimeManager::signal_module: id={}, signal=0x{:08X}", module_id, signal);

        // In a real implementation, this would write to SPU signal register

        0 // CELL_OK
    }

    // ========================================================================
    // Local Store Management
    // ========================================================================

    /// Get local store size
    pub fn get_local_store_size(&self) -> u32 {
        self.local_store_size
    }

    /// Get number of available SPUs
    pub fn get_num_spus(&self) -> u32 {
        self.num_spus
    }
}

impl Default for SpuRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Public API Functions
// ============================================================================

/// spu_initialize - Initialize SPU Runtime
pub fn spu_initialize(num_spus: u32) -> i32 {
    debug!("spu_initialize(num_spus={})", num_spus);
    crate::context::get_hle_context_mut().spu_runtime.init(num_spus)
}

/// spu_finalize - Finalize SPU Runtime
pub fn spu_finalize() -> i32 {
    debug!("spu_finalize()");
    crate::context::get_hle_context_mut().spu_runtime.finalize()
}

/// spu_image_import - Import SPU image
pub fn spu_image_import(
    _image_addr: u32,
    _src_addr: u32,
    _size: u32,
) -> i32 {
    debug!("spu_image_import()");
    
    // For HLE, just acknowledge
    0 // CELL_OK
}

/// spu_image_close - Close SPU image
pub fn spu_image_close(_image_addr: u32) -> i32 {
    debug!("spu_image_close()");
    0 // CELL_OK
}

/// spu_thread_group_create - Create SPU thread group
pub fn spu_thread_group_create(
    _group_id_addr: u32,
    num_threads: u32,
    _priority: u32,
    _attr_addr: u32,
) -> i32 {
    debug!("spu_thread_group_create(num_threads={})", num_threads);
    
    // For HLE, just acknowledge
    0 // CELL_OK
}

/// spu_thread_group_destroy - Destroy SPU thread group
pub fn spu_thread_group_destroy(_group_id: u32) -> i32 {
    debug!("spu_thread_group_destroy()");
    0 // CELL_OK
}

/// spu_thread_group_start - Start SPU thread group
pub fn spu_thread_group_start(_group_id: u32) -> i32 {
    debug!("spu_thread_group_start()");
    0 // CELL_OK
}

/// spu_thread_group_terminate - Terminate SPU thread group
pub fn spu_thread_group_terminate(_group_id: u32, _exit_status: i32) -> i32 {
    debug!("spu_thread_group_terminate()");
    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spu_runtime_manager_lifecycle() {
        let mut manager = SpuRuntimeManager::new();
        
        assert!(!manager.is_initialized());
        
        assert_eq!(manager.init(6), 0);
        assert!(manager.is_initialized());
        
        assert_eq!(manager.finalize(), 0);
        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_spu_runtime_manager_not_initialized() {
        let mut manager = SpuRuntimeManager::new();
        
        assert!(manager.load_module("/path/to/module.elf", "test").is_err());
        assert!(manager.unload_module(1) != 0);
    }

    #[test]
    fn test_spu_runtime_manager_load_module() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        let module_id = manager.load_module("/path/to/module.elf", "test_module");
        assert!(module_id.is_ok());
        let module_id = module_id.unwrap();
        
        assert_eq!(manager.module_count(), 1);
        
        let info = manager.get_module_info(module_id);
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "test_module");
        assert_eq!(info.unwrap().state, SpuModuleState::Loaded);
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_load_module_data() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        // Valid SPU ELF header (minimal)
        let elf_data = [
            0x7F, b'E', b'L', b'F', // Magic
            1, 2, 1, 0,             // 32-bit, big-endian, version
            0, 0, 0, 0, 0, 0, 0, 0, // Padding
            0, 2, 0, 0x17,          // Type=EXEC, Machine=SPU
            0, 0, 0, 1,             // Version
            0, 0, 0x10, 0x00,       // Entry point
            0, 0, 0, 0x34,          // Program header offset
            0, 0, 0, 0,             // Section header offset
            0, 0, 0, 0,             // Flags
            0, 0x34, 0, 0x20,       // ELF header size, PH entry size
            0, 1, 0, 0,             // Number of program headers
            0, 0, 0, 0,             // Section header entry size, count
        ];
        
        let module_id = manager.load_module_data("elf_module", &elf_data);
        assert!(module_id.is_ok());
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_invalid_elf() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        // Invalid data
        let bad_data = [0, 1, 2, 3, 4, 5];
        
        let result = manager.load_module_data("bad_module", &bad_data);
        assert!(result.is_err());
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_unload_module() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        let module_id = manager.load_module("/path/test.elf", "test").unwrap();
        assert_eq!(manager.module_count(), 1);
        
        assert_eq!(manager.unload_module(module_id), 0);
        assert_eq!(manager.module_count(), 0);
        
        // Unload again should fail
        assert!(manager.unload_module(module_id) != 0);
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_start_stop_module() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        let module_id = manager.load_module("/path/test.elf", "test").unwrap();
        
        // Start module
        assert_eq!(manager.start_module(module_id, 0, 0x12345678), 0);
        assert!(manager.is_module_running(module_id));
        assert_eq!(manager.get_module_state(module_id), Some(SpuModuleState::Running));
        
        // Stop module
        assert_eq!(manager.stop_module(module_id), 0);
        assert!(!manager.is_module_running(module_id));
        assert_eq!(manager.get_module_state(module_id), Some(SpuModuleState::Stopped));
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_module_communication() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        let module_id = manager.load_module("/path/test.elf", "test").unwrap();
        manager.start_module(module_id, 0, 0);
        
        // Send data
        assert_eq!(manager.send_to_module(module_id, 0xDEADBEEF), 0);
        
        // Receive data
        let data = manager.receive_from_module(module_id);
        assert!(data.is_ok());
        
        // Signal
        assert_eq!(manager.signal_module(module_id, 0x01), 0);
        
        manager.stop_module(module_id);
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_get_by_name() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        manager.load_module("/path/a.elf", "module_a").unwrap();
        manager.load_module("/path/b.elf", "module_b").unwrap();
        
        let module = manager.get_module_by_name("module_a");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name, "module_a");
        
        assert!(manager.get_module_by_name("nonexistent").is_none());
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_list_modules() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);
        
        manager.load_module("/path/a.elf", "a").unwrap();
        manager.load_module("/path/b.elf", "b").unwrap();
        manager.load_module("/path/c.elf", "c").unwrap();
        
        let modules = manager.list_modules();
        assert_eq!(modules.len(), 3);
        
        manager.finalize();
    }

    #[test]
    fn test_spu_runtime_manager_local_store() {
        let manager = SpuRuntimeManager::new();
        
        assert_eq!(manager.get_local_store_size(), 256 * 1024);
        assert_eq!(manager.get_num_spus(), 6);
    }

    #[test]
    fn test_spu_module_state_enum() {
        assert_eq!(SpuModuleState::NotLoaded as u32, 0);
        assert_eq!(SpuModuleState::Loading as u32, 1);
        assert_eq!(SpuModuleState::Loaded as u32, 2);
        assert_eq!(SpuModuleState::Running as u32, 3);
        assert_eq!(SpuModuleState::Stopped as u32, 4);
        assert_eq!(SpuModuleState::Failed as u32, 5);
    }

    #[test]
    fn test_spu_module_type_enum() {
        assert_eq!(SpuModuleType::Unknown as u32, 0);
        assert_eq!(SpuModuleType::Elf as u32, 1);
        assert_eq!(SpuModuleType::Sprx as u32, 2);
    }

    // ========================================================================
    // ELF Segment Parsing Tests
    // ========================================================================

    /// Helper: build a minimal SPU ELF with PT_LOAD segments
    fn make_spu_elf_with_segments(entry: u32, segments: &[(u32, u32, u32, u32, u32)]) -> Vec<u8> {
        // segments: (seg_type, offset, vaddr, file_size, flags)
        let phnum = segments.len() as u16;
        let phoff: u32 = 52; // right after ELF header
        let ph_table_size = phnum as usize * 32;
        let data_start = 52 + ph_table_size;

        let mut elf = vec![0u8; data_start + 256]; // extra space for segment data

        // ELF header
        elf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        elf[4] = 1; // 32-bit
        elf[5] = 2; // big-endian
        elf[6] = 1; // version
        elf[24..28].copy_from_slice(&entry.to_be_bytes());
        elf[28..32].copy_from_slice(&phoff.to_be_bytes());
        elf[44..46].copy_from_slice(&phnum.to_be_bytes());

        for (i, &(seg_type, offset, vaddr, file_size, flags)) in segments.iter().enumerate() {
            let poff = 52 + i * 32;
            elf[poff..poff + 4].copy_from_slice(&seg_type.to_be_bytes());
            elf[poff + 4..poff + 8].copy_from_slice(&offset.to_be_bytes()); // p_offset
            elf[poff + 8..poff + 12].copy_from_slice(&vaddr.to_be_bytes()); // p_vaddr
            // p_paddr at +12, skip
            elf[poff + 16..poff + 20].copy_from_slice(&file_size.to_be_bytes()); // p_filesz
            elf[poff + 20..poff + 24].copy_from_slice(&file_size.to_be_bytes()); // p_memsz = filesz
            elf[poff + 24..poff + 28].copy_from_slice(&flags.to_be_bytes()); // p_flags
        }

        // Write some recognizable data at the segment offsets
        for &(_, offset, _, file_size, _) in segments {
            let start = offset as usize;
            let end = (start + file_size as usize).min(elf.len());
            for j in start..end {
                if j < elf.len() {
                    elf[j] = 0xAA; // marker byte
                }
            }
        }

        elf
    }

    #[test]
    fn test_spu_elf_parse_segments() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);

        // ELF with 2 PT_LOAD segments: code at 0x0, data at 0x100
        let elf = make_spu_elf_with_segments(0x1000, &[
            (1, 200, 0x0000, 32, 5),  // PT_LOAD, executable (PF_R|PF_X)
            (1, 240, 0x0100, 16, 6),  // PT_LOAD, data (PF_R|PF_W)
        ]);

        let header = manager.parse_elf_header(&elf);
        assert!(header.valid);
        assert_eq!(header.entry, 0x1000);
        assert_eq!(header.segments.len(), 2);
        assert_eq!(header.segments[0].seg_type, 1);
        assert_eq!(header.segments[0].vaddr, 0x0000);
        assert_eq!(header.segments[0].file_size, 32);
        assert_eq!(header.segments[1].vaddr, 0x0100);

        manager.finalize();
    }

    #[test]
    fn test_spu_load_segments_to_local_store() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);

        let elf = make_spu_elf_with_segments(0x0, &[
            (1, 200, 0x0000, 32, 5), // code
            (1, 240, 0x1000, 16, 6), // data
        ]);

        let header = manager.parse_elf_header(&elf);
        let ls = manager.load_segments_to_local_store(&elf, &header);
        assert!(ls.is_ok());

        let ls = ls.unwrap();
        // Code segment should be loaded at vaddr 0x0
        assert_eq!(ls.memory[0], 0xAA); // our marker byte
        // Data segment at vaddr 0x1000
        assert_eq!(ls.memory[0x1000], 0xAA);
        // code_size should include the executable segment
        assert!(ls.code_size > 0);

        manager.finalize();
    }

    #[test]
    fn test_spu_load_module_data_with_segments() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);

        let elf = make_spu_elf_with_segments(0x0080, &[
            (1, 200, 0x0000, 32, 5), // code at 0
        ]);

        let module_id = manager.load_module_data("seg_test", &elf).unwrap();
        let info = manager.get_module_info(module_id).unwrap();
        assert_eq!(info.entry_point, 0x0080);
        assert!(info.local_store.is_some());
        assert!(info.elf_header.is_some());

        let ls = info.local_store.as_ref().unwrap();
        assert!(ls.code_size > 0);

        manager.finalize();
    }

    #[test]
    fn test_spu_segment_exceeds_local_store() {
        let mut manager = SpuRuntimeManager::new();
        manager.init(6);

        // Segment at vaddr way beyond 256KB local store
        let elf = make_spu_elf_with_segments(0x0, &[
            (1, 200, 0x3F000, 0x2000, 5), // 8KB at 252KB → exceeds 256KB
        ]);

        let header = manager.parse_elf_header(&elf);
        let result = manager.load_segments_to_local_store(&elf, &header);
        assert!(result.is_err());

        manager.finalize();
    }

    // ========================================================================
    // Relocation Tests
    // ========================================================================

    #[test]
    fn test_spu_apply_relocation_addr32() {
        let mut ls = SpuLocalStore {
            memory: vec![0u8; 256],
            code_size: 0,
            data_size: 0,
        };

        let symbols = vec![SpuSymbol {
            name: "test_sym".to_string(),
            value: 0x1000,
            size: 4,
            section_index: 1,
            is_import: false,
            resolved_address: Some(0x1000),
        }];

        let relocs = vec![SpuRelocation {
            offset: 0x10,
            sym_index: 0,
            rel_type: SpuRelocationType::Addr32,
            addend: 4,
        }];

        SpuRuntimeManager::apply_relocations(&mut ls, &relocs, &symbols, 0);

        // Should write 0x1000 + 4 = 0x1004 at offset 0x10
        let val = u32::from_be_bytes([ls.memory[0x10], ls.memory[0x11], ls.memory[0x12], ls.memory[0x13]]);
        assert_eq!(val, 0x1004);
    }

    #[test]
    fn test_spu_apply_relocation_rel32() {
        let mut ls = SpuLocalStore {
            memory: vec![0u8; 256],
            code_size: 0,
            data_size: 0,
        };

        let symbols = vec![SpuSymbol {
            name: "target".to_string(),
            value: 0x80,
            size: 4,
            section_index: 1,
            is_import: false,
            resolved_address: Some(0x80),
        }];

        let relocs = vec![SpuRelocation {
            offset: 0x20,
            sym_index: 0,
            rel_type: SpuRelocationType::Rel32,
            addend: 0,
        }];

        // base_address = 0, so P = 0 + 0x20 = 0x20
        // S + A - P = 0x80 + 0 - 0x20 = 0x60
        SpuRuntimeManager::apply_relocations(&mut ls, &relocs, &symbols, 0);

        let val = u32::from_be_bytes([ls.memory[0x20], ls.memory[0x21], ls.memory[0x22], ls.memory[0x23]]);
        assert_eq!(val, 0x60);
    }

    // ========================================================================
    // Symbol Resolution Tests
    // ========================================================================

    #[test]
    fn test_spu_resolve_symbols() {
        let mut symbols = vec![
            SpuSymbol {
                name: "my_func".to_string(),
                value: 0,
                size: 0,
                section_index: 0,
                is_import: true,
                resolved_address: None,
            },
            SpuSymbol {
                name: "my_data".to_string(),
                value: 0,
                size: 0,
                section_index: 0,
                is_import: true,
                resolved_address: None,
            },
            SpuSymbol {
                name: "local_sym".to_string(),
                value: 0x100,
                size: 4,
                section_index: 1,
                is_import: false,
                resolved_address: Some(0x100),
            },
        ];

        let mut exports = HashMap::new();
        exports.insert("my_func".to_string(), 0x2000u32);
        exports.insert("my_data".to_string(), 0x3000u32);

        let resolved = SpuRuntimeManager::resolve_symbols(&mut symbols, &exports);
        assert_eq!(resolved, 2);
        assert_eq!(symbols[0].resolved_address, Some(0x2000));
        assert_eq!(symbols[1].resolved_address, Some(0x3000));
        // Local symbol should be unchanged
        assert_eq!(symbols[2].resolved_address, Some(0x100));
    }

    #[test]
    fn test_spu_resolve_symbols_unresolved() {
        let mut symbols = vec![SpuSymbol {
            name: "missing_import".to_string(),
            value: 0,
            size: 0,
            section_index: 0,
            is_import: true,
            resolved_address: None,
        }];

        let exports = HashMap::new();
        let resolved = SpuRuntimeManager::resolve_symbols(&mut symbols, &exports);
        assert_eq!(resolved, 0);
        assert_eq!(symbols[0].resolved_address, None);
    }

    #[test]
    fn test_spu_relocation_type_values() {
        assert_eq!(SpuRelocationType::None as u32, 0);
        assert_eq!(SpuRelocationType::Addr32 as u32, 1);
        assert_eq!(SpuRelocationType::Addr16 as u32, 2);
        assert_eq!(SpuRelocationType::Rel32 as u32, 10);
    }
}
