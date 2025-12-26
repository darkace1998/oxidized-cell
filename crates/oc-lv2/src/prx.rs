//! PRX loading (sys_prx_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::sync::Arc;

/// PRX module states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrxModuleState {
    Loaded,
    Started,
    Stopped,
}

/// PRX module information
pub struct PrxModule {
    id: ObjectId,
    inner: Mutex<PrxModuleInner>,
}

#[derive(Debug)]
struct PrxModuleInner {
    name: String,
    path: String,
    state: PrxModuleState,
    entry_point: u64,
    _size: usize,
    info: PrxInfo,
    exports: Vec<PrxSymbol>,
    imports: Vec<PrxSymbol>,
}

/// PRX symbol information
#[derive(Debug, Clone)]
pub struct PrxSymbol {
    pub name: String,
    pub address: u64,
    pub size: usize,
    pub symbol_type: PrxSymbolType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrxSymbolType {
    Function,
    Data,
}

/// PRX module information structure
#[derive(Debug, Clone)]
pub struct PrxInfo {
    pub version: u32,
    pub sdk_version: u32,
    pub attribute: u32,
}

impl Default for PrxInfo {
    fn default() -> Self {
        Self {
            version: 1,
            sdk_version: 0x00360001,
            attribute: 0,
        }
    }
}

impl PrxModule {
    pub fn new(id: ObjectId, name: String, path: String, entry_point: u64, size: usize) -> Self {
        Self {
            id,
            inner: Mutex::new(PrxModuleInner {
                name,
                path,
                state: PrxModuleState::Loaded,
                entry_point,
                _size: size,
                info: PrxInfo::default(),
                exports: Vec::new(),
                imports: Vec::new(),
            }),
        }
    }

    pub fn start(&self) -> Result<(), KernelError> {
        let mut inner = self.inner.lock();
        if inner.state != PrxModuleState::Loaded && inner.state != PrxModuleState::Stopped {
            return Err(KernelError::PermissionDenied);
        }
        inner.state = PrxModuleState::Started;
        tracing::info!("Started PRX module {} (id: {})", inner.name, self.id);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), KernelError> {
        let mut inner = self.inner.lock();
        if inner.state != PrxModuleState::Started {
            return Err(KernelError::PermissionDenied);
        }
        inner.state = PrxModuleState::Stopped;
        tracing::info!("Stopped PRX module {} (id: {})", inner.name, self.id);
        Ok(())
    }

    pub fn state(&self) -> PrxModuleState {
        self.inner.lock().state
    }

    pub fn name(&self) -> String {
        self.inner.lock().name.clone()
    }

    pub fn path(&self) -> String {
        self.inner.lock().path.clone()
    }

    pub fn info(&self) -> PrxInfo {
        self.inner.lock().info.clone()
    }

    /// Add an exported symbol to the module
    pub fn add_export(&self, symbol: PrxSymbol) {
        self.inner.lock().exports.push(symbol);
    }

    /// Add an imported symbol to the module
    pub fn add_import(&self, symbol: PrxSymbol) {
        self.inner.lock().imports.push(symbol);
    }

    /// Get exported symbols
    pub fn exports(&self) -> Vec<PrxSymbol> {
        self.inner.lock().exports.clone()
    }

    /// Get imported symbols
    pub fn imports(&self) -> Vec<PrxSymbol> {
        self.inner.lock().imports.clone()
    }

    /// Resolve a symbol by name
    pub fn resolve_symbol(&self, name: &str) -> Option<PrxSymbol> {
        self.inner.lock()
            .exports
            .iter()
            .find(|s| s.name == name)
            .cloned()
    }

