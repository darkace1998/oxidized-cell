//! USB device implementations (/dev_usb000, /dev_usb001, etc.)
//!
//! Handles USB storage devices

use std::path::PathBuf;

/// Maximum number of USB devices
pub const MAX_USB_DEVICES: usize = 8;

/// USB device
pub struct UsbDevice {
    /// Device number (0-7)
    pub device_num: u8,
    /// Host path where the USB device is mounted
    pub host_path: Option<PathBuf>,
    /// Whether the device is connected
    pub connected: bool,
}

impl UsbDevice {
    /// Create a new USB device
    pub fn new(device_num: u8) -> Self {
        Self {
            device_num,
            host_path: None,
            connected: false,
        }
    }

    /// Connect a USB device
    pub fn connect(&mut self, host_path: PathBuf) -> Result<(), String> {
        if self.connected {
            return Err("Device already connected".to_string());
        }

        if !host_path.exists() {
            return Err(format!("Host path does not exist: {:?}", host_path));
        }

        self.host_path = Some(host_path);
        self.connected = true;
        tracing::info!("USB device {} connected: {:?}", self.device_num, self.host_path);
        
        Ok(())
    }

    /// Disconnect the USB device
    pub fn disconnect(&mut self) {
        self.host_path = None;
        self.connected = false;
        tracing::info!("USB device {} disconnected", self.device_num);
    }

    /// Check if the device is connected
    pub fn is_connected(&self) -> bool {
        self.connected && self.host_path.is_some()
    }

    /// Get the mount point
    pub fn mount_point(&self) -> String {
        format!("/dev_usb{:03}", self.device_num)
    }

    /// Resolve a virtual path to a host path
    pub fn resolve_path(&self, virtual_path: &str) -> Option<PathBuf> {
        if !self.is_connected() {
            return None;
        }

        let host_path = self.host_path.as_ref()?;
        let mount_point = self.mount_point();
        let relative = virtual_path
            .strip_prefix(&mount_point)
            .unwrap_or(virtual_path)
            .trim_start_matches('/');

        Some(host_path.join(relative))
    }
}

/// USB device manager
pub struct UsbManager {
    devices: Vec<UsbDevice>,
}

impl UsbManager {
    /// Create a new USB manager
    pub fn new() -> Self {
        let devices = (0..MAX_USB_DEVICES as u8)
            .map(UsbDevice::new)
            .collect();

        Self { devices }
    }

    /// Get a USB device by number
    pub fn get_device(&self, device_num: u8) -> Option<&UsbDevice> {
        self.devices.get(device_num as usize)
    }

    /// Get a mutable USB device by number
    pub fn get_device_mut(&mut self, device_num: u8) -> Option<&mut UsbDevice> {
        self.devices.get_mut(device_num as usize)
    }

    /// Connect a USB device
    pub fn connect_device(&mut self, device_num: u8, host_path: PathBuf) -> Result<(), String> {
        let device = self.get_device_mut(device_num)
            .ok_or_else(|| format!("Invalid device number: {}", device_num))?;
        
        device.connect(host_path)
    }

    /// Disconnect a USB device
    pub fn disconnect_device(&mut self, device_num: u8) -> Result<(), String> {
        let device = self.get_device_mut(device_num)
            .ok_or_else(|| format!("Invalid device number: {}", device_num))?;
        
        device.disconnect();
        Ok(())
    }

    /// Get all connected devices
    pub fn connected_devices(&self) -> Vec<&UsbDevice> {
        self.devices.iter().filter(|d| d.is_connected()).collect()
    }
}

impl Default for UsbManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usb_device_creation() {
        let usb = UsbDevice::new(0);
        assert_eq!(usb.device_num, 0);
        assert!(!usb.is_connected());
        assert_eq!(usb.mount_point(), "/dev_usb000");
    }

    #[test]
    fn test_usb_manager() {
        let mut manager = UsbManager::new();
        assert_eq!(manager.connected_devices().len(), 0);
        
        // Disconnect should work even if not connected
        assert!(manager.disconnect_device(0).is_ok());
    }

    #[test]
    fn test_mount_points() {
        assert_eq!(UsbDevice::new(0).mount_point(), "/dev_usb000");
        assert_eq!(UsbDevice::new(1).mount_point(), "/dev_usb001");
        assert_eq!(UsbDevice::new(7).mount_point(), "/dev_usb007");
    }
}
