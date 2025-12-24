//! File system (sys_fs_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use oc_vfs::VirtualFileSystem;
use parking_lot::Mutex;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;

/// File open flags
pub mod flags {
    pub const O_RDONLY: u32 = 0x0000;
    pub const O_WRONLY: u32 = 0x0001;
    pub const O_RDWR: u32 = 0x0002;
    pub const O_CREAT: u32 = 0x0200;
    pub const O_TRUNC: u32 = 0x0400;
    pub const O_APPEND: u32 = 0x0800;
}

/// File seek whence
pub mod seek {
    pub const SEEK_SET: u32 = 0;
    pub const SEEK_CUR: u32 = 1;
    pub const SEEK_END: u32 = 2;
}

/// File stat structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellFsStat {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub size: u64,
    pub blksize: u64,
}

impl Default for CellFsStat {
    fn default() -> Self {
        Self {
            mode: 0o100644, // Regular file, rw-r--r--
            uid: 0,
            gid: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            size: 0,
            blksize: 4096,
        }
    }
}

/// Directory entry
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CellFsDirent {
    pub d_type: u8,
    pub d_namlen: u8,
    pub d_name: [u8; 256],
}

impl CellFsDirent {
    pub fn new(name: &str, is_dir: bool) -> Self {
        let mut d_name = [0u8; 256];
        let bytes = name.as_bytes();
        let len = bytes.len().min(255);
        d_name[..len].copy_from_slice(&bytes[..len]);

        Self {
            d_type: if is_dir { 4 } else { 8 }, // DT_DIR = 4, DT_REG = 8
            d_namlen: len as u8,
            d_name,
        }
    }

    pub fn name(&self) -> String {
        let len = self.d_namlen as usize;
        String::from_utf8_lossy(&self.d_name[..len]).to_string()
    }
}

/// File descriptor
pub struct FileDescriptor {
    id: ObjectId,
    inner: Mutex<FileState>,
}

struct FileState {
    virtual_path: String,
    path: PathBuf,
    file: Option<std::fs::File>,
    flags: u32,
}

impl FileDescriptor {
    pub fn new(id: ObjectId, virtual_path: String, path: PathBuf, flags: u32) -> Result<Self, KernelError> {
        let file = Self::open_file(&path, flags)?;

        Ok(Self {
            id,
            inner: Mutex::new(FileState {
                virtual_path,
                path,
                file: Some(file),
                flags,
            }),
        })
    }

    fn open_file(path: &PathBuf, flags: u32) -> Result<std::fs::File, KernelError> {
        use std::fs::OpenOptions;

        let mut options = OpenOptions::new();

        // Set read/write mode
        if flags & flags::O_RDWR != 0 {
            options.read(true).write(true);
        } else if flags & flags::O_WRONLY != 0 {
            options.write(true);
        } else {
            options.read(true);
        }

        // Set create/truncate flags
        if flags & flags::O_CREAT != 0 {
            options.create(true);
        }
        if flags & flags::O_TRUNC != 0 {
            options.truncate(true);
        }
        if flags & flags::O_APPEND != 0 {
            options.append(true);
        }

        options
            .open(path)
            .map_err(|_| KernelError::PermissionDenied)
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, KernelError> {
        let mut state = self.inner.lock();
        let file = state.file.as_mut().ok_or(KernelError::InvalidId(self.id))?;

        file.read(buffer)
            .map_err(|_| KernelError::PermissionDenied)
    }

    pub fn write(&self, buffer: &[u8]) -> Result<usize, KernelError> {
        let mut state = self.inner.lock();
        let file = state.file.as_mut().ok_or(KernelError::InvalidId(self.id))?;

        file.write(buffer)
            .map_err(|_| KernelError::PermissionDenied)
    }

    pub fn seek(&self, offset: i64, whence: u32) -> Result<u64, KernelError> {
        let mut state = self.inner.lock();
        let file = state.file.as_mut().ok_or(KernelError::InvalidId(self.id))?;

        let seek_from = match whence {
            seek::SEEK_SET => SeekFrom::Start(offset as u64),
            seek::SEEK_CUR => SeekFrom::Current(offset),
            seek::SEEK_END => SeekFrom::End(offset),
            _ => return Err(KernelError::InvalidId(self.id)),
        };

        file.seek(seek_from)
            .map_err(|_| KernelError::PermissionDenied)
    }

    pub fn stat(&self) -> Result<CellFsStat, KernelError> {
        let state = self.inner.lock();
        let file = state.file.as_ref().ok_or(KernelError::InvalidId(self.id))?;

        let metadata = file.metadata().map_err(|_| KernelError::PermissionDenied)?;

        let mut stat = CellFsStat::default();
        stat.size = metadata.len();
        stat.mode = if metadata.is_dir() {
            0o040755 // Directory
        } else {
            0o100644 // Regular file
        };

        Ok(stat)
    }

    pub fn path(&self) -> PathBuf {
        self.inner.lock().path.clone()
    }

    pub fn virtual_path(&self) -> String {
        self.inner.lock().virtual_path.clone()
    }
}

impl KernelObject for FileDescriptor {
    fn object_type(&self) -> ObjectType {
        ObjectType::File
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Directory descriptor
pub struct DirectoryDescriptor {
    id: ObjectId,
    inner: Mutex<DirectoryState>,
}

struct DirectoryState {
    path: PathBuf,
    entries: Vec<CellFsDirent>,
    position: usize,
}

impl DirectoryDescriptor {
    pub fn new(id: ObjectId, path: PathBuf) -> Result<Self, KernelError> {
        let entries = Self::read_entries(&path)?;

        Ok(Self {
            id,
            inner: Mutex::new(DirectoryState {
                path,
                entries,
                position: 0,
            }),
        })
    }

    fn read_entries(path: &PathBuf) -> Result<Vec<CellFsDirent>, KernelError> {
        let read_dir = std::fs::read_dir(path).map_err(|_| KernelError::PermissionDenied)?;

        let mut entries = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|_| KernelError::PermissionDenied)?;
            let metadata = entry.metadata().map_err(|_| KernelError::PermissionDenied)?;
            let name = entry.file_name().to_string_lossy().to_string();

            entries.push(CellFsDirent::new(&name, metadata.is_dir()));
        }

        Ok(entries)
    }

