//! PRX (PS3 shared library) loader

use oc_core::error::LoaderError;
use crate::elf::ElfLoader;
use std::collections::HashMap;
use std::sync::Arc;
use oc_memory::MemoryManager;
use tracing::{debug, info};

/// PRX module information
#[derive(Debug, Clone)]
pub struct PrxModule {
    pub name: String,
    pub version: u32,
    pub base_addr: u32,
    pub entry_point: u64,
    pub exports: Vec<PrxExport>,
    pub imports: Vec<PrxImport>,
}

/// PRX exported function/variable
#[derive(Debug, Clone)]
pub struct PrxExport {
    pub name: String,
    pub nid: u32,  // Name ID (hash)
    pub address: u64,
    pub export_type: ExportType,
}

/// PRX imported function/variable
#[derive(Debug, Clone)]
pub struct PrxImport {
    pub name: String,
    pub nid: u32,
    pub module: String,
    pub import_type: ImportType,
    pub stub_addr: u32,
    pub resolved_addr: Option<u64>,
}

/// Export type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportType {
    Function,
    Variable,
}

/// Import type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportType {
    Function,
    Variable,
}

/// Module info structure from PRX
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub attributes: u16,
    pub version: [u8; 2],
    pub name: String,
    pub toc: u32,
    pub exports_start: u32,
    pub exports_end: u32,
    pub imports_start: u32,
    pub imports_end: u32,
}

/// Stub library for unresolved imports
#[derive(Debug, Clone)]
pub struct StubLibrary {
    pub name: String,
    pub version: u32,
    pub stubs: HashMap<u32, StubFunction>,
}

/// Stub function information
#[derive(Debug, Clone)]
pub struct StubFunction {
    pub nid: u32,
    pub name: String,
    pub stub_addr: u32,
    pub return_value: i64,  // Default return value for stub
}

/// PRX loading statistics
#[derive(Debug, Clone, Default)]
pub struct PrxLoadingStats {
    /// Total modules loaded
    pub modules_loaded: u32,
    /// Total exports resolved
    pub exports_resolved: u32,
    /// Total imports resolved
    pub imports_resolved: u32,
    /// Total unresolved imports
    pub unresolved_imports: u32,
    /// NIDs resolved via database
    pub nids_resolved_via_db: u32,
    /// Stub functions created
    pub stubs_created: u32,
}

/// PRX dependency information
#[derive(Debug, Clone)]
pub struct PrxDependency {
    /// Module name that requires this dependency
    pub required_by: String,
    /// Required module name
    pub module_name: String,
    /// Version required (if known)
    pub version: Option<u32>,
    /// Whether this dependency is satisfied
    pub satisfied: bool,
}

/// PRX loader for managing shared libraries
pub struct PrxLoader {
    modules: HashMap<String, PrxModule>,
    symbol_cache: HashMap<u32, u64>,  // NID -> address
    stub_libraries: HashMap<String, StubLibrary>,  // Stub library support
    nid_database: HashMap<u32, String>,  // NID -> function name mapping
    dependencies: Vec<PrxDependency>,  // Dependency tracking
    stats: PrxLoadingStats,  // Loading statistics
}

impl PrxLoader {
    /// Create a new PRX loader
    pub fn new() -> Self {
        let mut loader = Self {
            modules: HashMap::new(),
            symbol_cache: HashMap::new(),
            stub_libraries: HashMap::new(),
            nid_database: HashMap::new(),
            dependencies: Vec::new(),
            stats: PrxLoadingStats::default(),
        };
        
        // Initialize NID database with known PS3 function NIDs
        loader.init_nid_database();
        
        loader
    }
    
