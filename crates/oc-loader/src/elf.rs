//! ELF file parser

use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use oc_core::error::LoaderError;
use oc_memory::{MemoryManager, PageFlags};
use tracing::{debug, info, trace};

/// ELF file header (64-bit)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

/// ELF program header (64-bit)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

/// ELF section header (64-bit)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Elf64Shdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

/// ELF symbol table entry (64-bit)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

/// ELF relocation entry with addend (64-bit)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Elf64Rela {
    pub r_offset: u64,
    pub r_info: u64,
    pub r_addend: i64,
}

/// ELF dynamic entry (64-bit)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Elf64Dyn {
    pub d_tag: i64,
    pub d_val: u64,
}

/// ELF magic bytes
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// Program header types
pub mod pt {
    pub const NULL: u32 = 0;
    pub const LOAD: u32 = 1;
    pub const DYNAMIC: u32 = 2;
    pub const INTERP: u32 = 3;
    pub const NOTE: u32 = 4;
    pub const TLS: u32 = 7;
    pub const PROC1: u32 = 0x60000001;  // PS3-specific
    pub const PROC2: u32 = 0x60000002;  // PS3-specific
}

/// Section header types
pub mod sht {
    pub const NULL: u32 = 0;
    pub const PROGBITS: u32 = 1;
    pub const SYMTAB: u32 = 2;
    pub const STRTAB: u32 = 3;
    pub const RELA: u32 = 4;
    pub const HASH: u32 = 5;
    pub const DYNAMIC: u32 = 6;
    pub const NOBITS: u32 = 8;
    pub const REL: u32 = 9;
    pub const DYNSYM: u32 = 11;
}

/// Dynamic entry tags
pub mod dt {
    pub const NULL: i64 = 0;
    pub const NEEDED: i64 = 1;
    pub const PLTRELSZ: i64 = 2;
    pub const PLTGOT: i64 = 3;
    pub const HASH: i64 = 4;
    pub const STRTAB: i64 = 5;
    pub const SYMTAB: i64 = 6;
    pub const RELA: i64 = 7;
    pub const RELASZ: i64 = 8;
    pub const RELAENT: i64 = 9;
    pub const STRSZ: i64 = 10;
    pub const SYMENT: i64 = 11;
    pub const INIT: i64 = 12;
    pub const FINI: i64 = 13;
    pub const SONAME: i64 = 14;
    pub const RPATH: i64 = 15;
    pub const SYMBOLIC: i64 = 16;
    pub const REL: i64 = 17;
    pub const RELSZ: i64 = 18;
    pub const RELENT: i64 = 19;
    pub const PLTREL: i64 = 20;
    pub const DEBUG: i64 = 21;
    pub const JMPREL: i64 = 23;
}

/// Relocation types for PowerPC64
pub mod r_ppc64 {
    pub const NONE: u32 = 0;
    pub const ADDR32: u32 = 1;
    pub const ADDR64: u32 = 38;
    pub const RELATIVE: u32 = 22;
    pub const JMP_SLOT: u32 = 21;
    pub const GLOB_DAT: u32 = 20;
    pub const COPY: u32 = 19;
}

/// Symbol binding
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;
pub const STB_WEAK: u8 = 2;

/// Symbol type
pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;

/// ELF loader
pub struct ElfLoader {
    pub header: Elf64Header,
    pub phdrs: Vec<Elf64Phdr>,
    pub shdrs: Vec<Elf64Shdr>,
    pub symbols: Vec<Symbol>,
    pub entry_point: u64,
}

/// Parsed symbol information
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub value: u64,
    pub size: u64,
    pub bind: u8,
    pub sym_type: u8,
    pub section: u16,
}

impl Symbol {
    /// Get symbol binding from st_info
    pub fn binding(&self) -> u8 {
        self.bind
    }

    /// Get symbol type from st_info
    pub fn sym_type(&self) -> u8 {
        self.sym_type
    }

