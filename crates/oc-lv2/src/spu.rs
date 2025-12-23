//! SPU management (sys_spu_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::sync::Arc;

/// Maximum number of SPU threads per thread group
const MAX_SPU_THREADS: u32 = 6;

/// SPU local storage size
const SPU_LS_SIZE: u32 = 256 * 1024; // 256KB

/// SPU thread group attributes
#[derive(Debug, Clone)]
pub struct SpuThreadGroupAttributes {
    pub name: String,
    pub priority: u32,
    pub thread_type: u32,
}

impl Default for SpuThreadGroupAttributes {
    fn default() -> Self {
        Self {
            name: String::from("SPU_TG"),
            priority: 100,
            thread_type: 0,
        }
    }
}

/// SPU thread attributes
#[derive(Debug, Clone)]
pub struct SpuThreadAttributes {
    pub name: String,
    pub option: u32,
}

impl Default for SpuThreadAttributes {
    fn default() -> Self {
        Self {
            name: String::from("SPU_Thread"),
            option: 0,
        }
    }
}

/// SPU image information
#[derive(Debug, Clone)]
pub struct SpuImage {
    pub entry_point: u32,
    pub local_storage_size: u32,
    pub segments: Vec<SpuSegment>,
}

#[derive(Debug, Clone)]
pub struct SpuSegment {
    pub addr: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

/// SPU thread group
pub struct SpuThreadGroup {
    id: ObjectId,
    inner: Mutex<SpuThreadGroupState>,
    attributes: SpuThreadGroupAttributes,
}

#[derive(Debug)]
struct SpuThreadGroupState {
    threads: Vec<ObjectId>,
    status: SpuThreadGroupStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpuThreadGroupStatus {
    NotInitialized,
    Initialized,
    Running,
    Stopped,
}

impl SpuThreadGroup {
    pub fn new(id: ObjectId, attributes: SpuThreadGroupAttributes, num_threads: u32) -> Self {
        Self {
            id,
            inner: Mutex::new(SpuThreadGroupState {
                threads: Vec::with_capacity(num_threads as usize),
                status: SpuThreadGroupStatus::NotInitialized,
            }),
            attributes,
        }
    }

    pub fn add_thread(&self, thread_id: ObjectId) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        state.threads.push(thread_id);
        Ok(())
    }

    pub fn start(&self) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        if state.status == SpuThreadGroupStatus::Running {
            return Err(KernelError::PermissionDenied);
        }
        state.status = SpuThreadGroupStatus::Running;
        tracing::debug!("Started SPU thread group {}", self.id);
        Ok(())
    }

    pub fn join(&self) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        if state.status != SpuThreadGroupStatus::Running {
            return Err(KernelError::PermissionDenied);
        }
        state.status = SpuThreadGroupStatus::Stopped;
        tracing::debug!("Joined SPU thread group {}", self.id);
        Ok(())
    }
}

