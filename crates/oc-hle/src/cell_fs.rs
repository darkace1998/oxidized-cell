//! cellFs HLE - File System Operations
//!
//! This module provides HLE implementations for PS3 file system operations.
//! It bridges to the oc-vfs subsystem.

use std::collections::HashMap;
use tracing::{debug, trace};

/// Maximum path length
pub const CELL_FS_MAX_PATH_LENGTH: usize = 1024;

/// Maximum number of open files
pub const CELL_FS_MAX_FD: u32 = 1024;

/// File descriptor type
pub type CellFsFd = i32;

/// File open flags
pub mod flags {
    pub const CELL_FS_O_RDONLY: u32 = 0x000000;
    pub const CELL_FS_O_WRONLY: u32 = 0x000001;
    pub const CELL_FS_O_RDWR: u32 = 0x000002;
    pub const CELL_FS_O_ACCMODE: u32 = 0x000003;
    pub const CELL_FS_O_CREAT: u32 = 0x000200;
    pub const CELL_FS_O_EXCL: u32 = 0x000800;
    pub const CELL_FS_O_TRUNC: u32 = 0x000400;
    pub const CELL_FS_O_APPEND: u32 = 0x000008;
    pub const CELL_FS_O_MSELF: u32 = 0x001000;
}

/// Seek whence values
pub mod seek {
    pub const CELL_FS_SEEK_SET: u32 = 0;
    pub const CELL_FS_SEEK_CUR: u32 = 1;
    pub const CELL_FS_SEEK_END: u32 = 2;
}

/// File mode constants
pub mod mode {
    pub const CELL_FS_S_IFMT: u32 = 0o170000;
    pub const CELL_FS_S_IFDIR: u32 = 0o040000;
    pub const CELL_FS_S_IFREG: u32 = 0o100000;
    pub const CELL_FS_S_IRUSR: u32 = 0o000400;
    pub const CELL_FS_S_IWUSR: u32 = 0o000200;
    pub const CELL_FS_S_IXUSR: u32 = 0o000100;
}

/// File stat structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellFsStat {
    /// File mode
    pub mode: u32,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Access time
    pub atime: u64,
    /// Modification time
    pub mtime: u64,
    /// Creation time
    pub ctime: u64,
    /// File size
    pub size: u64,
    /// Block size
    pub blksize: u64,
}

impl Default for CellFsStat {
    fn default() -> Self {
        Self {
            mode: 0,
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
    /// Directory entry type
    pub d_type: u8,
    /// Directory entry name length
    pub d_namlen: u8,
    /// Directory entry name
    pub d_name: [u8; 256],
}

impl Default for CellFsDirent {
    fn default() -> Self {
        Self {
            d_type: 0,
            d_namlen: 0,
            d_name: [0; 256],
        }
    }
}

/// File handle type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileHandleType {
    File,
    Directory,
}

/// File handle information
#[derive(Debug, Clone)]
struct FileHandle {
    /// Type of handle (file or directory)
    handle_type: FileHandleType,
    /// File path
    path: String,
    /// Open flags
    flags: u32,
    /// Current position (for files)
    position: u64,
    /// File size (cached)
    size: u64,
}

/// File system manager
pub struct FsManager {
    /// Next file descriptor
    next_fd: i32,
    /// Open file handles
    handles: HashMap<i32, FileHandle>,
}

impl FsManager {
    /// Create a new file system manager
    pub fn new() -> Self {
        Self {
            next_fd: 3, // Start after stdin/stdout/stderr
            handles: HashMap::new(),
        }
    }

    /// Open a file
    pub fn open(&mut self, path: &str, flags: u32, mode: u32) -> Result<CellFsFd, i32> {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return Err(0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
        }

        if self.handles.len() >= CELL_FS_MAX_FD as usize {
            return Err(0x80010018u32 as i32); // CELL_FS_ERROR_EMFILE
        }

        let fd = self.next_fd;
        self.next_fd += 1;

        debug!("FsManager::open: path={}, flags=0x{:X}, mode=0x{:X}, fd={}", path, flags, mode, fd);

        // Create file handle
        let handle = FileHandle {
            handle_type: FileHandleType::File,
            path: path.to_string(),
            flags,
            position: 0,
            size: 0, // TODO: Get actual size from oc-vfs
        };

        self.handles.insert(fd, handle);

        // TODO: Actually open file through oc-vfs

        Ok(fd)
    }

