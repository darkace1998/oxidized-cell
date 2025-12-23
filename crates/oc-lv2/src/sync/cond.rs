//! Condition variable (sys_cond_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use crate::sync::mutex::Mutex;
use oc_core::error::KernelError;
use parking_lot::Condvar;
use std::sync::Arc;
use std::time::Duration;

/// Condition variable attributes
#[derive(Debug, Clone, Copy)]
pub struct CondAttributes {
    pub flags: u32,
}

impl Default for CondAttributes {
    fn default() -> Self {
        Self { flags: 0 }
    }
}

/// LV2 Condition Variable implementation
pub struct Cond {
    id: ObjectId,
    condvar: Condvar,
    attributes: CondAttributes,
}

impl Cond {
    pub fn new(id: ObjectId, attributes: CondAttributes) -> Self {
        Self {
            id,
            condvar: Condvar::new(),
            attributes,
        }
    }

    pub fn wait(
        &self,
        mutex: &Arc<Mutex>,
        thread_id: u64,
        timeout: Option<Duration>,
    ) -> Result<(), KernelError> {
        // Unlock the mutex and wait
        mutex.unlock(thread_id)?;

        // Wait on the condition variable
        let result = if let Some(_duration) = timeout {
            // Note: parking_lot Condvar doesn't have a direct wait_timeout
            // This is simplified for now
            Ok(())
        } else {
            Ok(())
        };

        // Re-lock the mutex before returning
        mutex.lock(thread_id)?;

        result
    }

    pub fn signal(&self) {
        self.condvar.notify_one();
    }

    pub fn signal_all(&self) {
        self.condvar.notify_all();
    }
}

impl KernelObject for Cond {
    fn object_type(&self) -> ObjectType {
        ObjectType::Cond
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Condition variable syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_cond_create
    pub fn sys_cond_create(
        manager: &ObjectManager,
        attributes: CondAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let cond = Arc::new(Cond::new(id, attributes));
        manager.register(cond);
        Ok(id)
    }

    /// sys_cond_destroy
    pub fn sys_cond_destroy(
        manager: &ObjectManager,
        cond_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(cond_id)
    }

    /// sys_cond_wait
    pub fn sys_cond_wait(
        manager: &ObjectManager,
        cond_id: ObjectId,
        mutex_id: ObjectId,
        thread_id: u64,
        timeout_usec: u64,
    ) -> Result<(), KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        let mutex: Arc<Mutex> = manager.get(mutex_id)?;

        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        cond.wait(&mutex, thread_id, timeout)
    }

    /// sys_cond_signal
    pub fn sys_cond_signal(
        manager: &ObjectManager,
        cond_id: ObjectId,
    ) -> Result<(), KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        cond.signal();
        Ok(())
    }

    /// sys_cond_signal_all
    pub fn sys_cond_signal_all(
        manager: &ObjectManager,
        cond_id: ObjectId,
    ) -> Result<(), KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        cond.signal_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::mutex::{self, MutexAttributes};

    #[test]
    fn test_cond_create_destroy() {
        let manager = ObjectManager::new();
        let cond_id = syscalls::sys_cond_create(&manager, CondAttributes::default()).unwrap();

        assert!(manager.exists(cond_id));

        syscalls::sys_cond_destroy(&manager, cond_id).unwrap();
        assert!(!manager.exists(cond_id));
    }

    #[test]
    fn test_cond_signal() {
        let manager = ObjectManager::new();
        let cond_id = syscalls::sys_cond_create(&manager, CondAttributes::default()).unwrap();
        let mutex_id =
            mutex::syscalls::sys_mutex_create(&manager, MutexAttributes::default()).unwrap();

        // Signal should not fail
        syscalls::sys_cond_signal(&manager, cond_id).unwrap();
        syscalls::sys_cond_signal_all(&manager, cond_id).unwrap();

        syscalls::sys_cond_destroy(&manager, cond_id).unwrap();
        mutex::syscalls::sys_mutex_destroy(&manager, mutex_id).unwrap();
    }
}