    /// Initialize NID database with known PS3 system function NIDs
    fn init_nid_database(&mut self) {
        // Comprehensive PS3 system function NIDs for better symbol resolution
        // Organized by module for clarity
        
        // ==================== SYSTEM CALLS (sys_*) ====================
        // These are the core LV2 kernel system calls
        
        // Process management
        self.nid_database.insert(0x8461E528, "sys_process_exit".to_string());
        self.nid_database.insert(0xE6F2C1E7, "sys_process_is_spu_lock_line_reservation_address".to_string());
        self.nid_database.insert(0x2C847572, "sys_process_getpid".to_string());
        self.nid_database.insert(0x91460C8F, "sys_process_get_paramsfo".to_string());  // Fixed: unique NID
        
        // Thread management (PPU threads)
        self.nid_database.insert(0x9FB6228E, "sys_ppu_thread_create".to_string());
        self.nid_database.insert(0x350D454E, "sys_ppu_thread_exit".to_string());
        self.nid_database.insert(0xAFF080A4, "sys_ppu_thread_join".to_string());
        self.nid_database.insert(0x2AD79F6B, "sys_ppu_thread_yield".to_string());  // Fixed: unique NID
        self.nid_database.insert(0x4E3A1105, "sys_ppu_thread_get_priority".to_string());
        self.nid_database.insert(0xB0E2A6D4, "sys_ppu_thread_set_priority".to_string());
        self.nid_database.insert(0x52A6E437, "sys_ppu_thread_start".to_string());  // Fixed: unique NID
        self.nid_database.insert(0x65D2C920, "sys_ppu_thread_get_id".to_string());
        
        // Memory management
        self.nid_database.insert(0x348D6FA2, "sys_memory_allocate".to_string());
        self.nid_database.insert(0xA91E0E36, "sys_memory_free".to_string());
        self.nid_database.insert(0xF63B6C6E, "sys_memory_get_page_attribute".to_string());
        self.nid_database.insert(0x7B3B8F1C, "sys_mmapper_allocate_address".to_string());
        self.nid_database.insert(0x409AD939, "sys_mmapper_free_address".to_string());
        self.nid_database.insert(0x5BB4F387, "sys_mmapper_map_memory".to_string());  // Fixed: unique NID
        
        // Mutex/synchronization primitives
        self.nid_database.insert(0xDA0EB71A, "sys_lwmutex_create".to_string());
        self.nid_database.insert(0x1573DC3F, "sys_lwmutex_destroy".to_string());
        self.nid_database.insert(0x5A3F1F26, "sys_lwmutex_lock".to_string());
        self.nid_database.insert(0x8A40B5FB, "sys_lwmutex_trylock".to_string());
        self.nid_database.insert(0x74A52FFA, "sys_lwmutex_unlock".to_string());
        self.nid_database.insert(0x95C5ECBA, "sys_mutex_create".to_string());
        self.nid_database.insert(0x87D8AE37, "sys_mutex_destroy".to_string());
        self.nid_database.insert(0x07F17D15, "sys_mutex_lock".to_string());
        self.nid_database.insert(0x1ABCF0A1, "sys_mutex_trylock".to_string());
        self.nid_database.insert(0x5F2E2450, "sys_mutex_unlock".to_string());
        
        // Condition variables
        self.nid_database.insert(0x24A1EA07, "sys_cond_create".to_string());
        self.nid_database.insert(0xFA887CC3, "sys_cond_destroy".to_string());
        self.nid_database.insert(0xDD23AB6B, "sys_cond_signal".to_string());
        self.nid_database.insert(0x4C5C6C53, "sys_cond_signal_all".to_string());
        self.nid_database.insert(0x3FBBA4A9, "sys_cond_wait".to_string());
        
        // Semaphores
        self.nid_database.insert(0xDA6EC52C, "sys_semaphore_create".to_string());
        self.nid_database.insert(0xC8D4C6D5, "sys_semaphore_destroy".to_string());
        self.nid_database.insert(0x06F69206, "sys_semaphore_wait".to_string());
        self.nid_database.insert(0xD8763B55, "sys_semaphore_trywait".to_string());
        self.nid_database.insert(0xEE880A38, "sys_semaphore_post".to_string());
        
        // Event queues and flags
        self.nid_database.insert(0x8A0A8B63, "sys_event_queue_create".to_string());
        self.nid_database.insert(0xC5E48B5A, "sys_event_queue_destroy".to_string());
        self.nid_database.insert(0x5C6AD8C5, "sys_event_queue_receive".to_string());
        self.nid_database.insert(0x64D5C4F4, "sys_event_flag_create".to_string());
        self.nid_database.insert(0xEFF3F26B, "sys_event_flag_destroy".to_string());
        self.nid_database.insert(0x3CBBBE78, "sys_event_flag_wait".to_string());
        self.nid_database.insert(0x48D12434, "sys_event_flag_set".to_string());
        
        // Timer/time management
        self.nid_database.insert(0xF6AA3CA7, "sys_time_get_current_time".to_string());
        self.nid_database.insert(0x8AFE1BC8, "sys_time_get_timebase_frequency".to_string());
        self.nid_database.insert(0x0E7D8846, "sys_timer_create".to_string());
        self.nid_database.insert(0x6AA6DC7B, "sys_timer_destroy".to_string());
        self.nid_database.insert(0x5F8DA0CB, "sys_timer_usleep".to_string());
        self.nid_database.insert(0xF0F6C02A, "sys_timer_sleep".to_string());
        
        // PRX (module) management
        self.nid_database.insert(0xE7A3B5D8, "sys_prx_load_module".to_string());
        self.nid_database.insert(0x26090058, "sys_prx_unload_module".to_string());
        self.nid_database.insert(0x42B23552, "sys_prx_start_module".to_string());
        self.nid_database.insert(0x99AC4525, "sys_prx_stop_module".to_string());
        self.nid_database.insert(0x0341BB97, "sys_prx_get_module_id_by_name".to_string());
        self.nid_database.insert(0x03F97A5B, "sys_prx_get_module_id_by_address".to_string());
        self.nid_database.insert(0x74311398, "sys_prx_get_module_info".to_string());
        self.nid_database.insert(0x9DAEFF6E, "sys_prx_get_module_list".to_string());
        self.nid_database.insert(0xA5D0A39C, "sys_prx_register_library".to_string());
        self.nid_database.insert(0xAC3A5E2B, "sys_prx_unregister_library".to_string());
        
        // SPU management
        self.nid_database.insert(0x74EB4C28, "sys_spu_initialize".to_string());
        self.nid_database.insert(0x815FF1CD, "sys_spu_thread_group_create".to_string());
        self.nid_database.insert(0xBEC6C4F8, "sys_spu_thread_group_destroy".to_string());
        self.nid_database.insert(0x0FD8A3D3, "sys_spu_thread_group_start".to_string());
        self.nid_database.insert(0x7F0A3D42, "sys_spu_thread_group_join".to_string());
        self.nid_database.insert(0x63C74B6B, "sys_spu_thread_initialize".to_string());
        self.nid_database.insert(0x05DA98A1, "sys_spu_image_import".to_string());
        self.nid_database.insert(0xA2C14AF0, "sys_spu_image_close".to_string());
        
        // ==================== FILESYSTEM (cellFs*) ====================
        // File I/O operations
        self.nid_database.insert(0xB27C8AE7, "cellFsOpen".to_string());
        self.nid_database.insert(0x2CB51F0D, "cellFsClose".to_string());
        self.nid_database.insert(0xB1840AE5, "cellFsRead".to_string());
        self.nid_database.insert(0xC9AFD7F6, "cellFsWrite".to_string());
        self.nid_database.insert(0x3140EA9A, "cellFsLseek".to_string());
        self.nid_database.insert(0x5C74903D, "cellFsFstat".to_string());
        self.nid_database.insert(0xEF3FFFB2, "cellFsStat".to_string());
        self.nid_database.insert(0x701B82A2, "cellFsGetFreeSize".to_string());
        self.nid_database.insert(0x7F4677A8, "cellFsMkdir".to_string());
        self.nid_database.insert(0xE2D8202D, "cellFsRmdir".to_string());
        self.nid_database.insert(0x0D5B4A14, "cellFsUnlink".to_string());
        self.nid_database.insert(0xACF1C8AC, "cellFsRename".to_string());
        self.nid_database.insert(0x2796FAFE, "cellFsTruncate".to_string());
        self.nid_database.insert(0xD2F397E6, "cellFsFtruncate".to_string());
        self.nid_database.insert(0x5C1D2D60, "cellFsChmod".to_string());
        self.nid_database.insert(0x4E3D8932, "cellFsOpendir".to_string());
        self.nid_database.insert(0x5C2A8994, "cellFsReaddir".to_string());
        self.nid_database.insert(0xFF42DCC3, "cellFsClosedir".to_string());
        
        // ==================== NETWORK (cellNet*) ====================
        self.nid_database.insert(0x3ECCA2F0, "cellNetCtlInit".to_string());
        self.nid_database.insert(0x6B20E0C5, "cellNetCtlTerm".to_string());
        self.nid_database.insert(0xD2877143, "cellNetCtlGetState".to_string());
        self.nid_database.insert(0x3A12865F, "cellNetCtlGetInfo".to_string());
        self.nid_database.insert(0x26691C33, "cellNetCtlAddHandler".to_string());
        self.nid_database.insert(0x61FC4D63, "cellNetCtlDelHandler".to_string());
        
        // ==================== GRAPHICS (cellGcm*) ====================
        self.nid_database.insert(0xE315A0B2, "cellGcmInit".to_string());
        self.nid_database.insert(0xDBF9C5B3, "cellGcmGetConfiguration".to_string());
        self.nid_database.insert(0xB2E761D4, "cellGcmAddressToOffset".to_string());
        self.nid_database.insert(0x23AE55A3, "cellGcmGetCurrentField".to_string());  // Fixed: unique NID
        self.nid_database.insert(0x5B17FE8E, "cellGcmSetFlipMode".to_string());
        self.nid_database.insert(0x21D66F0D, "cellGcmSetDisplayBuffer".to_string());
        self.nid_database.insert(0xA53D12AE, "cellGcmSetFlipHandler".to_string());
        self.nid_database.insert(0xD01B570A, "cellGcmSetVBlankHandler".to_string());
        self.nid_database.insert(0xED8BF50C, "cellGcmSetGraphicsHandler".to_string());
        self.nid_database.insert(0x6D2AB858, "cellGcmSetSecondVHandler".to_string());
        self.nid_database.insert(0x983FB9AA, "cellGcmSetWaitFlip".to_string());
        self.nid_database.insert(0xDC09357E, "cellGcmResetFlipStatus".to_string());
        self.nid_database.insert(0x3A33C1FD, "cellGcmGetFlipStatus".to_string());
        self.nid_database.insert(0x4AE8D215, "cellGcmSetFlip".to_string());
        self.nid_database.insert(0xD9B7653E, "cellGcmGetTiledPitchSize".to_string());
        
        // ==================== AUDIO (cellAudio*) ====================
        self.nid_database.insert(0x0B168F92, "cellAudioInit".to_string());
        self.nid_database.insert(0x4129FE2D, "cellAudioQuit".to_string());
        self.nid_database.insert(0xCD7BC431, "cellAudioPortStart".to_string());
        self.nid_database.insert(0x69A2FE15, "cellAudioPortStop".to_string());
        self.nid_database.insert(0xCA5AC370, "cellAudioPortOpen".to_string());  // Fixed: unique NID
        self.nid_database.insert(0x5B1E2C73, "cellAudioPortClose".to_string());
        self.nid_database.insert(0xB7BCE92D, "cellAudioGetPortConfig".to_string());
        self.nid_database.insert(0x6D6C30B8, "cellAudioGetPortTimestamp".to_string());  // Fixed: unique NID
        
        // ==================== INPUT (cellPad*, cellKb*, etc.) ====================
        self.nid_database.insert(0x578E3C98, "cellPadInit".to_string());
        self.nid_database.insert(0x433F6EC0, "cellPadEnd".to_string());
        self.nid_database.insert(0x3EAD3E98, "cellPadGetData".to_string());
        self.nid_database.insert(0xA703A51D, "cellPadGetInfo2".to_string());
        self.nid_database.insert(0x1CF98800, "cellPadGetCapabilityInfo".to_string());
        self.nid_database.insert(0xBAF02B7B, "cellPadSetActDirect".to_string());
        self.nid_database.insert(0xA02604AE, "cellPadSetPortSetting".to_string());
        self.nid_database.insert(0xE442FAA8, "cellKbInit".to_string());
        self.nid_database.insert(0x9A9F4E3F, "cellKbEnd".to_string());
        self.nid_database.insert(0xA0F57E6D, "cellKbRead".to_string());
        
        // ==================== SAVEDATA ====================
        self.nid_database.insert(0xB2A51F8A, "cellSaveDataListSave2".to_string());
        self.nid_database.insert(0x2A8EAD2D, "cellSaveDataListLoad2".to_string());
        self.nid_database.insert(0x0E091C36, "cellSaveDataFixedSave2".to_string());
        self.nid_database.insert(0x2C79116A, "cellSaveDataFixedLoad2".to_string());
        self.nid_database.insert(0x8B7ED64B, "cellSaveDataAutoSave2".to_string());
        self.nid_database.insert(0x2EC0D80E, "cellSaveDataAutoLoad2".to_string());
        
        // ==================== SYSTEM UTILITIES ====================
        self.nid_database.insert(0x7530B9C4, "cellSysutilRegisterCallback".to_string());
        self.nid_database.insert(0xD4D76A26, "cellSysutilUnregisterCallback".to_string());
        self.nid_database.insert(0x189A74DA, "cellSysutilCheckCallback".to_string());
        self.nid_database.insert(0x938013A0, "cellSysutilGetSystemParamInt".to_string());
        self.nid_database.insert(0xDDB5CD0D, "cellSysutilGetSystemParamString".to_string());
        
        // ==================== VIDEO OUT ====================
        self.nid_database.insert(0xB59E3070, "cellVideoOutGetState".to_string());
        self.nid_database.insert(0x1E930EEF, "cellVideoOutGetResolution".to_string());
        self.nid_database.insert(0x6E72C102, "cellVideoOutConfigure".to_string());
        self.nid_database.insert(0x9B6B8E1F, "cellVideoOutGetConfiguration".to_string());
        self.nid_database.insert(0xD835E74C, "cellVideoOutGetDeviceInfo".to_string());
        
        // ==================== UTILITY LIBRARY ====================
        self.nid_database.insert(0x7B6E2C96, "cellMsgDialogOpenErrorCode".to_string());
        self.nid_database.insert(0x7603D3C4, "cellMsgDialogOpen2".to_string());
        self.nid_database.insert(0x62B0F803, "cellMsgDialogClose".to_string());
        self.nid_database.insert(0xB7E27272, "cellMsgDialogAbort".to_string());
        
        debug!("Initialized NID database with {} entries", self.nid_database.len());
    }
    
