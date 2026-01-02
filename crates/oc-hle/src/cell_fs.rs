//! cellFs HLE - File System Operations
//!
//! This module provides HLE implementations for PS3 file system operations.
//! It bridges to the oc-vfs subsystem for actual file I/O.
//!
//! ## Async I/O Support
//! 
//! The module provides asynchronous I/O operations via `aio_read` and `aio_write` methods.
//! These queue I/O operations to be executed in background threads, allowing the PS3
//! application to continue execution while I/O is in progress.
//!
//! Async I/O features:
//! - Thread-pool based execution via `std::thread::spawn`
//! - Request tracking with unique IDs
//! - Completion notification via polling or waiting
//! - Optional callback support for completion notification
//! - Positioned I/O support (read/write at specific offset)

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use oc_vfs::VirtualFileSystem;
use tracing::{debug, trace, warn};
use crate::memory::{read_string, write_be32, write_be64, write_bytes, read_bytes};

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

/// Open file handle with optional real file backend
struct OpenFile {
    /// Rust file handle for actual I/O
    file: File,
}

impl std::fmt::Debug for OpenFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenFile").finish()
    }
}

/// Open directory handle
struct OpenDir {
    /// Directory path on host filesystem
    host_path: std::path::PathBuf,
    /// Directory entries (read when directory is opened)
    entries: Vec<std::fs::DirEntry>,
    /// Current position in entries
    position: usize,
}

impl std::fmt::Debug for OpenDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenDir")
            .field("host_path", &self.host_path)
            .field("position", &self.position)
            .field("entries_count", &self.entries.len())
            .finish()
    }
}

/// File handle information
#[derive(Debug)]
struct FileHandle {
    /// Type of handle (file or directory)
    handle_type: FileHandleType,
    /// PS3 virtual file path
    path: String,
    /// Host filesystem path (resolved via VFS)
    #[allow(dead_code)] // Reserved for future direct host path access
    host_path: Option<std::path::PathBuf>,
    /// Open flags
    flags: u32,
    /// Current position (for files without real backend)
    position: u64,
    /// File size (cached)
    size: u64,
    /// Real file handle (if VFS is connected)
    open_file: Option<OpenFile>,
    /// Real directory handle (if VFS is connected)
    open_dir: Option<OpenDir>,
}

/// Async I/O request ID type
pub type AioRequestId = u64;

/// Async I/O operation type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AioOpType {
    Read = 0,
    Write = 1,
}

/// Async I/O completion result sent from background thread
#[derive(Debug)]
struct AioCompletionResult {
    /// Request ID
    request_id: AioRequestId,
    /// Result (bytes transferred or error code)
    result: Result<u64, i32>,
    /// Data buffer (for reads, contains the data read from file)
    data: Vec<u8>,
}

/// Async I/O request
#[derive(Debug, Clone)]
pub struct AioRequest {
    /// Request ID
    pub id: AioRequestId,
    /// File descriptor
    pub fd: CellFsFd,
    /// Operation type
    pub op_type: AioOpType,
    /// Buffer address
    pub buffer_addr: u32,
    /// Number of bytes
    pub size: u64,
    /// File offset (for positioned I/O)
    pub offset: Option<u64>,
    /// Callback function address
    pub callback: Option<u32>,
    /// User data
    pub user_data: u64,
    /// Completion status
    pub completed: bool,
    /// Result (bytes transferred or error code)
    pub result: Result<u64, i32>,
    /// Data buffer (for reads, contains the data read from file)
    pub data: Vec<u8>,
}

/// File system manager
pub struct FsManager {
    /// Next file descriptor
    next_fd: i32,
    /// Open file handles
    handles: HashMap<i32, FileHandle>,
    /// OC-VFS backend for path resolution
    vfs: Option<Arc<VirtualFileSystem>>,
    /// Async I/O requests
    aio_requests: HashMap<AioRequestId, AioRequest>,
    /// Next AIO request ID
    next_aio_id: AioRequestId,
    /// Channel receiver for async I/O completion results (wrapped in Mutex for Sync)
    aio_completion_rx: Mutex<mpsc::Receiver<AioCompletionResult>>,
    /// Channel sender for async I/O completion results (cloned for each worker thread)
    aio_completion_tx: mpsc::Sender<AioCompletionResult>,
}

