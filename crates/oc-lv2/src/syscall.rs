//! System call dispatcher

use crate::fs;
use crate::objects::ObjectManager;
use crate::process::ProcessManager;
use crate::spu;
use crate::sync::{cond, event, mutex, rwlock, semaphore};
use crate::syscall_numbers::*;
use crate::thread::ThreadManager;
use oc_core::error::KernelError;
use std::sync::Arc;

/// System call handler with state management
pub struct SyscallHandler {
    object_manager: Arc<ObjectManager>,
    process_manager: Arc<ProcessManager>,
    thread_manager: Arc<ThreadManager>,
}

impl SyscallHandler {
    /// Create a new syscall handler
    pub fn new() -> Self {
        Self {
            object_manager: Arc::new(ObjectManager::new()),
            process_manager: Arc::new(ProcessManager::new()),
            thread_manager: Arc::new(ThreadManager::new()),
        }
    }

    /// Get object manager reference
    pub fn object_manager(&self) -> &Arc<ObjectManager> {
        &self.object_manager
    }

    /// Get process manager reference
    pub fn process_manager(&self) -> &Arc<ProcessManager> {
        &self.process_manager
    }

    /// Get thread manager reference
    pub fn thread_manager(&self) -> &Arc<ThreadManager> {
        &self.thread_manager
    }

