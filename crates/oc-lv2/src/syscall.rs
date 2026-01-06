//! System call dispatcher

use crate::fs;
use crate::memory::MemoryManager;
use crate::objects::ObjectManager;
use crate::process::ProcessManager;
use crate::prx;
use crate::spu;
use crate::sync::{barrier, cond, event, event_flag, mutex, rwlock, semaphore};
use crate::syscall_numbers::*;
use crate::thread::ThreadManager;
use crate::timer;
use oc_core::error::KernelError;
use oc_vfs::VirtualFileSystem;
use std::sync::Arc;

/// System call handler with state management
pub struct SyscallHandler {
    object_manager: Arc<ObjectManager>,
    process_manager: Arc<ProcessManager>,
    thread_manager: Arc<ThreadManager>,
    memory_manager: Arc<MemoryManager>,
    vfs: Arc<VirtualFileSystem>,
    /// Reference to emulator memory for reading/writing syscall data
    emulator_memory: Option<Arc<oc_memory::MemoryManager>>,
}

impl SyscallHandler {
    /// Create a new syscall handler
    pub fn new() -> Self {
        Self {
            object_manager: Arc::new(ObjectManager::new()),
            process_manager: Arc::new(ProcessManager::new()),
            thread_manager: Arc::new(ThreadManager::new()),
            memory_manager: Arc::new(MemoryManager::new()),
            vfs: Arc::new(VirtualFileSystem::new()),
            emulator_memory: None,
        }
    }

    /// Create a new syscall handler with a custom VFS
    pub fn with_vfs(vfs: Arc<VirtualFileSystem>) -> Self {
        Self {
            object_manager: Arc::new(ObjectManager::new()),
            process_manager: Arc::new(ProcessManager::new()),
            thread_manager: Arc::new(ThreadManager::new()),
            memory_manager: Arc::new(MemoryManager::new()),
            vfs,
            emulator_memory: None,
        }
    }

    /// Create a new syscall handler with emulator memory access
    pub fn with_emulator_memory(memory: Arc<oc_memory::MemoryManager>) -> Self {
        Self {
            object_manager: Arc::new(ObjectManager::new()),
            process_manager: Arc::new(ProcessManager::new()),
            thread_manager: Arc::new(ThreadManager::new()),
            memory_manager: Arc::new(MemoryManager::new()),
            vfs: Arc::new(VirtualFileSystem::new()),
            emulator_memory: Some(memory),
        }
    }

    /// Set the emulator memory reference
    pub fn set_emulator_memory(&mut self, memory: Arc<oc_memory::MemoryManager>) {
        self.emulator_memory = Some(memory);
    }

    /// Read a null-terminated string from emulator memory
    fn read_string(&self, addr: u64) -> Result<String, KernelError> {
        let mem = self.emulator_memory.as_ref()
            .ok_or(KernelError::InvalidArgument)?;
        
        // Read a chunk of bytes and find the null terminator
        const MAX_STRING_LEN: u32 = 4096;
        let bytes = mem.read_bytes(addr as u32, MAX_STRING_LEN)
            .map_err(|_| KernelError::MemoryAccess)?;
        
        // Find null terminator
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        
        String::from_utf8(bytes[..len].to_vec()).map_err(|_| KernelError::InvalidArgument)
    }

    /// Write bytes to emulator memory
    fn write_bytes(&self, addr: u64, data: &[u8]) -> Result<(), KernelError> {
        let mem = self.emulator_memory.as_ref()
            .ok_or(KernelError::InvalidArgument)?;
        
        mem.write_bytes(addr as u32, data)
            .map_err(|_| KernelError::MemoryAccess)
    }

