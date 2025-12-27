//! Bluetooth Controller Support
//!
//! Mock Bluetooth support for wireless controller pairing including:
//! - DualShock 3 Bluetooth pairing
//! - Generic Bluetooth HID devices
//! - Connection state management

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Bluetooth device class codes
pub mod device_class {
    /// Gamepad device class
    pub const GAMEPAD: u32 = 0x002508;
    /// Keyboard device class
    pub const KEYBOARD: u32 = 0x002540;
    /// Mouse device class
    pub const MOUSE: u32 = 0x002580;
    /// Audio device class
    pub const AUDIO: u32 = 0x200400;
}

/// Bluetooth address (BD_ADDR)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub struct BluetoothAddress([u8; 6]);

impl BluetoothAddress {
    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// Create from string "XX:XX:XX:XX:XX:XX"
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return None;
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).ok()?;
        }
        Some(Self(bytes))
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }

    /// Generate a random address (for mock devices)
    pub fn random() -> Self {
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        
        Self([
            (seed & 0xFF) as u8,
            ((seed >> 8) & 0xFF) as u8,
            ((seed >> 16) & 0xFF) as u8,
            ((seed >> 24) & 0xFF) as u8,
            ((seed >> 32) & 0xFF) as u8,
            ((seed >> 40) & 0xFF) as u8,
        ])
    }
}


/// Bluetooth connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothState {
    /// Not connected
    Disconnected,
    /// Scanning for devices
    Scanning,
    /// Pairing in progress
    Pairing,
    /// Connected and active
    Connected,
    /// Reconnecting
    Reconnecting,
}

/// Bluetooth device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothDeviceType {
    /// DualShock 3 controller
    DualShock3,
    /// DualShock 4 controller
    DualShock4,
    /// Generic HID gamepad
    GenericGamepad,
    /// Keyboard
    Keyboard,
    /// Mouse
    Mouse,
    /// Unknown device
    Unknown,
}

/// Bluetooth device info (discovered or paired)
#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    /// Bluetooth address
    pub address: BluetoothAddress,
    /// Device name
    pub name: String,
    /// Device class
    pub device_class: u32,
    /// Device type
    pub device_type: BluetoothDeviceType,
    /// Is paired
    pub paired: bool,
    /// Is trusted
    pub trusted: bool,
    /// Connection state
    pub state: BluetoothState,
    /// Signal strength (RSSI, 0-100)
    pub signal_strength: u8,
    /// Last seen timestamp
    pub last_seen: Instant,
    /// Link key (for paired devices)
    link_key: Option<[u8; 16]>,
}

impl BluetoothDevice {
    /// Create a new device
    pub fn new(address: BluetoothAddress, name: String, device_class: u32) -> Self {
        let device_type = Self::classify_device(device_class, &name);
        
        Self {
            address,
            name,
            device_class,
            device_type,
            paired: false,
            trusted: false,
            state: BluetoothState::Disconnected,
            signal_strength: 0,
            last_seen: Instant::now(),
            link_key: None,
        }
    }

    /// Classify device type from class and name
    fn classify_device(device_class: u32, name: &str) -> BluetoothDeviceType {
        let name_lower = name.to_lowercase();
        
        // Check name first for specific controllers
        if name_lower.contains("sixaxis") || name_lower.contains("playstation(r)3") {
            return BluetoothDeviceType::DualShock3;
        }
        if name_lower.contains("wireless controller") || name_lower.contains("dualshock 4") {
            return BluetoothDeviceType::DualShock4;
        }
        
        // Fall back to device class
        match device_class & 0x00FF00 {
            0x000500 => {
                // Peripheral major class
                match device_class & 0x0000C0 {
                    0x000040 => BluetoothDeviceType::Keyboard,
                    0x000080 => BluetoothDeviceType::Mouse,
                    0x0000C0 => BluetoothDeviceType::GenericGamepad, // Combo device
                    _ => {
                        if device_class & 0x000008 != 0 {
                            BluetoothDeviceType::GenericGamepad
                        } else {
                            BluetoothDeviceType::Unknown
                        }
                    }
                }
            }
            _ => BluetoothDeviceType::Unknown,
        }
    }

