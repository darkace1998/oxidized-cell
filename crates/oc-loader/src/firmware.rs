//! PS3 Firmware file support
//!
//! This module provides parsing and handling of PS3 firmware files.

use oc_core::error::LoaderError;
use tracing::{debug, info, warn};

/// PUP (PlayStation Update Package) file magic
pub const PUP_MAGIC: [u8; 8] = [0x53, 0x43, 0x45, 0x55, 0x46, 0x00, 0x00, 0x00]; // "SCEUF\0\0\0"

/// PUP file header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PupHeader {
    pub magic: [u8; 8],
    pub package_version: u64,
    pub image_version: u64,
    pub file_count: u64,
    pub header_length: u64,
    pub data_length: u64,
}

/// PUP file entry
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PupFileEntry {
    pub entry_id: u64,
    pub data_offset: u64,
    pub data_length: u64,
    pub padding: u64,
}

/// PUP hash entry
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PupHashEntry {
    pub entry_id: u64,
    pub hash: [u8; 20],
    pub padding: [u8; 4],
}

/// Known PUP entry IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum PupEntryId {
    /// Update version information
    UpdateVersion = 0x100,
    /// LV0 (bootloader)
    Lv0 = 0x200,
    /// LV1 (hypervisor)
    Lv1 = 0x201,
    /// LV2 (kernel/lv2_kernel.self)
    Lv2Kernel = 0x202,
    /// VSH modules
    Vsh = 0x300,
    /// Core OS files
    CoreOs = 0x301,
    /// Package metadata
    PkgMetadata = 0x400,
    /// Unknown entry
    Unknown = 0xFFFF,
}

impl From<u64> for PupEntryId {
    fn from(value: u64) -> Self {
        match value {
            0x100 => Self::UpdateVersion,
            0x200 => Self::Lv0,
            0x201 => Self::Lv1,
            0x202 => Self::Lv2Kernel,
            0x300 => Self::Vsh,
            0x301 => Self::CoreOs,
            0x400 => Self::PkgMetadata,
            _ => Self::Unknown,
        }
    }
}

/// Firmware version information
#[derive(Debug, Clone)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u16,
    pub build: u32,
}

impl FirmwareVersion {
    /// Parse from a version string (e.g., "04.90")
    pub fn parse(version_str: &str) -> Option<Self> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            Some(Self {
                major,
                minor,
                patch: 0,
                build: 0,
            })
        } else {
            None
        }
    }

    /// Parse from a u64 version number
    pub fn from_u64(version: u64) -> Self {
        Self {
            major: ((version >> 56) & 0xFF) as u8,
            minor: ((version >> 48) & 0xFF) as u8,
            patch: ((version >> 32) & 0xFFFF) as u16,
            build: (version & 0xFFFFFFFF) as u32,
        }
    }

    /// Convert to display string
    pub fn to_string(&self) -> String {
        format!("{}.{:02}", self.major, self.minor)
    }
}

/// Firmware status for detecting and reporting firmware state
#[derive(Debug, Clone)]
pub struct FirmwareStatus {
    /// Whether firmware is installed
    pub installed: bool,
    /// Installed firmware version (if any)
    pub version: Option<FirmwareVersion>,
    /// Path to installed firmware
    pub path: Option<std::path::PathBuf>,
    /// List of missing components
    pub missing_components: Vec<String>,
    /// List of available components
    pub available_components: Vec<String>,
}

impl FirmwareStatus {
    /// Create a new empty firmware status
    pub fn new() -> Self {
        Self {
            installed: false,
            version: None,
            path: None,
            missing_components: Vec::new(),
            available_components: Vec::new(),
        }
    }