    /// Check if symbol is global
    pub fn is_global(&self) -> bool {
        self.bind == STB_GLOBAL
    }

    /// Check if symbol is weak
    pub fn is_weak(&self) -> bool {
        self.bind == STB_WEAK
    }

    /// Check if symbol is function
    pub fn is_function(&self) -> bool {
        self.sym_type == STT_FUNC
    }
}

impl ElfLoader {
    /// Create a new ELF loader by parsing the file
    pub fn new<R: Read + Seek>(reader: &mut R) -> Result<Self, LoaderError> {
        let header = Self::parse_header(reader)?;
        let phdrs = Self::parse_phdrs(reader, &header)?;
        let shdrs = Self::parse_shdrs(reader, &header)?;
        
        info!(
            "ELF loaded: entry=0x{:x}, phdrs={}, shdrs={}",
            header.e_entry, phdrs.len(), shdrs.len()
        );

        Ok(Self {
            header,
            phdrs,
            shdrs,
            symbols: Vec::new(),
            entry_point: header.e_entry,
        })
    }

    /// Load ELF segments into memory
    pub fn load_segments<R: Read + Seek>(
        &self,
        reader: &mut R,
        memory: &Arc<MemoryManager>,
        base_addr: u32,
    ) -> Result<(), LoaderError> {
        info!("Loading ELF segments at base address 0x{:08x}", base_addr);

        for (i, phdr) in self.phdrs.iter().enumerate() {
            if phdr.p_type == pt::LOAD {
                self.load_segment(reader, memory, phdr, base_addr, i)?;
            }
        }

        Ok(())
    }