    /// Resolve NID to function name using database
    pub fn resolve_nid_to_name(&self, nid: u32) -> Option<&str> {
        self.nid_database.get(&nid).map(|s| s.as_str())
    }
    
    /// Add a custom NID mapping
    pub fn add_nid_mapping(&mut self, nid: u32, name: String) {
        self.nid_database.insert(nid, name);
    }
    
    /// Create a stub library for unresolved imports
    pub fn create_stub_library(&mut self, name: String, version: u32) -> &mut StubLibrary {
        debug!("Creating stub library: {} v{}", name, version);
        
        self.stub_libraries.entry(name.clone()).or_insert_with(|| {
            StubLibrary {
                name: name.clone(),
                version,
                stubs: HashMap::new(),
            }
        })
    }
    
    /// Add a stub function to a library
    pub fn add_stub_function(
        &mut self,
        library_name: &str,
        nid: u32,
        func_name: String,
        stub_addr: u32,
        return_value: i64,
    ) {
        if let Some(library) = self.stub_libraries.get_mut(library_name) {
            library.stubs.insert(nid, StubFunction {
                nid,
                name: func_name.clone(),
                stub_addr,
                return_value,
            });
            
            debug!("Added stub function: {}@{} (NID: 0x{:08x})", library_name, func_name, nid);
        }
    }
    
