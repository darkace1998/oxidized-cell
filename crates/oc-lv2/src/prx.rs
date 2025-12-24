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
    size: usize,
    info: PrxInfo,
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
                size,
                info: PrxInfo::default(),
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
}