    /// Check firmware status at the given path
    pub fn check(firmware_path: &std::path::Path) -> Self {
        use std::fs;
        
        let mut status = Self::new();
        
        if !firmware_path.exists() {
            status.missing_components.push("Firmware directory not found".to_string());
            return status;
        }
        
        status.path = Some(firmware_path.to_path_buf());
        
        // Check for version.txt
        let version_file = firmware_path.join("version.txt");
        if let Ok(version_str) = fs::read_to_string(&version_file) {
            status.version = FirmwareVersion::parse(version_str.trim());
            status.available_components.push("version.txt".to_string());
        } else {
            status.missing_components.push("version.txt".to_string());
        }
        
        // Check for essential components
        let essential_files = [
            ("vsh/module/lv2_kernel.self", "LV2 Kernel"),
            ("vsh/module/vsh.self", "VSH Module"),
            ("sys/external", "External Libraries"),
        ];
        
        for (path, name) in &essential_files {
            let full_path = firmware_path.join(path);
            if full_path.exists() {
                status.available_components.push(name.to_string());
            } else {
                status.missing_components.push(name.to_string());
            }
        }
        
        // Firmware is considered installed if we have version and LV2 kernel
        status.installed = status.version.is_some() && 
            status.available_components.contains(&"LV2 Kernel".to_string());
        
        status
    }
    
    /// Get a user-friendly error message if firmware is not properly installed
    pub fn get_error_message(&self) -> Option<String> {
        if self.installed {
            return None;
        }
        
        let mut msg = String::from("PS3 firmware is not properly installed.\n\n");
        
        if self.missing_components.is_empty() {
            msg.push_str("The firmware directory exists but appears to be incomplete.\n");
        } else {
            msg.push_str("Missing components:\n");
            for component in &self.missing_components {
                msg.push_str(&format!("  - {}\n", component));
            }
        }
        
        msg.push_str("\nTo install firmware:\n");
        msg.push_str("1. Download the official PS3 firmware (PS3UPDAT.PUP) from playstation.com\n");
        msg.push_str("2. Place it in the 'firmware/' directory\n");
        msg.push_str("3. The emulator will automatically extract the necessary files\n\n");
        msg.push_str("Alternatively, you can extract the firmware manually and place it in 'dev_flash/'");
        
        Some(msg)
    }
}

impl Default for FirmwareStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Firmware file entry (extracted from PUP)
#[derive(Debug, Clone)]
pub struct FirmwareFile {
    pub id: PupEntryId,
    pub raw_id: u64,
    pub offset: u64,
    pub size: u64,
}

/// PUP file loader
pub struct PupLoader {
    /// Parsed entries
    entries: Vec<FirmwareFile>,
    /// Firmware version
    version: Option<FirmwareVersion>,
}

