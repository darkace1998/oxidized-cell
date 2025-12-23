//! Process management (sys_process_*)

use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// Process ID type
pub type ProcessId = u32;

/// Process state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Suspended,
    Terminated,
}

/// Process information
pub struct ProcessInfo {
    pid: ProcessId,
    inner: Mutex<ProcessInfoInner>,
}

#[derive(Debug)]
struct ProcessInfoInner {
    state: ProcessState,
    sdk_version: u32,
    parent_pid: ProcessId,
}

impl ProcessInfo {
    pub fn new(pid: ProcessId, sdk_version: u32) -> Self {
        Self {
            pid,
            inner: Mutex::new(ProcessInfoInner {
                state: ProcessState::Running,
                sdk_version,
                parent_pid: 0,
            }),
        }
    }

    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    pub fn state(&self) -> ProcessState {
        self.inner.lock().state
    }

    pub fn set_state(&self, state: ProcessState) {
        self.inner.lock().state = state;
    }

    pub fn sdk_version(&self) -> u32 {
        self.inner.lock().sdk_version
    }
}

/// Process manager
pub struct ProcessManager {
    current_pid: AtomicU32,
    process: Mutex<Option<ProcessInfo>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        // Initialize with main process
        let main_process = ProcessInfo::new(1, 0x00360001); // SDK 3.60

        Self {
            current_pid: AtomicU32::new(1),
            process: Mutex::new(Some(main_process)),
        }
    }

    pub fn current_pid(&self) -> ProcessId {
        self.current_pid.load(Ordering::Relaxed)
    }

    pub fn get_process(&self) -> Option<ProcessId> {
        self.process.lock().as_ref().map(|p| p.pid())
    }

    pub fn get_sdk_version(&self) -> u32 {
        self.process
            .lock()
            .as_ref()
            .map(|p| p.sdk_version())
            .unwrap_or(0x00360001)
    }

    pub fn exit(&self, exit_code: i32) -> Result<(), KernelError> {
        if let Some(process) = self.process.lock().as_ref() {
            process.set_state(ProcessState::Terminated);
            tracing::info!("Process {} exited with code {}", process.pid(), exit_code);
        }
        Ok(())
    }

    pub fn get_state(&self) -> ProcessState {
        self.process
            .lock()
            .as_ref()
            .map(|p| p.state())
            .unwrap_or(ProcessState::Terminated)
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Process syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_process_getpid
    pub fn sys_process_getpid(manager: &ProcessManager) -> ProcessId {
        manager.current_pid()
    }

    /// sys_process_getppid
    pub fn sys_process_getppid(_manager: &ProcessManager) -> ProcessId {
        // In PS3, this typically returns the parent process ID
        // For simplicity, we return 0 (no parent)
        0
    }

    /// sys_process_exit
    pub fn sys_process_exit(manager: &ProcessManager, exit_code: i32) -> Result<(), KernelError> {
        manager.exit(exit_code)
    }

    /// sys_process_get_sdk_version
    pub fn sys_process_get_sdk_version(
        manager: &ProcessManager,
        pid: ProcessId,
    ) -> Result<u32, KernelError> {
        if pid == 0 || pid == manager.current_pid() {
            Ok(manager.get_sdk_version())
        } else {
            Err(KernelError::InvalidId(pid))
        }
    }

    /// sys_process_get_status
    pub fn sys_process_get_status(
        manager: &ProcessManager,
        pid: ProcessId,
    ) -> Result<ProcessState, KernelError> {
        if pid == 0 || pid == manager.current_pid() {
            Ok(manager.get_state())
        } else {
            Err(KernelError::InvalidId(pid))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_manager() {
        let manager = ProcessManager::new();

        let pid = syscalls::sys_process_getpid(&manager);
        assert_eq!(pid, 1);

        let sdk_version = syscalls::sys_process_get_sdk_version(&manager, pid).unwrap();
        assert_eq!(sdk_version, 0x00360001);

        let state = syscalls::sys_process_get_status(&manager, pid).unwrap();
        assert_eq!(state, ProcessState::Running);
    }

    #[test]
    fn test_process_exit() {
        let manager = ProcessManager::new();

        let pid = syscalls::sys_process_getpid(&manager);
        assert_eq!(manager.get_state(), ProcessState::Running);

        syscalls::sys_process_exit(&manager, 0).unwrap();
        assert_eq!(manager.get_state(), ProcessState::Terminated);
    }
}