    /// Load a single segment into memory
    fn load_segment<R: Read + Seek>(
        &self,
        reader: &mut R,
        memory: &Arc<MemoryManager>,
        phdr: &Elf64Phdr,
        base_addr: u32,
        index: usize,
    ) -> Result<(), LoaderError> {
        let vaddr = (base_addr as u64 + phdr.p_vaddr) as u32;
        let filesz = phdr.p_filesz as usize;
        let memsz = phdr.p_memsz as usize;

        debug!(
            "Loading segment {}: vaddr=0x{:08x}, filesz=0x{:x}, memsz=0x{:x}",
            index, vaddr, filesz, memsz
        );

        // Convert program header flags to page flags
        let mut flags = PageFlags::empty();
        if phdr.p_flags & 0x4 != 0 {
            flags |= PageFlags::READ;
        }
        if phdr.p_flags & 0x2 != 0 {
            flags |= PageFlags::WRITE;
        }
        if phdr.p_flags & 0x1 != 0 {
            flags |= PageFlags::EXECUTE;
        }

        // Calculate the total memory size to commit for this segment
        // memsz includes BSS (uninitialized data), filesz is just the file data
        // PS3 uses 32-bit addresses, so segment sizes should fit in u32
        let commit_size = u32::try_from(if memsz > 0 { memsz } else { filesz })
            .map_err(|_| LoaderError::InvalidElf(format!(
                "Segment {} size 0x{:x} exceeds 32-bit address space",
                index, if memsz > 0 { memsz } else { filesz }
            )))?;
        
        // Determine if we need temporary write permissions for loading
        // We need WRITE if we're going to write file data or zero-fill BSS
        let has_file_data = filesz > 0;
        let has_bss = memsz > filesz;  // BSS = uninitialized data that needs zeroing
        let needs_write = has_file_data || has_bss;
        let segment_has_write = flags.contains(PageFlags::WRITE);
        let needs_temp_write = needs_write && !segment_has_write;

        // Commit the memory region for this segment before writing
        // This ensures the pages are allocated and have the correct permissions
        // PS3 games can load segments at various addresses (e.g., 0x10000000)
        // that may not be pre-allocated during memory manager initialization
        if commit_size > 0 {
            // Add WRITE permission temporarily for loading if the segment doesn't already have it
            let load_flags = if needs_temp_write { flags | PageFlags::WRITE } else { flags };
            memory.set_page_flags(vaddr, commit_size, load_flags)
                .map_err(|e| LoaderError::InvalidElf(format!(
                    "Failed to commit memory region for segment {} at 0x{:08x} (size: 0x{:x}): {}",
                    index, vaddr, commit_size, e
                )))?;
            
            debug!(
                "Committed memory region: vaddr=0x{:08x}, size=0x{:x}, flags={:?}",
                vaddr, commit_size, load_flags
            );
        }

        // Read segment data from file
        if filesz > 0 {
            // Get file size for error context
            let file_size = reader.seek(SeekFrom::End(0)).unwrap_or(0);
            
            // Validate segment bounds
            let segment_end = phdr.p_offset + phdr.p_filesz;
            if segment_end > file_size {
                return Err(LoaderError::InvalidElf(format!(
                    "Segment {} extends beyond file: segment data at offset 0x{:x} with size 0x{:x} \
                     (ends at 0x{:x}) but file is only {} bytes (0x{:x}). \
                     This may indicate a corrupted or truncated file.",
                    index, phdr.p_offset, phdr.p_filesz, segment_end, file_size, file_size
                )));
            }
            
            reader
                .seek(SeekFrom::Start(phdr.p_offset))
                .map_err(|e| {
                    LoaderError::InvalidElf(format!(
                        "Failed to seek to segment {} data at offset 0x{:x}: {}",
                        index, phdr.p_offset, e
                    ))
                })?;

            let mut data = vec![0u8; filesz];
            reader
                .read_exact(&mut data)
                .map_err(|e| {
                    LoaderError::InvalidElf(format!(
                        "Failed to read segment {} data ({} bytes) at offset 0x{:x}: {} \
                         (file size: {} bytes). The file may be corrupted or truncated.",
                        index, filesz, phdr.p_offset, e, file_size
                    ))
                })?;

            debug!(
                "Read {} bytes for segment {} from offset 0x{:x}",
                filesz, index, phdr.p_offset
            );

            // Write to memory
            memory
                .write_bytes(vaddr, &data)
                .map_err(|e| LoaderError::InvalidElf(format!(
                    "Failed to write segment {} ({} bytes) to memory at 0x{:08x}: {}",
                    index, filesz, vaddr, e
                )))?;
        }

        // Zero out remaining memory (BSS section)
        if has_bss {
            let zero_size = memsz - filesz;
            let zeros = vec![0u8; zero_size];
            memory
                .write_bytes(vaddr + filesz as u32, &zeros)
                .map_err(|e| LoaderError::InvalidElf(format!("Failed to zero BSS: {}", e)))?;
        }

        // Apply final page permissions only if we added temporary WRITE
        if needs_temp_write {
            memory.set_page_flags(vaddr, commit_size, flags)
                .map_err(|e| LoaderError::InvalidElf(format!(
                    "Failed to set final page permissions for segment {} at 0x{:08x}: {}",
                    index, vaddr, e
                )))?;
        }

        Ok(())
    }