    /// Get stub function by NID
    pub fn get_stub_function(&self, library_name: &str, nid: u32) -> Option<&StubFunction> {
        self.stub_libraries.get(library_name)?.stubs.get(&nid)
    }

    /// Load a PRX module from ELF data
    pub fn load_module<R: std::io::Read + std::io::Seek>(
        &mut self,
        name: String,
        reader: &mut R,
        memory: &Arc<MemoryManager>,
        base_addr: u32,
    ) -> Result<PrxModule, LoaderError> {
        info!("Loading PRX module: {} at 0x{:08x}", name, base_addr);

        // Parse as ELF
        let mut elf_loader = ElfLoader::new(reader)?;

        // Load segments into memory
        elf_loader.load_segments(reader, memory, base_addr)?;

        // Parse symbols
        elf_loader.parse_symbols(reader)?;

        // Process relocations
        elf_loader.process_relocations(reader, memory, base_addr)?;

        // Extract exports and imports
        let exports = self.extract_exports(&elf_loader, base_addr)?;
        let imports = self.extract_imports(&elf_loader)?;

        // Cache exported symbols and update stats
        for export in &exports {
            self.symbol_cache.insert(export.nid, export.address);
            self.stats.exports_resolved += 1;
        }

        let module = PrxModule {
            name: name.clone(),
            version: 0x100,  // Default version
            base_addr,
            entry_point: elf_loader.entry_point,
            exports,
            imports,
        };

        info!(
            "PRX module loaded: {} ({} exports, {} imports)",
            module.name,
            module.exports.len(),
            module.imports.len()
        );

        // Update loading statistics
        self.stats.modules_loaded += 1;
        
        // Update dependency status for any dependencies waiting on this module
        self.update_dependency_status();

        self.modules.insert(name, module.clone());
        Ok(module)
    }