impl PupLoader {
    /// Create a new PUP loader
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            version: None,
        }
    }

    /// Check if data is a PUP file
    pub fn is_pup(data: &[u8]) -> bool {
        data.len() >= 8 && data[0..8] == PUP_MAGIC
    }

    /// Parse PUP header
    pub fn parse_header(data: &[u8]) -> Result<PupHeader, LoaderError> {
        if data.len() < 48 {
            return Err(LoaderError::InvalidPup("File too small".to_string()));
        }

        if !Self::is_pup(data) {
            return Err(LoaderError::InvalidPup("Invalid PUP magic".to_string()));
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&data[0..8]);

        let header = PupHeader {
            magic,
            package_version: u64::from_be_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15],
            ]),
            image_version: u64::from_be_bytes([
                data[16], data[17], data[18], data[19],
                data[20], data[21], data[22], data[23],
            ]),
            file_count: u64::from_be_bytes([
                data[24], data[25], data[26], data[27],
                data[28], data[29], data[30], data[31],
            ]),
            header_length: u64::from_be_bytes([
                data[32], data[33], data[34], data[35],
                data[36], data[37], data[38], data[39],
            ]),
            data_length: u64::from_be_bytes([
                data[40], data[41], data[42], data[43],
                data[44], data[45], data[46], data[47],
            ]),
        };

        info!(
            "PUP header: version=0x{:016x}, files={}, header_len=0x{:x}",
            header.image_version, header.file_count, header.header_length
        );

        Ok(header)
    }

    /// Parse PUP file entries
    pub fn parse(&mut self, data: &[u8]) -> Result<&[FirmwareFile], LoaderError> {
        let header = Self::parse_header(data)?;
        
        self.version = Some(FirmwareVersion::from_u64(header.image_version));
        self.entries.clear();

        // Parse file entries (after header)
        let entry_offset = 48usize;
        let entry_size = 32usize; // sizeof(PupFileEntry)

        for i in 0..header.file_count {
            let offset = entry_offset + (i as usize * entry_size);
            if data.len() < offset + entry_size {
                return Err(LoaderError::InvalidPup("Truncated file table".to_string()));
            }

            let entry_id = u64::from_be_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            let data_offset = u64::from_be_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
                data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
            ]);
            let data_length = u64::from_be_bytes([
                data[offset + 16], data[offset + 17], data[offset + 18], data[offset + 19],
                data[offset + 20], data[offset + 21], data[offset + 22], data[offset + 23],
            ]);

            debug!(
                "PUP entry {}: id=0x{:x}, offset=0x{:x}, size=0x{:x}",
                i, entry_id, data_offset, data_length
            );

            self.entries.push(FirmwareFile {
                id: PupEntryId::from(entry_id),
                raw_id: entry_id,
                offset: data_offset,
                size: data_length,
            });
        }

        Ok(&self.entries)
    }

    /// Get firmware version
    pub fn version(&self) -> Option<&FirmwareVersion> {
        self.version.as_ref()
    }

    /// Get all entries
    pub fn entries(&self) -> &[FirmwareFile] {
        &self.entries
    }

    /// Get entry by ID
    pub fn get_entry(&self, id: PupEntryId) -> Option<&FirmwareFile> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Extract a file from the PUP
    pub fn extract(&self, data: &[u8], entry: &FirmwareFile) -> Result<Vec<u8>, LoaderError> {
        let start = entry.offset as usize;
        let end = start + entry.size as usize;

        if data.len() < end {
            return Err(LoaderError::InvalidPup("Entry extends beyond file".to_string()));
        }

        Ok(data[start..end].to_vec())
    }

    /// Install firmware from a PUP file to a target directory
    ///
    /// This extracts the necessary files from the firmware update package
    /// and installs them to the specified directory (usually dev_flash/).
    pub fn install(
        &mut self,
        pup_data: &[u8],
        target_dir: &std::path::Path,
    ) -> Result<FirmwareVersion, LoaderError> {
        use std::fs;
        use std::io::Write;

        info!("Installing firmware to: {}", target_dir.display());

        // Parse the PUP file
        self.parse(pup_data)?;

        let version = self.version.clone()
            .ok_or_else(|| LoaderError::InvalidPup("Could not determine firmware version".to_string()))?;

        info!("Firmware version: {}", version.to_string());

        // Create target directory structure
        let dirs = [
            "vsh/module",
            "sys/external",
            "sys/internal",
        ];

        for dir in &dirs {
            let dir_path = target_dir.join(dir);
            fs::create_dir_all(&dir_path).map_err(|e| {
                LoaderError::InvalidPup(format!("Failed to create directory {}: {}", dir, e))
            })?;
        }

        // Extract important entries
        let mut extracted_count = 0u32;
        for entry in &self.entries {
            let extracted = self.extract(pup_data, entry)?;
            
            // Determine target path based on entry type
            let target_path = match entry.id {
                PupEntryId::Lv2Kernel => Some(target_dir.join("vsh/module/lv2_kernel.self")),
                PupEntryId::Vsh => Some(target_dir.join("vsh/module/vsh.self")),
                PupEntryId::Lv0 => Some(target_dir.join("sys/internal/lv0.self")),
                PupEntryId::Lv1 => Some(target_dir.join("sys/internal/lv1.self")),
                PupEntryId::CoreOs => Some(target_dir.join("sys/internal/core_os.self")),
                _ => {
                    // Save unknown entries by raw ID for potential future use
                    Some(target_dir.join(format!("sys/internal/entry_0x{:03x}.bin", entry.raw_id)))
                }
            };

            if let Some(path) = target_path {
                debug!("Extracting {:?} (id=0x{:03x}) to {}", entry.id, entry.raw_id, path.display());
                
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        LoaderError::InvalidPup(format!("Failed to create parent dir: {}", e))
                    })?;
                }

                let mut file = fs::File::create(&path).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to create file {}: {}", path.display(), e))
                })?;

                file.write_all(&extracted).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to write file {}: {}", path.display(), e))
                })?;
                
                extracted_count += 1;
            }
        }
        
        info!("Extracted {} firmware entries", extracted_count);

        // Write version info
        let version_file = target_dir.join("version.txt");
        fs::write(&version_file, version.to_string()).map_err(|e| {
            LoaderError::InvalidPup(format!("Failed to write version file: {}", e))
        })?;

        info!("Firmware {} installed successfully", version.to_string());
        Ok(version)
    }
    
    /// Auto-install firmware from a PUP file if found in common locations
    ///
    /// This looks for PS3UPDAT.PUP in common locations and automatically
    /// extracts it to the target directory.
    pub fn auto_install(target_dir: &std::path::Path) -> Result<Option<FirmwareVersion>, LoaderError> {
        use std::fs;
        
        // Check if firmware is already installed
        let status = FirmwareStatus::check(target_dir);
        if status.installed {
            info!("Firmware already installed: version {}", 
                status.version.as_ref().map(|v| v.to_string()).unwrap_or_default());
            return Ok(status.version);
        }
        
        // Look for PUP files in common locations
        let pup_search_paths = [
            "firmware/PS3UPDAT.PUP",
            "firmware/ps3updat.pup",
            "PS3UPDAT.PUP",
            "ps3updat.pup",
            "../firmware/PS3UPDAT.PUP",
        ];
        
        for pup_path in &pup_search_paths {
            let path = std::path::Path::new(pup_path);
            if path.exists() {
                info!("Found firmware update file: {}", pup_path);
                
                // Read the PUP file
                let pup_data = fs::read(path).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to read PUP file {}: {}", pup_path, e))
                })?;
                
                // Verify it's a valid PUP
                if !Self::is_pup(&pup_data) {
                    warn!("File {} is not a valid PUP file, skipping", pup_path);
                    continue;
                }
                
                // Create target directory if it doesn't exist
                fs::create_dir_all(target_dir).map_err(|e| {
                    LoaderError::InvalidPup(format!("Failed to create firmware directory: {}", e))
                })?;
                
                // Install the firmware
                let mut loader = Self::new();
                let version = loader.install(&pup_data, target_dir)?;
                
                info!("Automatically installed firmware version {}", version.to_string());
                return Ok(Some(version));
            }
        }
        
        // No PUP file found
        debug!("No firmware update file found in common locations");
        Ok(None)
    }
    
    /// Get a detailed error message for missing firmware
    pub fn get_missing_firmware_error() -> String {
        let mut msg = String::from(
            "PS3 firmware is required to run encrypted games (SELF/EBOOT.BIN files).\n\n"
        );
        
        msg.push_str("The emulator cannot decrypt PS3 executables without the official firmware.\n\n");
        
        msg.push_str("To install firmware:\n");
        msg.push_str("─────────────────────\n");
        msg.push_str("1. Download the official PS3 firmware from:\n");
        msg.push_str("   https://www.playstation.com/en-us/support/hardware/ps3/system-software/\n\n");
        msg.push_str("2. Place the downloaded file (PS3UPDAT.PUP) in one of these locations:\n");
        msg.push_str("   • firmware/PS3UPDAT.PUP (recommended)\n");
        msg.push_str("   • ./PS3UPDAT.PUP (current directory)\n\n");
        msg.push_str("3. Restart the emulator - firmware will be extracted automatically\n\n");
        
        msg.push_str("Alternative:\n");
        msg.push_str("────────────\n");
        msg.push_str("• Use decrypted game files (EBOOT.ELF instead of EBOOT.BIN)\n");
        msg.push_str("• These can be created with PS3 decryption tools\n");
        
        msg
    }
}