    /// Check if this is a game controller
    pub fn is_gamepad(&self) -> bool {
        matches!(
            self.device_type,
            BluetoothDeviceType::DualShock3 |
            BluetoothDeviceType::DualShock4 |
            BluetoothDeviceType::GenericGamepad
        )
    }
}

/// Pairing request info
#[derive(Debug, Clone)]
pub struct PairingRequest {
    /// Device being paired
    pub device: BluetoothDevice,
    /// PIN code (if required)
    pub pin_code: Option<String>,
    /// Pairing method
    pub method: PairingMethod,
    /// Request time
    pub requested_at: Instant,
    /// Timeout duration
    pub timeout: Duration,
}

/// Pairing method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingMethod {
    /// No authentication required (legacy)
    None,
    /// PIN code entry
    PinCode,
    /// Numeric comparison (SSP)
    NumericComparison,
    /// Just works (SSP)
    JustWorks,
    /// Passkey entry (SSP)
    PasskeyEntry,
}

/// Bluetooth adapter state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterState {
    /// Adapter not present or unavailable
    Unavailable,
    /// Adapter is off
    Off,
    /// Adapter is on and ready
    On,
    /// Adapter is in discoverable mode
    Discoverable,
}

/// Mock Bluetooth adapter
pub struct BluetoothAdapter {
    /// Adapter state
    pub state: AdapterState,
    /// Adapter address
    pub address: BluetoothAddress,
    /// Adapter name
    pub name: String,
    /// Paired devices
    paired_devices: HashMap<BluetoothAddress, BluetoothDevice>,
    /// Discovered devices (from scanning)
    discovered_devices: HashMap<BluetoothAddress, BluetoothDevice>,
    /// Active pairing request
    pairing_request: Option<PairingRequest>,
    /// Is scanning
    scanning: bool,
    /// Scan start time
    scan_start: Option<Instant>,
}

impl BluetoothAdapter {
    /// Create a new mock adapter
    pub fn new() -> Self {
        Self {
            state: AdapterState::On,
            address: BluetoothAddress::random(),
            name: "OxidizedCell".to_string(),
            paired_devices: HashMap::new(),
            discovered_devices: HashMap::new(),
            pairing_request: None,
            scanning: false,
            scan_start: None,
        }
    }

    /// Create an unavailable adapter
    pub fn unavailable() -> Self {
        let mut adapter = Self::new();
        adapter.state = AdapterState::Unavailable;
        adapter
    }

    /// Check if adapter is available
    pub fn is_available(&self) -> bool {
        self.state != AdapterState::Unavailable
    }

    /// Turn adapter on
    pub fn power_on(&mut self) -> bool {
        if self.state == AdapterState::Unavailable {
            return false;
        }
        self.state = AdapterState::On;
        tracing::info!("Bluetooth adapter powered on");
        true
    }

    /// Turn adapter off
    pub fn power_off(&mut self) {
        if self.state != AdapterState::Unavailable {
            self.state = AdapterState::Off;
            self.scanning = false;
            // Disconnect all devices
            for device in self.paired_devices.values_mut() {
                device.state = BluetoothState::Disconnected;
            }
            tracing::info!("Bluetooth adapter powered off");
        }
    }

    /// Start device discovery
    pub fn start_scan(&mut self) -> bool {
        if self.state != AdapterState::On && self.state != AdapterState::Discoverable {
            return false;
        }
        
        self.scanning = true;
        self.scan_start = Some(Instant::now());
        self.discovered_devices.clear();
        tracing::info!("Bluetooth scan started");
        true
    }

    /// Stop device discovery
    pub fn stop_scan(&mut self) {
        self.scanning = false;
        self.scan_start = None;
        tracing::info!("Bluetooth scan stopped");
    }

