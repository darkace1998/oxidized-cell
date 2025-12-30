//! cellNetCtl HLE - Network Control
//!
//! This module provides HLE implementations for PS3 network control operations.
//! Supports network state detection, connection management, and network information retrieval.

use std::collections::HashMap;
use tracing::{debug, trace};
use crate::memory::{write_be32, write_string};

// Error codes
pub const CELL_NET_CTL_ERROR_NOT_INITIALIZED: i32 = 0x80130101u32 as i32;
pub const CELL_NET_CTL_ERROR_NOT_TERMINATED: i32 = 0x80130102u32 as i32;
pub const CELL_NET_CTL_ERROR_HANDLER_MAX: i32 = 0x80130103u32 as i32;
pub const CELL_NET_CTL_ERROR_ID_NOT_FOUND: i32 = 0x80130104u32 as i32;
pub const CELL_NET_CTL_ERROR_INVALID_ID: i32 = 0x80130105u32 as i32;
pub const CELL_NET_CTL_ERROR_INVALID_CODE: i32 = 0x80130106u32 as i32;
pub const CELL_NET_CTL_ERROR_INVALID_ADDR: i32 = 0x80130107u32 as i32;
pub const CELL_NET_CTL_ERROR_NET_DISABLED: i32 = 0x80130181u32 as i32;

/// Network state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellNetCtlState {
    /// Not initialized
    NotInitialized = 0,
    /// Initializing
    Initializing = 1,
    /// Disconnected
    Disconnected = 2,
    /// Connecting
    Connecting = 3,
    /// Obtaining IP address
    ObtainingIp = 4,
    /// IP obtained
    IpObtained = 5,
}

/// Network information code
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellNetCtlInfoCode {
    /// Device
    Device = 0,
    /// Ether address
    EtherAddr = 1,
    /// MTU
    Mtu = 2,
    /// Link status
    Link = 3,
    /// Link type
    LinkType = 4,
    /// BSSID
    Bssid = 5,
    /// SSID
    Ssid = 6,
    /// WLAN security
    WlanSecurity = 7,
    /// IP address
    IpAddress = 10,
    /// Netmask
    Netmask = 11,
    /// Default route
    DefaultRoute = 12,
    /// Primary DNS
    PrimaryDns = 13,
    /// Secondary DNS
    SecondaryDns = 14,
    /// HTTP proxy config
    HttpProxyConfig = 20,
    /// HTTP proxy server
    HttpProxyServer = 21,
    /// HTTP proxy port
    HttpProxyPort = 22,
}

/// NAT type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellNetCtlNatType {
    /// Type 1 - Open
    Type1 = 1,
    /// Type 2 - Moderate
    Type2 = 2,
    /// Type 3 - Strict
    Type3 = 3,
}

/// Network info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CellNetCtlInfo {
    pub device: u32,
    pub ether_addr: [u8; 6],
    pub mtu: u32,
    pub link: u32,
    pub link_type: u32,
    pub bssid: [u8; 6],
    pub ssid: [u8; 32],
    pub wlan_security: u32,
    pub rssi_dbm: i8,
    pub channel: u8,
    pub ip_address: [u8; 16],
    pub netmask: [u8; 16],
    pub default_route: [u8; 16],
    pub primary_dns: [u8; 16],
    pub secondary_dns: [u8; 16],
    pub http_proxy_config: u32,
    pub http_proxy_server: [u8; 128],
    pub http_proxy_port: u16,
}

impl Default for CellNetCtlInfo {
    fn default() -> Self {
        Self {
            device: 0,
            ether_addr: [0; 6],
            mtu: 1500,
            link: 0,
            link_type: 0,
            bssid: [0; 6],
            ssid: [0; 32],
            wlan_security: 0,
            rssi_dbm: 0,
            channel: 0,
            ip_address: [0; 16],
            netmask: [0; 16],
            default_route: [0; 16],
            primary_dns: [0; 16],
            secondary_dns: [0; 16],
            http_proxy_config: 0,
            http_proxy_server: [0; 128],
            http_proxy_port: 0,
        }
    }
}

/// NAT information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellNetCtlNatInfo {
    pub size: u32,
    pub nat_type: u32,
    pub stun_status: u32,
    pub upnp_status: u32,
}