    /// Handle a system call
    pub fn handle(&self, syscall_num: u64, args: &[u64; 8]) -> Result<i64, KernelError> {
        use crate::process::syscalls as process_sc;
        use crate::thread::syscalls as thread_sc;

        match syscall_num {
            // Process management
            SYS_PROCESS_GETPID => {
                let pid = process_sc::sys_process_getpid(&self.process_manager);
                Ok(pid as i64)
            }

            SYS_PROCESS_EXIT => {
                let exit_code = args[0] as i32;
                tracing::info!("sys_process_exit({})", exit_code);
                process_sc::sys_process_exit(&self.process_manager, exit_code)?;
                Ok(0)
            }

            SYS_PROCESS_GET_SDK_VERSION => {
                let pid = args[0] as u32;
                let version = process_sc::sys_process_get_sdk_version(&self.process_manager, pid)?;
                Ok(version as i64)
            }

            // Thread management
            SYS_PPU_THREAD_YIELD => {
                thread_sc::sys_ppu_thread_yield(&self.thread_manager)?;
                Ok(0)
            }

            SYS_PPU_THREAD_GET_ID => {
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                Ok(thread_id as i64)
            }

            SYS_PPU_THREAD_CREATE => {
                let entry_point = args[0];
                let arg = args[1];
                let priority = args[2] as u32;
                let stack_size = args[3] as usize;
                let flags = args[4];
                let name = "thread"; // Would read from memory in real impl

                let thread_id = thread_sc::sys_ppu_thread_create(
                    &self.thread_manager,
                    entry_point,
                    arg,
                    priority,
                    stack_size,
                    flags,
                    name,
                )?;
                Ok(thread_id as i64)
            }

            SYS_PPU_THREAD_EXIT => {
                let exit_code = args[0];
                thread_sc::sys_ppu_thread_exit(&self.thread_manager, exit_code)?;
                Ok(0)
            }

            // Mutex
            SYS_MUTEX_CREATE => {
                let id = mutex::syscalls::sys_mutex_create(
                    &self.object_manager,
                    mutex::MutexAttributes::default(),
                )?;
                Ok(id as i64)
            }

            SYS_MUTEX_DESTROY => {
                let mutex_id = args[0] as u32;
                mutex::syscalls::sys_mutex_destroy(&self.object_manager, mutex_id)?;
                Ok(0)
            }

            SYS_MUTEX_LOCK => {
                let mutex_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                mutex::syscalls::sys_mutex_lock(&self.object_manager, mutex_id, thread_id)?;
                Ok(0)
            }

            SYS_MUTEX_TRYLOCK => {
                let mutex_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                mutex::syscalls::sys_mutex_trylock(&self.object_manager, mutex_id, thread_id)?;
                Ok(0)
            }

            SYS_MUTEX_UNLOCK => {
                let mutex_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                mutex::syscalls::sys_mutex_unlock(&self.object_manager, mutex_id, thread_id)?;
                Ok(0)
            }

            // Condition variable
            SYS_COND_CREATE => {
                let id = cond::syscalls::sys_cond_create(
                    &self.object_manager,
                    cond::CondAttributes::default(),
                )?;
                Ok(id as i64)
            }

            SYS_COND_DESTROY => {
                let cond_id = args[0] as u32;
                cond::syscalls::sys_cond_destroy(&self.object_manager, cond_id)?;
                Ok(0)
            }

            SYS_COND_SIGNAL => {
                let cond_id = args[0] as u32;
                cond::syscalls::sys_cond_signal(&self.object_manager, cond_id)?;
                Ok(0)
            }

            SYS_COND_SIGNAL_ALL => {
                let cond_id = args[0] as u32;
                cond::syscalls::sys_cond_signal_all(&self.object_manager, cond_id)?;
                Ok(0)
            }

            // RwLock
            SYS_RWLOCK_CREATE => {
                let id = rwlock::syscalls::sys_rwlock_create(
                    &self.object_manager,
                    rwlock::RwLockAttributes::default(),
                )?;
                Ok(id as i64)
            }

            SYS_RWLOCK_DESTROY => {
                let rwlock_id = args[0] as u32;
                rwlock::syscalls::sys_rwlock_destroy(&self.object_manager, rwlock_id)?;
                Ok(0)
            }

            SYS_RWLOCK_RLOCK => {
                let rwlock_id = args[0] as u32;
                rwlock::syscalls::sys_rwlock_rlock(&self.object_manager, rwlock_id)?;
                Ok(0)
            }

            SYS_RWLOCK_WLOCK => {
                let rwlock_id = args[0] as u32;
                rwlock::syscalls::sys_rwlock_wlock(&self.object_manager, rwlock_id)?;
                Ok(0)
            }

            SYS_RWLOCK_UNLOCK => {
                let rwlock_id = args[0] as u32;
                rwlock::syscalls::sys_rwlock_unlock(&self.object_manager, rwlock_id)?;
                Ok(0)
            }

            // Semaphore
            SYS_SEMAPHORE_CREATE => {
                let initial_count = args[0] as u32;
                let id = semaphore::syscalls::sys_semaphore_create(
                    &self.object_manager,
                    semaphore::SemaphoreAttributes::default(),
                    initial_count,
                )?;
                Ok(id as i64)
            }

            SYS_SEMAPHORE_DESTROY => {
                let sem_id = args[0] as u32;
                semaphore::syscalls::sys_semaphore_destroy(&self.object_manager, sem_id)?;
                Ok(0)
            }

            SYS_SEMAPHORE_WAIT => {
                let sem_id = args[0] as u32;
                let count = args[1] as u32;
                semaphore::syscalls::sys_semaphore_wait(&self.object_manager, sem_id, count)?;
                Ok(0)
            }

            SYS_SEMAPHORE_POST => {
                let sem_id = args[0] as u32;
                let count = args[1] as u32;
                semaphore::syscalls::sys_semaphore_post(&self.object_manager, sem_id, count)?;
                Ok(0)
            }

            // Event queue
            SYS_EVENT_QUEUE_CREATE => {
                let size = args[0] as usize;
                let id = event::syscalls::sys_event_queue_create(
                    &self.object_manager,
                    event::EventQueueAttributes::default(),
                    size,
                )?;
                Ok(id as i64)
            }

            SYS_EVENT_QUEUE_DESTROY => {
                let queue_id = args[0] as u32;
                event::syscalls::sys_event_queue_destroy(&self.object_manager, queue_id)?;
                Ok(0)
            }

            SYS_EVENT_PORT_CREATE => {
                let queue_id = args[0] as u32;
                let id = event::syscalls::sys_event_port_create(
                    &self.object_manager,
                    queue_id,
                    event::EventPortAttributes::default(),
                )?;
                Ok(id as i64)
            }

            SYS_EVENT_PORT_DESTROY => {
                let port_id = args[0] as u32;
                event::syscalls::sys_event_port_destroy(&self.object_manager, port_id)?;
                Ok(0)
            }

            SYS_EVENT_PORT_SEND => {
                let port_id = args[0] as u32;
                let data1 = args[1];
                let data2 = args[2];
                let data3 = args[3];
                event::syscalls::sys_event_port_send(
                    &self.object_manager,
                    port_id,
                    data1,
                    data2,
                    data3,
                )?;
                Ok(0)
            }

            // SPU thread group
            SYS_SPU_THREAD_GROUP_CREATE => {
                let num_threads = args[0] as u32;
                let priority = args[1] as i32;
                let id = spu::syscalls::sys_spu_thread_group_create(
                    &self.object_manager,
                    spu::SpuThreadGroupAttributes::default(),
                    num_threads,
                    priority,
                )?;
                Ok(id as i64)
            }

            SYS_SPU_THREAD_GROUP_DESTROY => {
                let group_id = args[0] as u32;
                spu::syscalls::sys_spu_thread_group_destroy(&self.object_manager, group_id)?;
                Ok(0)
            }

            SYS_SPU_THREAD_GROUP_START => {
                let group_id = args[0] as u32;
                spu::syscalls::sys_spu_thread_group_start(&self.object_manager, group_id)?;
                Ok(0)
            }

            SYS_SPU_THREAD_INITIALIZE => {
                let group_id = args[0] as u32;
                let thread_num = args[1] as u32;
                let thread_id = spu::syscalls::sys_spu_thread_initialize(
                    &self.object_manager,
                    group_id,
                    thread_num,
                    spu::SpuThreadAttributes::default(),
                )?;
                Ok(thread_id as i64)
            }

            SYS_SPU_IMAGE_OPEN => {
                let thread_id = args[0] as u32;
                let entry_point = args[1] as u32;
                spu::syscalls::sys_spu_image_open(&self.object_manager, thread_id, entry_point)?;
                Ok(0)
            }

            // File system
            SYS_FS_OPEN => {
                // In real impl, would read path from memory at args[0]
                let path = "/dev_hdd0/test.txt";
                let flags = args[1] as u32;
                let mode = args[2] as u32;
                let fd =
                    fs::syscalls::sys_fs_open(&self.object_manager, path, flags, mode)?;
                Ok(fd as i64)
            }

            SYS_FS_CLOSE => {
                let fd = args[0] as u32;
                fs::syscalls::sys_fs_close(&self.object_manager, fd)?;
                Ok(0)
            }

            // Time
            SYS_TIME_GET_SYSTEM_TIME => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                Ok(duration.as_micros() as i64)
            }

            SYS_TIME_GET_TIMEBASE_FREQUENCY => Ok(79800000), // 79.8 MHz timebase

            // Memory
            SYS_MEMORY_ALLOCATE => {
                tracing::debug!("sys_memory_allocate(size=0x{:x})", args[0]);
                Ok(0) // Would allocate memory and return address
            }

            SYS_MEMORY_FREE => {
                tracing::debug!("sys_memory_free(addr=0x{:x})", args[0]);
                Ok(0)
            }

            // TTY
            SYS_TTY_WRITE => {
                let _ch = args[0] as u32;
                let _buf = args[1] as u32;
                let len = args[2] as u32;
                Ok(len as i64)
            }

            _ => {
                tracing::warn!("Unknown syscall {}", syscall_num);
                Err(KernelError::UnknownSyscall(syscall_num))
            }
        }
    }
}