    /// Parse section headers
    pub fn parse_shdrs<R: Read + Seek>(
        reader: &mut R,
        header: &Elf64Header,
    ) -> Result<Vec<Elf64Shdr>, LoaderError> {
        if header.e_shoff == 0 || header.e_shnum == 0 {
            debug!("No section headers present (e_shoff=0 or e_shnum=0)");
            return Ok(Vec::new());
        }

        // Get file size for validation
        let file_size = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        
        debug!(
            "Parsing {} section headers at offset 0x{:x} (entry size: {} bytes)",
            header.e_shnum, header.e_shoff, header.e_shentsize
        );
        
        // Validate section header table bounds
        let shtab_end = header.e_shoff + (header.e_shnum as u64 * header.e_shentsize as u64);
        if shtab_end > file_size {
            // Section headers are optional for execution, so just warn and return empty
            debug!(
                "Section header table extends beyond file: table ends at 0x{:x} but file is {} bytes. \
                 Skipping section headers (this is usually fine for execution).",
                shtab_end, file_size
            );
            return Ok(Vec::new());
        }

        let mut shdrs = Vec::with_capacity(header.e_shnum as usize);

        for i in 0..header.e_shnum {
            let offset = header.e_shoff + (i as u64 * header.e_shentsize as u64);
            reader
                .seek(SeekFrom::Start(offset))
                .map_err(|e| {
                    LoaderError::InvalidElf(format!(
                        "Failed to seek to section header {} at offset 0x{:x}: {}",
                        i, offset, e
                    ))
                })?;

            let mut buf = [0u8; 64];
            reader
                .read_exact(&mut buf)
                .map_err(|e| {
                    LoaderError::InvalidElf(format!(
                        "Failed to read section header {} (64 bytes) at offset 0x{:x}: {} (file size: {} bytes)",
                        i, offset, e, file_size
                    ))
                })?;

            let shdr = Elf64Shdr {
                sh_name: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
                sh_type: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
                sh_flags: u64::from_be_bytes([
                    buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
                ]),
                sh_addr: u64::from_be_bytes([
                    buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
                ]),
                sh_offset: u64::from_be_bytes([
                    buf[24], buf[25], buf[26], buf[27], buf[28], buf[29], buf[30], buf[31],
                ]),
                sh_size: u64::from_be_bytes([
                    buf[32], buf[33], buf[34], buf[35], buf[36], buf[37], buf[38], buf[39],
                ]),
                sh_link: u32::from_be_bytes([buf[40], buf[41], buf[42], buf[43]]),
                sh_info: u32::from_be_bytes([buf[44], buf[45], buf[46], buf[47]]),
                sh_addralign: u64::from_be_bytes([
                    buf[48], buf[49], buf[50], buf[51], buf[52], buf[53], buf[54], buf[55],
                ]),
                sh_entsize: u64::from_be_bytes([
                    buf[56], buf[57], buf[58], buf[59], buf[60], buf[61], buf[62], buf[63],
                ]),
            };

            shdrs.push(shdr);
        }

        Ok(shdrs)
    }