impl Default for CellNetCtlNatInfo {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            nat_type: CellNetCtlNatType::Type2 as u32,
            stun_status: 0,
            upnp_status: 0,
        }
    }
}

/// Network event handler callback
pub type NetCtlHandler = u32;

/// Handler entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct HandlerEntry {
    handler: NetCtlHandler,
    arg: u32,
}

/// Network backend that interfaces with system network
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct NetworkBackend {
    /// Whether network is available
    is_connected: bool,
    /// System hostname
    hostname: String,
    /// Available network interfaces
    interfaces: Vec<NetworkInterface>,
}

/// Network interface information
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct NetworkInterface {
    /// Interface name
    name: String,
    /// MAC address
    mac_address: [u8; 6],
    /// IP address (v4 or v6)
    ip_address: std::net::IpAddr,
    /// Netmask
    netmask: std::net::IpAddr,
    /// Gateway/default route
    gateway: std::net::IpAddr,
    /// MTU
    mtu: u32,
    /// Is interface up
    is_up: bool,
}

impl NetworkBackend {
    fn new() -> Self {
        Self {
            is_connected: false,
            hostname: String::from("ps3"),
            interfaces: Vec::new(),
        }
    }

    /// Query system network state
    fn query_system_network(&mut self) -> Result<(), i32> {
        trace!("NetworkBackend::query_system_network: querying system network state");
        
        // In a real implementation:
        // 1. Use platform-specific APIs to get network interfaces
        //    - Windows: GetAdaptersInfo/GetAdaptersAddresses
        //    - Linux: getifaddrs/netlink
        //    - macOS: getifaddrs
        // 2. Get default route/gateway
        // 3. Test connectivity (ping/HTTP check)
        
        // Simulate a connected network
        self.is_connected = true;
        
        // Add a mock ethernet interface
        use std::net::{IpAddr, Ipv4Addr};
        let interface = NetworkInterface {
            name: String::from("eth0"),
            mac_address: [0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E],
            ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            netmask: IpAddr::V4(Ipv4Addr::new(255, 255, 255, 0)),
            gateway: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            mtu: 1500,
            is_up: true,
        };
        
        self.interfaces.clear();
        self.interfaces.push(interface);
        
        Ok(())
    }

    /// Get primary interface
    fn get_primary_interface(&self) -> Option<&NetworkInterface> {
        self.interfaces.iter().find(|iface| iface.is_up)
    }

    /// Configure IP address
    fn configure_ip(&mut self, ip: std::net::IpAddr, netmask: std::net::IpAddr, gateway: std::net::IpAddr) -> Result<(), i32> {
        trace!("NetworkBackend::configure_ip: ip={}, netmask={}, gateway={}", ip, netmask, gateway);
        
        // In a real implementation:
        // 1. Validate IP configuration
        // 2. Use platform-specific APIs to set IP address
        // 3. Update routing table
        
        if let Some(interface) = self.interfaces.get_mut(0) {
            interface.ip_address = ip;
            interface.netmask = netmask;
            interface.gateway = gateway;
        }
        
        Ok(())
    }

    /// Configure DNS servers
    fn configure_dns(&mut self, primary_dns: std::net::IpAddr, secondary_dns: std::net::IpAddr) -> Result<(), i32> {
        trace!("NetworkBackend::configure_dns: primary={}, secondary={}", primary_dns, secondary_dns);
        
        // In a real implementation:
        // 1. Update system DNS configuration
        //    - Windows: Registry or netsh
        //    - Linux: /etc/resolv.conf or systemd-resolved
        //    - macOS: scutil
        
        Ok(())
    }

    /// Test network connectivity
    fn test_connectivity(&mut self) -> bool {
        // In a real implementation:
        // 1. Ping gateway
        // 2. DNS lookup test
        // 3. HTTP connectivity check
        
        self.is_connected
    }
}

/// Network control manager
pub struct NetCtlManager {
    is_initialized: bool,
    state: CellNetCtlState,
    info: CellNetCtlInfo,
    nat_info: CellNetCtlNatInfo,
    handlers: HashMap<u32, HandlerEntry>,
    next_handler_id: u32,
    dialog_active: bool,
    /// Network backend
    backend: NetworkBackend,
}

