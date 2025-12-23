//! Read-write lock (sys_rwlock_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::RwLock as ParkingRwLock;
use std::sync::Arc;

/// RwLock attributes
#[derive(Debug, Clone, Copy)]
pub struct RwLockAttributes {
    pub flags: u32,
}

impl Default for RwLockAttributes {
    fn default() -> Self {
        Self { flags: 0 }
    }
}

/// LV2 RwLock implementation
pub struct RwLock {
    id: ObjectId,
    inner: ParkingRwLock<()>,
    attributes: RwLockAttributes,
}

impl RwLock {
    pub fn new(id: ObjectId, attributes: RwLockAttributes) -> Self {
        Self {
            id,
            inner: ParkingRwLock::new(()),
            attributes,
        }
    }

    pub fn rlock(&self) -> Result<(), KernelError> {
        let _guard = self.inner.read();
        // In real implementation, we'd track the guard
        Ok(())
    }

    pub fn try_rlock(&self) -> Result<(), KernelError> {
        let _ = self.inner
            .try_read()
            .ok_or(KernelError::WouldBlock)?;
        Ok(())
    }

    pub fn wlock(&self) -> Result<(), KernelError> {
        let _guard = self.inner.write();
        // In real implementation, we'd track the guard
        Ok(())
    }

    pub fn try_wlock(&self) -> Result<(), KernelError> {
        let _ = self.inner
            .try_write()
            .ok_or(KernelError::WouldBlock)?;
        Ok(())
    }

    pub fn unlock(&self) -> Result<(), KernelError> {
        // parking_lot automatically unlocks when guard is dropped
        Ok(())
    }
}

impl KernelObject for RwLock {
    fn object_type(&self) -> ObjectType {
        ObjectType::RwLock
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// RwLock syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_rwlock_create
    pub fn sys_rwlock_create(
        manager: &ObjectManager,
        attributes: RwLockAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let rwlock = Arc::new(RwLock::new(id, attributes));
        manager.register(rwlock);
        Ok(id)
    }

    /// sys_rwlock_destroy
    pub fn sys_rwlock_destroy(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(rwlock_id)
    }

    /// sys_rwlock_rlock
    pub fn sys_rwlock_rlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.rlock()
    }

    /// sys_rwlock_try_rlock
    pub fn sys_rwlock_try_rlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.try_rlock()
    }

    /// sys_rwlock_wlock
    pub fn sys_rwlock_wlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.wlock()
    }

    /// sys_rwlock_try_wlock
    pub fn sys_rwlock_try_wlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.try_wlock()
    }

    /// sys_rwlock_unlock
    pub fn sys_rwlock_unlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.unlock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rwlock_basic() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Multiple read locks should work
        syscalls::sys_rwlock_rlock(&manager, rwlock_id).unwrap();
        syscalls::sys_rwlock_try_rlock(&manager, rwlock_id).unwrap();

        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_write() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Write lock
        syscalls::sys_rwlock_wlock(&manager, rwlock_id).unwrap();
        syscalls::sys_rwlock_unlock(&manager, rwlock_id).unwrap();

        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }
}