impl Default for PupLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// HLE handling strategy for a firmware PRX module
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareModuleStrategy {
    /// Fully handled by HLE — no native PRX needed
    Hle,
    /// Partially handled; some functions need native PRX
    HlePartial,
    /// Must be loaded natively from firmware
    Native,
    /// Not required for typical game execution
    Optional,
}

/// Firmware PRX module descriptor
#[derive(Debug, Clone)]
pub struct FirmwareModuleInfo {
    /// Module file name (e.g. "libsysutil_np.sprx")
    pub filename: &'static str,
    /// Logical module name used by HLE dispatcher (e.g. "cellSysutil")
    pub module_name: &'static str,
    /// Path inside dev_flash (e.g. "sys/external")
    pub firmware_path: &'static str,
    /// HLE handling strategy
    pub strategy: FirmwareModuleStrategy,
    /// Short description
    pub description: &'static str,
}

/// Registry that maps PS3 system PRX modules to their HLE counterparts.
///
/// When a game tries to load a system library (via `sys_prx_load_module`),
/// the registry is consulted:
///   - If the module is marked [`FirmwareModuleStrategy::Hle`], loading
///     succeeds immediately and all functions are handled by HLE stubs.
///   - If the module is [`FirmwareModuleStrategy::Native`], the loader
///     will attempt to read the actual `.sprx` from the extracted firmware.
///   - [`FirmwareModuleStrategy::HlePartial`] modules are HLE-handled but
///     may log warnings for unimplemented functions.
///   - [`FirmwareModuleStrategy::Optional`] modules return success but are
///     completely stubbed (no-ops).
pub struct FirmwareModuleRegistry {
    modules: Vec<FirmwareModuleInfo>,
}