impl NetCtlManager {
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            state: CellNetCtlState::NotInitialized,
            info: CellNetCtlInfo::default(),
            nat_info: CellNetCtlNatInfo::default(),
            handlers: HashMap::new(),
            next_handler_id: 1,
            dialog_active: false,
            backend: NetworkBackend::new(),
        }
    }

    /// Initialize network control
    pub fn init(&mut self) -> Result<(), i32> {
        if self.is_initialized {
            return Ok(());
        }

        self.is_initialized = true;
        self.state = CellNetCtlState::Disconnected;
        
        // Query system network state through backend
        self.backend.query_system_network()?;
        
        // Update network info from backend
        if let Some(interface) = self.backend.get_primary_interface() {
            self.info.ether_addr = interface.mac_address;
            self.info.mtu = interface.mtu;
            self.info.link = if interface.is_up { 1 } else { 0 };
            
            // Convert IP address
            match interface.ip_address {
                std::net::IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    self.info.ip_address[0..4].copy_from_slice(&octets);
                }
                std::net::IpAddr::V6(ipv6) => {
                    self.info.ip_address.copy_from_slice(&ipv6.octets());
                }
            }
            
            // If network is connected, update state
            if self.backend.is_connected {
                self.state = CellNetCtlState::IpObtained;
            }
        }
        
        // Initialize default network info
        self.info.device = 0;

        Ok(())
    }

    /// Terminate network control
    pub fn term(&mut self) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        self.handlers.clear();
        self.is_initialized = false;
        self.state = CellNetCtlState::NotInitialized;

        Ok(())
    }

    /// Get current network state
    pub fn get_state(&self) -> Result<CellNetCtlState, i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        Ok(self.state)
    }

    /// Set network state (for testing/simulation)
    pub fn set_state(&mut self, state: CellNetCtlState) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        self.state = state;
        Ok(())
    }

    /// Get network information by code
    pub fn get_info(&self, _code: CellNetCtlInfoCode) -> Result<CellNetCtlInfo, i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        Ok(self.info.clone())
    }

    /// Set IP address (for simulation)
    pub fn set_ip_address(&mut self, ip: &str) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        let bytes = ip.as_bytes();
        let len = bytes.len().min(15);
        self.info.ip_address[..len].copy_from_slice(&bytes[..len]);
        self.info.ip_address[len] = 0;

        Ok(())
    }

    /// Get NAT information
    pub fn get_nat_info(&self) -> Result<CellNetCtlNatInfo, i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        Ok(self.nat_info)
    }

    /// Add event handler
    pub fn add_handler(&mut self, handler: NetCtlHandler, arg: u32) -> Result<u32, i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        if self.handlers.len() >= 8 {
            return Err(CELL_NET_CTL_ERROR_HANDLER_MAX);
        }

        let id = self.next_handler_id;
        self.next_handler_id += 1;

        self.handlers.insert(id, HandlerEntry { handler, arg });

        Ok(id)
    }

    /// Remove event handler
    pub fn remove_handler(&mut self, id: u32) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        self.handlers.remove(&id).ok_or(CELL_NET_CTL_ERROR_ID_NOT_FOUND)?;
        Ok(())
    }

    /// Start network dialog
    pub fn start_dialog(&mut self) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        self.dialog_active = true;
        Ok(())
    }

    /// Unload network dialog
    pub fn unload_dialog(&mut self) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        self.dialog_active = false;
        Ok(())
    }

    /// Check if dialog is active
    pub fn is_dialog_active(&self) -> bool {
        self.dialog_active
    }

    /// Get handler count
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Configure IP address manually
    pub fn configure_ip(&mut self, ip: [u8; 4], netmask: [u8; 4], gateway: [u8; 4]) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        use std::net::{IpAddr, Ipv4Addr};
        
        let ip_addr = IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]));
        let netmask_addr = IpAddr::V4(Ipv4Addr::new(netmask[0], netmask[1], netmask[2], netmask[3]));
        let gateway_addr = IpAddr::V4(Ipv4Addr::new(gateway[0], gateway[1], gateway[2], gateway[3]));
        
        self.backend.configure_ip(ip_addr, netmask_addr, gateway_addr)?;
        
        // Update info
        self.info.ip_address[0..4].copy_from_slice(&ip);
        self.info.netmask[0..4].copy_from_slice(&netmask);
        self.info.default_route[0..4].copy_from_slice(&gateway);
        
        trace!("NetCtlManager::configure_ip: {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
        
        Ok(())
    }

    /// Configure DNS servers
    pub fn configure_dns(&mut self, primary: [u8; 4], secondary: [u8; 4]) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        use std::net::{IpAddr, Ipv4Addr};
        
        let primary_addr = IpAddr::V4(Ipv4Addr::new(primary[0], primary[1], primary[2], primary[3]));
        let secondary_addr = IpAddr::V4(Ipv4Addr::new(secondary[0], secondary[1], secondary[2], secondary[3]));
        
        self.backend.configure_dns(primary_addr, secondary_addr)?;
        
        // Update info
        self.info.primary_dns[0..4].copy_from_slice(&primary);
        self.info.secondary_dns[0..4].copy_from_slice(&secondary);
        
        trace!("NetCtlManager::configure_dns: primary={}.{}.{}.{}, secondary={}.{}.{}.{}", 
               primary[0], primary[1], primary[2], primary[3],
               secondary[0], secondary[1], secondary[2], secondary[3]);
        
        Ok(())
    }

    /// Test network connectivity
    pub fn test_connectivity(&mut self) -> Result<bool, i32> {
        if !self.is_initialized {
            return Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED);
        }

        let connected = self.backend.test_connectivity();
        
        if connected {
            self.state = CellNetCtlState::IpObtained;
        } else {
            self.state = CellNetCtlState::Disconnected;
        }
        
        Ok(connected)
    }
}