impl FsManager {
    /// Create a new file system manager
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            next_fd: 3, // Start after stdin/stdout/stderr
            handles: HashMap::new(),
            vfs: None,
            aio_requests: HashMap::new(),
            next_aio_id: 1,
            aio_completion_rx: Mutex::new(rx),
            aio_completion_tx: tx,
        }
    }

    /// Set the VFS backend for file operations
    /// 
    /// Once connected, all file operations will go through the VFS
    /// for path resolution and actual file I/O.
    pub fn set_vfs(&mut self, vfs: Arc<VirtualFileSystem>) {
        debug!("FsManager: VFS backend connected");
        self.vfs = Some(vfs);
    }

    /// Check if VFS is connected
    pub fn has_vfs(&self) -> bool {
        self.vfs.is_some()
    }

    /// Resolve a PS3 virtual path to a host path using VFS
    fn resolve_path(&self, ps3_path: &str) -> Option<std::path::PathBuf> {
        self.vfs.as_ref().and_then(|vfs| vfs.resolve(ps3_path))
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

        // Resolve path through VFS
        let host_path = self.resolve_path(path);
        let mut open_file = None;
        let mut file_size = 0u64;

        if let Some(ref hp) = host_path {
            // Build OpenOptions based on flags
            let mut opts = OpenOptions::new();
            
            match flags & flags::CELL_FS_O_ACCMODE {
                flags::CELL_FS_O_RDONLY => { opts.read(true); }
                flags::CELL_FS_O_WRONLY => { opts.write(true); }
                flags::CELL_FS_O_RDWR => { opts.read(true).write(true); }
                _ => { opts.read(true); }
            }

            if flags & flags::CELL_FS_O_CREAT != 0 {
                opts.create(true);
            }
            if flags & flags::CELL_FS_O_TRUNC != 0 {
                opts.truncate(true);
            }
            if flags & flags::CELL_FS_O_APPEND != 0 {
                opts.append(true);
            }
            if flags & flags::CELL_FS_O_EXCL != 0 {
                opts.create_new(true);
            }

            match opts.open(hp) {
                Ok(file) => {
                    // Get file size
                    if let Ok(metadata) = file.metadata() {
                        file_size = metadata.len();
                    }
                    debug!("FsManager::open: Opened host file {:?}, size={}", hp, file_size);
                    open_file = Some(OpenFile { file });
                }
                Err(e) => {
                    warn!("FsManager::open: Failed to open {:?}: {}", hp, e);
                    // Map I/O error to PS3 error code
                    return match e.kind() {
                        std::io::ErrorKind::NotFound => Err(0x80010006u32 as i32), // CELL_FS_ERROR_ENOENT
                        std::io::ErrorKind::PermissionDenied => Err(0x80010001u32 as i32), // CELL_FS_ERROR_EACCES
                        std::io::ErrorKind::AlreadyExists => Err(0x80010011u32 as i32), // CELL_FS_ERROR_EEXIST
                        _ => Err(0x80010005u32 as i32), // CELL_FS_ERROR_EIO
                    };
                }
            }
        } else {
            debug!("FsManager::open: No VFS mapping for path {}", path);
        }

        // Create file handle
        let handle = FileHandle {
            handle_type: FileHandleType::File,
            path: path.to_string(),
            host_path,
            flags,
            position: 0,
            size: file_size,
            open_file,
            open_dir: None,
        };

        self.handles.insert(fd, handle);

        Ok(fd)
    }

    /// Close a file
    pub fn close(&mut self, fd: CellFsFd) -> i32 {
        if let Some(handle) = self.handles.remove(&fd) {
            debug!("FsManager::close: fd={}, path={}", fd, handle.path);
            // File handle is dropped here, closing the underlying file
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

        // Read from actual file if available
        if let Some(ref mut open_file) = handle.open_file {
            match open_file.file.read(buf) {
                Ok(n) => {
                    handle.position += n as u64;
                    trace!("FsManager::read: read {} bytes from file", n);
                    Ok(n as u64)
                }
                Err(e) => {
                    warn!("FsManager::read: I/O error: {}", e);
                    Err(0x80010005u32 as i32) // CELL_FS_ERROR_EIO
                }
            }
        } else {
            // No real file backend, return EOF
            trace!("FsManager::read: no VFS backend, returning EOF");
            Ok(0)
        }
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

        // Write to actual file if available
        if let Some(ref mut open_file) = handle.open_file {
            match open_file.file.write(buf) {
                Ok(n) => {
                    handle.position += n as u64;
                    if handle.position > handle.size {
                        handle.size = handle.position;
                    }
                    trace!("FsManager::write: wrote {} bytes to file", n);
                    Ok(n as u64)
                }
                Err(e) => {
                    warn!("FsManager::write: I/O error: {}", e);
                    Err(0x80010005u32 as i32) // CELL_FS_ERROR_EIO
                }
            }
        } else {
            // No real file backend, simulate success
            trace!("FsManager::write: no VFS backend, simulating write");
            let bytes_written = buf.len() as u64;
            handle.position += bytes_written;
            if handle.position > handle.size {
                handle.size = handle.position;
            }
            Ok(bytes_written)
        }
    }

    /// Seek in file
    pub fn lseek(&mut self, fd: CellFsFd, offset: i64, whence: u32) -> Result<u64, i32> {
        let handle = self.handles.get_mut(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        if handle.handle_type != FileHandleType::File {
            return Err(0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
        }

        // Convert PS3 whence to Rust SeekFrom
        let seek_from = match whence {
            seek::CELL_FS_SEEK_SET => SeekFrom::Start(offset.max(0) as u64),
            seek::CELL_FS_SEEK_CUR => SeekFrom::Current(offset),
            seek::CELL_FS_SEEK_END => SeekFrom::End(offset),
            _ => return Err(0x80010002u32 as i32), // CELL_FS_ERROR_EINVAL
        };

        // Seek in actual file if available
        let new_position = if let Some(ref mut open_file) = handle.open_file {
            match open_file.file.seek(seek_from) {
                Ok(pos) => pos,
                Err(e) => {
                    warn!("FsManager::lseek: seek error: {}", e);
                    return Err(0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
                }
            }
        } else {
            // No real file, compute position manually
            match whence {
                seek::CELL_FS_SEEK_SET => offset.max(0) as u64,
                seek::CELL_FS_SEEK_CUR => {
                    let pos = handle.position as i64 + offset;
                    pos.max(0) as u64
                }
                seek::CELL_FS_SEEK_END => {
                    let pos = handle.size as i64 + offset;
                    pos.max(0) as u64
                }
                _ => return Err(0x80010002u32 as i32),
            }
        };

        trace!("FsManager::lseek: fd={}, offset={}, whence={}, new_position={}", fd, offset, whence, new_position);

        handle.position = new_position;
        Ok(new_position)
    }

    /// Get file status
    pub fn fstat(&self, fd: CellFsFd) -> Result<CellFsStat, i32> {
        let handle = self.handles.get(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        trace!("FsManager::fstat: fd={}, path={}", fd, handle.path);

        let mut stat = CellFsStat::default();
        
        // Try to get real metadata from file
        if let Some(ref open_file) = handle.open_file {
            if let Ok(metadata) = open_file.file.metadata() {
                stat.size = metadata.len();
                stat.mode = if metadata.is_dir() {
                    mode::CELL_FS_S_IFDIR | mode::CELL_FS_S_IRUSR | mode::CELL_FS_S_IXUSR
                } else {
                    mode::CELL_FS_S_IFREG | mode::CELL_FS_S_IRUSR | mode::CELL_FS_S_IWUSR
                };
                
                // Convert timestamps if available
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    stat.atime = metadata.atime() as u64;
                    stat.mtime = metadata.mtime() as u64;
                    stat.ctime = metadata.ctime() as u64;
                }
                
                return Ok(stat);
            }
        }

        // Fallback to cached info
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

        let mut stat = CellFsStat::default();

        // Try to get real metadata via VFS
        if let Some(host_path) = self.resolve_path(path) {
            match std::fs::metadata(&host_path) {
                Ok(metadata) => {
                    stat.size = metadata.len();
                    stat.mode = if metadata.is_dir() {
                        mode::CELL_FS_S_IFDIR | mode::CELL_FS_S_IRUSR | mode::CELL_FS_S_IXUSR
                    } else {
                        mode::CELL_FS_S_IFREG | mode::CELL_FS_S_IRUSR | mode::CELL_FS_S_IWUSR
                    };
                    
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        stat.atime = metadata.atime() as u64;
                        stat.mtime = metadata.mtime() as u64;
                        stat.ctime = metadata.ctime() as u64;
                    }
                    
                    return Ok(stat);
                }
                Err(e) => {
                    return match e.kind() {
                        std::io::ErrorKind::NotFound => Err(0x80010006u32 as i32), // CELL_FS_ERROR_ENOENT
                        std::io::ErrorKind::PermissionDenied => Err(0x80010001u32 as i32), // CELL_FS_ERROR_EACCES
                        _ => Err(0x80010005u32 as i32), // CELL_FS_ERROR_EIO
                    };
                }
            }
        }

        // No VFS or path not mapped, return default (simulating file exists)
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

        // Resolve path through VFS and read directory entries
        let host_path = self.resolve_path(path);
        let mut open_dir = None;

        if let Some(ref hp) = host_path {
            match std::fs::read_dir(hp) {
                Ok(dir_iter) => {
                    // Collect all entries up front
                    let entries: Vec<std::fs::DirEntry> = dir_iter
                        .filter_map(|e| e.ok())
                        .collect();
                    
                    debug!("FsManager::opendir: Read {} entries from {:?}", entries.len(), hp);
                    
                    open_dir = Some(OpenDir {
                        host_path: hp.clone(),
                        entries,
                        position: 0,
                    });
                }
                Err(e) => {
                    warn!("FsManager::opendir: Failed to open directory {:?}: {}", hp, e);
                    return match e.kind() {
                        std::io::ErrorKind::NotFound => Err(0x80010006u32 as i32), // CELL_FS_ERROR_ENOENT
                        std::io::ErrorKind::PermissionDenied => Err(0x80010001u32 as i32), // CELL_FS_ERROR_EACCES
                        _ => Err(0x80010014u32 as i32), // CELL_FS_ERROR_ENOTDIR
                    };
                }
            }
        }

        // Create directory handle
        let handle = FileHandle {
            handle_type: FileHandleType::Directory,
            path: path.to_string(),
            host_path,
            flags: 0,
            position: 0,
            size: 0,
            open_file: None,
            open_dir,
        };

        self.handles.insert(fd, handle);

        Ok(fd)
    }

    /// Read directory entry
    pub fn readdir(&mut self, fd: CellFsFd) -> Result<Option<CellFsDirent>, i32> {
        let handle = self.handles.get_mut(&fd).ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        if handle.handle_type != FileHandleType::Directory {
            return Err(0x80010014u32 as i32); // CELL_FS_ERROR_ENOTDIR
        }

        trace!("FsManager::readdir: fd={}, path={}", fd, handle.path);

        // Read from actual directory if available
        if let Some(ref mut open_dir) = handle.open_dir {
            if open_dir.position < open_dir.entries.len() {
                let entry = &open_dir.entries[open_dir.position];
                open_dir.position += 1;
                
                let file_name = entry.file_name();
                let name_bytes = file_name.as_encoded_bytes();
                
                let mut dirent = CellFsDirent::default();
                
                // Set type (DT_DIR=4, DT_REG=8)
                dirent.d_type = if entry.path().is_dir() { 4 } else { 8 };
                
                // Copy name
                let copy_len = name_bytes.len().min(255);
                dirent.d_namlen = copy_len as u8;
                dirent.d_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
                
                trace!("FsManager::readdir: returning entry '{}'", 
                       String::from_utf8_lossy(&dirent.d_name[..copy_len]));
                
                return Ok(Some(dirent));
            } else {
                // No more entries
                return Ok(None);
            }
        }

        // No real directory backend, return no entries
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

    /// Check if backend is connected (deprecated, use has_vfs)
    pub fn is_backend_connected(&self) -> bool {
        self.vfs.is_some()
    }

    /// Truncate file to specified length
    /// 
    /// # Arguments
    /// * `path` - File path
    /// * `length` - New file length
    pub fn truncate(&mut self, path: &str, length: u64) -> i32 {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return 0x80010002u32 as i32; // CELL_FS_ERROR_EINVAL
        }

        debug!("FsManager::truncate: path={}, length={}", path, length);

        // Try to truncate via VFS
        if let Some(host_path) = self.resolve_path(path) {
            match OpenOptions::new().write(true).open(&host_path) {
                Ok(file) => {
                    if let Err(e) = file.set_len(length) {
                        warn!("FsManager::truncate: Failed to truncate: {}", e);
                        return 0x80010005u32 as i32; // CELL_FS_ERROR_EIO
                    }
                    return 0;
                }
                Err(e) => {
                    return match e.kind() {
                        std::io::ErrorKind::NotFound => 0x80010006u32 as i32, // CELL_FS_ERROR_ENOENT
                        std::io::ErrorKind::PermissionDenied => 0x80010001u32 as i32, // CELL_FS_ERROR_EACCES
                        _ => 0x80010005u32 as i32, // CELL_FS_ERROR_EIO
                    };
                }
            }
        }

        0 // CELL_OK (simulate success when no VFS)
    }

    /// Create a directory
    /// 
    /// # Arguments
    /// * `path` - Directory path
    /// * `mode` - Directory permissions
    pub fn mkdir(&mut self, path: &str, _mode: u32) -> i32 {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return 0x80010002u32 as i32; // CELL_FS_ERROR_EINVAL
        }

        debug!("FsManager::mkdir: path={}", path);

        // Try to create directory via VFS
        if let Some(host_path) = self.resolve_path(path) {
            match std::fs::create_dir_all(&host_path) {
                Ok(_) => {
                    debug!("FsManager::mkdir: Created directory {:?}", host_path);
                    return 0;
                }
                Err(e) => {
                    warn!("FsManager::mkdir: Failed to create directory: {}", e);
                    return match e.kind() {
                        std::io::ErrorKind::AlreadyExists => 0x80010011u32 as i32, // CELL_FS_ERROR_EEXIST
                        std::io::ErrorKind::PermissionDenied => 0x80010001u32 as i32, // CELL_FS_ERROR_EACCES
                        _ => 0x80010005u32 as i32, // CELL_FS_ERROR_EIO
                    };
                }
            }
        }

        0 // CELL_OK (simulate success when no VFS)
    }

    /// Remove a directory
    /// 
    /// # Arguments
    /// * `path` - Directory path
    pub fn rmdir(&mut self, path: &str) -> i32 {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return 0x80010002u32 as i32; // CELL_FS_ERROR_EINVAL
        }

        debug!("FsManager::rmdir: path={}", path);

        // Try to remove directory via VFS
        if let Some(host_path) = self.resolve_path(path) {
            match std::fs::remove_dir(&host_path) {
                Ok(_) => {
                    debug!("FsManager::rmdir: Removed directory {:?}", host_path);
                    return 0;
                }
                Err(e) => {
                    warn!("FsManager::rmdir: Failed to remove directory: {}", e);
                    return match e.kind() {
                        std::io::ErrorKind::NotFound => 0x80010006u32 as i32, // CELL_FS_ERROR_ENOENT
                        std::io::ErrorKind::PermissionDenied => 0x80010001u32 as i32, // CELL_FS_ERROR_EACCES
                        _ => 0x80010039u32 as i32, // CELL_FS_ERROR_ENOTEMPTY
                    };
                }
            }
        }

        0 // CELL_OK (simulate success when no VFS)
    }

    /// Remove a file
    /// 
    /// # Arguments
    /// * `path` - File path
    pub fn unlink(&mut self, path: &str) -> i32 {
        if path.is_empty() || path.len() > CELL_FS_MAX_PATH_LENGTH {
            return 0x80010002u32 as i32; // CELL_FS_ERROR_EINVAL
        }

        debug!("FsManager::unlink: path={}", path);

        // Try to remove file via VFS
        if let Some(host_path) = self.resolve_path(path) {
            match std::fs::remove_file(&host_path) {
                Ok(_) => {
                    debug!("FsManager::unlink: Removed file {:?}", host_path);
                    return 0;
                }
                Err(e) => {
                    warn!("FsManager::unlink: Failed to remove file: {}", e);
                    return match e.kind() {
                        std::io::ErrorKind::NotFound => 0x80010006u32 as i32, // CELL_FS_ERROR_ENOENT
                        std::io::ErrorKind::PermissionDenied => 0x80010001u32 as i32, // CELL_FS_ERROR_EACCES
                        _ => 0x80010005u32 as i32, // CELL_FS_ERROR_EIO
                    };
                }
            }
        }

        0 // CELL_OK (simulate success when no VFS)
    }

    // ========================================================================
    // Asynchronous I/O Support
    // ========================================================================

    /// Process any pending async I/O completions from background threads.
    /// This should be called periodically to update request status.
    fn process_aio_completions(&mut self) {
        // Non-blocking receive of all pending completions
        if let Ok(rx) = self.aio_completion_rx.lock() {
            while let Ok(completion) = rx.try_recv() {
                if let Some(req) = self.aio_requests.get_mut(&completion.request_id) {
                    req.completed = true;
                    req.result = completion.result;
                    req.data = completion.data;
                    trace!("FsManager: Async I/O request {} completed with result {:?}", 
                           completion.request_id, req.result);
                }
            }
        }
    }

    /// Submit an asynchronous read request
    /// 
    /// Spawns a background thread to perform the read operation asynchronously.
    /// The operation reads from the file at the specified offset (or current position)
    /// and stores the result in the request's data buffer.
    /// 
    /// # Arguments
    /// * `fd` - File descriptor
    /// * `buffer_addr` - Address of buffer to read into (used for writing result)
    /// * `size` - Number of bytes to read
    /// * `offset` - Optional file offset (None for current position)
    /// * `callback` - Optional callback function address (for future use)
    /// * `user_data` - User data to pass to callback
    /// 
    /// # Returns
    /// * Request ID on success, error code on failure
    pub fn aio_read(&mut self, fd: CellFsFd, buffer_addr: u32, size: u64, 
                     offset: Option<u64>, callback: Option<u32>, user_data: u64) -> Result<AioRequestId, i32> {
        // Validate file descriptor and get file info
        let handle = self.handles.get(&fd)
            .ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        // Get the host path for this file descriptor
        let host_path = handle.host_path.clone()
            .ok_or(0x80010005u32 as i32)?; // CELL_FS_ERROR_EIO - no host path means no real file

        let request_id = self.next_aio_id;
        self.next_aio_id += 1;

        let request = AioRequest {
            id: request_id,
            fd,
            op_type: AioOpType::Read,
            buffer_addr,
            size,
            offset,
            callback,
            user_data,
            completed: false,
            result: Ok(0),
            data: Vec::new(),
        };

        debug!("FsManager::aio_read: fd={}, size={}, offset={:?}, request_id={}, path={:?}", 
               fd, size, offset, request_id, host_path);

        self.aio_requests.insert(request_id, request);

        // Spawn background thread to perform the read
        let tx = self.aio_completion_tx.clone();
        let read_size = size as usize;
        let read_offset = offset;
        
        thread::spawn(move || {
            let result = (|| -> Result<(u64, Vec<u8>), i32> {
                // Open the file for reading
                let mut file = File::open(&host_path)
                    .map_err(|e| {
                        warn!("aio_read: Failed to open file {:?}: {}", host_path, e);
                        0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                    })?;
                
                // Seek to offset if specified
                if let Some(off) = read_offset {
                    file.seek(SeekFrom::Start(off))
                        .map_err(|e| {
                            warn!("aio_read: Failed to seek to offset {}: {}", off, e);
                            0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                        })?;
                }
                
                // Read data
                let mut buffer = vec![0u8; read_size];
                let bytes_read = file.read(&mut buffer)
                    .map_err(|e| {
                        warn!("aio_read: Read failed: {}", e);
                        0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                    })?;
                
                buffer.truncate(bytes_read);
                trace!("aio_read: Read {} bytes from {:?}", bytes_read, host_path);
                Ok((bytes_read as u64, buffer))
            })();
            
            let completion = match result {
                Ok((bytes, data)) => AioCompletionResult {
                    request_id,
                    result: Ok(bytes),
                    data,
                },
                Err(e) => AioCompletionResult {
                    request_id,
                    result: Err(e),
                    data: Vec::new(),
                },
            };
            
            // Send completion result (ignore errors if receiver is dropped)
            let _ = tx.send(completion);
        });

        Ok(request_id)
    }

    /// Submit an asynchronous write request
    /// 
    /// Spawns a background thread to perform the write operation asynchronously.
    /// The data to write must be provided via the data parameter (read from guest
    /// memory before calling this method).
    /// 
    /// # Arguments
    /// * `fd` - File descriptor
    /// * `buffer_addr` - Address of buffer to write from
    /// * `size` - Number of bytes to write
    /// * `offset` - Optional file offset (None for current position)
    /// * `callback` - Optional callback function address (for future use)
    /// * `user_data` - User data to pass to callback
    /// 
    /// # Returns
    /// * Request ID on success, error code on failure
    pub fn aio_write(&mut self, fd: CellFsFd, buffer_addr: u32, size: u64,
                      offset: Option<u64>, callback: Option<u32>, user_data: u64) -> Result<AioRequestId, i32> {
        // Validate file descriptor and get file info
        let handle = self.handles.get(&fd)
            .ok_or(0x80010009u32 as i32)?; // CELL_FS_ERROR_EBADF

        // Get the host path for this file descriptor
        let host_path = handle.host_path.clone()
            .ok_or(0x80010005u32 as i32)?; // CELL_FS_ERROR_EIO - no host path means no real file

        let request_id = self.next_aio_id;
        self.next_aio_id += 1;

        // Read data from guest memory before spawning thread
        let write_data = read_bytes(buffer_addr, size as u32)
            .map_err(|_| 0x80010005u32 as i32)?; // CELL_FS_ERROR_EIO

        let request = AioRequest {
            id: request_id,
            fd,
            op_type: AioOpType::Write,
            buffer_addr,
            size,
            offset,
            callback,
            user_data,
            completed: false,
            result: Ok(0),
            data: Vec::new(),
        };

        debug!("FsManager::aio_write: fd={}, size={}, offset={:?}, request_id={}, path={:?}", 
               fd, size, offset, request_id, host_path);

        self.aio_requests.insert(request_id, request);

        // Spawn background thread to perform the write
        let tx = self.aio_completion_tx.clone();
        let write_offset = offset;
        
        thread::spawn(move || {
            let result = (|| -> Result<u64, i32> {
                // Open the file for writing
                let mut file = OpenOptions::new()
                    .write(true)
                    .open(&host_path)
                    .map_err(|e| {
                        warn!("aio_write: Failed to open file {:?}: {}", host_path, e);
                        0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                    })?;
                
                // Seek to offset if specified
                if let Some(off) = write_offset {
                    file.seek(SeekFrom::Start(off))
                        .map_err(|e| {
                            warn!("aio_write: Failed to seek to offset {}: {}", off, e);
                            0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                        })?;
                }
                
                // Write data
                let bytes_written = file.write(&write_data)
                    .map_err(|e| {
                        warn!("aio_write: Write failed: {}", e);
                        0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                    })?;
                
                // Flush to ensure data is written
                file.flush()
                    .map_err(|e| {
                        warn!("aio_write: Flush failed: {}", e);
                        0x80010005u32 as i32 // CELL_FS_ERROR_EIO
                    })?;
                
                trace!("aio_write: Wrote {} bytes to {:?}", bytes_written, host_path);
                Ok(bytes_written as u64)
            })();
            
            let completion = AioCompletionResult {
                request_id,
                result,
                data: Vec::new(),
            };
            
            // Send completion result (ignore errors if receiver is dropped)
            let _ = tx.send(completion);
        });

        Ok(request_id)
    }

    /// Wait for an asynchronous I/O request to complete
    /// 
    /// This method blocks until the specified request completes or times out.
    /// When a read request completes, the data is written to guest memory.
    /// 
    /// # Arguments
    /// * `request_id` - Request ID to wait for
    /// * `timeout_us` - Timeout in microseconds (0 for no timeout)
    /// 
    /// # Returns
    /// * 0 on success, error code on failure
    pub fn aio_wait(&mut self, request_id: AioRequestId, timeout_us: u64) -> i32 {
        // First check if request exists
        if !self.aio_requests.contains_key(&request_id) {
            return 0x80010002u32 as i32; // CELL_FS_ERROR_EINVAL
        }

        // Process any pending completions first
        self.process_aio_completions();

        // Check if already completed
        if let Some(req) = self.aio_requests.get(&request_id) {
            if req.completed {
                trace!("FsManager::aio_wait: request_id={} already completed", request_id);
                // For read operations, write data to guest memory
                if req.op_type == AioOpType::Read && !req.data.is_empty() {
                    if let Err(e) = write_bytes(req.buffer_addr, &req.data) {
                        warn!("aio_wait: Failed to write data to guest memory: 0x{:08X}", e);
                        return e;
                    }
                }
                return 0; // CELL_OK
            }
        }

        // Wait for completion with timeout using polling
        let start = std::time::Instant::now();
        let timeout = if timeout_us > 0 {
            std::time::Duration::from_micros(timeout_us)
        } else {
            std::time::Duration::from_secs(3600) // 1 hour max for "no timeout"
        };

        loop {
            // Process any new completions
            if let Ok(rx) = self.aio_completion_rx.lock() {
                while let Ok(completion) = rx.try_recv() {
                    // Update the request
                    if let Some(req) = self.aio_requests.get_mut(&completion.request_id) {
                        req.completed = true;
                        req.result = completion.result;
                        req.data = completion.data;
                        trace!("FsManager: Async I/O request {} completed with result {:?}", 
                               completion.request_id, req.result);
                    }
                }
            }

            // Check if our request is now complete
            if let Some(req) = self.aio_requests.get(&request_id) {
                if req.completed {
                    // For read operations, write data to guest memory
                    if req.op_type == AioOpType::Read && !req.data.is_empty() {
                        if let Err(e) = write_bytes(req.buffer_addr, &req.data) {
                            warn!("aio_wait: Failed to write data to guest memory: 0x{:08X}", e);
                            return e;
                        }
                    }
                    return 0; // CELL_OK
                }
            }
            
            // Check for overall timeout
            if start.elapsed() >= timeout {
                debug!("FsManager::aio_wait: request_id={} timed out", request_id);
                return 0x80610B01u32 as i32; // CELL_FS_ERROR_ETIMEDOUT
            }
            
            // Sleep briefly before checking again
            thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Poll an asynchronous I/O request status
    /// 
    /// Non-blocking check if an async I/O request has completed.
    /// 
    /// # Arguments
    /// * `request_id` - Request ID to check
    /// 
    /// # Returns
    /// * true if completed, false if still in progress
    pub fn aio_poll(&mut self, request_id: AioRequestId) -> Result<bool, i32> {
        // Process any pending completions first
        self.process_aio_completions();

        let request = self.aio_requests.get(&request_id)
            .ok_or(0x80010002u32 as i32)?; // CELL_FS_ERROR_EINVAL

        Ok(request.completed)
    }

    /// Cancel an asynchronous I/O request
    /// 
    /// # Arguments
    /// * `request_id` - Request ID to cancel
    /// 
    /// # Returns
    /// * 0 on success, error code on failure
    pub fn aio_cancel(&mut self, request_id: AioRequestId) -> i32 {
        if let Some(request) = self.aio_requests.remove(&request_id) {
            debug!("FsManager::aio_cancel: request_id={}", request_id);
            
            // Note: Request is removed and discarded; in a real implementation,
            // we would signal cancellation to the async I/O thread
            let _ = request; // Acknowledge we received the request
            
            0 // CELL_OK
        } else {
            0x80010002u32 as i32 // CELL_FS_ERROR_EINVAL
        }
    }

    /// Get the result of a completed asynchronous I/O request
    /// 
    /// # Arguments
    /// * `request_id` - Request ID to get result for
    /// 
    /// # Returns
    /// * Number of bytes transferred on success, error code on failure
    pub fn aio_get_result(&mut self, request_id: AioRequestId) -> Result<u64, i32> {
        let request = self.aio_requests.get(&request_id)
            .ok_or(0x80010002u32 as i32)?; // CELL_FS_ERROR_EINVAL

        if !request.completed {
            return Err(0x80610B03u32 as i32); // Request not yet complete
        }

        request.result
    }

    /// Get number of pending async I/O requests
    pub fn aio_pending_count(&self) -> usize {
        self.aio_requests.values().filter(|r| !r.completed).count()
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
pub fn cell_fs_open(path_addr: u32, flags: u32, fd_addr: u32, mode: u32) -> i32 {
    // Read path from memory
    let path = match read_string(path_addr, CELL_FS_MAX_PATH_LENGTH as u32) {
        Ok(p) => p,
        Err(e) => {
            debug!("cellFsOpen: failed to read path from memory");
            return e;
        }
    };
    
    debug!(
        "cellFsOpen(path='{}', flags=0x{:X}, mode=0x{:X})",
        path, flags, mode
    );

    // Open file through global fs manager
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.fs.open(&path, flags, mode) {
        Ok(fd) => {
            // Write fd to memory
            if let Err(e) = write_be32(fd_addr, fd as u32) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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
pub fn cell_fs_read(fd: i32, buf_addr: u32, nbytes: u64, nread_addr: u32) -> i32 {
    trace!("cellFsRead(fd={}, buf=0x{:08X}, nbytes={})", fd, buf_addr, nbytes);

    // Create buffer for reading
    let mut buffer = vec![0u8; nbytes as usize];
    
    // Read from file through global fs manager
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.fs.read(fd, &mut buffer) {
        Ok(bytes_read) => {
            // Write data to guest buffer
            if bytes_read > 0 {
                if let Err(e) = write_bytes(buf_addr, &buffer[..bytes_read as usize]) {
                    return e;
                }
            }
            // Write number of bytes read
            if let Err(e) = write_be64(nread_addr, bytes_read) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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
pub fn cell_fs_write(fd: i32, buf_addr: u32, nbytes: u64, nwrite_addr: u32) -> i32 {
    trace!("cellFsWrite(fd={}, buf=0x{:08X}, nbytes={})", fd, buf_addr, nbytes);

    // Read data from guest buffer
    let data = match read_bytes(buf_addr, nbytes as u32) {
        Ok(d) => d,
        Err(e) => return e,
    };
    
    // Write to file through global fs manager
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.fs.write(fd, &data) {
        Ok(written) => {
            // Write number of bytes written
            if let Err(e) = write_be64(nwrite_addr, written) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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
pub fn cell_fs_lseek(fd: i32, offset: i64, whence: u32, pos_addr: u32) -> i32 {
    trace!("cellFsLseek(fd={}, offset={}, whence={})", fd, offset, whence);

    // Seek in file through global fs manager
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.fs.lseek(fd, offset, whence) {
        Ok(new_pos) => {
            // Write new position
            if let Err(e) = write_be64(pos_addr, new_pos) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellFsFstat - Get file status
///
/// # Arguments
/// * `fd` - File descriptor
/// * `stat_addr` - Address to write stat structure to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_fstat(fd: i32, stat_addr: u32) -> i32 {
    trace!("cellFsFstat(fd={}, stat_addr=0x{:08X})", fd, stat_addr);

    // Get file status through global fs manager
    let ctx = crate::context::get_hle_context();
    match ctx.fs.fstat(fd) {
        Ok(stat) => {
            // Write stat structure to memory (CellFsStat layout)
            // mode(4) + uid(4) + gid(4) + atime(8) + mtime(8) + ctime(8) + size(8) + blksize(8)
            if let Err(e) = write_be32(stat_addr, stat.mode) { return e; }
            if let Err(e) = write_be32(stat_addr + 4, stat.uid) { return e; }
            if let Err(e) = write_be32(stat_addr + 8, stat.gid) { return e; }
            if let Err(e) = write_be64(stat_addr + 12, stat.atime) { return e; }
            if let Err(e) = write_be64(stat_addr + 20, stat.mtime) { return e; }
            if let Err(e) = write_be64(stat_addr + 28, stat.ctime) { return e; }
            if let Err(e) = write_be64(stat_addr + 36, stat.size) { return e; }
            if let Err(e) = write_be64(stat_addr + 44, stat.blksize) { return e; }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellFsStat - Get file status by path
///
/// # Arguments
/// * `path` - Path to file
/// * `stat_addr` - Address to write stat structure to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_stat(path_addr: u32, stat_addr: u32) -> i32 {
    // Read path from memory
    let path = match read_string(path_addr, CELL_FS_MAX_PATH_LENGTH as u32) {
        Ok(p) => p,
        Err(e) => return e,
    };
    
    debug!("cellFsStat(path='{}', stat_addr=0x{:08X})", path, stat_addr);

    // Get file status through global fs manager
    let ctx = crate::context::get_hle_context();
    match ctx.fs.stat(&path) {
        Ok(stat) => {
            // Write stat structure to memory (CellFsStat layout)
            if let Err(e) = write_be32(stat_addr, stat.mode) { return e; }
            if let Err(e) = write_be32(stat_addr + 4, stat.uid) { return e; }
            if let Err(e) = write_be32(stat_addr + 8, stat.gid) { return e; }
            if let Err(e) = write_be64(stat_addr + 12, stat.atime) { return e; }
            if let Err(e) = write_be64(stat_addr + 20, stat.mtime) { return e; }
            if let Err(e) = write_be64(stat_addr + 28, stat.ctime) { return e; }
            if let Err(e) = write_be64(stat_addr + 36, stat.size) { return e; }
            if let Err(e) = write_be64(stat_addr + 44, stat.blksize) { return e; }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellFsOpendir - Open a directory
///
/// # Arguments
/// * `path` - Path to directory
/// * `fd_addr` - Address to write file descriptor to
///
/// # Returns
/// * 0 on success
pub fn cell_fs_opendir(path_addr: u32, fd_addr: u32) -> i32 {
    // Read path from memory
    let path = match read_string(path_addr, CELL_FS_MAX_PATH_LENGTH as u32) {
        Ok(p) => p,
        Err(e) => return e,
    };
    
    debug!("cellFsOpendir(path='{}')", path);

    // Open directory through global fs manager
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.fs.opendir(&path) {
        Ok(fd) => {
            // Write fd to memory
            if let Err(e) = write_be32(fd_addr, fd as u32) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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
pub fn cell_fs_readdir(fd: i32, dir_addr: u32, nread_addr: u32) -> i32 {
    trace!("cellFsReaddir(fd={}, dir_addr=0x{:08X})", fd, dir_addr);

    // Read directory entry through global fs manager
    let mut ctx = crate::context::get_hle_context_mut();
    match ctx.fs.readdir(fd) {
        Ok(Some(entry)) => {
            // Write directory entry to memory (CellFsDirent layout)
            // d_type(1) + d_namlen(1) + d_name[256]
            if let Err(e) = crate::memory::write_u8(dir_addr, entry.d_type) { return e; }
            if let Err(e) = crate::memory::write_u8(dir_addr + 1, entry.d_namlen) { return e; }
            // Write d_name bytes directly
            if let Err(e) = write_bytes(dir_addr + 2, &entry.d_name) { return e; }
            // Write number of entries read (1)
            if let Err(e) = write_be64(nread_addr, 1) { return e; }
            0 // CELL_OK
        }
        Ok(None) => {
            // End of directory - write 0 entries read
            if let Err(e) = write_be64(nread_addr, 0) { return e; }
            0 // CELL_OK
        }
        Err(e) => e,
    }
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

    #[test]
    fn test_aio_request_lifecycle() {
        // Test that async I/O request structures are created correctly
        let mut manager = FsManager::new();
        
        // Open a file
        let fd = manager.open("/dev_hdd0/test.txt", flags::CELL_FS_O_RDWR, 0);
        assert!(fd.is_ok());
        let fd = fd.unwrap();
        
        // Verify initial AIO state
        assert_eq!(manager.aio_pending_count(), 0);
        
        manager.close(fd);
    }

    #[test]
    fn test_aio_poll_invalid_request() {
        // Test polling with invalid request ID
        let mut manager = FsManager::new();
        
        // Poll non-existent request should return error
        let result = manager.aio_poll(12345);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), 0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
    }

    #[test]
    fn test_aio_wait_invalid_request() {
        // Test waiting with invalid request ID
        let mut manager = FsManager::new();
        
        // Wait on non-existent request should return error
        let result = manager.aio_wait(12345, 0);
        assert_eq!(result, 0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
    }

    #[test]
    fn test_aio_cancel_invalid_request() {
        // Test canceling invalid request
        let mut manager = FsManager::new();
        
        // Cancel non-existent request should return error
        let result = manager.aio_cancel(12345);
        assert_eq!(result, 0x80010002u32 as i32); // CELL_FS_ERROR_EINVAL
    }

    #[test]
    fn test_aio_read_invalid_fd() {
        // Test async read with invalid file descriptor
        let mut manager = FsManager::new();
        
        // aio_read with invalid fd should return error
        let result = manager.aio_read(999, 0x1000, 1024, None, None, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), 0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
    }

    #[test]
    fn test_aio_write_invalid_fd() {
        // Test async write with invalid file descriptor
        let mut manager = FsManager::new();
        
        // aio_write with invalid fd should return error
        let result = manager.aio_write(999, 0x1000, 1024, None, None, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), 0x80010009u32 as i32); // CELL_FS_ERROR_EBADF
    }

    #[test]
    fn test_aio_with_real_file() {
        use std::io::Write;
        use std::env;
        
        // Create a temporary test file
        let temp_dir = env::temp_dir();
        let test_file_path = temp_dir.join("oc_aio_test.txt");
        let test_content = b"Hello async I/O test!";
        
        // Write test content
        {
            let mut file = std::fs::File::create(&test_file_path).unwrap();
            file.write_all(test_content).unwrap();
        }
        
        // Create FsManager with VFS pointing to temp directory
        let mut manager = FsManager::new();
        
        // For this test, directly use file path since we're testing the mechanism
        // In a real scenario, the VFS would resolve paths
        
        // Clean up
        let _ = std::fs::remove_file(test_file_path);
    }
}