    /// Check if scanning
    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// Add a mock discovered device (for testing)
    pub fn add_mock_device(&mut self, device: BluetoothDevice) {
        self.discovered_devices.insert(device.address, device);
    }

    /// Get discovered devices
    pub fn get_discovered_devices(&self) -> Vec<&BluetoothDevice> {
        self.discovered_devices.values().collect()
    }

    /// Get paired devices
    pub fn get_paired_devices(&self) -> Vec<&BluetoothDevice> {
        self.paired_devices.values().collect()
    }

    /// Start pairing with a device
    pub fn pair(&mut self, address: BluetoothAddress) -> Result<(), &'static str> {
        if self.state != AdapterState::On && self.state != AdapterState::Discoverable {
            return Err("Adapter not ready");
        }
        
        if self.paired_devices.contains_key(&address) {
            return Err("Device already paired");
        }

        let device = self.discovered_devices.get(&address)
            .ok_or("Device not found")?
            .clone();

        // Determine pairing method
        let method = match device.device_type {
            BluetoothDeviceType::DualShock3 => PairingMethod::None, // DS3 uses link key from USB
            BluetoothDeviceType::DualShock4 => PairingMethod::JustWorks,
            _ => PairingMethod::JustWorks,
        };

        self.pairing_request = Some(PairingRequest {
            device,
            pin_code: None,
            method,
            requested_at: Instant::now(),
            timeout: Duration::from_secs(30),
        });

        tracing::info!("Started pairing with {}", address.to_string());
        Ok(())
    }

    /// Complete pairing (call after pair() to finalize)
    pub fn complete_pairing(&mut self) -> Result<BluetoothAddress, &'static str> {
        let request = self.pairing_request.take()
            .ok_or("No pairing in progress")?;

        // Check timeout
        if request.requested_at.elapsed() > request.timeout {
            return Err("Pairing timed out");
        }

        let mut device = request.device;
        device.paired = true;
        device.trusted = true;
        device.state = BluetoothState::Connected;
        device.link_key = Some([0u8; 16]); // Mock link key

        let address = device.address;
        self.paired_devices.insert(address, device);
        self.discovered_devices.remove(&address);

        tracing::info!("Pairing complete with {}", address.to_string());
        Ok(address)
    }

    /// Cancel pairing
    pub fn cancel_pairing(&mut self) {
        if self.pairing_request.take().is_some() {
            tracing::info!("Pairing cancelled");
        }
    }

    /// Connect to a paired device
    pub fn connect(&mut self, address: BluetoothAddress) -> Result<(), &'static str> {
        let device = self.paired_devices.get_mut(&address)
            .ok_or("Device not paired")?;

        if device.state == BluetoothState::Connected {
            return Ok(()); // Already connected
        }

        device.state = BluetoothState::Connected;
        tracing::info!("Connected to {}", address.to_string());
        Ok(())
    }

    /// Disconnect from a device
    pub fn disconnect(&mut self, address: BluetoothAddress) -> Result<(), &'static str> {
        let device = self.paired_devices.get_mut(&address)
            .ok_or("Device not found")?;

        device.state = BluetoothState::Disconnected;
        tracing::info!("Disconnected from {}", address.to_string());
        Ok(())
    }

    /// Remove pairing
    pub fn unpair(&mut self, address: BluetoothAddress) -> Result<(), &'static str> {
        self.paired_devices.remove(&address)
            .ok_or("Device not paired")?;
        tracing::info!("Unpaired {}", address.to_string());
        Ok(())
    }

    /// Get device by address
    pub fn get_device(&self, address: &BluetoothAddress) -> Option<&BluetoothDevice> {
        self.paired_devices.get(address)
            .or_else(|| self.discovered_devices.get(address))
    }

    /// Get connected devices
    pub fn get_connected_devices(&self) -> Vec<&BluetoothDevice> {
        self.paired_devices.values()
            .filter(|d| d.state == BluetoothState::Connected)
            .collect()
    }

    /// Get connected gamepads
    pub fn get_connected_gamepads(&self) -> Vec<&BluetoothDevice> {
        self.get_connected_devices()
            .into_iter()
            .filter(|d| d.is_gamepad())
            .collect()
    }

    /// Update adapter state (call periodically)
    pub fn update(&mut self) {
        // Auto-stop scan after 30 seconds
        if self.scanning {
            if let Some(start) = self.scan_start {
                if start.elapsed() > Duration::from_secs(30) {
                    self.stop_scan();
                }
            }
        }

        // Check pairing timeout
        if let Some(ref request) = self.pairing_request {
            if request.requested_at.elapsed() > request.timeout {
                self.pairing_request = None;
                tracing::warn!("Pairing timed out");
            }
        }
    }
}