impl KernelObject for SpuThreadGroup {
    fn object_type(&self) -> ObjectType {
        ObjectType::SpuThreadGroup
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// SPU thread
pub struct SpuThread {
    id: ObjectId,
    group_id: ObjectId,
    inner: Mutex<SpuThreadState>,
    attributes: SpuThreadAttributes,
}

#[derive(Debug)]
struct SpuThreadState {
    image: Option<SpuImage>,
    status: SpuThreadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpuThreadStatus {
    NotInitialized,
    Initialized,
    Running,
    Stopped,
}

impl SpuThread {
    pub fn new(id: ObjectId, group_id: ObjectId, attributes: SpuThreadAttributes) -> Self {
        Self {
            id,
            group_id,
            inner: Mutex::new(SpuThreadState {
                image: None,
                status: SpuThreadStatus::NotInitialized,
            }),
            attributes,
        }
    }

    pub fn initialize(&self, image: SpuImage) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        state.image = Some(image);
        state.status = SpuThreadStatus::Initialized;
        tracing::debug!("Initialized SPU thread {}", self.id);
        Ok(())
    }

    pub fn get_group_id(&self) -> ObjectId {
        self.group_id
    }
}

impl KernelObject for SpuThread {
    fn object_type(&self) -> ObjectType {
        ObjectType::SpuThread
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// SPU syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_spu_thread_group_create
    pub fn sys_spu_thread_group_create(
        manager: &ObjectManager,
        attributes: SpuThreadGroupAttributes,
        num_threads: u32,
        _priority: i32,
    ) -> Result<ObjectId, KernelError> {
        if num_threads == 0 || num_threads > MAX_SPU_THREADS {
            return Err(KernelError::ResourceLimit);
        }

        let id = manager.next_id();
        let group = Arc::new(SpuThreadGroup::new(id, attributes, num_threads));
        manager.register(group);
        Ok(id)
    }

    /// sys_spu_thread_group_destroy
    pub fn sys_spu_thread_group_destroy(
        manager: &ObjectManager,
        group_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(group_id)
    }

    /// sys_spu_thread_group_start
    pub fn sys_spu_thread_group_start(
        manager: &ObjectManager,
        group_id: ObjectId,
    ) -> Result<(), KernelError> {
        let group: Arc<SpuThreadGroup> = manager.get(group_id)?;
        group.start()
    }

    /// sys_spu_thread_group_join
    pub fn sys_spu_thread_group_join(
        manager: &ObjectManager,
        group_id: ObjectId,
    ) -> Result<(), KernelError> {
        let group: Arc<SpuThreadGroup> = manager.get(group_id)?;
        group.join()
    }

    /// sys_spu_thread_initialize
    pub fn sys_spu_thread_initialize(
        manager: &ObjectManager,
        group_id: ObjectId,
        _thread_num: u32,
        attributes: SpuThreadAttributes,
    ) -> Result<ObjectId, KernelError> {
        let group: Arc<SpuThreadGroup> = manager.get(group_id)?;

        let thread_id = manager.next_id();
        let thread = Arc::new(SpuThread::new(thread_id, group_id, attributes));

        group.add_thread(thread_id)?;
        manager.register(thread);

        Ok(thread_id)
    }

    /// sys_spu_image_open
    pub fn sys_spu_image_open(
        manager: &ObjectManager,
        thread_id: ObjectId,
        entry_point: u32,
    ) -> Result<(), KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;

        // Create a simple image with just the entry point
        let image = SpuImage {
            entry_point,
            local_storage_size: SPU_LS_SIZE,
            segments: Vec::new(),
        };

        thread.initialize(image)
    }

    /// sys_spu_thread_write_ls
    pub fn sys_spu_thread_write_ls(
        manager: &ObjectManager,
        thread_id: ObjectId,
        addr: u32,
        data: &[u8],
    ) -> Result<(), KernelError> {
        let _thread: Arc<SpuThread> = manager.get(thread_id)?;
        // In a real implementation, would write to SPU local storage
        tracing::debug!("SPU thread {} write LS at 0x{:x}, {} bytes", thread_id, addr, data.len());
        Ok(())
    }

    /// sys_spu_thread_read_ls
    pub fn sys_spu_thread_read_ls(
        manager: &ObjectManager,
        thread_id: ObjectId,
        addr: u32,
        size: u32,
    ) -> Result<Vec<u8>, KernelError> {
        let _thread: Arc<SpuThread> = manager.get(thread_id)?;
        // In a real implementation, would read from SPU local storage
        tracing::debug!("SPU thread {} read LS at 0x{:x}, {} bytes", thread_id, addr, size);
        Ok(vec![0u8; size as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spu_thread_group() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager,
            SpuThreadGroupAttributes::default(),
            2,
            100,
        )
        .unwrap();

        assert!(manager.exists(group_id));

        // Initialize threads
        let thread_id1 = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            0,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        let thread_id2 = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            1,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        // Open images
        syscalls::sys_spu_image_open(&manager, thread_id1, 0x1000).unwrap();
        syscalls::sys_spu_image_open(&manager, thread_id2, 0x2000).unwrap();

        // Start group
        syscalls::sys_spu_thread_group_start(&manager, group_id).unwrap();

        // Join group
        syscalls::sys_spu_thread_group_join(&manager, group_id).unwrap();

        // Destroy
        syscalls::sys_spu_thread_group_destroy(&manager, group_id).unwrap();
    }

    #[test]
    fn test_spu_ls_access() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager,
            SpuThreadGroupAttributes::default(),
            1,
            100,
        )
        .unwrap();

        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            0,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        syscalls::sys_spu_image_open(&manager, thread_id, 0x1000).unwrap();

        // Write to LS
        let data = vec![1, 2, 3, 4];
        syscalls::sys_spu_thread_write_ls(&manager, thread_id, 0x100, &data).unwrap();

        // Read from LS
        let read_data = syscalls::sys_spu_thread_read_ls(&manager, thread_id, 0x100, 4).unwrap();
        assert_eq!(read_data.len(), 4);

        syscalls::sys_spu_thread_group_destroy(&manager, group_id).unwrap();
    }
}