impl FirmwareModuleRegistry {
    /// Create a new registry pre-populated with known PS3 system modules
    pub fn new() -> Self {
        let mut registry = Self {
            modules: Vec::new(),
        };
        registry.register_default_modules();
        registry
    }

    /// Register all known PS3 system PRX modules
    fn register_default_modules(&mut self) {
        use FirmwareModuleStrategy::*;

        // ── Core system libraries ──────────────────────────────────
        self.add("liblv2.sprx",           "liblv2",       "sys/external", Hle,        "LV2 kernel interface");
        self.add("libsysmodule.sprx",     "cellSysmodule","sys/external", Hle,        "System module loader");
        self.add("libsysutil.sprx",       "cellSysutil",  "sys/external", Hle,        "System utility functions");
        self.add("libsysutil_np.sprx",    "sceNp",        "sys/external", Optional,   "NP (PSN) utilities");
        self.add("libsysutil_np2.sprx",   "sceNp2",       "sys/external", Optional,   "NP v2 utilities");

        // ── Graphics ───────────────────────────────────────────────
        self.add("libgcm_sys.sprx",       "cellGcmSys",   "sys/external", Hle,        "GCM graphics interface");
        self.add("libresc.sprx",          "cellResc",     "sys/external", Hle,        "Resolution converter");

        // ── Audio ──────────────────────────────────────────────────
        self.add("libmixer.sprx",         "cellAudio",    "sys/external", Hle,        "Audio mixer");
        self.add("libatrac3plus.sprx",    "cellAtrac",    "sys/external", HlePartial, "ATRAC3+ decoder");
        self.add("libadec.sprx",          "cellAdec",     "sys/external", HlePartial, "Audio decoder");

        // ── Video ──────────────────────────────────────────────────
        self.add("libvdec.sprx",          "cellVdec",     "sys/external", HlePartial, "Video decoder");
        self.add("libdmux.sprx",          "cellDmux",     "sys/external", HlePartial, "Demultiplexer");
        self.add("libvpost.sprx",         "cellVpost",    "sys/external", HlePartial, "Video post-processor");
        self.add("libpamf.sprx",          "cellPamf",     "sys/external", HlePartial, "PAMF container");

        // ── Image decoding ─────────────────────────────────────────
        self.add("libpngdec.sprx",        "cellPngDec",   "sys/external", Hle,        "PNG decoder");
        self.add("libjpgdec.sprx",        "cellJpgDec",   "sys/external", Hle,        "JPEG decoder");
        self.add("libgifdec.sprx",        "cellGifDec",   "sys/external", Hle,        "GIF decoder");

        // ── Fonts ──────────────────────────────────────────────────
        self.add("libfont.sprx",          "cellFont",     "sys/external", Hle,        "Font library");
        self.add("libfontFT.sprx",        "cellFontFT",   "sys/external", Hle,        "FreeType font library");
        self.add("libfreetype.sprx",      "cellFreetype", "sys/external", Optional,   "FreeType engine");

        // ── Input ──────────────────────────────────────────────────
        self.add("libpad.sprx",           "cellPad",      "sys/external", Hle,        "Gamepad input");
        self.add("libkb.sprx",            "cellKb",       "sys/external", Hle,        "Keyboard input");
        self.add("libmouse.sprx",         "cellMouse",    "sys/external", Hle,        "Mouse input");
        self.add("libmic.sprx",           "cellMic",      "sys/external", Hle,        "Microphone input");

        // ── File system / storage ──────────────────────────────────
        self.add("libfs.sprx",            "cellFs",       "sys/external", Hle,        "File system");
        self.add("libsavedata.sprx",      "cellSaveData", "sys/external", Hle,        "Save data management");
        self.add("libgame.sprx",          "cellGame",     "sys/external", Hle,        "Game data management");

        // ── Networking ─────────────────────────────────────────────
        self.add("libnet.sprx",           "cellNet",      "sys/external", Optional,   "Network core");
        self.add("libnetctl.sprx",        "cellNetCtl",   "sys/external", Hle,        "Network control");
        self.add("libhttp.sprx",          "cellHttp",     "sys/external", Hle,        "HTTP client");
        self.add("libssl.sprx",           "cellSsl",      "sys/external", Hle,        "SSL/TLS");

        // ── SPU / SPURS ────────────────────────────────────────────
        self.add("libspurs_jq.sprx",      "cellSpursJq",  "sys/external", Hle,        "SPURS job queue");
        self.add("libsre.sprx",           "libsre",       "sys/external", Hle,        "SPU runtime extensions");

        // ── Miscellaneous ──────────────────────────────────────────
        self.add("libmsgdialog.sprx",     "cellMsgDialog","sys/external", Hle,        "Message dialog");
        self.add("libperf.sprx",          "cellPerf",     "sys/external", Optional,   "Performance profiler");
        self.add("libusbd.sprx",          "cellUsbd",     "sys/external", Optional,   "USB driver");
        self.add("libcamera.sprx",        "cellCamera",   "sys/external", Optional,   "Camera capture");
        self.add("libgem.sprx",           "cellGem",      "sys/external", Optional,   "PS Move support");

        debug!("Registered {} firmware module mappings", self.modules.len());
    }

