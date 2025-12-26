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

/// PRX loader for managing shared libraries
pub struct PrxLoader {
    modules: HashMap<String, PrxModule>,
    symbol_cache: HashMap<u32, u64>,  // NID -> address
    stub_libraries: HashMap<String, StubLibrary>,  // Stub library support
    nid_database: HashMap<u32, String>,  // NID -> function name mapping
}

impl PrxLoader {
    /// Create a new PRX loader
    pub fn new() -> Self {
        let mut loader = Self {
            modules: HashMap::new(),
            symbol_cache: HashMap::new(),
            stub_libraries: HashMap::new(),
            nid_database: HashMap::new(),
        };
        
        // Initialize NID database with known PS3 function NIDs
        loader.init_nid_database();
        
        loader
    }
    
    /// Initialize NID database with known PS3 system function NIDs
    fn init_nid_database(&mut self) {
        // Common PS3 system function NIDs (sample - real database would be much larger)
        let known_nids = [
            (0x9FB6228E, "sys_ppu_thread_create"),
            (0x350D454E, "sys_ppu_thread_exit"),
            (0x8461E528, "sys_process_exit"),
            (0xDA0EB71A, "sys_lwmutex_create"),
            (0x1573DC3F, "sys_lwmutex_destroy"),
            (0xE7A3B5D8, "sys_prx_load_module"),
            (0x26090058, "sys_prx_unload_module"),
            (0xB27C8AE7, "cellFsOpen"),
            (0x2CB51F0D, "cellFsClose"),
            (0xB1840AE5, "cellFsRead"),
            (0xC9AFD7F6, "cellFsWrite"),
        ];
        
        for (nid, name) in &known_nids {
            self.nid_database.insert(*nid, name.to_string());
        }
        
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

        // Cache exported symbols
        for export in &exports {
            self.symbol_cache.insert(export.nid, export.address);
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
                    
                    // Create stub for this import
                    let stub_library = self.create_stub_library(import.module.clone(), 1);
                    stub_library.stubs.insert(import.nid, StubFunction {
                        nid: import.nid,
                        name: func_name.clone(),
                        stub_addr: import.stub_addr,
                        return_value: 0,  // Default return value
                    });
                    
                    debug!("Created stub for {} (NID: 0x{:08x})", func_name, import.nid);
                } else {
                    unresolved.push(format!("{}@{} (NID: 0x{:08x})", import.module, import.name, import.nid));
                    debug!("Unresolved import: {} (NID: 0x{:08x})", import.name, import.nid);
                }
            }
        }

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