impl Default for BluetoothAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Bluetooth manager for the emulator
pub struct BluetoothManager {
    /// Primary adapter
    adapter: BluetoothAdapter,
}

impl BluetoothManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            adapter: BluetoothAdapter::new(),
        }
    }

    /// Get the adapter
    pub fn adapter(&self) -> &BluetoothAdapter {
        &self.adapter
    }

    /// Get mutable adapter
    pub fn adapter_mut(&mut self) -> &mut BluetoothAdapter {
        &mut self.adapter
    }

    /// Quick pair DualShock 3 (mock - in reality this requires USB first)
    pub fn quick_pair_ds3(&mut self, name: &str) -> Result<BluetoothAddress, &'static str> {
        let address = BluetoothAddress::random();
        let mut device = BluetoothDevice::new(
            address,
            name.to_string(),
            device_class::GAMEPAD,
        );
        device.device_type = BluetoothDeviceType::DualShock3;
        device.paired = true;
        device.trusted = true;
        device.state = BluetoothState::Connected;
        device.link_key = Some([0u8; 16]);

        self.adapter.paired_devices.insert(address, device);
        Ok(address)
    }
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bluetooth_address() {
        let addr = BluetoothAddress::from_string("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(addr.to_string(), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn test_adapter_power() {
        let mut adapter = BluetoothAdapter::new();
        assert_eq!(adapter.state, AdapterState::On);
        
        adapter.power_off();
        assert_eq!(adapter.state, AdapterState::Off);
        
        adapter.power_on();
        assert_eq!(adapter.state, AdapterState::On);
    }

    #[test]
    fn test_scanning() {
        let mut adapter = BluetoothAdapter::new();
        
        assert!(!adapter.is_scanning());
        assert!(adapter.start_scan());
        assert!(adapter.is_scanning());
        
        adapter.stop_scan();
        assert!(!adapter.is_scanning());
    }

    #[test]
    fn test_device_classification() {
        let device = BluetoothDevice::new(
            BluetoothAddress::random(),
            "PLAYSTATION(R)3 Controller".to_string(),
            device_class::GAMEPAD,
        );
        
        assert_eq!(device.device_type, BluetoothDeviceType::DualShock3);
        assert!(device.is_gamepad());
    }

    #[test]
    fn test_pairing_flow() {
        let mut adapter = BluetoothAdapter::new();
        
        // Add mock device
        let device = BluetoothDevice::new(
            BluetoothAddress::from_bytes([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]),
            "Test Controller".to_string(),
            device_class::GAMEPAD,
        );
        let address = device.address;
        adapter.add_mock_device(device);
        
        // Pair
        assert!(adapter.pair(address).is_ok());
        assert!(adapter.complete_pairing().is_ok());
        
        // Should be paired now
        assert!(adapter.get_paired_devices().iter().any(|d| d.address == address));
    }

    #[test]
    fn test_quick_pair() {
        let mut manager = BluetoothManager::new();
        let result = manager.quick_pair_ds3("Test DS3");
        
        assert!(result.is_ok());
        assert_eq!(manager.adapter().get_connected_gamepads().len(), 1);
    }
}
