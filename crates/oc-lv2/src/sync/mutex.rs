//! Mutex (sys_mutex_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex as ParkingMutex;
use std::sync::Arc;

/// Mutex attributes
#[derive(Debug, Clone, Copy)]
pub struct MutexAttributes {
    pub protocol: u32,
    pub recursive: bool,
    pub adaptive: bool,
}

impl Default for MutexAttributes {
    fn default() -> Self {
        Self {
            protocol: 1, // PTHREAD_PRIO_NONE
            recursive: false,
            adaptive: false,
        }
    }
}

/// LV2 Mutex implementation
pub struct Mutex {
    id: ObjectId,
    inner: ParkingMutex<MutexState>,
    attributes: MutexAttributes,
}

#[derive(Debug)]
struct MutexState {
    owner: Option<u64>, // Thread ID of owner
    lock_count: u32,    // For recursive mutexes
}

impl Mutex {
    pub fn new(id: ObjectId, attributes: MutexAttributes) -> Self {
        Self {
            id,
            inner: ParkingMutex::new(MutexState {
                owner: None,
                lock_count: 0,
            }),
            attributes,
        }
    }

    pub fn lock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.inner.lock();

        match state.owner {
            None => {
                state.owner = Some(thread_id);
                state.lock_count = 1;
                Ok(())
            }
            Some(owner) if owner == thread_id => {
                if self.attributes.recursive {
                    state.lock_count += 1;
                    Ok(())
                } else {
                    Err(KernelError::WouldBlock)
                }
            }
            Some(_) => {
                // Wait for unlock - this will block
                drop(state);
                let mut state = self.inner.lock();
                state.owner = Some(thread_id);
                state.lock_count = 1;
                Ok(())
            }
        }
    }

    pub fn trylock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = match self.inner.try_lock() {
            Some(s) => s,
            None => return Err(KernelError::WouldBlock),
        };

        match state.owner {
            None => {
                state.owner = Some(thread_id);
                state.lock_count = 1;
                Ok(())
            }
            Some(owner) if owner == thread_id && self.attributes.recursive => {
                state.lock_count += 1;
                Ok(())
            }
            Some(_) => Err(KernelError::WouldBlock),
        }
    }

    pub fn unlock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.inner.lock();

        match state.owner {
            None => Err(KernelError::PermissionDenied),
            Some(owner) if owner != thread_id => Err(KernelError::PermissionDenied),
            Some(_) => {
                if state.lock_count > 1 {
                    state.lock_count -= 1;
                } else {
                    state.owner = None;
                    state.lock_count = 0;
                }
                Ok(())
            }
        }
    }
}

impl KernelObject for Mutex {
    fn object_type(&self) -> ObjectType {
        ObjectType::Mutex
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Mutex syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_mutex_create
    pub fn sys_mutex_create(
        manager: &ObjectManager,
        attributes: MutexAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let mutex = Arc::new(Mutex::new(id, attributes));
        manager.register(mutex);
        Ok(id)
    }

    /// sys_mutex_destroy
    pub fn sys_mutex_destroy(
        manager: &ObjectManager,
        mutex_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(mutex_id)
    }

    /// sys_mutex_lock
    pub fn sys_mutex_lock(
        manager: &ObjectManager,
        mutex_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let mutex: Arc<Mutex> = manager.get(mutex_id)?;
        mutex.lock(thread_id)
    }

    /// sys_mutex_trylock
    pub fn sys_mutex_trylock(
        manager: &ObjectManager,
        mutex_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let mutex: Arc<Mutex> = manager.get(mutex_id)?;
        mutex.trylock(thread_id)
    }

    /// sys_mutex_unlock
    pub fn sys_mutex_unlock(
        manager: &ObjectManager,
        mutex_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let mutex: Arc<Mutex> = manager.get(mutex_id)?;
        mutex.unlock(thread_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_basic() {
        let manager = ObjectManager::new();
        let mutex_id = syscalls::sys_mutex_create(&manager, MutexAttributes::default()).unwrap();

        // Lock
        syscalls::sys_mutex_lock(&manager, mutex_id, 1).unwrap();

        // Trylock should fail when already locked
        assert!(syscalls::sys_mutex_trylock(&manager, mutex_id, 2).is_err());

        // Unlock
        syscalls::sys_mutex_unlock(&manager, mutex_id, 1).unwrap();

        // Now trylock should succeed
        syscalls::sys_mutex_trylock(&manager, mutex_id, 2).unwrap();
        syscalls::sys_mutex_unlock(&manager, mutex_id, 2).unwrap();

        // Destroy
        syscalls::sys_mutex_destroy(&manager, mutex_id).unwrap();
    }

    #[test]
    fn test_mutex_recursive() {
        let manager = ObjectManager::new();
        let mut attrs = MutexAttributes::default();
        attrs.recursive = true;
        let mutex_id = syscalls::sys_mutex_create(&manager, attrs).unwrap();

        // Lock multiple times with same thread
        syscalls::sys_mutex_lock(&manager, mutex_id, 1).unwrap();
        syscalls::sys_mutex_lock(&manager, mutex_id, 1).unwrap();
        syscalls::sys_mutex_lock(&manager, mutex_id, 1).unwrap();

        // Unlock three times
        syscalls::sys_mutex_unlock(&manager, mutex_id, 1).unwrap();
        syscalls::sys_mutex_unlock(&manager, mutex_id, 1).unwrap();
        syscalls::sys_mutex_unlock(&manager, mutex_id, 1).unwrap();

        // Destroy
        syscalls::sys_mutex_destroy(&manager, mutex_id).unwrap();
    }
}

