//! Blu-ray disc device (/dev_bdvd)
//!
//! Handles mounting and accessing Blu-ray disc images

use std::path::PathBuf;

/// Blu-ray disc device
pub struct BdvdDevice {
    /// Path to ISO image or directory
    pub image_path: Option<PathBuf>,
    /// Whether a disc is mounted
    pub mounted: bool,
}

impl BdvdDevice {
    /// Create a new BDVD device
    pub fn new() -> Self {
        Self {
            image_path: None,
            mounted: false,
        }
    }

    /// Mount a disc image
    pub fn mount(&mut self, image_path: PathBuf) -> Result<(), String> {
        if !image_path.exists() {
            return Err(format!("Image path does not exist: {:?}", image_path));
        }

        self.image_path = Some(image_path);
        self.mounted = true;
        tracing::info!("Blu-ray disc mounted: {:?}", self.image_path);
        
        Ok(())
    }

    /// Unmount the disc
    pub fn unmount(&mut self) {
        self.image_path = None;
        self.mounted = false;
        tracing::info!("Blu-ray disc unmounted");
    }

    /// Check if a disc is mounted
    pub fn is_mounted(&self) -> bool {
        self.mounted && self.image_path.is_some()
    }

    /// Get the mount point
    pub fn mount_point(&self) -> &'static str {
        "/dev_bdvd"
    }

    /// Get the image path
    pub fn image_path(&self) -> Option<&PathBuf> {
        self.image_path.as_ref()
    }

    /// Resolve a virtual path to the image
    pub fn resolve_path(&self, virtual_path: &str) -> Option<PathBuf> {
        if !self.is_mounted() {
            return None;
        }

        let image_path = self.image_path.as_ref()?;
        let relative = virtual_path
            .strip_prefix(self.mount_point())
            .unwrap_or(virtual_path)
            .trim_start_matches('/');

        Some(image_path.join(relative))
    }
}

impl Default for BdvdDevice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bdvd_device_creation() {
        let bdvd = BdvdDevice::new();
        assert!(!bdvd.is_mounted());
        assert_eq!(bdvd.mount_point(), "/dev_bdvd");
    }

    #[test]
    fn test_bdvd_unmount() {
        let mut bdvd = BdvdDevice::new();
        bdvd.unmount();
        assert!(!bdvd.is_mounted());
    }

    #[test]
    fn test_resolve_path() {
        let bdvd = BdvdDevice::new();
        
        // Without mounting, should return None
        assert!(bdvd.resolve_path("/dev_bdvd/PS3_GAME/USRDIR/EBOOT.BIN").is_none());
    }
}
