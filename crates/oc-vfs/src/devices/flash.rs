//! Flash device (/dev_flash)
//!
//! Handles the PS3 firmware flash storage

use std::path::PathBuf;

/// Flash device regions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashRegion {
    /// System firmware
    Flash,
    /// Flash2 region
    Flash2,
    /// Flash3 region
    Flash3,
}

impl FlashRegion {
    pub fn mount_point(&self) -> &'static str {
        match self {
            FlashRegion::Flash => "/dev_flash",
            FlashRegion::Flash2 => "/dev_flash2",
            FlashRegion::Flash3 => "/dev_flash3",
        }
    }
}

/// Flash device
pub struct FlashDevice {
    /// Flash region
    pub region: FlashRegion,
    /// Host path where the flash is stored
    pub host_path: PathBuf,
    /// Read-only flag
    pub read_only: bool,
}

impl FlashDevice {
    /// Create a new flash device
    pub fn new(region: FlashRegion, host_path: PathBuf) -> Self {
        Self {
            region,
            host_path,
            read_only: true, // Flash is typically read-only
        }
    }

    /// Create a main flash device
    pub fn new_flash(host_path: PathBuf) -> Self {
        Self::new(FlashRegion::Flash, host_path)
    }

    /// Create a flash2 device
    pub fn new_flash2(host_path: PathBuf) -> Self {
        Self::new(FlashRegion::Flash2, host_path)
    }

    /// Create a flash3 device
    pub fn new_flash3(host_path: PathBuf) -> Self {
        Self::new(FlashRegion::Flash3, host_path)
    }

    /// Get the mount point
    pub fn mount_point(&self) -> &'static str {
        self.region.mount_point()
    }

    /// Check if the device is mounted
    pub fn is_mounted(&self) -> bool {
        self.host_path.exists()
    }

    /// Resolve a virtual path to a host path
    pub fn resolve_path(&self, virtual_path: &str) -> PathBuf {
        let mount_point = self.mount_point();
        let relative = virtual_path
            .strip_prefix(mount_point)
            .unwrap_or(virtual_path)
            .trim_start_matches('/');

        self.host_path.join(relative)
    }

    /// Initialize common flash directories
    pub fn init_directories(&self) -> std::io::Result<()> {
        if self.region == FlashRegion::Flash {
            // Create common PS3 flash directories
            let dirs = [
                "vsh",
                "vsh/module",
                "sys",
                "sys/external",
            ];

            for dir in &dirs {
                let path = self.host_path.join(dir);
                if !path.exists() {
                    std::fs::create_dir_all(&path)?;
                    tracing::debug!("Created flash directory: {:?}", path);
                }
            }
        }

        Ok(())
    }

    /// Set read-only mode
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Check if read-only
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_device_creation() {
        let flash = FlashDevice::new_flash(PathBuf::from("/tmp/dev_flash"));
        assert_eq!(flash.region, FlashRegion::Flash);
        assert_eq!(flash.mount_point(), "/dev_flash");
        assert!(flash.is_read_only());
    }

    #[test]
    fn test_flash_regions() {
        assert_eq!(FlashRegion::Flash.mount_point(), "/dev_flash");
        assert_eq!(FlashRegion::Flash2.mount_point(), "/dev_flash2");
        assert_eq!(FlashRegion::Flash3.mount_point(), "/dev_flash3");
    }

    #[test]
    fn test_resolve_path() {
        let flash = FlashDevice::new_flash(PathBuf::from("/tmp/dev_flash"));
        let resolved = flash.resolve_path("/dev_flash/vsh/module/vsh.self");

        assert!(resolved.to_string_lossy().contains("dev_flash"));
        assert!(resolved.to_string_lossy().contains("vsh"));
        assert!(resolved.to_string_lossy().contains("module"));
    }

    #[test]
    fn test_read_only() {
        let mut flash = FlashDevice::new_flash(PathBuf::from("/tmp/dev_flash"));
        assert!(flash.is_read_only());
        
        flash.set_read_only(false);
        assert!(!flash.is_read_only());
    }
}
