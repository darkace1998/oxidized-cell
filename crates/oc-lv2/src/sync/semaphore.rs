//! Semaphore (sys_semaphore_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::sync::Arc;

/// Semaphore attributes
#[derive(Debug, Clone, Copy)]
pub struct SemaphoreAttributes {
    pub protocol: u32,
    pub max_value: u32,
}

impl Default for SemaphoreAttributes {
    fn default() -> Self {
        Self {
            protocol: 1,
            max_value: 65535,
        }
    }
}

/// LV2 Semaphore implementation
pub struct Semaphore {
    id: ObjectId,
    inner: Mutex<SemaphoreState>,
    attributes: SemaphoreAttributes,
}

#[derive(Debug)]
struct SemaphoreState {
    count: u32,
}

impl Semaphore {
    pub fn new(id: ObjectId, attributes: SemaphoreAttributes, initial_count: u32) -> Self {
        Self {
            id,
            inner: Mutex::new(SemaphoreState {
                count: initial_count.min(attributes.max_value),
            }),
            attributes,
        }
    }

    pub fn wait(&self, count: u32) -> Result<(), KernelError> {
        let mut state = self.inner.lock();

        if state.count >= count {
            state.count -= count;
            Ok(())
        } else {
            Err(KernelError::WouldBlock)
        }
    }

    pub fn trywait(&self, count: u32) -> Result<(), KernelError> {
        let mut state = match self.inner.try_lock() {
            Some(s) => s,
            None => return Err(KernelError::WouldBlock),
        };

        if state.count >= count {
            state.count -= count;
            Ok(())
        } else {
            Err(KernelError::WouldBlock)
        }
    }

    pub fn post(&self, count: u32) -> Result<(), KernelError> {
        let mut state = self.inner.lock();

        let new_count = state.count.saturating_add(count);
        if new_count > self.attributes.max_value {
            return Err(KernelError::ResourceLimit);
        }

        state.count = new_count;
        Ok(())
    }

    pub fn get_value(&self) -> u32 {
        self.inner.lock().count
    }
}

impl KernelObject for Semaphore {
    fn object_type(&self) -> ObjectType {
        ObjectType::Semaphore
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Semaphore syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_semaphore_create
    pub fn sys_semaphore_create(
        manager: &ObjectManager,
        attributes: SemaphoreAttributes,
        initial_count: u32,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let semaphore = Arc::new(Semaphore::new(id, attributes, initial_count));
        manager.register(semaphore);
        Ok(id)
    }

    /// sys_semaphore_destroy
    pub fn sys_semaphore_destroy(
        manager: &ObjectManager,
        sem_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(sem_id)
    }

    /// sys_semaphore_wait
    pub fn sys_semaphore_wait(
        manager: &ObjectManager,
        sem_id: ObjectId,
        count: u32,
    ) -> Result<(), KernelError> {
        let semaphore: Arc<Semaphore> = manager.get(sem_id)?;
        semaphore.wait(count)
    }

    /// sys_semaphore_trywait
    pub fn sys_semaphore_trywait(
        manager: &ObjectManager,
        sem_id: ObjectId,
        count: u32,
    ) -> Result<(), KernelError> {
        let semaphore: Arc<Semaphore> = manager.get(sem_id)?;
        semaphore.trywait(count)
    }

    /// sys_semaphore_post
    pub fn sys_semaphore_post(
        manager: &ObjectManager,
        sem_id: ObjectId,
        count: u32,
    ) -> Result<(), KernelError> {
        let semaphore: Arc<Semaphore> = manager.get(sem_id)?;
        semaphore.post(count)
    }

    /// sys_semaphore_get_value
    pub fn sys_semaphore_get_value(
        manager: &ObjectManager,
        sem_id: ObjectId,
    ) -> Result<u32, KernelError> {
        let semaphore: Arc<Semaphore> = manager.get(sem_id)?;
        Ok(semaphore.get_value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semaphore_basic() {
        let manager = ObjectManager::new();
        let sem_id = syscalls::sys_semaphore_create(
            &manager,
            SemaphoreAttributes::default(),
            5,
        )
        .unwrap();

        // Wait should succeed
        syscalls::sys_semaphore_wait(&manager, sem_id, 2).unwrap();
        assert_eq!(
            syscalls::sys_semaphore_get_value(&manager, sem_id).unwrap(),
            3
        );

        // Post
        syscalls::sys_semaphore_post(&manager, sem_id, 4).unwrap();
        assert_eq!(
            syscalls::sys_semaphore_get_value(&manager, sem_id).unwrap(),
            7
        );

        syscalls::sys_semaphore_destroy(&manager, sem_id).unwrap();
    }

    #[test]
    fn test_semaphore_trywait() {
        let manager = ObjectManager::new();
        let sem_id = syscalls::sys_semaphore_create(
            &manager,
            SemaphoreAttributes::default(),
            2,
        )
        .unwrap();

        // Trywait should succeed
        syscalls::sys_semaphore_trywait(&manager, sem_id, 1).unwrap();
        assert_eq!(
            syscalls::sys_semaphore_get_value(&manager, sem_id).unwrap(),
            1
        );

        // Trywait more than available should fail
        assert!(syscalls::sys_semaphore_trywait(&manager, sem_id, 5).is_err());

        syscalls::sys_semaphore_destroy(&manager, sem_id).unwrap();
    }
}