    /// Parse symbol table
    pub fn parse_symbols<R: Read + Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), LoaderError> {
        // Find symbol table section
        let symtab_idx = self
            .shdrs
            .iter()
            .position(|sh| sh.sh_type == sht::SYMTAB || sh.sh_type == sht::DYNSYM);

        let symtab_idx = match symtab_idx {
            Some(idx) => idx,
            None => {
                debug!("No symbol table found");
                return Ok(());
            }
        };

        let symtab = &self.shdrs[symtab_idx];
        let strtab = &self.shdrs[symtab.sh_link as usize];

        // Read string table
        reader
            .seek(SeekFrom::Start(strtab.sh_offset))
            .map_err(|e| LoaderError::InvalidElf(e.to_string()))?;

        let mut strtab_data = vec![0u8; strtab.sh_size as usize];
        reader
            .read_exact(&mut strtab_data)
            .map_err(|e| LoaderError::InvalidElf(e.to_string()))?;

        // Read symbol table
        reader
            .seek(SeekFrom::Start(symtab.sh_offset))
            .map_err(|e| LoaderError::InvalidElf(e.to_string()))?;

        let num_symbols = (symtab.sh_size / symtab.sh_entsize) as usize;
        self.symbols.reserve(num_symbols);

        for _ in 0..num_symbols {
            let mut buf = [0u8; 24];
            reader
                .read_exact(&mut buf)
                .map_err(|e| LoaderError::InvalidElf(e.to_string()))?;

            let st_name = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            let st_info = buf[4];
            let _st_other = buf[5];
            let st_shndx = u16::from_be_bytes([buf[6], buf[7]]);
            let st_value = u64::from_be_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]);
            let st_size = u64::from_be_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]);

            // Extract symbol name from string table
            let name = if st_name > 0 && (st_name as usize) < strtab_data.len() {
                let name_start = st_name as usize;
                let name_end = strtab_data[name_start..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|pos| name_start + pos)
                    .unwrap_or(strtab_data.len());

                String::from_utf8_lossy(&strtab_data[name_start..name_end]).to_string()
            } else {
                String::new()
            };

            let symbol = Symbol {
                name,
                value: st_value,
                size: st_size,
                bind: st_info >> 4,
                sym_type: st_info & 0xf,
                section: st_shndx,
            };

            self.symbols.push(symbol);
        }

        info!("Loaded {} symbols", self.symbols.len());
        Ok(())
    }

    /// Resolve a symbol by name
    pub fn resolve_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|sym| sym.name == name)
    }

    /// Process relocations
    pub fn process_relocations<R: Read + Seek>(
        &self,
        reader: &mut R,
        memory: &Arc<MemoryManager>,
        base_addr: u32,
    ) -> Result<(), LoaderError> {
        info!("Processing relocations with base address 0x{:08x}", base_addr);

        for shdr in &self.shdrs {
            if shdr.sh_type == sht::RELA {
                self.process_rela_section(reader, memory, shdr, base_addr)?;
            }
        }

        Ok(())
    }

    /// Process a RELA relocation section
    fn process_rela_section<R: Read + Seek>(
        &self,
        reader: &mut R,
        memory: &Arc<MemoryManager>,
        shdr: &Elf64Shdr,
        base_addr: u32,
    ) -> Result<(), LoaderError> {
        reader
            .seek(SeekFrom::Start(shdr.sh_offset))
            .map_err(|e| LoaderError::InvalidElf(e.to_string()))?;

        let num_relocations = (shdr.sh_size / std::mem::size_of::<Elf64Rela>() as u64) as usize;

        for _ in 0..num_relocations {
            let mut buf = [0u8; 24];
            reader
                .read_exact(&mut buf)
                .map_err(|e| LoaderError::InvalidElf(e.to_string()))?;

            let r_offset = u64::from_be_bytes([
                buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
            ]);
            let r_info = u64::from_be_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]);
            let r_addend = i64::from_be_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]);

            let r_type = (r_info & 0xffffffff) as u32;
            let r_sym = (r_info >> 32) as usize;

            self.apply_relocation(memory, r_offset, r_type, r_sym, r_addend, base_addr)?;
        }

        Ok(())
    }

    /// Apply a single relocation
    fn apply_relocation(
        &self,
        memory: &Arc<MemoryManager>,
        offset: u64,
        rel_type: u32,
        sym_idx: usize,
        addend: i64,
        base_addr: u32,
    ) -> Result<(), LoaderError> {
        let addr = (base_addr as u64 + offset) as u32;

        let sym_value = if sym_idx < self.symbols.len() {
            self.symbols[sym_idx].value
        } else {
            0
        };

        trace!(
            "Applying relocation: type={}, addr=0x{:08x}, sym_value=0x{:x}, addend={}",
            rel_type,
            addr,
            sym_value,
            addend
        );

        match rel_type {
            r_ppc64::NONE => {
                // No relocation needed
            }
            r_ppc64::ADDR64 => {
                // S + A
                let value = (sym_value as i64 + addend) as u64;
                memory
                    .write(addr, value.to_be_bytes())
                    .map_err(|e| LoaderError::InvalidElf(format!("Relocation failed: {}", e)))?;
            }
            r_ppc64::ADDR32 => {
                // S + A (truncated to 32 bits)
                let value = ((sym_value as i64 + addend) as u32).to_be_bytes();
                memory
                    .write(addr, value)
                    .map_err(|e| LoaderError::InvalidElf(format!("Relocation failed: {}", e)))?;
            }
            r_ppc64::RELATIVE => {
                // B + A
                let value = (base_addr as i64 + addend) as u64;
                memory
                    .write(addr, value.to_be_bytes())
                    .map_err(|e| LoaderError::InvalidElf(format!("Relocation failed: {}", e)))?;
            }
            r_ppc64::GLOB_DAT | r_ppc64::JMP_SLOT => {
                // S
                memory
                    .write(addr, sym_value.to_be_bytes())
                    .map_err(|e| LoaderError::InvalidElf(format!("Relocation failed: {}", e)))?;
            }
            _ => {
                debug!("Unsupported relocation type: {}", rel_type);
            }
        }

        Ok(())
    }

    /// Parse ELF header from reader
    pub fn parse_header<R: Read + Seek>(reader: &mut R) -> Result<Elf64Header, LoaderError> {
        // Get file size for better error messages
        let file_size = reader.seek(SeekFrom::End(0)).map_err(|e| {
            LoaderError::InvalidElf(format!("Failed to determine file size: {}", e))
        })?;
        
        debug!("Parsing ELF header, file size: {} bytes (0x{:x})", file_size, file_size);
        
        if file_size < 64 {
            return Err(LoaderError::InvalidElf(format!(
                "File too small to be a valid ELF: {} bytes (minimum 64 bytes required for ELF64 header)",
                file_size
            )));
        }
        
        reader.seek(SeekFrom::Start(0)).map_err(|e| {
            LoaderError::InvalidElf(format!("Failed to seek to file start: {}", e))
        })?;

        let mut header = Elf64Header::default();
        
        // Read ident (16 bytes)
        reader.read_exact(&mut header.e_ident).map_err(|e| {
            LoaderError::InvalidElf(format!(
                "Failed to read ELF ident (16 bytes) at offset 0: {} (file size: {} bytes)",
                e, file_size
            ))
        })?;
        
        // Verify magic
        if header.e_ident[0..4] != ELF_MAGIC {
            let magic_bytes: String = header.e_ident[0..4]
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            return Err(LoaderError::InvalidElf(format!(
                "Invalid ELF magic bytes: {} (expected: 7F 45 4C 46 / \\x7FELF)",
                magic_bytes
            )));
        }
        
        // Check 64-bit (e_ident[4] == ELFCLASS64 == 2)
        if header.e_ident[4] != 2 {
            return Err(LoaderError::InvalidElf(format!(
                "Not a 64-bit ELF: class={} (expected: 2 for ELFCLASS64). PS3 requires 64-bit executables.",
                header.e_ident[4]
            )));
        }
        
        // Check big-endian (e_ident[5] == ELFDATA2MSB == 2, PS3 is big-endian)
        if header.e_ident[5] != 2 {
            return Err(LoaderError::InvalidElf(format!(
                "Not big-endian ELF: data encoding={} (expected: 2 for ELFDATA2MSB/big-endian). PS3 uses big-endian.",
                header.e_ident[5]
            )));
        }
        
        debug!("ELF ident valid: 64-bit big-endian, version={}", header.e_ident[6]);
        
        // Read rest of header (48 bytes at offset 16)
        let mut buf = [0u8; 48];
        reader.read_exact(&mut buf).map_err(|e| {
            LoaderError::InvalidElf(format!(
                "Failed to read ELF header fields (48 bytes) at offset 16: {} (file size: {} bytes)",
                e, file_size
            ))
        })?;
        
        header.e_type = u16::from_be_bytes([buf[0], buf[1]]);
        header.e_machine = u16::from_be_bytes([buf[2], buf[3]]);
        header.e_version = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        header.e_entry = u64::from_be_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);
        header.e_phoff = u64::from_be_bytes([buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23]]);
        header.e_shoff = u64::from_be_bytes([buf[24], buf[25], buf[26], buf[27], buf[28], buf[29], buf[30], buf[31]]);
        header.e_flags = u32::from_be_bytes([buf[32], buf[33], buf[34], buf[35]]);
        header.e_ehsize = u16::from_be_bytes([buf[36], buf[37]]);
        header.e_phentsize = u16::from_be_bytes([buf[38], buf[39]]);
        header.e_phnum = u16::from_be_bytes([buf[40], buf[41]]);
        header.e_shentsize = u16::from_be_bytes([buf[42], buf[43]]);
        header.e_shnum = u16::from_be_bytes([buf[44], buf[45]]);
        header.e_shstrndx = u16::from_be_bytes([buf[46], buf[47]]);
        
        Ok(header)
    }
    
    /// Parse program headers
    pub fn parse_phdrs<R: Read + Seek>(reader: &mut R, header: &Elf64Header) -> Result<Vec<Elf64Phdr>, LoaderError> {
        // Get file size for validation
        let file_size = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        
        debug!(
            "Parsing {} program headers at offset 0x{:x} (entry size: {} bytes)",
            header.e_phnum, header.e_phoff, header.e_phentsize
        );
        
        // Validate program header table bounds
        let phtab_end = header.e_phoff + (header.e_phnum as u64 * header.e_phentsize as u64);
        if phtab_end > file_size {
            return Err(LoaderError::InvalidElf(format!(
                "Program header table extends beyond file: table ends at offset 0x{:x} but file is only {} bytes. \
                 Header claims {} program headers at offset 0x{:x} with entry size {} bytes.",
                phtab_end, file_size, header.e_phnum, header.e_phoff, header.e_phentsize
            )));
        }
        
        let mut phdrs = Vec::with_capacity(header.e_phnum as usize);
        
        for i in 0..header.e_phnum {
            let offset = header.e_phoff + (i as u64 * header.e_phentsize as u64);
            reader.seek(SeekFrom::Start(offset)).map_err(|e| {
                LoaderError::InvalidElf(format!(
                    "Failed to seek to program header {} at offset 0x{:x}: {}",
                    i, offset, e
                ))
            })?;
            
            let mut buf = [0u8; 56];
            reader.read_exact(&mut buf).map_err(|e| {
                LoaderError::InvalidElf(format!(
                    "Failed to read program header {} (56 bytes) at offset 0x{:x}: {} (file size: {} bytes)",
                    i, offset, e, file_size
                ))
            })?;
            
            let phdr = Elf64Phdr {
                p_type: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
                p_flags: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
                p_offset: u64::from_be_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]),
                p_vaddr: u64::from_be_bytes([buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23]]),
                p_paddr: u64::from_be_bytes([buf[24], buf[25], buf[26], buf[27], buf[28], buf[29], buf[30], buf[31]]),
                p_filesz: u64::from_be_bytes([buf[32], buf[33], buf[34], buf[35], buf[36], buf[37], buf[38], buf[39]]),
                p_memsz: u64::from_be_bytes([buf[40], buf[41], buf[42], buf[43], buf[44], buf[45], buf[46], buf[47]]),
                p_align: u64::from_be_bytes([buf[48], buf[49], buf[50], buf[51], buf[52], buf[53], buf[54], buf[55]]),
            };
            
            phdrs.push(phdr);
        }
        
        Ok(phdrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_magic() {
        assert_eq!(ELF_MAGIC, [0x7F, b'E', b'L', b'F']);
    }

    #[test]
    fn test_symbol_binding() {
        let sym = Symbol {
            name: "test".to_string(),
            value: 0x1000,
            size: 100,
            bind: STB_GLOBAL,
            sym_type: STT_FUNC,
            section: 1,
        };

        assert!(sym.is_global());
        assert!(!sym.is_weak());
        assert!(sym.is_function());
    }

    #[test]
    fn test_program_header_types() {
        assert_eq!(pt::LOAD, 1);
        assert_eq!(pt::DYNAMIC, 2);
        assert_eq!(pt::TLS, 7);
    }

    #[test]
    fn test_section_header_types() {
        assert_eq!(sht::SYMTAB, 2);
        assert_eq!(sht::STRTAB, 3);
        assert_eq!(sht::RELA, 4);
    }
}