    /// Get entry point address
    pub fn entry_point(&self) -> u64 {
        self.inner.lock().entry_point
    }
}

impl KernelObject for PrxModule {
    fn object_type(&self) -> ObjectType {
        ObjectType::PrxModule
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// PRX syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_prx_load_module
    pub fn sys_prx_load_module(
        manager: &ObjectManager,
        path: &str,
        _flags: u64,
        _options: u64,
    ) -> Result<ObjectId, KernelError> {
        // In real implementation, would load the PRX file from path
        // For now, create a placeholder module
        
        let id = manager.next_id();
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let module = Arc::new(PrxModule::new(
            id,
            name.clone(),
            path.to_string(),
            0x1000, // Placeholder entry point
            0x10000, // Placeholder size
        ));

        manager.register(module);
        tracing::info!("Loaded PRX module '{}' with id {}", name, id);
        Ok(id)
    }

    /// sys_prx_start_module
    pub fn sys_prx_start_module(
        manager: &ObjectManager,
        module_id: ObjectId,
        _args: u64,
        _argp: u64,
    ) -> Result<(), KernelError> {
        let module: Arc<PrxModule> = manager.get(module_id)?;
        module.start()
    }

    /// sys_prx_stop_module
    pub fn sys_prx_stop_module(
        manager: &ObjectManager,
        module_id: ObjectId,
        _args: u64,
        _argp: u64,
    ) -> Result<(), KernelError> {
        let module: Arc<PrxModule> = manager.get(module_id)?;
        module.stop()
    }

    /// sys_prx_unload_module
    pub fn sys_prx_unload_module(
        manager: &ObjectManager,
        module_id: ObjectId,
        _flags: u64,
    ) -> Result<(), KernelError> {
        let module: Arc<PrxModule> = manager.get(module_id)?;
        
        // Ensure module is stopped before unloading
        if module.state() == PrxModuleState::Started {
            return Err(KernelError::PermissionDenied);
        }

        tracing::info!("Unloading PRX module '{}'", module.name());
        manager.unregister(module_id)
    }

    /// sys_prx_get_module_list
    pub fn sys_prx_get_module_list(
        manager: &ObjectManager,
        _flags: u64,
        max_count: usize,
    ) -> Result<Vec<ObjectId>, KernelError> {
        let objects = manager.list();
        
        // Filter only PRX modules
        let modules: Vec<ObjectId> = objects
            .iter()
            .filter(|obj| obj.object_type() == ObjectType::PrxModule)
            .take(max_count)
            .map(|obj| obj.id())
            .collect();

        Ok(modules)
    }

    /// sys_prx_get_module_info
    pub fn sys_prx_get_module_info(
        manager: &ObjectManager,
        module_id: ObjectId,
    ) -> Result<PrxInfo, KernelError> {
        let module: Arc<PrxModule> = manager.get(module_id)?;
        Ok(module.info())
    }

    /// sys_prx_register_module - Register module with exports
    pub fn sys_prx_register_module(
        manager: &ObjectManager,
        module_id: ObjectId,
        exports: Vec<PrxSymbol>,
    ) -> Result<(), KernelError> {
        let module: Arc<PrxModule> = manager.get(module_id)?;
        for export in exports {
            module.add_export(export);
        }
        tracing::info!("Registered exports for module '{}'", module.name());
        Ok(())
    }

    /// sys_prx_resolve_symbol - Resolve a symbol across all loaded modules
    pub fn sys_prx_resolve_symbol(
        manager: &ObjectManager,
        symbol_name: &str,
    ) -> Result<u64, KernelError> {
        let objects = manager.list();
        
        // Search through all PRX modules
        for obj in objects.iter() {
            if obj.object_type() == ObjectType::PrxModule {
                let module: Arc<PrxModule> = manager.get(obj.id())?;
                if let Some(symbol) = module.resolve_symbol(symbol_name) {
                    tracing::debug!("Resolved symbol '{}' to address 0x{:x}", symbol_name, symbol.address);
                    return Ok(symbol.address);
                }
            }
        }
        
        Err(KernelError::PermissionDenied)
    }

    /// sys_prx_link_module - Link a module by resolving its imports
    pub fn sys_prx_link_module(
        manager: &ObjectManager,
        module_id: ObjectId,
    ) -> Result<(), KernelError> {
        let module: Arc<PrxModule> = manager.get(module_id)?;
        let imports = module.imports();
        
        let mut resolved_count = 0;
        for import in imports.iter() {
            match sys_prx_resolve_symbol(manager, &import.name) {
                Ok(address) => {
                    tracing::debug!("Resolved import '{}' to 0x{:x}", import.name, address);
                    resolved_count += 1;
                }
                Err(_) => {
                    tracing::warn!("Failed to resolve import '{}'", import.name);
                }
            }
        }
        
        tracing::info!("Linked module '{}': resolved {}/{} imports", 
                      module.name(), resolved_count, imports.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prx_load_unload() {
        let manager = ObjectManager::new();

        // Load module
        let module_id = syscalls::sys_prx_load_module(
            &manager,
            "/dev_flash/sys/internal/liblv2.sprx",
            0,
            0,
        )
        .unwrap();

        assert!(manager.exists(module_id));

        // Unload module
        syscalls::sys_prx_unload_module(&manager, module_id, 0).unwrap();
        assert!(!manager.exists(module_id));
    }

    #[test]
    fn test_prx_start_stop() {
        let manager = ObjectManager::new();

        // Load module
        let module_id = syscalls::sys_prx_load_module(
            &manager,
            "/dev_flash/sys/internal/liblv2.sprx",
            0,
            0,
        )
        .unwrap();

        let module: Arc<PrxModule> = manager.get(module_id).unwrap();
        assert_eq!(module.state(), PrxModuleState::Loaded);

        // Start module
        syscalls::sys_prx_start_module(&manager, module_id, 0, 0).unwrap();
        assert_eq!(module.state(), PrxModuleState::Started);

        // Stop module
        syscalls::sys_prx_stop_module(&manager, module_id, 0, 0).unwrap();
        assert_eq!(module.state(), PrxModuleState::Stopped);

        // Unload
        syscalls::sys_prx_unload_module(&manager, module_id, 0).unwrap();
    }

    #[test]
    fn test_prx_get_module_list() {
        let manager = ObjectManager::new();

        // Load multiple modules
        let module_id1 = syscalls::sys_prx_load_module(&manager, "/module1.sprx", 0, 0).unwrap();
        let module_id2 = syscalls::sys_prx_load_module(&manager, "/module2.sprx", 0, 0).unwrap();
        let module_id3 = syscalls::sys_prx_load_module(&manager, "/module3.sprx", 0, 0).unwrap();

        // Get module list
        let modules = syscalls::sys_prx_get_module_list(&manager, 0, 10).unwrap();
        assert_eq!(modules.len(), 3);
        assert!(modules.contains(&module_id1));
        assert!(modules.contains(&module_id2));
        assert!(modules.contains(&module_id3));

        // Test with limited count
        let limited_modules = syscalls::sys_prx_get_module_list(&manager, 0, 2).unwrap();
        assert_eq!(limited_modules.len(), 2);

        // Cleanup
        syscalls::sys_prx_unload_module(&manager, module_id1, 0).unwrap();
        syscalls::sys_prx_unload_module(&manager, module_id2, 0).unwrap();
        syscalls::sys_prx_unload_module(&manager, module_id3, 0).unwrap();
    }

    #[test]
    fn test_prx_cannot_unload_started_module() {
        let manager = ObjectManager::new();

        let module_id = syscalls::sys_prx_load_module(&manager, "/module.sprx", 0, 0).unwrap();
        syscalls::sys_prx_start_module(&manager, module_id, 0, 0).unwrap();

        // Should fail to unload a started module
        let result = syscalls::sys_prx_unload_module(&manager, module_id, 0);
        assert!(result.is_err());

        // Stop first, then unload should work
        syscalls::sys_prx_stop_module(&manager, module_id, 0, 0).unwrap();
        syscalls::sys_prx_unload_module(&manager, module_id, 0).unwrap();
    }

    #[test]
    fn test_prx_symbol_resolution() {
        let manager = ObjectManager::new();

        // Load module
        let module_id = syscalls::sys_prx_load_module(&manager, "/module.sprx", 0, 0).unwrap();

        // Add some exports
        let exports = vec![
            PrxSymbol {
                name: "test_function".to_string(),
                address: 0x10000,
                size: 64,
                symbol_type: PrxSymbolType::Function,
            },
            PrxSymbol {
                name: "test_data".to_string(),
                address: 0x20000,
                size: 128,
                symbol_type: PrxSymbolType::Data,
            },
        ];

        syscalls::sys_prx_register_module(&manager, module_id, exports).unwrap();

        // Resolve symbols
        let func_addr = syscalls::sys_prx_resolve_symbol(&manager, "test_function").unwrap();
        assert_eq!(func_addr, 0x10000);

        let data_addr = syscalls::sys_prx_resolve_symbol(&manager, "test_data").unwrap();
        assert_eq!(data_addr, 0x20000);

        // Try to resolve non-existent symbol
        let result = syscalls::sys_prx_resolve_symbol(&manager, "nonexistent");
        assert!(result.is_err());

        syscalls::sys_prx_unload_module(&manager, module_id, 0).unwrap();
    }

    #[test]
    fn test_prx_linking() {
        let manager = ObjectManager::new();

        // Load library module with exports
        let lib_id = syscalls::sys_prx_load_module(&manager, "/lib.sprx", 0, 0).unwrap();
        let lib_exports = vec![
            PrxSymbol {
                name: "lib_function".to_string(),
                address: 0x30000,
                size: 32,
                symbol_type: PrxSymbolType::Function,
            },
        ];
        syscalls::sys_prx_register_module(&manager, lib_id, lib_exports).unwrap();

        // Load app module with imports
        let app_id = syscalls::sys_prx_load_module(&manager, "/app.sprx", 0, 0).unwrap();
        let app: Arc<PrxModule> = manager.get(app_id).unwrap();
        app.add_import(PrxSymbol {
            name: "lib_function".to_string(),
            address: 0,
            size: 0,
            symbol_type: PrxSymbolType::Function,
        });

        // Link the app module
        syscalls::sys_prx_link_module(&manager, app_id).unwrap();

        // Cleanup
        syscalls::sys_prx_unload_module(&manager, app_id, 0).unwrap();
        syscalls::sys_prx_unload_module(&manager, lib_id, 0).unwrap();
    }
}