    pub fn readdir(&self) -> Result<Option<CellFsDirent>, KernelError> {
        let mut state = self.inner.lock();

        if state.position >= state.entries.len() {
            return Ok(None);
        }

        let entry = state.entries[state.position].clone();
        state.position += 1;

        Ok(Some(entry))
    }
}

impl KernelObject for DirectoryDescriptor {
    fn object_type(&self) -> ObjectType {
        ObjectType::Directory
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// File system syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_fs_open
    pub fn sys_fs_open(
        manager: &ObjectManager,
        vfs: &VirtualFileSystem,
        virtual_path: &str,
        flags: u32,
        _mode: u32,
    ) -> Result<ObjectId, KernelError> {
        // Resolve virtual path to host path using VFS
        let host_path = vfs
            .resolve(virtual_path)
            .unwrap_or_else(|| PathBuf::from(virtual_path));
        
        tracing::debug!(
            "sys_fs_open: virtual_path={}, host_path={:?}, flags={:#x}",
            virtual_path,
            host_path,
            flags
        );

        let id = manager.next_id();
        let fd = Arc::new(FileDescriptor::new(
            id,
            virtual_path.to_string(),
            host_path,
            flags,
        )?);
        manager.register(fd);
        Ok(id)
    }

    /// sys_fs_close
    pub fn sys_fs_close(manager: &ObjectManager, fd: ObjectId) -> Result<(), KernelError> {
        manager.unregister(fd)
    }

    /// sys_fs_read
    pub fn sys_fs_read(
        manager: &ObjectManager,
        fd: ObjectId,
        buffer: &mut [u8],
    ) -> Result<usize, KernelError> {
        let file: Arc<FileDescriptor> = manager.get(fd)?;
        file.read(buffer)
    }

    /// sys_fs_write
    pub fn sys_fs_write(
        manager: &ObjectManager,
        fd: ObjectId,
        buffer: &[u8],
    ) -> Result<usize, KernelError> {
        let file: Arc<FileDescriptor> = manager.get(fd)?;
        file.write(buffer)
    }

    /// sys_fs_lseek
    pub fn sys_fs_lseek(
        manager: &ObjectManager,
        fd: ObjectId,
        offset: i64,
        whence: u32,
    ) -> Result<u64, KernelError> {
        let file: Arc<FileDescriptor> = manager.get(fd)?;
        file.seek(offset, whence)
    }

    /// sys_fs_fstat
    pub fn sys_fs_fstat(
        manager: &ObjectManager,
        fd: ObjectId,
    ) -> Result<CellFsStat, KernelError> {
        let file: Arc<FileDescriptor> = manager.get(fd)?;
        file.stat()
    }

    /// sys_fs_stat
    pub fn sys_fs_stat(vfs: &VirtualFileSystem, virtual_path: &str) -> Result<CellFsStat, KernelError> {
        // Resolve virtual path to host path using VFS
        let host_path = vfs
            .resolve(virtual_path)
            .unwrap_or_else(|| PathBuf::from(virtual_path));
        
        tracing::debug!(
            "sys_fs_stat: virtual_path={}, host_path={:?}",
            virtual_path,
            host_path
        );

        let metadata = std::fs::metadata(host_path).map_err(|_| KernelError::PermissionDenied)?;

        let mut stat = CellFsStat::default();
        stat.size = metadata.len();
        stat.mode = if metadata.is_dir() {
            0o040755 // Directory
        } else {
            0o100644 // Regular file
        };

        Ok(stat)
    }

    /// sys_fs_opendir
    pub fn sys_fs_opendir(
        manager: &ObjectManager,
        vfs: &VirtualFileSystem,
        virtual_path: &str,
    ) -> Result<ObjectId, KernelError> {
        // Resolve virtual path to host path using VFS
        let host_path = vfs
            .resolve(virtual_path)
            .unwrap_or_else(|| PathBuf::from(virtual_path));
        
        tracing::debug!(
            "sys_fs_opendir: virtual_path={}, host_path={:?}",
            virtual_path,
            host_path
        );

        let id = manager.next_id();
        let dir = Arc::new(DirectoryDescriptor::new(id, host_path)?);
        manager.register(dir);
        Ok(id)
    }

    /// sys_fs_readdir
    pub fn sys_fs_readdir(
        manager: &ObjectManager,
        dir_id: ObjectId,
    ) -> Result<Option<CellFsDirent>, KernelError> {
        let dir: Arc<DirectoryDescriptor> = manager.get(dir_id)?;
        dir.readdir()
    }

    /// sys_fs_closedir
    pub fn sys_fs_closedir(manager: &ObjectManager, dir_id: ObjectId) -> Result<(), KernelError> {
        manager.unregister(dir_id)
    }

    /// sys_fs_mkdir
    pub fn sys_fs_mkdir(
        vfs: &VirtualFileSystem,
        virtual_path: &str,
        _mode: u32,
    ) -> Result<(), KernelError> {
        // Resolve virtual path to host path using VFS
        let host_path = vfs
            .resolve(virtual_path)
            .unwrap_or_else(|| PathBuf::from(virtual_path));
        
        tracing::debug!(
            "sys_fs_mkdir: virtual_path={}, host_path={:?}",
            virtual_path,
            host_path
        );

        // Use create_dir (not create_dir_all) to match POSIX mkdir semantics
        // Parent directories must already exist
        std::fs::create_dir(host_path).map_err(|_| KernelError::PermissionDenied)
    }

    /// sys_fs_rmdir
    pub fn sys_fs_rmdir(vfs: &VirtualFileSystem, virtual_path: &str) -> Result<(), KernelError> {
        // Resolve virtual path to host path using VFS
        let host_path = vfs
            .resolve(virtual_path)
            .unwrap_or_else(|| PathBuf::from(virtual_path));
        
        tracing::debug!(
            "sys_fs_rmdir: virtual_path={}, host_path={:?}",
            virtual_path,
            host_path
        );

        std::fs::remove_dir(host_path).map_err(|_| KernelError::PermissionDenied)
    }

    /// sys_fs_unlink
    pub fn sys_fs_unlink(vfs: &VirtualFileSystem, virtual_path: &str) -> Result<(), KernelError> {
        // Resolve virtual path to host path using VFS
        let host_path = vfs
            .resolve(virtual_path)
            .unwrap_or_else(|| PathBuf::from(virtual_path));
        
        tracing::debug!(
            "sys_fs_unlink: virtual_path={}, host_path={:?}",
            virtual_path,
            host_path
        );

        std::fs::remove_file(host_path).map_err(|_| KernelError::PermissionDenied)
    }

    /// sys_fs_rename
    pub fn sys_fs_rename(
        vfs: &VirtualFileSystem,
        old_virtual_path: &str,
        new_virtual_path: &str,
    ) -> Result<(), KernelError> {
        // Resolve virtual paths to host paths using VFS
        let old_host_path = vfs
            .resolve(old_virtual_path)
            .unwrap_or_else(|| PathBuf::from(old_virtual_path));
        let new_host_path = vfs
            .resolve(new_virtual_path)
            .unwrap_or_else(|| PathBuf::from(new_virtual_path));
        
        tracing::debug!(
            "sys_fs_rename: old={:?} -> new={:?}",
            old_host_path,
            new_host_path
        );

        std::fs::rename(old_host_path, new_host_path).map_err(|_| KernelError::PermissionDenied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_fs_stat() {
        let vfs = VirtualFileSystem::new();
        // Test stat on current directory
        let stat = syscalls::sys_fs_stat(&vfs, ".").unwrap();
        assert!(stat.mode & 0o040000 != 0); // Directory bit
    }

    #[test]
    fn test_fs_open_close() {
        let manager = ObjectManager::new();
        let vfs = VirtualFileSystem::new();

        // Create a temp file for testing
        let temp_path = std::env::temp_dir().join("test_oc_lv2.txt");
        {
            let mut file = std::fs::File::create(&temp_path).unwrap();
            file.write_all(b"test data").unwrap();
        }

        let fd = syscalls::sys_fs_open(
            &manager,
            &vfs,
            temp_path.to_str().unwrap(),
            flags::O_RDONLY,
            0,
        )
        .unwrap();

        assert!(manager.exists(fd));

        syscalls::sys_fs_close(&manager, fd).unwrap();
        assert!(!manager.exists(fd));

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_fs_read_write() {
        let manager = ObjectManager::new();
        let vfs = VirtualFileSystem::new();

        // Create a temp file for testing
        let temp_path = std::env::temp_dir().join("test_oc_lv2_rw.txt");

        let fd = syscalls::sys_fs_open(
            &manager,
            &vfs,
            temp_path.to_str().unwrap(),
            flags::O_RDWR | flags::O_CREAT | flags::O_TRUNC,
            0o644,
        )
        .unwrap();

        // Write
        let write_data = b"Hello, PS3!";
        let written = syscalls::sys_fs_write(&manager, fd, write_data).unwrap();
        assert_eq!(written, write_data.len());

        // Seek to start
        syscalls::sys_fs_lseek(&manager, fd, 0, seek::SEEK_SET).unwrap();

        // Read
        let mut read_buffer = vec![0u8; 20];
        let read_count = syscalls::sys_fs_read(&manager, fd, &mut read_buffer).unwrap();
        assert_eq!(read_count, write_data.len());
        assert_eq!(&read_buffer[..read_count], write_data);

        syscalls::sys_fs_close(&manager, fd).unwrap();

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_vfs_integration() {
        let manager = ObjectManager::new();
        let vfs = VirtualFileSystem::new();

        // Create a temp directory for testing
        let temp_dir = std::env::temp_dir().join("test_oc_lv2_vfs");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Mount /dev_hdd0 to temp directory
        vfs.mount("/dev_hdd0", temp_dir.clone());

        // Create a test file in the temp directory
        let test_file_path = temp_dir.join("test_file.txt");
        {
            let mut file = std::fs::File::create(&test_file_path).unwrap();
            file.write_all(b"VFS test data").unwrap();
        }

        // Open the file using virtual path
        let fd = syscalls::sys_fs_open(
            &manager,
            &vfs,
            "/dev_hdd0/test_file.txt",
            flags::O_RDONLY,
            0,
        )
        .unwrap();

        // Read the file
        let mut buffer = vec![0u8; 20];
        let bytes_read = syscalls::sys_fs_read(&manager, fd, &mut buffer).unwrap();
        assert_eq!(bytes_read, 13);
        assert_eq!(&buffer[..bytes_read], b"VFS test data");

        // Close the file
        syscalls::sys_fs_close(&manager, fd).unwrap();

        // Test stat with virtual path
        let stat = syscalls::sys_fs_stat(&vfs, "/dev_hdd0/test_file.txt").unwrap();
        assert_eq!(stat.size, 13);

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_fs_directory_operations() {
        let vfs = VirtualFileSystem::new();

        // Create a temp directory for testing
        let temp_dir = std::env::temp_dir().join("test_oc_lv2_dirops");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Mount /dev_hdd0 to temp directory
        vfs.mount("/dev_hdd0", temp_dir.clone());

        // Test mkdir - parent directory must exist
        // Create parent first
        std::fs::create_dir_all(temp_dir.join("parent")).unwrap();
        syscalls::sys_fs_mkdir(&vfs, "/dev_hdd0/parent/test_new_dir", 0o755).unwrap();
        assert!(temp_dir.join("parent/test_new_dir").exists());

        // Test rmdir
        syscalls::sys_fs_rmdir(&vfs, "/dev_hdd0/parent/test_new_dir").unwrap();
        assert!(!temp_dir.join("parent/test_new_dir").exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_fs_file_operations() {
        let vfs = VirtualFileSystem::new();

        // Create a temp directory for testing
        let temp_dir = std::env::temp_dir().join("test_oc_lv2_fileops");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Mount /dev_hdd0 to temp directory
        vfs.mount("/dev_hdd0", temp_dir.clone());

        // Create a test file
        let test_file = temp_dir.join("test_file.txt");
        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            file.write_all(b"test content").unwrap();
        }

        // Test rename
        syscalls::sys_fs_rename(&vfs, "/dev_hdd0/test_file.txt", "/dev_hdd0/renamed_file.txt").unwrap();
        assert!(!temp_dir.join("test_file.txt").exists());
        assert!(temp_dir.join("renamed_file.txt").exists());

        // Test unlink
        syscalls::sys_fs_unlink(&vfs, "/dev_hdd0/renamed_file.txt").unwrap();
        assert!(!temp_dir.join("renamed_file.txt").exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}