impl Default for NetCtlManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellNetCtlInit - Initialize network control
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_init() -> i32 {
    debug!("cellNetCtlInit()");

    match crate::context::get_hle_context_mut().net_ctl.init() {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellNetCtlTerm - Terminate network control
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_term() -> i32 {
    debug!("cellNetCtlTerm()");

    match crate::context::get_hle_context_mut().net_ctl.term() {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellNetCtlGetState - Get network state
///
/// # Arguments
/// * `state` - State address
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_get_state(state_addr: u32) -> i32 {
    trace!("cellNetCtlGetState(state_addr=0x{:08X})", state_addr);

    match crate::context::get_hle_context().net_ctl.get_state() {
        Ok(state) => {
            // Write state to memory
            if let Err(e) = write_be32(state_addr, state as u32) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellNetCtlGetInfo - Get network information
///
/// # Arguments
/// * `code` - Information code
/// * `info` - Info buffer address
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_get_info(code: u32, info_addr: u32) -> i32 {
    trace!("cellNetCtlGetInfo(code={}, info_addr=0x{:08X})", code, info_addr);

    let ctx = crate::context::get_hle_context();
    let info = match ctx.net_ctl.get_info(CellNetCtlInfoCode::Device) {
        Ok(i) => i,
        Err(e) => return e,
    };
    
    // Write info based on code
    // Each code corresponds to a specific field in CellNetCtlInfo
    match code {
        1 => { // CELL_NET_CTL_INFO_DEVICE
            if let Err(e) = write_be32(info_addr, info.device) { return e; }
        }
        2 => { // CELL_NET_CTL_INFO_ETHER_ADDR
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.ether_addr) { return e; }
        }
        3 => { // CELL_NET_CTL_INFO_MTU
            if let Err(e) = write_be32(info_addr, info.mtu) { return e; }
        }
        4 => { // CELL_NET_CTL_INFO_LINK
            if let Err(e) = write_be32(info_addr, info.link) { return e; }
        }
        5 => { // CELL_NET_CTL_INFO_LINK_TYPE
            if let Err(e) = write_be32(info_addr, info.link_type) { return e; }
        }
        6 => { // CELL_NET_CTL_INFO_BSSID
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.bssid) { return e; }
        }
        7 => { // CELL_NET_CTL_INFO_SSID
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.ssid) { return e; }
        }
        8 => { // CELL_NET_CTL_INFO_WLAN_SECURITY
            if let Err(e) = write_be32(info_addr, info.wlan_security) { return e; }
        }
        9 => { // CELL_NET_CTL_INFO_RXQ_LINK_QUALITY (rssi_dbm)
            if let Err(e) = crate::memory::write_u8(info_addr, info.rssi_dbm as u8) { return e; }
        }
        10 => { // CELL_NET_CTL_INFO_IP_ADDRESS
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.ip_address) { return e; }
        }
        11 => { // CELL_NET_CTL_INFO_NETMASK
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.netmask) { return e; }
        }
        12 => { // CELL_NET_CTL_INFO_DEFAULT_ROUTE
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.default_route) { return e; }
        }
        13 => { // CELL_NET_CTL_INFO_PRIMARY_DNS
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.primary_dns) { return e; }
        }
        14 => { // CELL_NET_CTL_INFO_SECONDARY_DNS
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.secondary_dns) { return e; }
        }
        15 => { // CELL_NET_CTL_INFO_HTTP_PROXY_CONFIG
            if let Err(e) = write_be32(info_addr, info.http_proxy_config) { return e; }
        }
        16 => { // CELL_NET_CTL_INFO_HTTP_PROXY_SERVER
            if let Err(e) = crate::memory::write_bytes(info_addr, &info.http_proxy_server) { return e; }
        }
        17 => { // CELL_NET_CTL_INFO_HTTP_PROXY_PORT
            if let Err(e) = crate::memory::write_be16(info_addr, info.http_proxy_port) { return e; }
        }
        _ => return CELL_NET_CTL_ERROR_INVALID_CODE,
    }
    
    0 // CELL_OK
}