impl Default for SyscallHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_handler() {
        let handler = SyscallHandler::new();
        let args = [0u64; 8];

        // Test getpid
        let result = handler.handle(SYS_PROCESS_GETPID, &args).unwrap();
        assert_eq!(result, 1);

        // Test get_sdk_version
        let result = handler
            .handle(SYS_PROCESS_GET_SDK_VERSION, &args)
            .unwrap();
        assert_eq!(result, 0x00360001);
    }

    #[test]
    fn test_mutex_syscalls() {
        let handler = SyscallHandler::new();
        let args = [0u64; 8];

        // Create mutex
        let mutex_id = handler.handle(SYS_MUTEX_CREATE, &args).unwrap();
        assert!(mutex_id > 0);

        // Lock mutex
        let mut lock_args = [0u64; 8];
        lock_args[0] = mutex_id as u64;
        handler.handle(SYS_MUTEX_LOCK, &lock_args).unwrap();

        // Unlock mutex
        handler.handle(SYS_MUTEX_UNLOCK, &lock_args).unwrap();

        // Destroy mutex
        handler.handle(SYS_MUTEX_DESTROY, &lock_args).unwrap();
    }

    #[test]
    fn test_event_syscalls() {
        let handler = SyscallHandler::new();

        // Create event queue
        let mut args = [0u64; 8];
        args[0] = 10; // Size
        let queue_id = handler.handle(SYS_EVENT_QUEUE_CREATE, &args).unwrap();
        assert!(queue_id > 0);

        // Create event port
        let mut port_args = [0u64; 8];
        port_args[0] = queue_id as u64;
        let port_id = handler.handle(SYS_EVENT_PORT_CREATE, &port_args).unwrap();
        assert!(port_id > 0);

        // Send event
        let mut send_args = [0u64; 8];
        send_args[0] = port_id as u64;
        send_args[1] = 0x123;
        send_args[2] = 0x456;
        send_args[3] = 0x789;
        handler.handle(SYS_EVENT_PORT_SEND, &send_args).unwrap();

        // Cleanup
        let mut destroy_args = [0u64; 8];
        destroy_args[0] = port_id as u64;
        handler
            .handle(SYS_EVENT_PORT_DESTROY, &destroy_args)
            .unwrap();

        destroy_args[0] = queue_id as u64;
        handler
            .handle(SYS_EVENT_QUEUE_DESTROY, &destroy_args)
            .unwrap();
    }
}

