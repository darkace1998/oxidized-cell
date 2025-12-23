//! Error types for the oxidized-cell emulator

use thiserror::Error;

/// Main error type for the emulator
#[derive(Error, Debug)]
pub enum EmulatorError {
    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),

    #[error("PPU error: {0}")]
    Ppu(#[from] PpuError),

    #[error("SPU error: {0}")]
    Spu(#[from] SpuError),

    #[error("RSX error: {0}")]
    Rsx(#[from] RsxError),

    #[error("Kernel error: {0}")]
    Kernel(#[from] KernelError),

    #[error("Loader error: {0}")]
    Loader(#[from] LoaderError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Game not found: {0}")]
    GameNotFound(String),

    #[error("Unsupported feature: {0}")]
    Unsupported(String),
}

/// Memory-related errors
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Out of memory")]
    OutOfMemory,

    #[error("Invalid address: 0x{0:08x}")]
    InvalidAddress(u32),

    #[error("Access violation at 0x{addr:08x}: {kind}")]
    AccessViolation { addr: u32, kind: AccessKind },

    #[error("Reservation conflict at 0x{0:08x}")]
    ReservationConflict(u32),

    #[error("Alignment error: address 0x{addr:08x} not aligned to {align}")]
    AlignmentError { addr: u32, align: u32 },
}

/// PPU (PowerPC Processing Unit) errors
#[derive(Error, Debug)]
pub enum PpuError {
    #[error("Invalid instruction at 0x{addr:08x}: 0x{opcode:08x}")]
    InvalidInstruction { addr: u32, opcode: u32 },

    #[error("Syscall failed: {0}")]
    SyscallFailed(i32),

    #[error("Thread error: {0}")]
    ThreadError(String),

    #[error("Memory error at 0x{addr:08x}: {message}")]
    MemoryError { addr: u32, message: String },

    #[error("Trap at 0x{addr:08x}")]
    Trap { addr: u32 },
}

/// SPU (Synergistic Processing Unit) errors
#[derive(Error, Debug)]
pub enum SpuError {
    #[error("Invalid instruction at 0x{addr:05x}: 0x{opcode:08x}")]
    InvalidInstruction { addr: u32, opcode: u32 },

    #[error("MFC error: {0}")]
    MfcError(String),

    #[error("Channel timeout: {0}")]
    ChannelTimeout(u32),

    #[error("Invalid SPU ID: {0}")]
    InvalidSpuId(u32),
}

/// RSX (Reality Synthesizer) graphics errors
#[derive(Error, Debug)]
pub enum RsxError {
    #[error("Invalid command: 0x{0:08x}")]
    InvalidCommand(u32),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),

    #[error("Vulkan error: {0}")]
    Vulkan(String),

    #[error("Surface error: {0}")]
    Surface(String),
}

/// Kernel (LV2) errors
#[derive(Error, Debug)]
pub enum KernelError {
    #[error("Unknown syscall: {0}")]
    UnknownSyscall(u64),

    #[error("Invalid ID: {0}")]
    InvalidId(u32),

    #[error("Resource limit exceeded")]
    ResourceLimit,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Would block")]
    WouldBlock,

    #[error("Timeout")]
    Timeout,
}

/// Loader errors
#[derive(Error, Debug)]
pub enum LoaderError {
    #[error("Invalid ELF: {0}")]
    InvalidElf(String),

    #[error("Invalid SELF: {0}")]
    InvalidSelf(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Missing PRX: {0}")]
    MissingPrx(String),
}

/// Kind of memory access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
    Execute,
}

impl std::fmt::Display for AccessKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Execute => write!(f, "execute"),
        }
    }
}

/// Result type alias for emulator operations
pub type Result<T> = std::result::Result<T, EmulatorError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MemoryError::InvalidAddress(0x12345678);
        assert_eq!(format!("{}", err), "Invalid address: 0x12345678");

        let err = MemoryError::AccessViolation {
            addr: 0xDEADBEEF,
            kind: AccessKind::Write,
        };
        assert_eq!(
            format!("{}", err),
            "Access violation at 0xdeadbeef: write"
        );
    }

    #[test]
    fn test_error_conversion() {
        let mem_err = MemoryError::OutOfMemory;
        let emu_err: EmulatorError = mem_err.into();
        assert!(matches!(emu_err, EmulatorError::Memory(_)));
    }
}