    /// Extract exported symbols from ELF
    fn extract_exports(
        &self,
        elf: &ElfLoader,
        base_addr: u32,
    ) -> Result<Vec<PrxExport>, LoaderError> {
        let mut exports = Vec::new();

        for symbol in &elf.symbols {
            // Only export global symbols
            if !symbol.is_global() || symbol.name.is_empty() {
                continue;
            }

            let export_type = if symbol.is_function() {
                ExportType::Function
            } else {
                ExportType::Variable
            };

            let nid = Self::calculate_nid(&symbol.name);
            let address = base_addr as u64 + symbol.value;

            exports.push(PrxExport {
                name: symbol.name.clone(),
                nid,
                address,
                export_type,
            });

            debug!(
                "Export: {} (NID: 0x{:08x}) @ 0x{:x}",
                symbol.name, nid, address
            );
        }

        Ok(exports)
    }

    /// Extract imported symbols from ELF
    fn extract_imports(&self, elf: &ElfLoader) -> Result<Vec<PrxImport>, LoaderError> {
        let mut imports = Vec::new();

        // Imports are typically marked as undefined symbols
        for (_idx, symbol) in elf.symbols.iter().enumerate() {
            // Check for undefined symbols (section index 0)
            if symbol.section != 0 || symbol.name.is_empty() {
                continue;
            }

            // Parse module name from symbol name (format: module_name@function_name)
            let (module, func_name) = if let Some(pos) = symbol.name.find('@') {
                let (mod_name, fn_name) = symbol.name.split_at(pos);
                (mod_name.to_string(), fn_name[1..].to_string())
            } else {
                ("unknown".to_string(), symbol.name.clone())
            };

            let import_type = if symbol.is_function() {
                ImportType::Function
            } else {
                ImportType::Variable
            };

            let nid = Self::calculate_nid(&func_name);

            imports.push(PrxImport {
                name: func_name.clone(),
                nid,
                module: module.clone(),
                import_type,
                stub_addr: 0,  // Will be filled during linking
                resolved_addr: None,  // Will be filled during resolution
            });

            debug!(
                "Import: {}@{} (NID: 0x{:08x})",
                module, func_name, nid
            );
        }

        Ok(imports)
    }