    fn add(
        &mut self,
        filename: &'static str,
        module_name: &'static str,
        firmware_path: &'static str,
        strategy: FirmwareModuleStrategy,
        description: &'static str,
    ) {
        self.modules.push(FirmwareModuleInfo {
            filename,
            module_name,
            firmware_path,
            strategy,
            description,
        });
    }

    /// Look up a module by its PRX filename (e.g. "libsysutil.sprx")
    pub fn find_by_filename(&self, filename: &str) -> Option<&FirmwareModuleInfo> {
        // Normalise: strip leading path components if present
        let basename = filename.rsplit('/').next().unwrap_or(filename);
        self.modules.iter().find(|m| m.filename.eq_ignore_ascii_case(basename))
    }

    /// Look up a module by its HLE module name (e.g. "cellSysutil")
    pub fn find_by_module_name(&self, module_name: &str) -> Option<&FirmwareModuleInfo> {
        self.modules.iter().find(|m| m.module_name == module_name)
    }

    /// Check whether a given PRX filename can be handled by HLE
    /// (i.e. does not require native firmware loading)
    pub fn is_hle_handled(&self, filename: &str) -> bool {
        self.find_by_filename(filename)
            .map(|m| matches!(m.strategy, FirmwareModuleStrategy::Hle | FirmwareModuleStrategy::HlePartial | FirmwareModuleStrategy::Optional))
            .unwrap_or(false)
    }