/// cellNetCtlNetStartDialogLoadAsync - Start network configuration dialog
///
/// # Arguments
/// * `param` - Dialog parameters
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_net_start_dialog_load_async(_param_addr: u32) -> i32 {
    debug!("cellNetCtlNetStartDialogLoadAsync()");

    match crate::context::get_hle_context_mut().net_ctl.start_dialog() {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellNetCtlNetStartDialogUnloadAsync - Unload network dialog
///
/// # Arguments
/// * `result` - Result address
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_net_start_dialog_unload_async(_result_addr: u32) -> i32 {
    debug!("cellNetCtlNetStartDialogUnloadAsync()");

    match crate::context::get_hle_context_mut().net_ctl.unload_dialog() {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellNetCtlGetNatInfo - Get NAT information
///
/// # Arguments
/// * `natInfo` - NAT info address
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_get_nat_info(nat_info_addr: u32) -> i32 {
    trace!("cellNetCtlGetNatInfo(nat_info_addr=0x{:08X})", nat_info_addr);

    match crate::context::get_hle_context().net_ctl.get_nat_info() {
        Ok(nat_info) => {
            // Write NAT info to memory (CellNetCtlNatInfo structure)
            // size(4) + nat_type(4) + stun_status(4) + upnp_status(4)
            if let Err(e) = write_be32(nat_info_addr, nat_info.size) { return e; }
            if let Err(e) = write_be32(nat_info_addr + 4, nat_info.nat_type) { return e; }
            if let Err(e) = write_be32(nat_info_addr + 8, nat_info.stun_status) { return e; }
            if let Err(e) = write_be32(nat_info_addr + 12, nat_info.upnp_status) { return e; }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellNetCtlAddHandler - Add event handler
///
/// # Arguments
/// * `handler` - Handler callback address
/// * `arg` - Handler argument
/// * `hid` - Handler ID address
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_add_handler(handler: u32, arg: u32, hid_addr: u32) -> i32 {
    debug!("cellNetCtlAddHandler(handler=0x{:08X}, arg=0x{:08X}, hid_addr=0x{:08X})", handler, arg, hid_addr);

    match crate::context::get_hle_context_mut().net_ctl.add_handler(handler, arg) {
        Ok(hid) => {
            // Write handler ID to memory
            if let Err(e) = write_be32(hid_addr, hid) {
                return e;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellNetCtlDelHandler - Remove event handler
///
/// # Arguments
/// * `hid` - Handler ID
///
/// # Returns
/// * 0 on success
pub fn cell_net_ctl_del_handler(hid: u32) -> i32 {
    debug!("cellNetCtlDelHandler(hid={})", hid);

    match crate::context::get_hle_context_mut().net_ctl.remove_handler(hid) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_net_ctl_manager_new() {
        let manager = NetCtlManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.handler_count(), 0);
    }

    #[test]
    fn test_net_ctl_manager_init_term() {
        let mut manager = NetCtlManager::new();

        manager.init().unwrap();
        assert!(manager.is_initialized());
        // State can be either Disconnected or IpObtained depending on backend simulation
        let state = manager.get_state().unwrap();
        assert!(state == CellNetCtlState::Disconnected || state == CellNetCtlState::IpObtained);

        manager.term().unwrap();
        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_net_ctl_manager_double_init() {
        let mut manager = NetCtlManager::new();

        manager.init().unwrap();
        // Second init should be OK (idempotent)
        manager.init().unwrap();
        assert!(manager.is_initialized());
    }

    #[test]
    fn test_net_ctl_manager_term_without_init() {
        let mut manager = NetCtlManager::new();

        assert_eq!(manager.term(), Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED));
    }

    #[test]
    fn test_net_ctl_manager_get_state_without_init() {
        let manager = NetCtlManager::new();

        assert_eq!(manager.get_state(), Err(CELL_NET_CTL_ERROR_NOT_INITIALIZED));
    }

    #[test]
    fn test_net_ctl_manager_set_state() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        manager.set_state(CellNetCtlState::IpObtained).unwrap();
        assert_eq!(manager.get_state().unwrap(), CellNetCtlState::IpObtained);
    }

    #[test]
    fn test_net_ctl_manager_handlers() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        let id1 = manager.add_handler(0x1000, 0).unwrap();
        let id2 = manager.add_handler(0x2000, 0).unwrap();

        assert_ne!(id1, id2);
        assert_eq!(manager.handler_count(), 2);

        manager.remove_handler(id1).unwrap();
        assert_eq!(manager.handler_count(), 1);
    }

    #[test]
    fn test_net_ctl_manager_handler_max() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        // Add maximum handlers
        for i in 0..8 {
            manager.add_handler(0x1000 + i, 0).unwrap();
        }

        // 9th handler should fail
        assert_eq!(manager.add_handler(0x9000, 0), Err(CELL_NET_CTL_ERROR_HANDLER_MAX));
    }

    #[test]
    fn test_net_ctl_manager_remove_invalid_handler() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        assert_eq!(manager.remove_handler(999), Err(CELL_NET_CTL_ERROR_ID_NOT_FOUND));
    }

    #[test]
    fn test_net_ctl_manager_dialog() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        assert!(!manager.is_dialog_active());
        manager.start_dialog().unwrap();
        assert!(manager.is_dialog_active());
        manager.unload_dialog().unwrap();
        assert!(!manager.is_dialog_active());
    }

    #[test]
    fn test_net_ctl_manager_get_info() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        let info = manager.get_info(CellNetCtlInfoCode::Mtu).unwrap();
        assert_eq!(info.mtu, 1500);
    }

    #[test]
    fn test_net_ctl_manager_set_ip() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        manager.set_ip_address("192.168.1.1").unwrap();
        let info = manager.get_info(CellNetCtlInfoCode::IpAddress).unwrap();
        assert_eq!(&info.ip_address[..12], b"192.168.1.1\0");
    }

    #[test]
    fn test_net_ctl_manager_nat_info() {
        let mut manager = NetCtlManager::new();
        manager.init().unwrap();

        let nat_info = manager.get_nat_info().unwrap();
        assert_eq!(nat_info.nat_type, CellNetCtlNatType::Type2 as u32);
    }

    #[test]
    fn test_net_ctl_init() {
        let result = cell_net_ctl_init();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_net_ctl_state() {
        assert_eq!(CellNetCtlState::NotInitialized as u32, 0);
        assert_eq!(CellNetCtlState::Disconnected as u32, 2);
        assert_eq!(CellNetCtlState::IpObtained as u32, 5);
    }

    #[test]
    fn test_net_ctl_info_default() {
        let info = CellNetCtlInfo::default();
        assert_eq!(info.mtu, 1500);
    }

    #[test]
    fn test_net_ctl_nat_types() {
        assert_eq!(CellNetCtlNatType::Type1 as u32, 1);
        assert_eq!(CellNetCtlNatType::Type2 as u32, 2);
        assert_eq!(CellNetCtlNatType::Type3 as u32, 3);
    }

    #[test]
    fn test_net_ctl_error_codes() {
        assert_ne!(CELL_NET_CTL_ERROR_NOT_INITIALIZED, 0);
        assert_ne!(CELL_NET_CTL_ERROR_HANDLER_MAX, 0);
        assert_ne!(CELL_NET_CTL_ERROR_ID_NOT_FOUND, 0);
        assert_ne!(CELL_NET_CTL_ERROR_NET_DISABLED, 0);
    }
}