    /// Read bytes from emulator memory
    fn read_bytes(&self, addr: u64, size: usize) -> Result<Vec<u8>, KernelError> {
        let mem = self.emulator_memory.as_ref()
            .ok_or(KernelError::InvalidArgument)?;
        
        mem.read_bytes(addr as u32, size as u32)
            .map_err(|_| KernelError::MemoryAccess)
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

    /// Get memory manager reference
    pub fn memory_manager(&self) -> &Arc<MemoryManager> {
        &self.memory_manager
    }

    /// Get VFS reference
    pub fn vfs(&self) -> &Arc<VirtualFileSystem> {
        &self.vfs
    }

    /// Handle a system call
    pub fn handle(&self, syscall_num: u64, args: &[u64; 8]) -> Result<i64, KernelError> {
        use crate::memory::syscalls as memory_sc;
        use crate::process::syscalls as process_sc;
        use crate::thread::syscalls as thread_sc;
        use crate::time::syscalls as time_sc;

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

            SYS_PROCESS_GETPPID => {
                let pid = process_sc::sys_process_getppid(&self.process_manager);
                Ok(pid as i64)
            }

            SYS_PROCESS_GET_STATUS => {
                let pid = args[0] as u32;
                let status = process_sc::sys_process_get_status(&self.process_manager, pid)?;
                Ok(status as i64)
            }

            SYS_PROCESS_GET_PARAMSFO => {
                let buffer = args[0];
                let size = process_sc::sys_process_get_paramsfo(&self.process_manager, buffer)?;
                Ok(size as i64)
            }

            SYS_GAME_PROCESS_EXITSPAWN => {
                // In real impl, would read path and args from memory
                let path = "/dev_hdd0/game/NPEB00000/USRDIR/EBOOT.BIN";
                process_sc::sys_game_process_exitspawn(
                    &self.process_manager,
                    path,
                    &[],
                    &[],
                    0,
                    0,
                    1000,
                    0,
                )?;
                Ok(0)
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

            SYS_PPU_THREAD_START => {
                let thread_id = args[0];
                thread_sc::sys_ppu_thread_start(&self.thread_manager, thread_id)?;
                Ok(0)
            }

            SYS_PPU_THREAD_JOIN => {
                let thread_id = args[0];
                let exit_status = thread_sc::sys_ppu_thread_join(&self.thread_manager, thread_id)?;
                Ok(exit_status as i64)
            }

            SYS_PPU_THREAD_DETACH => {
                let thread_id = args[0];
                thread_sc::sys_ppu_thread_detach(&self.thread_manager, thread_id)?;
                Ok(0)
            }

            SYS_PPU_THREAD_GET_PRIORITY => {
                let thread_id = args[0];
                let priority = thread_sc::sys_ppu_thread_get_priority(&self.thread_manager, thread_id)?;
                Ok(priority as i64)
            }

            SYS_PPU_THREAD_SET_PRIORITY => {
                let thread_id = args[0];
                let priority = args[1] as u32;
                thread_sc::sys_ppu_thread_set_priority(&self.thread_manager, thread_id, priority)?;
                Ok(0)
            }

            SYS_PPU_THREAD_GET_AFFINITY_MASK => {
                let thread_id = args[0];
                let mask = thread_sc::sys_ppu_thread_get_affinity_mask(&self.thread_manager, thread_id)?;
                Ok(mask as i64)
            }

            SYS_PPU_THREAD_SET_AFFINITY_MASK => {
                let thread_id = args[0];
                let mask = args[1];
                thread_sc::sys_ppu_thread_set_affinity_mask(&self.thread_manager, thread_id, mask)?;
                Ok(0)
            }

            SYS_PPU_THREAD_GET_TLS => {
                let thread_id = args[0];
                let tls_pointer = thread_sc::sys_ppu_thread_get_tls(&self.thread_manager, thread_id)?;
                Ok(tls_pointer as i64)
            }

            SYS_PPU_THREAD_SET_TLS => {
                let thread_id = args[0];
                let tls_pointer = args[1];
                thread_sc::sys_ppu_thread_set_tls(&self.thread_manager, thread_id, tls_pointer)?;
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

            SYS_COND_WAIT => {
                let cond_id = args[0] as u32;
                let mutex_id = args[1] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                let timeout_usec = args[2];
                cond::syscalls::sys_cond_wait(&self.object_manager, cond_id, mutex_id, thread_id, timeout_usec)?;
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
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                rwlock::syscalls::sys_rwlock_rlock(&self.object_manager, rwlock_id, thread_id)?;
                Ok(0)
            }

            SYS_RWLOCK_TRYRLOCK => {
                let rwlock_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                rwlock::syscalls::sys_rwlock_try_rlock(&self.object_manager, rwlock_id, thread_id)?;
                Ok(0)
            }

            SYS_RWLOCK_WLOCK => {
                let rwlock_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                rwlock::syscalls::sys_rwlock_wlock(&self.object_manager, rwlock_id, thread_id)?;
                Ok(0)
            }

            SYS_RWLOCK_TRYWLOCK => {
                let rwlock_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                rwlock::syscalls::sys_rwlock_try_wlock(&self.object_manager, rwlock_id, thread_id)?;
                Ok(0)
            }

            SYS_RWLOCK_UNLOCK => {
                let rwlock_id = args[0] as u32;
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                rwlock::syscalls::sys_rwlock_unlock(&self.object_manager, rwlock_id, thread_id)?;
                Ok(0)
            }

            SYS_RWLOCK_RLOCK_TIMEOUT => {
                let rwlock_id = args[0] as u32;
                let timeout_usec = args[1];
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                let result = rwlock::syscalls::sys_rwlock_rlock_timeout(
                    &self.object_manager,
                    rwlock_id,
                    thread_id,
                    timeout_usec,
                )?;
                match result {
                    rwlock::RwLockWaitResult::Acquired => Ok(0),
                    rwlock::RwLockWaitResult::TimedOut => Err(KernelError::WouldBlock),
                }
            }

            SYS_RWLOCK_WLOCK_TIMEOUT => {
                let rwlock_id = args[0] as u32;
                let timeout_usec = args[1];
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                let result = rwlock::syscalls::sys_rwlock_wlock_timeout(
                    &self.object_manager,
                    rwlock_id,
                    thread_id,
                    timeout_usec,
                )?;
                match result {
                    rwlock::RwLockWaitResult::Acquired => Ok(0),
                    rwlock::RwLockWaitResult::TimedOut => Err(KernelError::WouldBlock),
                }
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

            SYS_SEMAPHORE_TRYWAIT => {
                let sem_id = args[0] as u32;
                let count = args[1] as u32;
                semaphore::syscalls::sys_semaphore_trywait(&self.object_manager, sem_id, count)?;
                Ok(0)
            }

            SYS_SEMAPHORE_GET_VALUE => {
                let sem_id = args[0] as u32;
                let value = semaphore::syscalls::sys_semaphore_get_value(&self.object_manager, sem_id)?;
                Ok(value as i64)
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

            SYS_EVENT_QUEUE_RECEIVE => {
                let queue_id = args[0] as u32;
                let timeout_usec = args[1];
                let _event = event::syscalls::sys_event_queue_receive(&self.object_manager, queue_id, timeout_usec)?;
                // In real implementation, would write event data to memory
                Ok(0)
            }

            SYS_EVENT_QUEUE_TRYRECEIVE => {
                let queue_id = args[0] as u32;
                let result = event::syscalls::sys_event_queue_tryreceive(&self.object_manager, queue_id);
                match result {
                    Ok(_event) => Ok(0),
                    Err(KernelError::WouldBlock) => Ok(-1),
                    Err(e) => Err(e),
                }
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

            SYS_SPU_THREAD_GROUP_JOIN => {
                let group_id = args[0] as u32;
                spu::syscalls::sys_spu_thread_group_join(&self.object_manager, group_id)?;
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

            SYS_SPU_THREAD_WRITE_LS => {
                let thread_id = args[0] as u32;
                let addr = args[1] as u32;
                // In real implementation, would read data from memory
                let data = vec![0u8; args[2] as usize];
                spu::syscalls::sys_spu_thread_write_ls(&self.object_manager, thread_id, addr, &data)?;
                Ok(0)
            }

            SYS_SPU_THREAD_READ_LS => {
                let thread_id = args[0] as u32;
                let addr = args[1] as u32;
                let size = args[2] as u32;
                let data = spu::syscalls::sys_spu_thread_read_ls(&self.object_manager, thread_id, addr, size)?;
                // In real implementation, would write data to memory
                Ok(data.len() as i64)
            }

            // File system
            SYS_FS_OPEN => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/test.txt".to_string());
                let flags = args[1] as u32;
                let mode = args[2] as u32;
                tracing::debug!("sys_fs_open({}, 0x{:x}, 0x{:x})", path, flags, mode);
                let fd =
                    fs::syscalls::sys_fs_open(&self.object_manager, &self.vfs, &path, flags, mode)?;
                Ok(fd as i64)
            }

            SYS_FS_CLOSE => {
                let fd = args[0] as u32;
                fs::syscalls::sys_fs_close(&self.object_manager, fd)?;
                Ok(0)
            }

            SYS_FS_READ => {
                let fd = args[0] as u32;
                let buf_addr = args[1];
                let size = args[2] as usize;
                let mut buffer = vec![0u8; size];
                let bytes_read = fs::syscalls::sys_fs_read(&self.object_manager, fd, &mut buffer)?;
                // Write data back to emulator memory
                if bytes_read > 0 {
                    let _ = self.write_bytes(buf_addr, &buffer[..bytes_read]);
                }
                Ok(bytes_read as i64)
            }

            SYS_FS_WRITE => {
                let fd = args[0] as u32;
                let buf_addr = args[1];
                let size = args[2] as usize;
                // Read data from emulator memory
                let buffer = self.read_bytes(buf_addr, size).unwrap_or_else(|_| vec![0u8; size]);
                let bytes_written = fs::syscalls::sys_fs_write(&self.object_manager, fd, &buffer)?;
                Ok(bytes_written as i64)
            }

            SYS_FS_LSEEK => {
                let fd = args[0] as u32;
                let offset = args[1] as i64;
                let whence = args[2] as u32;
                let pos = fs::syscalls::sys_fs_lseek(&self.object_manager, fd, offset, whence)?;
                Ok(pos as i64)
            }

            SYS_FS_FSTAT => {
                let fd = args[0] as u32;
                let _stat = fs::syscalls::sys_fs_fstat(&self.object_manager, fd)?;
                // In real implementation, would write stat to memory at args[1]
                Ok(0)
            }

            SYS_FS_STAT => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/test.txt".to_string());
                tracing::debug!("sys_fs_stat({})", path);
                let _stat = fs::syscalls::sys_fs_stat(&self.vfs, &path)?;
                // In real implementation, would write stat to memory at args[1]
                Ok(0)
            }

            SYS_FS_OPENDIR => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/".to_string());
                tracing::debug!("sys_fs_opendir({})", path);
                let dir_id = fs::syscalls::sys_fs_opendir(&self.object_manager, &self.vfs, &path)?;
                Ok(dir_id as i64)
            }

            SYS_FS_READDIR => {
                let dir_id = args[0] as u32;
                let entry = fs::syscalls::sys_fs_readdir(&self.object_manager, dir_id)?;
                // In real implementation, would write entry to memory at args[1]
                if entry.is_some() {
                    Ok(0)
                } else {
                    Ok(-1) // End of directory
                }
            }

            SYS_FS_CLOSEDIR => {
                let dir_id = args[0] as u32;
                fs::syscalls::sys_fs_closedir(&self.object_manager, dir_id)?;
                Ok(0)
            }

            SYS_FS_MKDIR => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/test_dir".to_string());
                let mode = args[1] as u32;
                tracing::debug!("sys_fs_mkdir({}, 0x{:x})", path, mode);
                fs::syscalls::sys_fs_mkdir(&self.vfs, &path, mode)?;
                Ok(0)
            }

            SYS_FS_RMDIR => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/test_dir".to_string());
                tracing::debug!("sys_fs_rmdir({})", path);
                fs::syscalls::sys_fs_rmdir(&self.vfs, &path)?;
                Ok(0)
            }

            SYS_FS_UNLINK => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/test_file.txt".to_string());
                tracing::debug!("sys_fs_unlink({})", path);
                fs::syscalls::sys_fs_unlink(&self.vfs, &path)?;
                Ok(0)
            }

            SYS_FS_RENAME => {
                // Read paths from emulator memory
                let old_path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_hdd0/old_file.txt".to_string());
                let new_path = self.read_string(args[1]).unwrap_or_else(|_| "/dev_hdd0/new_file.txt".to_string());
                tracing::debug!("sys_fs_rename({}, {})", old_path, new_path);
                fs::syscalls::sys_fs_rename(&self.vfs, &old_path, &new_path)?;
                Ok(0)
            }

            // Time
            SYS_TIME_GET_SYSTEM_TIME => {
                let time = time_sc::sys_time_get_current_time();
                Ok(time as i64)
            }

            SYS_TIME_GET_TIMEBASE_FREQUENCY => {
                let freq = time_sc::sys_time_get_timebase_frequency();
                Ok(freq as i64)
            }

            SYS_TIME_USLEEP => {
                let usec = args[0];
                time_sc::sys_time_usleep(usec)?;
                Ok(0)
            }

            // Memory
            SYS_MEMORY_ALLOCATE => {
                let size = args[0] as usize;
                let page_size = args[1] as usize;
                let flags = args[2];
                
                let page_size = if page_size == 0 {
                    crate::memory::PAGE_SIZE
                } else {
                    page_size
                };
                
                let addr = memory_sc::sys_memory_allocate(
                    &self.memory_manager,
                    size,
                    page_size,
                    flags,
                )?;
                Ok(addr as i64)
            }

            SYS_MEMORY_FREE => {
                let addr = args[0];
                memory_sc::sys_memory_free(&self.memory_manager, addr)?;
                Ok(0)
            }

            SYS_MEMORY_GET_PAGE_ATTRIBUTE => {
                let addr = args[0];
                let _attr = memory_sc::sys_memory_get_page_attribute(&self.memory_manager, addr)?;
                // In real implementation, would write to memory pointed by args[1]
                // For now, return success
                Ok(0)
            }

            SYS_MEMORY_GET_USER_MEMORY_SIZE => {
                let (total, _available) = memory_sc::sys_memory_get_user_memory_size();
                // In real implementation, would write to memory pointed by args[0] and args[1]
                // For now, return total size as the result
                Ok(total as i64)
            }

            SYS_MMAPPER_ALLOCATE_MEMORY => {
                let size = args[0] as usize;
                let page_size = args[1] as usize;
                let flags = args[2];
                
                let page_size = if page_size == 0 {
                    crate::memory::PAGE_SIZE
                } else {
                    page_size
                };
                
                let addr = memory_sc::sys_mmapper_allocate_memory(
                    &self.memory_manager,
                    size,
                    page_size,
                    flags,
                )?;
                Ok(addr as i64)
            }

            SYS_MMAPPER_MAP_MEMORY => {
                let addr = args[0];
                let size = args[1] as usize;
                let flags = args[2];
                memory_sc::sys_mmapper_map_memory(&self.memory_manager, addr, size, flags)?;
                Ok(0)
            }

            // PRX modules
            SYS_PRX_LOAD_MODULE => {
                // Read path from emulator memory at args[0]
                let path = self.read_string(args[0]).unwrap_or_else(|_| "/dev_flash/sys/internal/liblv2.sprx".to_string());
                let flags = args[1];
                let options = args[2];
                tracing::debug!("sys_prx_load_module({}, 0x{:x}, 0x{:x})", path, flags, options);
                let module_id = prx::syscalls::sys_prx_load_module(
                    &self.object_manager,
                    &path,
                    flags,
                    options,
                )?;
                Ok(module_id as i64)
            }

            SYS_PRX_START_MODULE => {
                let module_id = args[0] as u32;
                let module_args = args[1];
                let argp = args[2];
                prx::syscalls::sys_prx_start_module(
                    &self.object_manager,
                    module_id,
                    module_args,
                    argp,
                )?;
                Ok(0)
            }

            SYS_PRX_STOP_MODULE => {
                let module_id = args[0] as u32;
                let module_args = args[1];
                let argp = args[2];
                prx::syscalls::sys_prx_stop_module(
                    &self.object_manager,
                    module_id,
                    module_args,
                    argp,
                )?;
                Ok(0)
            }

            SYS_PRX_UNLOAD_MODULE => {
                let module_id = args[0] as u32;
                let flags = args[1];
                prx::syscalls::sys_prx_unload_module(&self.object_manager, module_id, flags)?;
                Ok(0)
            }

            SYS_PRX_GET_MODULE_LIST => {
                let flags = args[0];
                let max_count = args[1] as usize;
                let modules = prx::syscalls::sys_prx_get_module_list(
                    &self.object_manager,
                    flags,
                    max_count,
                )?;
                // In real implementation, would write module list to memory
                Ok(modules.len() as i64)
            }

            SYS_PRX_GET_MODULE_INFO => {
                let module_id = args[0] as u32;
                let _info = prx::syscalls::sys_prx_get_module_info(&self.object_manager, module_id)?;
                // In real implementation, would write info to memory at args[1]
                Ok(0)
            }

            // Event flags
            SYS_EVENT_FLAG_CREATE => {
                let mut attrs = event_flag::EventFlagAttributes::default();
                attrs.initial_pattern = args[0];
                let id = event_flag::syscalls::sys_event_flag_create(&self.object_manager, attrs)?;
                Ok(id as i64)
            }

            SYS_EVENT_FLAG_DESTROY => {
                let event_flag_id = args[0] as u32;
                event_flag::syscalls::sys_event_flag_destroy(&self.object_manager, event_flag_id)?;
                Ok(0)
            }

            SYS_EVENT_FLAG_WAIT => {
                let event_flag_id = args[0] as u32;
                let bit_pattern = args[1];
                let mode = args[2] as u32;
                let timeout_usec = args[3];
                let thread_id = thread_sc::sys_ppu_thread_get_id(&self.thread_manager);
                let pattern = event_flag::syscalls::sys_event_flag_wait(
                    &self.object_manager,
                    event_flag_id,
                    thread_id,
                    bit_pattern,
                    mode,
                    timeout_usec,
                )?;
                Ok(pattern as i64)
            }

            SYS_EVENT_FLAG_TRYWAIT => {
                let event_flag_id = args[0] as u32;
                let bit_pattern = args[1];
                let mode = args[2] as u32;
                let pattern = event_flag::syscalls::sys_event_flag_trywait(
                    &self.object_manager,
                    event_flag_id,
                    bit_pattern,
                    mode,
                )?;
                Ok(pattern as i64)
            }

            SYS_EVENT_FLAG_SET => {
                let event_flag_id = args[0] as u32;
                let bit_pattern = args[1];
                event_flag::syscalls::sys_event_flag_set(&self.object_manager, event_flag_id, bit_pattern)?;
                Ok(0)
            }

            SYS_EVENT_FLAG_CLEAR => {
                let event_flag_id = args[0] as u32;
                let bit_pattern = args[1];
                event_flag::syscalls::sys_event_flag_clear(&self.object_manager, event_flag_id, bit_pattern)?;
                Ok(0)
            }

            SYS_EVENT_FLAG_GET => {
                let event_flag_id = args[0] as u32;
                let pattern = event_flag::syscalls::sys_event_flag_get(&self.object_manager, event_flag_id)?;
                Ok(pattern as i64)
            }

            SYS_EVENT_FLAG_CANCEL => {
                let event_flag_id = args[0] as u32;
                let thread_id = args[1];
                event_flag::syscalls::sys_event_flag_cancel(&self.object_manager, event_flag_id, thread_id)?;
                Ok(0)
            }

            // Barrier
            SYS_BARRIER_CREATE => {
                let count = args[0] as u32;
                let id = barrier::syscalls::sys_barrier_create(
                    &self.object_manager,
                    count,
                    barrier::BarrierAttributes::default(),
                )?;
                Ok(id as i64)
            }

            SYS_BARRIER_DESTROY => {
                let barrier_id = args[0] as u32;
                barrier::syscalls::sys_barrier_destroy(&self.object_manager, barrier_id)?;
                Ok(0)
            }

            SYS_BARRIER_WAIT => {
                let barrier_id = args[0] as u32;
                let timeout_usec = args[1];
                let result = barrier::syscalls::sys_barrier_wait(
                    &self.object_manager,
                    barrier_id,
                    timeout_usec,
                )?;
                match result {
                    barrier::BarrierWaitResult::Serial => Ok(1),
                    barrier::BarrierWaitResult::Participant => Ok(0),
                    barrier::BarrierWaitResult::TimedOut => Err(KernelError::WouldBlock),
                }
            }

            // Timer
            SYS_TIMER_CREATE => {
                let id = timer::syscalls::sys_timer_create(
                    &self.object_manager,
                    timer::TimerAttributes::default(),
                )?;
                Ok(id as i64)
            }

            SYS_TIMER_DESTROY => {
                let timer_id = args[0] as u32;
                timer::syscalls::sys_timer_destroy(&self.object_manager, timer_id)?;
                Ok(0)
            }

            SYS_TIMER_GET_INFORMATION => {
                let timer_id = args[0] as u32;
                let _info = timer::syscalls::sys_timer_get_information(&self.object_manager, timer_id)?;
                // In real implementation, would write info to memory at args[1]
                Ok(0)
            }

            SYS_TIMER_START => {
                let timer_id = args[0] as u32;
                let base_time = args[1];
                let period = args[2];
                timer::syscalls::sys_timer_start(&self.object_manager, timer_id, base_time, period)?;
                Ok(0)
            }

            SYS_TIMER_STOP => {
                let timer_id = args[0] as u32;
                timer::syscalls::sys_timer_stop(&self.object_manager, timer_id)?;
                Ok(0)
            }

            SYS_TIMER_CONNECT_EVENT_QUEUE => {
                let timer_id = args[0] as u32;
                let event_queue_id = args[1] as u32;
                let event_source = args[2];
                timer::syscalls::sys_timer_connect_event_queue(
                    &self.object_manager,
                    timer_id,
                    event_queue_id,
                    event_source,
                )?;
                Ok(0)
            }

            SYS_TIMER_DISCONNECT_EVENT_QUEUE => {
                let timer_id = args[0] as u32;
                timer::syscalls::sys_timer_disconnect_event_queue(&self.object_manager, timer_id)?;
                Ok(0)
            }

            SYS_TIMER_USLEEP => {
                let duration_usec = args[0];
                timer::syscalls::sys_timer_usleep(duration_usec)?;
                Ok(0)
            }

            SYS_TIMER_SLEEP => {
                let duration_sec = args[0] as u32;
                timer::syscalls::sys_timer_sleep(duration_sec)?;
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

    #[test]
    fn test_memory_syscalls() {
        let handler = SyscallHandler::new();

        // Allocate memory
        let mut args = [0u64; 8];
        args[0] = 0x10000; // size
        args[1] = 0; // page_size (use default)
        args[2] = 0; // flags
        let addr = handler.handle(SYS_MEMORY_ALLOCATE, &args).unwrap();
        assert!(addr > 0);

        // Get page attribute
        let mut attr_args = [0u64; 8];
        attr_args[0] = addr as u64;
        handler
            .handle(SYS_MEMORY_GET_PAGE_ATTRIBUTE, &attr_args)
            .unwrap();

        // Get user memory size
        let size_result = handler
            .handle(SYS_MEMORY_GET_USER_MEMORY_SIZE, &args)
            .unwrap();
        assert!(size_result > 0);

        // Free memory
        let mut free_args = [0u64; 8];
        free_args[0] = addr as u64;
        handler.handle(SYS_MEMORY_FREE, &free_args).unwrap();
    }

    #[test]
    fn test_time_syscalls() {
        let handler = SyscallHandler::new();
        let args = [0u64; 8];

        // Get system time
        let time1 = handler.handle(SYS_TIME_GET_SYSTEM_TIME, &args).unwrap();
        assert!(time1 > 0);

        // Get timebase frequency
        let freq = handler
            .handle(SYS_TIME_GET_TIMEBASE_FREQUENCY, &args)
            .unwrap();
        assert_eq!(freq, 79_800_000);

        // Sleep for a short time
        let mut sleep_args = [0u64; 8];
        sleep_args[0] = 1000; // 1ms
        handler.handle(SYS_TIME_USLEEP, &sleep_args).unwrap();

        // Check time advanced
        let time2 = handler.handle(SYS_TIME_GET_SYSTEM_TIME, &args).unwrap();
        assert!(time2 >= time1);
    }

    #[test]
    fn test_prx_syscalls() {
        let handler = SyscallHandler::new();
        let mut args = [0u64; 8];

        // Load module
        args[0] = 0; // path pointer (placeholder)
        args[1] = 0; // flags
        args[2] = 0; // options
        let module_id = handler.handle(SYS_PRX_LOAD_MODULE, &args).unwrap();
        assert!(module_id > 0);

        // Start module
        let mut start_args = [0u64; 8];
        start_args[0] = module_id as u64;
        handler.handle(SYS_PRX_START_MODULE, &start_args).unwrap();

        // Stop module
        handler.handle(SYS_PRX_STOP_MODULE, &start_args).unwrap();

        // Unload module
        let mut unload_args = [0u64; 8];
        unload_args[0] = module_id as u64;
        handler.handle(SYS_PRX_UNLOAD_MODULE, &unload_args).unwrap();
    }

    #[test]
    fn test_mmapper_syscalls() {
        let handler = SyscallHandler::new();

        // Allocate memory with mmapper
        let mut args = [0u64; 8];
        args[0] = 0x10000; // size
        args[1] = 0; // page_size (use default)
        args[2] = 0; // flags
        let addr = handler.handle(SYS_MMAPPER_ALLOCATE_MEMORY, &args).unwrap();
        assert!(addr > 0);

        // Map memory
        let mut map_args = [0u64; 8];
        map_args[0] = addr as u64;
        map_args[1] = 0x10000; // size
        map_args[2] = 0; // flags
        handler.handle(SYS_MMAPPER_MAP_MEMORY, &map_args).unwrap();

        // Free memory
        let mut free_args = [0u64; 8];
        free_args[0] = addr as u64;
        handler.handle(SYS_MEMORY_FREE, &free_args).unwrap();
    }

    #[test]
    fn test_process_paramsfo() {
        let handler = SyscallHandler::new();
        let mut args = [0u64; 8];
        args[0] = 0; // buffer pointer (placeholder)
        
        let size = handler.handle(SYS_PROCESS_GET_PARAMSFO, &args).unwrap();
        assert!(size > 0);
    }

    #[test]
    fn test_event_flag_syscalls() {
        let handler = SyscallHandler::new();

        // Create event flag with initial pattern 0
        let mut args = [0u64; 8];
        args[0] = 0; // initial pattern
        let event_flag_id = handler.handle(SYS_EVENT_FLAG_CREATE, &args).unwrap();
        assert!(event_flag_id > 0);

        // Set some bits
        let mut set_args = [0u64; 8];
        set_args[0] = event_flag_id as u64;
        set_args[1] = 0x0F; // bit pattern
        handler.handle(SYS_EVENT_FLAG_SET, &set_args).unwrap();

        // Get pattern
        let mut get_args = [0u64; 8];
        get_args[0] = event_flag_id as u64;
        let pattern = handler.handle(SYS_EVENT_FLAG_GET, &get_args).unwrap();
        assert_eq!(pattern, 0x0F);

        // Trywait should succeed
        let mut wait_args = [0u64; 8];
        wait_args[0] = event_flag_id as u64;
        wait_args[1] = 0x01; // bit pattern
        wait_args[2] = 0x0002; // OR mode
        let result = handler.handle(SYS_EVENT_FLAG_TRYWAIT, &wait_args).unwrap();
        assert_eq!(result, 0x0F);

        // Destroy
        let mut destroy_args = [0u64; 8];
        destroy_args[0] = event_flag_id as u64;
        handler.handle(SYS_EVENT_FLAG_DESTROY, &destroy_args).unwrap();
    }

    #[test]
    fn test_barrier_syscalls() {
        let handler = SyscallHandler::new();

        // Create barrier with count 1
        let mut args = [0u64; 8];
        args[0] = 1; // count
        let barrier_id = handler.handle(SYS_BARRIER_CREATE, &args).unwrap();
        assert!(barrier_id > 0);

        // Wait should immediately succeed (single thread barrier)
        let mut wait_args = [0u64; 8];
        wait_args[0] = barrier_id as u64;
        wait_args[1] = 0; // no timeout
        let result = handler.handle(SYS_BARRIER_WAIT, &wait_args).unwrap();
        assert_eq!(result, 1); // Serial thread

        // Destroy
        let mut destroy_args = [0u64; 8];
        destroy_args[0] = barrier_id as u64;
        handler.handle(SYS_BARRIER_DESTROY, &destroy_args).unwrap();
    }

    #[test]
    fn test_timer_syscalls() {
        let handler = SyscallHandler::new();

        // Create timer
        let args = [0u64; 8];
        let timer_id = handler.handle(SYS_TIMER_CREATE, &args).unwrap();
        assert!(timer_id > 0);

        // Start timer
        let mut start_args = [0u64; 8];
        start_args[0] = timer_id as u64;
        start_args[1] = 1_000_000; // 1 second
        start_args[2] = 0; // no period
        handler.handle(SYS_TIMER_START, &start_args).unwrap();

        // Get information
        let mut info_args = [0u64; 8];
        info_args[0] = timer_id as u64;
        handler.handle(SYS_TIMER_GET_INFORMATION, &info_args).unwrap();

        // Stop timer
        let mut stop_args = [0u64; 8];
        stop_args[0] = timer_id as u64;
        handler.handle(SYS_TIMER_STOP, &stop_args).unwrap();

        // Destroy
        let mut destroy_args = [0u64; 8];
        destroy_args[0] = timer_id as u64;
        handler.handle(SYS_TIMER_DESTROY, &destroy_args).unwrap();
    }

    #[test]
    fn test_thread_affinity_syscalls() {
        let handler = SyscallHandler::new();

        // Create a thread
        let mut args = [0u64; 8];
        args[0] = 0x1000; // entry point
        args[1] = 0; // arg
        args[2] = 1000; // priority
        args[3] = 0x4000; // stack size
        args[4] = 0; // flags
        let thread_id = handler.handle(SYS_PPU_THREAD_CREATE, &args).unwrap();
        assert!(thread_id > 0);

        // Get affinity
        let mut aff_args = [0u64; 8];
        aff_args[0] = thread_id as u64;
        let affinity = handler.handle(SYS_PPU_THREAD_GET_AFFINITY_MASK, &aff_args).unwrap();
        assert_eq!(affinity, 0xFF);

        // Set affinity
        aff_args[1] = 0x03;
        handler.handle(SYS_PPU_THREAD_SET_AFFINITY_MASK, &aff_args).unwrap();

        // Verify
        let new_affinity = handler.handle(SYS_PPU_THREAD_GET_AFFINITY_MASK, &aff_args).unwrap();
        assert_eq!(new_affinity, 0x03);
    }

    #[test]
    fn test_thread_tls_syscalls() {
        let handler = SyscallHandler::new();

        // Create a thread
        let mut args = [0u64; 8];
        args[0] = 0x1000; // entry point
        args[1] = 0; // arg
        args[2] = 1000; // priority
        args[3] = 0x4000; // stack size
        args[4] = 0; // flags
        let thread_id = handler.handle(SYS_PPU_THREAD_CREATE, &args).unwrap();
        assert!(thread_id > 0);

        // Get TLS (should be 0 initially)
        let mut tls_args = [0u64; 8];
        tls_args[0] = thread_id as u64;
        let tls = handler.handle(SYS_PPU_THREAD_GET_TLS, &tls_args).unwrap();
        assert_eq!(tls, 0);

        // Set TLS
        tls_args[1] = 0xDEADBEEF;
        handler.handle(SYS_PPU_THREAD_SET_TLS, &tls_args).unwrap();

        // Verify
        let new_tls = handler.handle(SYS_PPU_THREAD_GET_TLS, &tls_args).unwrap();
        assert_eq!(new_tls, 0xDEADBEEF);
    }
}

