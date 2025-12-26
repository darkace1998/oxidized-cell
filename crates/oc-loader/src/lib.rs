//! ELF/SELF loader for oxidized-cell

pub mod crypto;
pub mod elf;
pub mod firmware;
pub mod pkg;
pub mod prx;
pub mod self_file;

// Re-export main types
pub use elf::{ElfLoader, Elf64Header, Elf64Phdr, Elf64Shdr, Symbol};
pub use self_file::{SelfLoader, SelfHeader, AppInfo};
pub use prx::{PrxLoader, PrxModule, PrxExport, PrxImport, ExportType, ImportType};
pub use crypto::{CryptoEngine, KeyType, KeyEntry};
pub use firmware::{PupLoader, PupHeader, PupEntryId, FirmwareVersion, FirmwareFile};
pub use pkg::{PkgLoader, PkgHeader, PkgType, PkgFileEntry, PkgMetadataEntry};