    /// Get the HLE module name that handles a given PRX filename,
    /// or `None` if it must be loaded natively.
    pub fn get_hle_module_name(&self, filename: &str) -> Option<&'static str> {
        self.find_by_filename(filename)
            .filter(|m| m.strategy != FirmwareModuleStrategy::Native)
            .map(|m| m.module_name)
    }

    /// Resolve a `sys_prx_load_module` path to a handling decision.
    ///
    /// `prx_path` is the full PS3 VFS path such as
    /// `/dev_flash/sys/external/libsysutil.sprx`.
    ///
    /// Returns `Some(module_info)` if the module is known, `None` otherwise.
    pub fn resolve_prx_path(&self, prx_path: &str) -> Option<&FirmwareModuleInfo> {
        // Extract the basename from the full path
        let basename = prx_path.rsplit('/').next().unwrap_or(prx_path);
        self.find_by_filename(basename)
    }

    /// Get all registered modules
    pub fn modules(&self) -> &[FirmwareModuleInfo] {
        &self.modules
    }

    /// Get modules that require native firmware loading
    pub fn native_modules(&self) -> Vec<&FirmwareModuleInfo> {
        self.modules.iter()
            .filter(|m| m.strategy == FirmwareModuleStrategy::Native)
            .collect()
    }

    /// Get modules fully handled by HLE
    pub fn hle_modules(&self) -> Vec<&FirmwareModuleInfo> {
        self.modules.iter()
            .filter(|m| m.strategy == FirmwareModuleStrategy::Hle)
            .collect()
    }

    /// Check if all required firmware modules are available
    ///
    /// `firmware_dir` should point to the dev_flash root.
    /// Returns a list of modules that are needed natively but missing from disk.
    pub fn check_missing_native(&self, firmware_dir: &std::path::Path) -> Vec<&FirmwareModuleInfo> {
        self.modules.iter()
            .filter(|m| {
                if m.strategy != FirmwareModuleStrategy::Native {
                    return false;
                }
                let path = firmware_dir.join(m.firmware_path).join(m.filename);
                !path.exists()
            })
            .collect()
    }
}