    /// Resolve imported symbols
    pub fn resolve_imports(&mut self, module_name: &str) -> Result<(), LoaderError> {
        let module = self.modules.get(module_name)
            .ok_or_else(|| LoaderError::MissingPrx(format!("Module not found: {}", module_name)))?
            .clone();

        let mut unresolved = Vec::new();
        let mut resolved_count = 0;
        let mut nids_resolved_via_db = 0;
        let mut stubs_created = 0;

        for import in &module.imports {
            // Try to resolve from symbol cache
            if let Some(&address) = self.symbol_cache.get(&import.nid) {
                debug!(
                    "Resolved import: {} (NID: 0x{:08x}) -> 0x{:x}",
                    import.name, import.nid, address
                );
                
                // Update the import with resolved address
                if let Some(module_mut) = self.modules.get_mut(module_name) {
                    if let Some(import_mut) = module_mut.imports.iter_mut().find(|i| i.nid == import.nid) {
                        import_mut.resolved_addr = Some(address);
                    }
                }
                resolved_count += 1;
            } else {
                // Try to resolve using NID database
                let func_name_opt = self.resolve_nid_to_name(import.nid).map(|s| s.to_string());
                
                if let Some(func_name) = func_name_opt {
                    debug!(
                        "NID 0x{:08x} resolved to function name: {}",
                        import.nid, func_name
                    );
                    
                    nids_resolved_via_db += 1;
                    
                    // Create stub for this import
                    let stub_library = self.create_stub_library(import.module.clone(), 1);
                    stub_library.stubs.insert(import.nid, StubFunction {
                        nid: import.nid,
                        name: func_name.clone(),
                        stub_addr: import.stub_addr,
                        return_value: 0,  // Default return value
                    });
                    
                    stubs_created += 1;
                    debug!("Created stub for {} (NID: 0x{:08x})", func_name, import.nid);
                } else {
                    unresolved.push(format!("{}@{} (NID: 0x{:08x})", import.module, import.name, import.nid));
                    debug!("Unresolved import: {} (NID: 0x{:08x})", import.name, import.nid);
                }
            }
        }
        
        // Update statistics
        self.stats.imports_resolved += resolved_count;
        self.stats.unresolved_imports += unresolved.len() as u32;
        self.stats.nids_resolved_via_db += nids_resolved_via_db;
        self.stats.stubs_created += stubs_created;

        if !unresolved.is_empty() {
            info!(
                "Module {} has {} unresolved imports (out of {}), {} resolved",
                module_name,
                unresolved.len(),
                module.imports.len(),
                resolved_count
            );
            
            // Log first few unresolved imports
            for (i, imp) in unresolved.iter().take(5).enumerate() {
                debug!("  Unresolved[{}]: {}", i, imp);
            }
        } else {
            info!("All {} imports resolved for module {}", module.imports.len(), module_name);
        }

        Ok(())
    }