    /// Close a file
    pub fn close(&mut self, fd: CellFsFd) -> i32 {
        if let Some(handle) = self.handles.remove(&fd) {
            debug!("FsManager::close: fd={}, path={}", fd, handle.path);
            // TODO: Close file through oc-vfs
            0 // CELL_OK
        } else {
            debug!("FsManager::close: invalid fd={}", fd);
            0x80010009u32 as i32 // CELL_FS_ERROR_EBADF
        }
    }

    /// Read from file
    pub fn read(&mut self, fd: CellFsFd, buf: &mut [u8]) -> Result<u64, i32> {
        let handle = self.handles.get_mut(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        if handle.handle_type != FileHandleType::File {
            return Err(0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
        }

        // Check if opened for reading
        if (handle.flags & flags::CELL_FS_O_ACCMODE) == flags::CELL_FS_O_WRONLY {
            return Err(0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
        }

        trace!("FsManager::read: fd={}, position={}, len={}", fd, handle.position, buf.len());

        // TODO: Read from file through oc-vfs
        // For now, simulate reading 0 bytes (EOF)
        let bytes_read = 0u64;
        
        handle.position += bytes_read;
        Ok(bytes_read)
    }

    /// Write to file
    pub fn write(&mut self, fd: CellFsFd, buf: &[u8]) -> Result<u64, i32> {
        let handle = self.handles.get_mut(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        if handle.handle_type != FileHandleType::File {
            return Err(0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
        }

        // Check if opened for writing
        if (handle.flags & flags::CELL_FS_O_ACCMODE) == flags::CELL_FS_O_RDONLY {
            return Err(0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
        }

        trace!("FsManager::write: fd={}, position={}, len={}", fd, handle.position, buf.len());

        // TODO: Write to file through oc-vfs
        // For now, simulate writing all bytes
        let bytes_written = buf.len() as u64;
        
        handle.position += bytes_written;
        if handle.position > handle.size {
            handle.size = handle.position;
        }
        
        Ok(bytes_written)
    }

    /// Seek in file
    pub fn lseek(&mut self, fd: CellFsFd, offset: i64, whence: u32) -> Result<u64, i32> {
        let handle = self.handles.get_mut(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        if handle.handle_type != FileHandleType::File {
            return Err(0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
        }

        let new_position = match whence {
            seek::CELL_FS_SEEK_SET => offset.max(0) as u64,
            seek::CELL_FS_SEEK_CUR => {
                let pos = handle.position as i64 + offset;
                pos.max(0) as u64
            }
            seek::CELL_FS_SEEK_END => {
                let pos = handle.size as i64 + offset;
                pos.max(0) as u64
            }
            _ => return Err(0x80010002u32 as i32), // CELL_FS_ERROR_EINVAL
        };

        trace!("FsManager::lseek: fd={}, offset={}, whence={}, new_position={}", fd, offset, whence, new_position);

        handle.position = new_position;
        Ok(new_position)
    }

    /// Get file status
    pub fn fstat(&self, fd: CellFsFd) -> Result<CellFsStat, i32> {
        let handle = self.handles.get(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        trace!("FsManager::fstat: fd={}, path={}", fd, handle.path);

        // TODO: Get actual file status from oc-vfs
        // For now, return default stat with file size
        let mut stat = CellFsStat::default();
        stat.size = handle.size;
        stat.mode = mode::CELL_FS_S_IFREG | mode::CELL_FS_S_IRUSR | mode::CELL_FS_S_IWUSR;
        
        Ok(stat)
    }

    /// Get file status by path
    pub fn stat(&self, path: &str) -> Result<CellFsStat, i32> {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return Err(0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
        }

        trace!("FsManager::stat: path={}", path);

        // TODO: Get actual file status from oc-vfs
        // For now, return default stat
        let mut stat = CellFsStat::default();
        stat.mode = mode::CELL_FS_S_IFREG | mode::CELL_FS_S_IRUSR | mode::CELL_FS_S_IWUSR;
        
        Ok(stat)
    }

    /// Open a directory
    pub fn opendir(&mut self, path: &str) -> Result<CellFsFd, i32> {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return Err(0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
        }

        if self.handles.len() >= CELL_FS_MAX_FD as usize {
            return Err(0x80010018u32 as i32); // CELL_FS_ERROR_EMFILE
        }

        let fd = self.next_fd;
        self.next_fd += 1;

        debug!("FsManager::opendir: path={}, fd={}", path, fd);

        // Create directory handle
        let handle = FileHandle {
            handle_type: FileHandleType::Directory,
            path: path.to_string(),
            flags: 0,
            position: 0,
            size: 0,
        };

        self.handles.insert(fd, handle);

        // TODO: Actually open directory through oc-vfs

        Ok(fd)
    }

    /// Read directory entry
    pub fn readdir(&mut self, fd: CellFsFd) -> Result<Option<CellFsDirent>, i32> {
        let handle = self.handles.get_mut(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        if handle.handle_type != FileHandleType::Directory {
            return Err(0x80010014u32 as i32); // CELL_FS_ERROR_ENOTDIR
        }

        trace!("FsManager::readdir: fd={}, path={}", fd, handle.path);

        // TODO: Read directory entry through oc-vfs
        // For now, return None (no more entries)
        Ok(None)
    }

    /// Close a directory
    pub fn closedir(&mut self, fd: CellFsFd) -> i32 {
        if let Some(handle) = self.handles.get(&fd) {
            if handle.handle_type != FileHandleType::Directory {
                return 0x80010014u32 as i32; // CELL_FS_ERROR_ENOTDIR
            }
            
            debug!("FsManager::closedir: fd={}, path={}", fd, handle.path);
            self.handles.remove(&fd);
            // TODO: Close directory through oc-vfs
            0 // CELL_OK
        } else {
            debug!("FsManager::closedir: invalid fd={}", fd);
            0x80010009u32 as i32 // CELL_FS_ERROR_EBADF
        }
    }

    /// Get number of open handles
    pub fn open_count(&self) -> usize {
        self.handles.len()
    }

    /// Check if a file descriptor is valid
    pub fn is_valid_fd(&self, fd: CellFsFd) -> bool {
        self.handles.contains_key(&fd)
    }
}

impl Default for FsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellFsOpen - Open a file
///
/// # Arguments
/// * `path` - Path to file
/// * `flags` - Open flags
/// * `fd_addr` - Address to write file descriptor to
/// * `mode` - File mode
///
/// # Returns
/// * 0 on success
pub fn cell_fs_open(path_addr: u32, flags: u32, _fd_addr: u32, mode: u32) -> i32 {
    debug!(
        "cellFsOpen(path=0x{:08X}, flags=0x{:X}, mode=0x{:X})",
        path_addr, flags, mode
    );

    // TODO: Read path from memory
    // TODO: Open file through global fs manager
    // TODO: Write fd to memory

    0 // CELL_OK
}

/// cellFsClose - Close a file
///
/// # Arguments
/// * `fd` - File descriptor
///
/// # Returns
/// * 0 on success
pub fn cell_fs_close(fd: i32) -> i32 {
    debug!("cellFsClose(fd={})", fd);

    crate::context::get_hle_context_mut().fs.close(fd)
}

/// cellFsRead - Read from file
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer address
/// * `nbytes` - Number of bytes to read
/// * `nread_addr` - Address to write number of bytes read
///
/// # Returns
/// * 0 on success
pub fn cell_fs_read(fd: i32, _buf_addr: u32, nbytes: u64, _nread_addr: u32) -> i32 {
    trace!("cellFsRead(fd={}, nbytes={})", fd, nbytes);

    // TODO: Read from file through global fs manager
    // TODO: Write data to buffer
    // TODO: Write number of bytes read

    0 // CELL_OK
}

/// cellFsWrite - Write to file
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer address
/// * `nbytes` - Number of bytes to write
/// * `nwrite_addr` - Address to write number of bytes written
///
/// # Returns
/// * 0 on success
pub fn cell_fs_write(fd: i32, _buf_addr: u32, nbytes: u64, _nwrite_addr: u32) -> i32 {
    trace!("cellFsWrite(fd={}, nbytes={})", fd, nbytes);

    // TODO: Write to file through global fs manager
    // TODO: Read data from buffer
    // TODO: Write number of bytes written

    0 // CELL_OK
}

/// cellFsLseek - Seek in file
///
/// # Arguments
/// * `fd` - File descriptor
/// * `offset` - Offset to seek to
/// * `whence` - Seek mode (SEEK_SET, SEEK_CUR, SEEK_END)
/// * `pos_addr` - Address to write new position to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_lseek(fd: i32, offset: i64, whence: u32, _pos_addr: u32) -> i32 {
    trace!("cellFsLseek(fd={}, offset={}, whence={})", fd, offset, whence);

    // TODO: Seek in file through global fs manager
    // TODO: Write new position

    0 // CELL_OK
}

/// cellFsFstat - Get file status
///
/// # Arguments
/// * `fd` - File descriptor
/// * `stat_addr` - Address to write stat structure to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_fstat(fd: i32, _stat_addr: u32) -> i32 {
    trace!("cellFsFstat(fd={})", fd);

    // TODO: Get file status through global fs manager
    // TODO: Write stat structure to memory

    0 // CELL_OK
}

/// cellFsStat - Get file status by path
///
/// # Arguments
/// * `path` - Path to file
/// * `stat_addr` - Address to write stat structure to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_stat(path_addr: u32, _stat_addr: u32) -> i32 {
    debug!("cellFsStat(path=0x{:08X})", path_addr);

    // TODO: Read path from memory
    // TODO: Get file status through global fs manager
    // TODO: Write stat structure to memory

    0 // CELL_OK
}

/// cellFsOpendir - Open a directory
///
/// # Arguments
/// * `path` - Path to directory
/// * `fd_addr` - Address to write file descriptor to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_opendir(path_addr: u32, _fd_addr: u32) -> i32 {
    debug!("cellFsOpendir(path=0x{:08X})", path_addr);

    // TODO: Read path from memory
    // TODO: Open directory through global fs manager
    // TODO: Write fd to memory

    0 // CELL_OK
}

/// cellFsReaddir - Read directory entry
///
/// # Arguments
/// * `fd` - File descriptor
/// * `dir_addr` - Address to write directory entry to
/// * `nread_addr` - Address to write number of entries read
///
/// # Returns
/// * 0 on success
pub fn cell_fs_readdir(fd: i32, _dir_addr: u32, _nread_addr: u32) -> i32 {
    trace!("cellFsReaddir(fd={})", fd);

    // TODO: Read directory entry through global fs manager
    // TODO: Write entry to memory

    0 // CELL_OK
}

/// cellFsClosedir - Close a directory
///
/// # Arguments
/// * `fd` - File descriptor
///
/// # Returns
/// * 0 on success
pub fn cell_fs_closedir(fd: i32) -> i32 {
    debug!("cellFsClosedir(fd={})", fd);

    crate::context::get_hle_context_mut().fs.closedir(fd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fs_manager() {
        let mut manager = FsManager::new();
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_RDONLY, 0);
        assert!(fd.is_ok());
        assert_eq!(manager.close(fd.unwrap()), 0);
    }

    #[test]
    fn test_fs_manager_file_lifecycle() {
        let mut manager = FsManager::new();
        
        // Open a file
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_RDWR, 0);
        assert!(fd.is_ok());
        let fd = fd.unwrap();
        
        // Verify it's valid
        assert!(manager.is_valid_fd(fd));
        assert_eq!(manager.open_count(), 1);
        
        // Read and write
        let mut buf = [0u8; 10];
        assert!(manager.read(fd, &mut buf).is_ok());
        assert!(manager.write(fd, &buf).is_ok());
        
        // Close
        assert_eq!(manager.close(fd), 0);
        assert!(!manager.is_valid_fd(fd));
        assert_eq!(manager.open_count(), 0);
    }

    #[test]
    fn test_fs_manager_seek() {
        let mut manager = FsManager::new();
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_RDONLY, 0).unwrap();
        
        // Seek to position 100
        let pos = manager.lseek(fd, 100, seek::CELL_FS_SEEK_SET);
        assert!(pos.is_ok());
        assert_eq!(pos.unwrap(), 100);
        
        // Seek relative
        let pos = manager.lseek(fd, 50, seek::CELL_FS_SEEK_CUR);
        assert!(pos.is_ok());
        assert_eq!(pos.unwrap(), 150);
        
        // Seek from end
        let pos = manager.lseek(fd, -10, seek::CELL_FS_SEEK_END);
        assert!(pos.is_ok());
        
        manager.close(fd);
    }

    #[test]
    fn test_fs_manager_stat() {
        let mut manager = FsManager::new();
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_RDONLY, 0).unwrap();
        
        // Test fstat
        let stat = manager.fstat(fd);
        assert!(stat.is_ok());
        let stat = stat.unwrap();
        assert_eq!(stat.blksize, 4096);
        
        // Test stat by path
        let stat = manager.stat("/dev_hdd0/test.txt");
        assert!(stat.is_ok());
        
        manager.close(fd);
    }

    #[test]
    fn test_fs_manager_directory() {
        let mut manager = FsManager::new();
        
        // Open directory
        let fd = manager.opendir("/dev_hdd0");
        assert!(fd.is_ok());
        let fd = fd.unwrap();
        
        // Read directory entries
        let entry = manager.readdir(fd);
        assert!(entry.is_ok());
        
        // Close directory
        assert_eq!(manager.closedir(fd), 0);
    }

    #[test]
    fn test_fs_manager_error_handling() {
        let mut manager = FsManager::new();
        
        // Invalid fd
        assert!(manager.close(999) != 0);
        assert!(manager.fstat(999).is_err());
        
        // Empty path
        assert!(manager.open("", flags::CELL_FS_O_RDONLY, 0).is_err());
        assert!(manager.stat("").is_err());
        assert!(manager.opendir("").is_err());
    }

    #[test]
    fn test_fs_manager_write_permission() {
        let mut manager = FsManager::new();
        
        // Open read-only
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_RDONLY, 0).unwrap();
        
        // Try to write (should fail)
        let buf = [1u8; 10];
        assert!(manager.write(fd, &buf).is_err());
        
        manager.close(fd);
        
        // Open write-only
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_WRONLY, 0).unwrap();
        
        // Try to read (should fail)
        let mut buf = [0u8; 10];
        assert!(manager.read(fd, &mut buf).is_err());
        
        manager.close(fd);
    }

    #[test]
    fn test_fs_manager_multiple_files() {
        let mut manager = FsManager::new();
        
        // Open multiple files
        let fd1 = manager.open("/dev_hdd0/file1.txt", flags::CELL_FS_O_RDONLY, 0).unwrap();
        let fd2 = manager.open("/dev_hdd0/file2.txt", flags::CELL_FS_O_RDONLY, 0).unwrap();
        let fd3 = manager.open("/dev_hdd0/file3.txt", flags::CELL_FS_O_RDONLY, 0).unwrap();
        
        assert_eq!(manager.open_count(), 3);
        assert_ne!(fd1, fd2);
        assert_ne!(fd2, fd3);
        
        manager.close(fd1);
        manager.close(fd2);
        manager.close(fd3);
        
        assert_eq!(manager.open_count(), 0);
    }

    #[test]
    fn test_fs_stat_default() {
        let stat = CellFsStat::default();
        assert_eq!(stat.blksize, 4096);
        assert_eq!(stat.size, 0);
    }

    #[test]
    fn test_fs_open_flags() {
        use flags::*;
        assert_eq!(CELL_FS_O_RDONLY, 0x000000);
        assert_eq!(CELL_FS_O_WRONLY, 0x000001);
        assert_eq!(CELL_FS_O_RDWR, 0x000002);
    }

    #[test]
    fn test_fs_seek_constants() {
        use seek::*;
        assert_eq!(CELL_FS_SEEK_SET, 0);
        assert_eq!(CELL_FS_SEEK_CUR, 1);
        assert_eq!(CELL_FS_SEEK_END, 2);
    }

    #[test]
    fn test_fs_mode_constants() {
        use mode::*;
        assert_eq!(CELL_FS_S_IFDIR, 0o040000);
        assert_eq!(CELL_FS_S_IFREG, 0o100000);
    }
}