impl Default for FirmwareModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pup_magic() {
        assert_eq!(PUP_MAGIC, [0x53, 0x43, 0x45, 0x55, 0x46, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_is_pup() {
        let pup_data = [0x53, 0x43, 0x45, 0x55, 0x46, 0x00, 0x00, 0x00, 0x00];
        assert!(PupLoader::is_pup(&pup_data));

        let not_pup = [0x00, 0x00, 0x00, 0x00];
        assert!(!PupLoader::is_pup(&not_pup));
    }

    #[test]
    fn test_firmware_version() {
        let version = FirmwareVersion::parse("04.90");
        assert!(version.is_some());
        let v = version.unwrap();
        assert_eq!(v.major, 4);
        assert_eq!(v.minor, 90);
    }

    #[test]
    fn test_entry_id_conversion() {
        assert_eq!(PupEntryId::from(0x100), PupEntryId::UpdateVersion);
        assert_eq!(PupEntryId::from(0x202), PupEntryId::Lv2Kernel);
        assert_eq!(PupEntryId::from(0x9999), PupEntryId::Unknown);
    }

    // =================================================================
    // FirmwareModuleRegistry tests
    // =================================================================

    #[test]
    fn test_firmware_module_registry_creation() {
        let registry = FirmwareModuleRegistry::new();
        assert!(!registry.modules().is_empty());
    }

    #[test]
    fn test_find_by_filename() {
        let registry = FirmwareModuleRegistry::new();
        
        let sysutil = registry.find_by_filename("libsysutil.sprx");
        assert!(sysutil.is_some());
        let info = sysutil.unwrap();
        assert_eq!(info.module_name, "cellSysutil");
        assert_eq!(info.strategy, FirmwareModuleStrategy::Hle);
    }

    #[test]
    fn test_find_by_filename_case_insensitive() {
        let registry = FirmwareModuleRegistry::new();
        assert!(registry.find_by_filename("LIBSYSUTIL.SPRX").is_some());
        assert!(registry.find_by_filename("LibSysUtil.sprx").is_some());
    }

    #[test]
    fn test_find_by_module_name() {
        let registry = FirmwareModuleRegistry::new();
        
        let gcm = registry.find_by_module_name("cellGcmSys");
        assert!(gcm.is_some());
        assert_eq!(gcm.unwrap().filename, "libgcm_sys.sprx");
    }

    #[test]
    fn test_is_hle_handled() {
        let registry = FirmwareModuleRegistry::new();
        
        // Core system modules should be HLE-handled
        assert!(registry.is_hle_handled("libsysutil.sprx"));
        assert!(registry.is_hle_handled("libgcm_sys.sprx"));
        assert!(registry.is_hle_handled("libpad.sprx"));
        assert!(registry.is_hle_handled("libfs.sprx"));
        
        // Optional modules should also be HLE-handled
        assert!(registry.is_hle_handled("libperf.sprx"));
        
        // Unknown module should not be HLE-handled
        assert!(!registry.is_hle_handled("unknown_module.sprx"));
    }

    #[test]
    fn test_get_hle_module_name() {
        let registry = FirmwareModuleRegistry::new();
        
        assert_eq!(registry.get_hle_module_name("libpad.sprx"), Some("cellPad"));
        assert_eq!(registry.get_hle_module_name("libhttp.sprx"), Some("cellHttp"));
        assert_eq!(registry.get_hle_module_name("unknown.sprx"), None);
    }

    #[test]
    fn test_resolve_prx_path() {
        let registry = FirmwareModuleRegistry::new();
        
        // Full path resolution
        let info = registry.resolve_prx_path("/dev_flash/sys/external/libsysutil.sprx");
        assert!(info.is_some());
        assert_eq!(info.unwrap().module_name, "cellSysutil");
        
        // Basename only
        let info = registry.resolve_prx_path("libnet.sprx");
        assert!(info.is_some());
    }

    #[test]
    fn test_hle_modules_list() {
        let registry = FirmwareModuleRegistry::new();
        let hle = registry.hle_modules();
        
        // Should have several HLE modules
        assert!(hle.len() >= 10);
        
        // All returned modules should be Hle strategy
        for m in &hle {
            assert_eq!(m.strategy, FirmwareModuleStrategy::Hle);
        }
    }

    #[test]
    fn test_firmware_module_registry_covers_all_hle_modules() {
        let registry = FirmwareModuleRegistry::new();
        
        // Verify key modules expected by games
        let required = [
            "cellSysutil", "cellGcmSys", "cellPad", "cellFs",
            "cellAudio", "cellGame", "cellSaveData",
            "cellFont", "cellPngDec", "cellJpgDec",
            "cellNetCtl", "cellHttp", "cellSsl",
            "cellVdec", "cellAdec", "cellDmux",
        ];
        
        for name in &required {
            assert!(
                registry.find_by_module_name(name).is_some(),
                "Missing firmware module mapping for {}",
                name
            );
        }
    }
}
