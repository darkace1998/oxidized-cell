//! ELF/SELF loader for oxidized-cell
//!
//! This crate handles loading PS3 executables:
//! - ELF files (plain executables)
//! - SELF files (encrypted executables)
//! - PRX files (loadable modules)
//! - PUP files (firmware updates)
//! - PKG files (game packages)
//!
//! ## Decryption Support
//!
//! To decrypt SELF files, you need to install the PS3 firmware:
//! 1. Download the official PS3 firmware (PUP file) from Sony
//! 2. Install it using `PupLoader` or place in the `dev_flash/` directory
//! 3. The crypto engine will extract necessary keys automatically
//!
//! Alternatively, you can provide a `keys.txt` file with decryption keys.

pub mod crypto;
pub mod elf;
pub mod firmware;
pub mod pkg;
pub mod prx;
pub mod self_file;

// Re-export main types
pub use elf::{ElfLoader, Elf64Header, Elf64Phdr, Elf64Shdr, Symbol};
pub use self_file::{SelfLoader, SelfHeader, AppInfo};
pub use prx::{PrxLoader, PrxModule, PrxExport, PrxImport, ExportType, ImportType, PrxLoadingStats, PrxDependency};
pub use crypto::{CryptoEngine, KeyType, KeyEntry, SelfKeySet, KeyStats};
pub use firmware::{PupLoader, PupHeader, PupEntryId, FirmwareVersion, FirmwareFile, FirmwareStatus, FirmwareModuleRegistry, FirmwareModuleInfo, FirmwareModuleStrategy}; 
pub use pkg::{PkgLoader, PkgHeader, PkgType, PkgFileEntry, PkgMetadataEntry};
