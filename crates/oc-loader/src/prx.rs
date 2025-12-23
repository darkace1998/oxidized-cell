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

/// PRX loader for managing shared libraries
pub struct PrxLoader {
    modules: HashMap<String, PrxModule>,
    symbol_cache: HashMap<u32, u64>,  // NID -> address
}

impl PrxLoader {
    /// Create a new PRX loader
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            symbol_cache: HashMap::new(),
        }
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

        for import in &module.imports {
            if let Some(&address) = self.symbol_cache.get(&import.nid) {
                debug!(
                    "Resolved import: {} (NID: 0x{:08x}) -> 0x{:x}",
                    import.name, import.nid, address
                );
            } else {
                unresolved.push(import.name.clone());
                debug!("Unresolved import: {} (NID: 0x{:08x})", import.name, import.nid);
            }
        }

        if !unresolved.is_empty() {
            info!(
                "Module {} has {} unresolved imports",
                module_name,
                unresolved.len()
            );
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
        // Simple hash for demonstration
        // Real implementation would use SHA-1 and take first 4 bytes
        let mut hash: u32 = 0x811c9dc5; // FNV-1a offset basis
        for byte in name.bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(0x01000193); // FNV-1a prime
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
