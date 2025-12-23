//! Hard disk device implementations
//!
//! Handles /dev_hdd0 (internal HDD) and /dev_hdd1 (removable HDD)

use std::path::PathBuf;

/// HDD device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HddType {
    /// Internal HDD (/dev_hdd0)
    Internal,
    /// Removable HDD (/dev_hdd1)
    Removable,
}

impl HddType {
    pub fn mount_point(&self) -> &'static str {
        match self {
            HddType::Internal => "/dev_hdd0",
            HddType::Removable => "/dev_hdd1",
        }
    }
}

/// Hard disk device
pub struct HddDevice {
    /// Device type
    pub device_type: HddType,
    /// Host path where the HDD is mounted
    pub host_path: PathBuf,
    /// Device capacity in bytes (optional)
    pub capacity: Option<u64>,
}

impl HddDevice {
    /// Create a new HDD device
    pub fn new(device_type: HddType, host_path: PathBuf) -> Self {
        Self {
            device_type,
            host_path,
            capacity: None,
        }
    }

    /// Create an internal HDD device
    pub fn new_internal(host_path: PathBuf) -> Self {
        Self::new(HddType::Internal, host_path)
    }

    /// Create a removable HDD device
    pub fn new_removable(host_path: PathBuf) -> Self {
        Self::new(HddType::Removable, host_path)
    }

    /// Get the mount point
    pub fn mount_point(&self) -> &'static str {
        self.device_type.mount_point()
    }

    /// Check if the device is mounted
    pub fn is_mounted(&self) -> bool {
        self.host_path.exists()
    }

    /// Get device capacity (if available)
    pub fn capacity(&self) -> Option<u64> {
        self.capacity
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

    /// Initialize common directories on the device
    pub fn init_directories(&self) -> std::io::Result<()> {
        if self.device_type == HddType::Internal {
            // Create common PS3 directories
            let dirs = [
                "game",
                "savedata",
                "photo",
                "music",
                "video",
                "tmp",
            ];
            
            for dir in &dirs {
                let path = self.host_path.join(dir);
                if !path.exists() {
                    std::fs::create_dir_all(&path)?;
                    tracing::debug!("Created directory: {:?}", path);
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdd_device_creation() {
        let hdd = HddDevice::new_internal(PathBuf::from("/tmp/dev_hdd0"));
        assert_eq!(hdd.device_type, HddType::Internal);
        assert_eq!(hdd.mount_point(), "/dev_hdd0");
    }

    #[test]
    fn test_hdd_types() {
        assert_eq!(HddType::Internal.mount_point(), "/dev_hdd0");
        assert_eq!(HddType::Removable.mount_point(), "/dev_hdd1");
    }

    #[test]
    fn test_resolve_path() {
        let hdd = HddDevice::new_internal(PathBuf::from("/tmp/dev_hdd0"));
        let resolved = hdd.resolve_path("/dev_hdd0/game/test.elf");
        
        assert!(resolved.to_string_lossy().contains("dev_hdd0"));
        assert!(resolved.to_string_lossy().contains("game"));
        assert!(resolved.to_string_lossy().contains("test.elf"));
    }
}