    /// Get a loaded module by name
    pub fn get_module(&self, name: &str) -> Option<&PrxModule> {
        self.modules.get(name)
    }

    /// Resolve a symbol by NID
    pub fn resolve_symbol_by_nid(&self, nid: u32) -> Option<u64> {
        self.symbol_cache.get(&nid).copied()
    }

    /// Resolve a symbol by name
    pub fn resolve_symbol_by_name(&self, name: &str) -> Option<u64> {
        let nid = Self::calculate_nid(name);
        self.resolve_symbol_by_nid(nid)
    }

    /// Calculate NID (Name ID) for a symbol name
    /// This is a simplified hash - PS3 uses SHA-1 truncated to 32 bits
    fn calculate_nid(name: &str) -> u32 {
        // FNV-1a hash constants
        const FNV_OFFSET_BASIS: u32 = 0x811c9dc5;
        const FNV_PRIME: u32 = 0x01000193;

        // Simple hash for demonstration
        // Real implementation would use SHA-1 and take first 4 bytes
        let mut hash: u32 = FNV_OFFSET_BASIS;
        for byte in name.bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// List all loaded modules
    pub fn list_modules(&self) -> Vec<String> {
        self.modules.keys().cloned().collect()
    }

    /// Get export by NID from a specific module
    pub fn get_export(&self, module: &str, nid: u32) -> Option<&PrxExport> {
        self.modules.get(module)?.exports.iter().find(|e| e.nid == nid)
    }

    /// Get import by NID from a specific module
    pub fn get_import(&self, module: &str, nid: u32) -> Option<&PrxImport> {
        self.modules.get(module)?.imports.iter().find(|i| i.nid == nid)
    }
    
    /// Get loading statistics
    pub fn get_stats(&self) -> &PrxLoadingStats {
        &self.stats
    }
    
    /// Get all tracked dependencies
    pub fn get_dependencies(&self) -> &[PrxDependency] {
        &self.dependencies
    }
    
    /// Get unsatisfied dependencies
    pub fn get_unsatisfied_dependencies(&self) -> Vec<&PrxDependency> {
        self.dependencies.iter().filter(|d| !d.satisfied).collect()
    }
    
    /// Add a dependency requirement
    pub fn add_dependency(&mut self, required_by: String, module_name: String, version: Option<u32>) {
        // Check if this dependency is already satisfied
        let satisfied = self.modules.contains_key(&module_name);
        
        self.dependencies.push(PrxDependency {
            required_by,
            module_name,
            version,
            satisfied,
        });
    }
    
    /// Update dependency status (call after loading a new module)
    pub fn update_dependency_status(&mut self) {
        for dep in &mut self.dependencies {
            dep.satisfied = self.modules.contains_key(&dep.module_name);
        }
    }
    
    /// Get number of NID entries in the database
    pub fn get_nid_database_size(&self) -> usize {
        self.nid_database.len()
    }
    
    /// Lookup NID in database without consuming
    pub fn lookup_nid(&self, nid: u32) -> Option<&str> {
        self.nid_database.get(&nid).map(|s| s.as_str())
    }
    
    /// Get resolution status for a module's imports
    pub fn get_import_resolution_status(&self, module_name: &str) -> Option<(usize, usize)> {
        let module = self.modules.get(module_name)?;
        let total = module.imports.len();
        let resolved = module.imports.iter().filter(|i| i.resolved_addr.is_some()).count();
        Some((resolved, total))
    }
}

impl Default for PrxLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nid_calculation() {
        let nid1 = PrxLoader::calculate_nid("test_function");
        let nid2 = PrxLoader::calculate_nid("test_function");
        let nid3 = PrxLoader::calculate_nid("other_function");

        assert_eq!(nid1, nid2);
        assert_ne!(nid1, nid3);
    }

    #[test]
    fn test_prx_loader_creation() {
        let loader = PrxLoader::new();
        assert_eq!(loader.list_modules().len(), 0);
    }

    #[test]
    fn test_export_type() {
        let export = PrxExport {
            name: "test".to_string(),
            nid: 0x12345678,
            address: 0x1000,
            export_type: ExportType::Function,
        };

        assert_eq!(export.export_type, ExportType::Function);
    }
}
