//! Kernel object lifetime management

use oc_core::error::KernelError;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Kernel object ID type
pub type ObjectId = u32;

/// Kernel object types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Mutex,
    Cond,
    RwLock,
    Semaphore,
    EventQueue,
    EventPort,
    SpuThreadGroup,
    SpuThread,
    File,
    Directory,
    PrxModule,
}

/// Trait for kernel objects
pub trait KernelObject: Send + Sync + std::any::Any {
    fn object_type(&self) -> ObjectType;
    fn id(&self) -> ObjectId;
    
    /// Helper for downcasting
    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync>;
}

/// Object manager for tracking kernel objects
pub struct ObjectManager {
    next_id: AtomicU32,
    objects: RwLock<HashMap<ObjectId, Arc<dyn KernelObject>>>,
}

impl ObjectManager {
    /// Create a new object manager
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1), // Start IDs from 1 (0 is invalid)
            objects: RwLock::new(HashMap::new()),
        }
    }

    /// Generate a new unique object ID
    pub fn next_id(&self) -> ObjectId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Register a kernel object
    pub fn register(&self, object: Arc<dyn KernelObject>) -> ObjectId {
        let id = object.id();
        self.objects.write().insert(id, object);
        id
    }

    /// Unregister a kernel object
    pub fn unregister(&self, id: ObjectId) -> Result<(), KernelError> {
        self.objects
            .write()
            .remove(&id)
            .ok_or(KernelError::InvalidId(id))?;
        Ok(())
    }

    /// Get a kernel object by ID
    pub fn get<T: KernelObject + 'static>(&self, id: ObjectId) -> Result<Arc<T>, KernelError> {
        let objects = self.objects.read();
        let object = objects.get(&id).ok_or(KernelError::InvalidId(id))?;

        // Use the as_any helper for downcasting
        let any = Arc::clone(object).as_any();
        any.downcast::<T>()
            .map_err(|_| KernelError::InvalidId(id))
    }

    /// Check if an object exists
    pub fn exists(&self, id: ObjectId) -> bool {
        self.objects.read().contains_key(&id)
    }

    /// Get count of objects
    pub fn count(&self) -> usize {
        self.objects.read().len()
    }

    /// Get count of objects by type
    pub fn count_by_type(&self, obj_type: ObjectType) -> usize {
        self.objects
            .read()
            .values()
            .filter(|obj| obj.object_type() == obj_type)
            .count()
    }

    /// List all registered objects
    pub fn list(&self) -> Vec<Arc<dyn KernelObject>> {
        self.objects.read().values().cloned().collect()
    }
}

impl Default for ObjectManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        id: ObjectId,
        obj_type: ObjectType,
    }

    impl KernelObject for TestObject {
        fn object_type(&self) -> ObjectType {
            self.obj_type
        }

        fn id(&self) -> ObjectId {
            self.id
        }

        fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
            self
        }
    }

    #[test]
    fn test_object_manager() {
        let manager = ObjectManager::new();

        let obj1 = Arc::new(TestObject {
            id: manager.next_id(),
            obj_type: ObjectType::Mutex,
        });
        let id1 = obj1.id();

        manager.register(Arc::clone(&obj1) as Arc<dyn KernelObject>);

        assert!(manager.exists(id1));
        assert_eq!(manager.count(), 1);

        let retrieved: Arc<TestObject> = manager.get(id1).unwrap();
        assert_eq!(retrieved.id(), id1);

        manager.unregister(id1).unwrap();
        assert!(!manager.exists(id1));
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_object_types() {
        let manager = ObjectManager::new();

        let mutex = Arc::new(TestObject {
            id: manager.next_id(),
            obj_type: ObjectType::Mutex,
        });
        manager.register(Arc::clone(&mutex) as Arc<dyn KernelObject>);

        let cond = Arc::new(TestObject {
            id: manager.next_id(),
            obj_type: ObjectType::Cond,
        });
        manager.register(Arc::clone(&cond) as Arc<dyn KernelObject>);

        assert_eq!(manager.count(), 2);
        assert_eq!(manager.count_by_type(ObjectType::Mutex), 1);
        assert_eq!(manager.count_by_type(ObjectType::Cond), 1);
    }
}
