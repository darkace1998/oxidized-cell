//! cellSsl HLE - SSL/TLS module
//!
//! This module provides HLE implementations for the PS3's SSL/TLS library.
//! Supports SSL initialization, certificate management, and certificate information retrieval.

use std::collections::HashMap;
use tracing::trace;

// Error codes
pub const CELL_SSL_ERROR_NOT_INITIALIZED: i32 = 0x80720001u32 as i32;
pub const CELL_SSL_ERROR_ALREADY_INITIALIZED: i32 = 0x80720002u32 as i32;
pub const CELL_SSL_ERROR_INVALID_PARAM: i32 = 0x80720003u32 as i32;
pub const CELL_SSL_ERROR_NO_MEMORY: i32 = 0x80720004u32 as i32;
pub const CELL_SSL_ERROR_INVALID_CERT: i32 = 0x80720005u32 as i32;
pub const CELL_SSL_ERROR_CERT_NOT_FOUND: i32 = 0x80720006u32 as i32;
pub const CELL_SSL_ERROR_VERIFY_FAILED: i32 = 0x80720007u32 as i32;

/// SSL certificate ID
pub type SslCertId = u32;

/// SSL context ID
pub type SslCtxId = u32;

/// SSL callback function
pub type SslCallback = extern "C" fn(ssl_ctx_id: SslCtxId, reason: i32, arg: *mut u8) -> i32;

/// Certificate type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSslCertType {
    /// PEM format
    Pem = 0,
    /// DER format
    Der = 1,
    /// PKCS12 format
    Pkcs12 = 2,
}

/// Certificate verification result
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSslVerifyResult {
    /// Certificate is valid
    Ok = 0,
    /// Certificate has expired
    Expired = 1,
    /// Certificate is not yet valid
    NotYetValid = 2,
    /// Certificate issuer not found
    IssuerNotFound = 3,
    /// Certificate revoked
    Revoked = 4,
    /// Certificate signature invalid
    SignatureInvalid = 5,
}

/// Certificate entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CertEntry {
    cert_type: CellSslCertType,
    subject_name: String,
    issuer_name: String,
    serial_number: Vec<u8>,
    not_before: u64,
    not_after: u64,
    public_key: Vec<u8>,
    is_ca: bool,
    /// Raw DER-encoded certificate data
    raw_der: Vec<u8>,
}

impl CertEntry {
    fn new(cert_type: CellSslCertType) -> Self {
        Self {
            cert_type,
            subject_name: String::new(),
            issuer_name: String::new(),
            serial_number: Vec::new(),
            not_before: 0,
            not_after: u64::MAX,
            public_key: Vec::new(),
            is_ca: false,
            raw_der: Vec::new(),
        }
    }
}

/// TLS handshake state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsHandshakeState {
    /// Initial state
    Initial,
    /// ClientHello sent
    ClientHelloSent,
    /// ServerHello received
    ServerHelloReceived,
    /// Certificate received
    CertificateReceived,
    /// ServerKeyExchange received
    ServerKeyExchangeReceived,
    /// ServerHelloDone received
    ServerHelloDone,
    /// ClientKeyExchange sent
    ClientKeyExchangeSent,
    /// ChangeCipherSpec sent
    ChangeCipherSpecSent,
    /// Finished sent/received
    Finished,
    /// Handshake failed
    Failed,
}

/// TLS cipher suite
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsCipherSuite {
    /// TLS_RSA_WITH_AES_128_CBC_SHA
    RsaAes128CbcSha = 0x002F,
    /// TLS_RSA_WITH_AES_256_CBC_SHA
    RsaAes256CbcSha = 0x0035,
    /// TLS_RSA_WITH_AES_128_CBC_SHA256
    RsaAes128CbcSha256 = 0x003C,
    /// TLS_RSA_WITH_AES_256_CBC_SHA256
    RsaAes256CbcSha256 = 0x003D,
}

/// TLS session entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TlsSession {
    /// Session ID
    pub session_id: u32,
    /// SSL context ID this session belongs to
    pub ctx_id: SslCtxId,
    /// Current handshake state
    pub state: TlsHandshakeState,
    /// Negotiated cipher suite
    pub cipher_suite: Option<TlsCipherSuite>,
    /// Server hostname for SNI
    pub server_hostname: String,
    /// Client random (32 bytes)
    pub client_random: [u8; 32],
    /// Server random (32 bytes)
    pub server_random: [u8; 32],
    /// Pre-master secret (48 bytes for RSA)
    pub pre_master_secret: Vec<u8>,
    /// Master secret (48 bytes)
    pub master_secret: [u8; 48],
    /// Server certificate chain
    pub server_certificates: Vec<SslCertId>,
    /// Whether certificate verification passed
    pub verify_result: CellSslVerifyResult,
}

impl TlsSession {
    fn new(session_id: u32, ctx_id: SslCtxId, hostname: &str) -> Self {
        // Generate client_random with simple PRNG (timestamp + counter)
        let mut client_random = [0u8; 32];
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // First 4 bytes: timestamp (big-endian, per TLS spec)
        client_random[0..4].copy_from_slice(&(timestamp as u32).to_be_bytes());
        // Remaining 28 bytes: deterministic fill from session_id + timestamp
        for i in 4..32 {
            client_random[i] = ((session_id as u64 * 7 + i as u64 * 13 + timestamp) & 0xFF) as u8;
        }

        Self {
            session_id,
            ctx_id,
            state: TlsHandshakeState::Initial,
            cipher_suite: None,
            server_hostname: hostname.to_string(),
            client_random,
            server_random: [0u8; 32],
            pre_master_secret: Vec::new(),
            master_secret: [0u8; 48],
            server_certificates: Vec::new(),
            verify_result: CellSslVerifyResult::Ok,
        }
    }
}

/// DER X.509 certificate parser
pub struct DerCertParser;

impl DerCertParser {
    /// Parse a DER-encoded X.509 certificate and extract fields
    pub fn parse(data: &[u8]) -> Result<ParsedCert, i32> {
        if data.len() < 10 {
            return Err(CELL_SSL_ERROR_INVALID_CERT);
        }

        // DER SEQUENCE tag (0x30)
        if data[0] != 0x30 {
            return Err(CELL_SSL_ERROR_INVALID_CERT);
        }

        // Parse TLV length
        let (_cert_len, cert_offset) = Self::parse_tlv_length(&data[1..])?;
        let cert_data = &data[1 + cert_offset..];

        // First inner SEQUENCE is the TBSCertificate
        if cert_data.is_empty() || cert_data[0] != 0x30 {
            return Err(CELL_SSL_ERROR_INVALID_CERT);
        }

        let (tbs_len, tbs_offset) = Self::parse_tlv_length(&cert_data[1..])?;
        let tbs = &cert_data[1 + tbs_offset..1 + tbs_offset + tbs_len];

        let mut pos = 0;
        
        // version (optional, tagged [0])
        if pos < tbs.len() && tbs[pos] == 0xA0 {
            let (vlen, voff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            pos += 1 + voff + vlen;
        }

        // serialNumber (INTEGER)
        let serial_number = if pos < tbs.len() && tbs[pos] == 0x02 {
            let (slen, soff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            let serial = tbs[pos + 1 + soff..pos + 1 + soff + slen].to_vec();
            pos += 1 + soff + slen;
            serial
        } else {
            Vec::new()
        };

        // signature algorithm (SEQUENCE) - skip
        if pos < tbs.len() && tbs[pos] == 0x30 {
            let (alen, aoff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            pos += 1 + aoff + alen;
        }

        // issuer (SEQUENCE of RDNs)
        let issuer = if pos < tbs.len() && tbs[pos] == 0x30 {
            let (ilen, ioff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            let name = Self::parse_rdn_sequence(&tbs[pos + 1 + ioff..pos + 1 + ioff + ilen]);
            pos += 1 + ioff + ilen;
            name
        } else {
            String::new()
        };

        // validity (SEQUENCE of two UTCTime/GeneralizedTime)
        let (not_before, not_after) = if pos < tbs.len() && tbs[pos] == 0x30 {
            let (vlen, voff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            let validity_data = &tbs[pos + 1 + voff..pos + 1 + voff + vlen];
            pos += 1 + voff + vlen;
            Self::parse_validity(validity_data)
        } else {
            (0, u64::MAX)
        };

        // subject (SEQUENCE of RDNs)
        let subject = if pos < tbs.len() && tbs[pos] == 0x30 {
            let (slen, soff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            let name = Self::parse_rdn_sequence(&tbs[pos + 1 + soff..pos + 1 + soff + slen]);
            pos += 1 + soff + slen;
            name
        } else {
            String::new()
        };

        // subjectPublicKeyInfo (SEQUENCE)
        let public_key = if pos < tbs.len() && tbs[pos] == 0x30 {
            let (plen, poff) = Self::parse_tlv_length(&tbs[pos + 1..])?;
            tbs[pos..pos + 1 + poff + plen].to_vec()
        } else {
            Vec::new()
        };

        Ok(ParsedCert {
            subject,
            issuer,
            serial_number,
            not_before,
            not_after,
            public_key,
        })
    }

    /// Parse TLV length field. Returns (length, bytes_consumed).
    fn parse_tlv_length(data: &[u8]) -> Result<(usize, usize), i32> {
        if data.is_empty() {
            return Err(CELL_SSL_ERROR_INVALID_CERT);
        }

        if data[0] < 0x80 {
            // Short form: single byte
            Ok((data[0] as usize, 1))
        } else if data[0] == 0x81 {
            // Long form: 1 byte length
            if data.len() < 2 { return Err(CELL_SSL_ERROR_INVALID_CERT); }
            Ok((data[1] as usize, 2))
        } else if data[0] == 0x82 {
            // Long form: 2 byte length
            if data.len() < 3 { return Err(CELL_SSL_ERROR_INVALID_CERT); }
            Ok((((data[1] as usize) << 8) | data[2] as usize, 3))
        } else if data[0] == 0x83 {
            // Long form: 3 byte length
            if data.len() < 4 { return Err(CELL_SSL_ERROR_INVALID_CERT); }
            Ok((((data[1] as usize) << 16) | ((data[2] as usize) << 8) | data[3] as usize, 4))
        } else {
            Err(CELL_SSL_ERROR_INVALID_CERT)
        }
    }

    /// Parse an RDN SEQUENCE into a display name like "CN=Test, O=Org"
    fn parse_rdn_sequence(data: &[u8]) -> String {
        let mut parts = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            // Each RDN is a SET
            if data[pos] != 0x31 {
                break;
            }
            if let Ok((set_len, set_off)) = Self::parse_tlv_length(&data[pos + 1..]) {
                let set_data = &data[pos + 1 + set_off..pos + 1 + set_off + set_len];
                
                // Inside the SET is a SEQUENCE with OID + value
                if !set_data.is_empty() && set_data[0] == 0x30 {
                    if let Ok((seq_len, seq_off)) = Self::parse_tlv_length(&set_data[1..]) {
                        let seq_data = &set_data[1 + seq_off..1 + seq_off + seq_len];
                        if let Some(part) = Self::parse_attribute_type_and_value(seq_data) {
                            parts.push(part);
                        }
                    }
                }
                
                pos += 1 + set_off + set_len;
            } else {
                break;
            }
        }

        parts.join(", ")
    }

    /// Parse an AttributeTypeAndValue (OID + value)
    fn parse_attribute_type_and_value(data: &[u8]) -> Option<String> {
        if data.is_empty() || data[0] != 0x06 {
            return None; // Must start with OID
        }

        let (oid_len, oid_off) = Self::parse_tlv_length(&data[1..]).ok()?;
        let oid_data = &data[1 + oid_off..1 + oid_off + oid_len];
        let oid_name = Self::oid_to_name(oid_data);

        let value_pos = 1 + oid_off + oid_len;
        if value_pos >= data.len() {
            return None;
        }

        // Value is a string type (UTF8String, PrintableString, etc.)
        let tag = data[value_pos];
        if tag != 0x0C && tag != 0x13 && tag != 0x16 && tag != 0x1E {
            return None;
        }

        let (val_len, val_off) = Self::parse_tlv_length(&data[value_pos + 1..]).ok()?;
        let value_data = &data[value_pos + 1 + val_off..value_pos + 1 + val_off + val_len];
        let value = String::from_utf8_lossy(value_data).to_string();

        Some(format!("{}={}", oid_name, value))
    }

    /// Map known X.500 OIDs to short names
    fn oid_to_name(oid: &[u8]) -> &'static str {
        match oid {
            [0x55, 0x04, 0x03] => "CN",     // commonName
            [0x55, 0x04, 0x06] => "C",      // countryName
            [0x55, 0x04, 0x07] => "L",      // localityName
            [0x55, 0x04, 0x08] => "ST",     // stateOrProvinceName
            [0x55, 0x04, 0x0A] => "O",      // organizationName
            [0x55, 0x04, 0x0B] => "OU",     // organizationalUnitName
            _ => "OID",
        }
    }

    /// Parse validity period (two UTCTime or GeneralizedTime values)
    fn parse_validity(data: &[u8]) -> (u64, u64) {
        let mut pos = 0;
        
        let not_before = if pos < data.len() {
            let tag = data[pos];
            if tag == 0x17 || tag == 0x18 { // UTCTime or GeneralizedTime
                if let Ok((len, off)) = Self::parse_tlv_length(&data[pos + 1..]) {
                    let time_str = std::str::from_utf8(&data[pos + 1 + off..pos + 1 + off + len]).unwrap_or("");
                    pos += 1 + off + len;
                    Self::parse_utc_time(time_str, tag == 0x18)
                } else { 0 }
            } else { 0 }
        } else { 0 };

        let not_after = if pos < data.len() {
            let tag = data[pos];
            if tag == 0x17 || tag == 0x18 {
                if let Ok((len, off)) = Self::parse_tlv_length(&data[pos + 1..]) {
                    let time_str = std::str::from_utf8(&data[pos + 1 + off..pos + 1 + off + len]).unwrap_or("");
                    Self::parse_utc_time(time_str, tag == 0x18)
                } else { u64::MAX }
            } else { u64::MAX }
        } else { u64::MAX };

        (not_before, not_after)
    }

    /// Parse UTCTime (YYMMDDHHMMSSZ) or GeneralizedTime (YYYYMMDDHHMMSSZ) to Unix timestamp
    fn parse_utc_time(s: &str, is_generalized: bool) -> u64 {
        let s = s.trim_end_matches('Z');
        
        let (year, rest) = if is_generalized && s.len() >= 4 {
            (s[..4].parse::<u64>().unwrap_or(2000), &s[4..])
        } else if s.len() >= 2 {
            let yy = s[..2].parse::<u64>().unwrap_or(0);
            let year = if yy >= 50 { 1900 + yy } else { 2000 + yy };
            (year, &s[2..])
        } else {
            return 0;
        };

        let month = if rest.len() >= 2 { rest[..2].parse::<u64>().unwrap_or(1) } else { 1 };
        let day = if rest.len() >= 4 { rest[2..4].parse::<u64>().unwrap_or(1) } else { 1 };
        let hour = if rest.len() >= 6 { rest[4..6].parse::<u64>().unwrap_or(0) } else { 0 };
        let min = if rest.len() >= 8 { rest[6..8].parse::<u64>().unwrap_or(0) } else { 0 };
        let sec = if rest.len() >= 10 { rest[8..10].parse::<u64>().unwrap_or(0) } else { 0 };

        // Approximate Unix timestamp
        // Leap year count: years divisible by 4, minus centuries, plus quadricentennials
        let leap_years = (year - 1969) / 4 - (year - 1901) / 100 + (year - 1601) / 400;
        let days_since_epoch = (year - 1970) * 365 + leap_years
            + Self::days_before_month(month, year) + day - 1;
        days_since_epoch * 86400 + hour * 3600 + min * 60 + sec
    }

    /// Days before a given month in a year
    fn days_before_month(month: u64, year: u64) -> u64 {
        const DAYS: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let m = (month.clamp(1, 12) - 1) as usize;
        let mut d = DAYS[m];
        if month > 2 && (year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)) {
            d += 1; // Leap year
        }
        d
    }
}

/// Parsed certificate data
#[derive(Debug, Clone)]
pub struct ParsedCert {
    pub subject: String,
    pub issuer: String,
    pub serial_number: Vec<u8>,
    pub not_before: u64,
    pub not_after: u64,
    pub public_key: Vec<u8>,
}

/// SSL context entry
#[allow(dead_code)]
#[derive(Debug)]
struct SslContextEntry {
    verify_mode: u32,
    verify_callback: Option<u32>,
    certificates: Vec<SslCertId>,
}

impl SslContextEntry {
    fn new() -> Self {
        Self {
            verify_mode: 0,
            verify_callback: None,
            certificates: Vec::new(),
        }
    }
}

/// SSL manager
pub struct SslManager {
    is_initialized: bool,
    pool_size: u32,
    certificates: HashMap<SslCertId, CertEntry>,
    contexts: HashMap<SslCtxId, SslContextEntry>,
    next_cert_id: SslCertId,
    next_ctx_id: SslCtxId,
    ca_certificates: Vec<SslCertId>,
    /// Active TLS sessions
    sessions: HashMap<u32, TlsSession>,
    next_session_id: u32,
}

impl SslManager {
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            pool_size: 0,
            certificates: HashMap::new(),
            contexts: HashMap::new(),
            next_cert_id: 1,
            next_ctx_id: 1,
            ca_certificates: Vec::new(),
            sessions: HashMap::new(),
            next_session_id: 1,
        }
    }

    /// Initialize SSL library
    pub fn init(&mut self, pool_size: u32) -> Result<(), i32> {
        if self.is_initialized {
            return Err(CELL_SSL_ERROR_ALREADY_INITIALIZED);
        }

        self.is_initialized = true;
        self.pool_size = pool_size;

        Ok(())
    }

    /// Shutdown SSL library
    pub fn end(&mut self) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        self.certificates.clear();
        self.contexts.clear();
        self.ca_certificates.clear();
        self.sessions.clear();
        self.is_initialized = false;

        Ok(())
    }

    /// Load a certificate
    pub fn load_certificate(&mut self, cert_type: CellSslCertType) -> Result<SslCertId, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert_id = self.next_cert_id;
        self.next_cert_id += 1;

        let cert = CertEntry::new(cert_type);
        self.certificates.insert(cert_id, cert);

        Ok(cert_id)
    }

    /// Unload a certificate
    pub fn unload_certificate(&mut self, cert_id: SslCertId) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        self.certificates.remove(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;

        // Also remove from CA list if present
        self.ca_certificates.retain(|&id| id != cert_id);

        Ok(())
    }

    /// Add certificate to CA store
    pub fn add_ca_certificate(&mut self, cert_id: SslCertId) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        if !self.certificates.contains_key(&cert_id) {
            return Err(CELL_SSL_ERROR_CERT_NOT_FOUND);
        }

        if !self.ca_certificates.contains(&cert_id) {
            self.ca_certificates.push(cert_id);
        }

        Ok(())
    }

    /// Create SSL context
    pub fn create_context(&mut self) -> Result<SslCtxId, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let ctx_id = self.next_ctx_id;
        self.next_ctx_id += 1;

        self.contexts.insert(ctx_id, SslContextEntry::new());

        Ok(ctx_id)
    }

    /// Destroy SSL context
    pub fn destroy_context(&mut self, ctx_id: SslCtxId) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        self.contexts.remove(&ctx_id).ok_or(CELL_SSL_ERROR_INVALID_PARAM)?;
        Ok(())
    }

    /// Get certificate serial number
    pub fn get_serial_number(&self, cert_id: SslCertId) -> Result<Vec<u8>, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        Ok(cert.serial_number.clone())
    }

    /// Get certificate subject name
    pub fn get_subject_name(&self, cert_id: SslCertId) -> Result<String, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        Ok(cert.subject_name.clone())
    }

    /// Get certificate issuer name
    pub fn get_issuer_name(&self, cert_id: SslCertId) -> Result<String, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        Ok(cert.issuer_name.clone())
    }

    /// Get certificate validity period
    pub fn get_validity(&self, cert_id: SslCertId) -> Result<(u64, u64), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        Ok((cert.not_before, cert.not_after))
    }

    /// Get certificate public key
    pub fn get_public_key(&self, cert_id: SslCertId) -> Result<Vec<u8>, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        Ok(cert.public_key.clone())
    }

    /// Set certificate data (for testing/simulation)
    pub fn set_certificate_data(&mut self, cert_id: SslCertId, subject: &str, issuer: &str, serial: &[u8]) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get_mut(&cert_id).ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        cert.subject_name = subject.to_string();
        cert.issuer_name = issuer.to_string();
        cert.serial_number = serial.to_vec();

        Ok(())
    }

    /// Load a certificate from DER-encoded data
    pub fn load_certificate_from_der(&mut self, data: &[u8]) -> Result<SslCertId, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        // Parse the DER certificate
        let parsed = DerCertParser::parse(data)?;

        let cert_id = self.next_cert_id;
        self.next_cert_id += 1;

        let cert = CertEntry {
            cert_type: CellSslCertType::Der,
            subject_name: parsed.subject,
            issuer_name: parsed.issuer,
            serial_number: parsed.serial_number,
            not_before: parsed.not_before,
            not_after: parsed.not_after,
            public_key: parsed.public_key,
            is_ca: false,
            raw_der: data.to_vec(),
        };

        self.certificates.insert(cert_id, cert);
        Ok(cert_id)
    }

    /// Load a certificate from PEM-encoded data
    pub fn load_certificate_from_pem(&mut self, pem_data: &str) -> Result<SslCertId, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        // Extract base64-encoded DER data between BEGIN/END markers
        let der_data = Self::pem_to_der(pem_data)?;
        self.load_certificate_from_der(&der_data)
    }

    /// Convert PEM to DER (strip headers and base64-decode)
    fn pem_to_der(pem: &str) -> Result<Vec<u8>, i32> {
        let mut in_cert = false;
        let mut base64_data = String::new();

        for line in pem.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("-----BEGIN") {
                in_cert = true;
                continue;
            }
            if trimmed.starts_with("-----END") {
                break;
            }
            if in_cert {
                base64_data.push_str(trimmed);
            }
        }

        if base64_data.is_empty() {
            return Err(CELL_SSL_ERROR_INVALID_CERT);
        }

        // Simple base64 decode (RFC 4648)
        Self::base64_decode(&base64_data).map_err(|_| CELL_SSL_ERROR_INVALID_CERT)
    }

    /// Simple base64 decoder
    fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
        fn decode_char(c: u8) -> Result<u8, ()> {
            match c {
                b'A'..=b'Z' => Ok(c - b'A'),
                b'a'..=b'z' => Ok(c - b'a' + 26),
                b'0'..=b'9' => Ok(c - b'0' + 52),
                b'+' => Ok(62),
                b'/' => Ok(63),
                _ => Err(()),
            }
        }

        let bytes: Vec<u8> = input.bytes().filter(|b| *b != b'\n' && *b != b'\r' && *b != b' ').collect();
        let mut output = Vec::with_capacity(bytes.len() * 3 / 4);

        let mut i = 0;
        while i + 3 < bytes.len() {
            if bytes[i] == b'=' { break; }
            
            let a = decode_char(bytes[i])?;
            let b = decode_char(bytes[i + 1])?;
            output.push((a << 2) | (b >> 4));

            if bytes[i + 2] != b'=' {
                let c = decode_char(bytes[i + 2])?;
                output.push((b << 4) | (c >> 2));
                
                if bytes[i + 3] != b'=' {
                    let d = decode_char(bytes[i + 3])?;
                    output.push((c << 6) | d);
                }
            }
            
            i += 4;
        }

        Ok(output)
    }

    /// Create a TLS session for connecting to a server
    pub fn create_session(&mut self, ctx_id: SslCtxId, hostname: &str) -> Result<u32, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        if !self.contexts.contains_key(&ctx_id) {
            return Err(CELL_SSL_ERROR_INVALID_PARAM);
        }

        let session_id = self.next_session_id;
        self.next_session_id += 1;

        let session = TlsSession::new(session_id, ctx_id, hostname);
        self.sessions.insert(session_id, session);

        trace!("SslManager::create_session: id={}, ctx={}, host={}", session_id, ctx_id, hostname);
        Ok(session_id)
    }

    /// Destroy a TLS session
    pub fn destroy_session(&mut self, session_id: u32) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        self.sessions.remove(&session_id).ok_or(CELL_SSL_ERROR_INVALID_PARAM)?;
        Ok(())
    }

    /// Perform TLS 1.2 handshake (simulated state machine)
    ///
    /// This simulates the TLS 1.2 handshake flow:
    /// 1. ClientHello → ServerHello
    /// 2. Certificate → ServerKeyExchange → ServerHelloDone
    /// 3. ClientKeyExchange → ChangeCipherSpec → Finished
    pub fn perform_handshake(&mut self, session_id: u32) -> Result<TlsHandshakeState, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let session = self.sessions.get_mut(&session_id)
            .ok_or(CELL_SSL_ERROR_INVALID_PARAM)?;

        if session.state != TlsHandshakeState::Initial {
            return Err(CELL_SSL_ERROR_INVALID_PARAM);
        }

        // Step 1: ClientHello
        trace!("TLS handshake: ClientHello for {}", session.server_hostname);
        session.state = TlsHandshakeState::ClientHelloSent;

        // Step 2: ServerHello (simulated — select cipher suite)
        session.cipher_suite = Some(TlsCipherSuite::RsaAes128CbcSha256);
        // Simulate server_random
        for i in 0..32 {
            session.server_random[i] = ((session.session_id as u64 * 11 + i as u64 * 17 + 0xDEADBEEF) & 0xFF) as u8;
        }
        session.state = TlsHandshakeState::ServerHelloReceived;
        trace!("TLS handshake: ServerHello received, cipher={:?}", session.cipher_suite);

        // Step 3: Certificate received
        session.state = TlsHandshakeState::CertificateReceived;

        // Step 4: ServerKeyExchange
        session.state = TlsHandshakeState::ServerKeyExchangeReceived;

        // Step 5: ServerHelloDone
        session.state = TlsHandshakeState::ServerHelloDone;

        // Step 6: ClientKeyExchange — generate pre-master secret
        session.pre_master_secret = vec![0x03, 0x03]; // TLS 1.2 version
        for i in 0..46 {
            session.pre_master_secret.push(
                ((session.client_random[i % 32] as u16 + session.server_random[i % 32] as u16 + i as u16) & 0xFF) as u8
            );
        }
        session.state = TlsHandshakeState::ClientKeyExchangeSent;

        // Step 7: Derive master secret (simplified PRF)
        Self::derive_master_secret(session);
        
        // Step 8: ChangeCipherSpec + Finished
        session.state = TlsHandshakeState::ChangeCipherSpecSent;
        session.state = TlsHandshakeState::Finished;
        
        trace!("TLS handshake: complete for {}", session.server_hostname);

        Ok(TlsHandshakeState::Finished)
    }

    /// Derive master secret from pre-master secret and randoms (simplified TLS PRF)
    fn derive_master_secret(session: &mut TlsSession) {
        // Simplified key derivation: HMAC-like combination of pre_master_secret + randoms
        let mut seed = Vec::new();
        seed.extend_from_slice(&session.client_random);
        seed.extend_from_slice(&session.server_random);

        for i in 0..48 {
            let idx = i % session.pre_master_secret.len();
            let seed_idx = i % seed.len();
            session.master_secret[i] = session.pre_master_secret[idx]
                .wrapping_add(seed[seed_idx])
                .wrapping_add(i as u8);
        }
    }

    /// Validate a certificate against the CA store
    pub fn validate_certificate(&self, cert_id: SslCertId) -> Result<CellSslVerifyResult, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let cert = self.certificates.get(&cert_id)
            .ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;

        // Check expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now < cert.not_before {
            return Ok(CellSslVerifyResult::NotYetValid);
        }

        if now > cert.not_after {
            return Ok(CellSslVerifyResult::Expired);
        }

        // Check if issuer is in CA store
        let issuer_found = self.ca_certificates.iter().any(|&ca_id| {
            if let Some(ca_cert) = self.certificates.get(&ca_id) {
                ca_cert.subject_name == cert.issuer_name
            } else {
                false
            }
        });

        // Self-signed certificates are accepted if they're in the CA store
        let is_self_signed = cert.subject_name == cert.issuer_name;
        let is_in_ca_store = self.ca_certificates.contains(&cert_id);

        if !issuer_found && !is_self_signed && !is_in_ca_store {
            // If CA store is empty, accept (simulated mode)
            if !self.ca_certificates.is_empty() {
                return Ok(CellSslVerifyResult::IssuerNotFound);
            }
        }

        Ok(CellSslVerifyResult::Ok)
    }

    /// Validate a full certificate chain
    ///
    /// The chain should be ordered from leaf to root. Each cert's issuer must match
    /// the next cert's subject. Only the root (last) certificate needs to be in the
    /// CA store or be self-signed.
    pub fn validate_chain(&self, cert_ids: &[SslCertId]) -> Result<CellSslVerifyResult, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        if cert_ids.is_empty() {
            return Err(CELL_SSL_ERROR_INVALID_PARAM);
        }

        // Check expiry for all certificates in the chain
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for &cert_id in cert_ids {
            let cert = self.certificates.get(&cert_id)
                .ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
            if now < cert.not_before {
                return Ok(CellSslVerifyResult::NotYetValid);
            }
            if now > cert.not_after {
                return Ok(CellSslVerifyResult::Expired);
            }
        }

        // Verify chain linkage: each cert's issuer should match the next cert's subject
        for i in 0..cert_ids.len() - 1 {
            let cert = self.certificates.get(&cert_ids[i])
                .ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
            let issuer_cert = self.certificates.get(&cert_ids[i + 1])
                .ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;

            if cert.issuer_name != issuer_cert.subject_name {
                return Ok(CellSslVerifyResult::IssuerNotFound);
            }
        }

        // The root (last cert) must be in the CA store or be self-signed
        let root_id = cert_ids[cert_ids.len() - 1];
        let root_cert = self.certificates.get(&root_id)
            .ok_or(CELL_SSL_ERROR_CERT_NOT_FOUND)?;
        let is_self_signed = root_cert.subject_name == root_cert.issuer_name;
        let is_in_ca_store = self.ca_certificates.contains(&root_id);

        if !is_self_signed && !is_in_ca_store && !self.ca_certificates.is_empty() {
            return Ok(CellSslVerifyResult::IssuerNotFound);
        }

        Ok(CellSslVerifyResult::Ok)
    }

    /// Get TLS session state
    pub fn get_session_state(&self, session_id: u32) -> Result<TlsHandshakeState, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let session = self.sessions.get(&session_id)
            .ok_or(CELL_SSL_ERROR_INVALID_PARAM)?;

        Ok(session.state)
    }

    /// Get the negotiated cipher suite for a session
    pub fn get_session_cipher(&self, session_id: u32) -> Result<Option<TlsCipherSuite>, i32> {
        if !self.is_initialized {
            return Err(CELL_SSL_ERROR_NOT_INITIALIZED);
        }

        let session = self.sessions.get(&session_id)
            .ok_or(CELL_SSL_ERROR_INVALID_PARAM)?;

        Ok(session.cipher_suite)
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get certificate count
    pub fn certificate_count(&self) -> usize {
        self.certificates.len()
    }

    /// Get context count
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// Get CA certificate count
    pub fn ca_certificate_count(&self) -> usize {
        self.ca_certificates.len()
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Default for SslManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellSslInit - Initialize SSL library
pub fn cell_ssl_init(pool_size: u32) -> i32 {
    trace!("cellSslInit called with pool_size: {}", pool_size);

    match crate::context::get_hle_context_mut().ssl.init(pool_size) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellSslEnd - Terminate SSL library
pub fn cell_ssl_end() -> i32 {
    trace!("cellSslEnd called");

    match crate::context::get_hle_context_mut().ssl.end() {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellSslCertificateLoader - Load certificate
pub unsafe fn cell_ssl_certificate_loader(
    cert_id: *mut SslCertId,
    _cert_path: *const u8,
    _buffer: *mut u8,
    _size: u32,
) -> i32 {
    trace!("cellSslCertificateLoader called");

    if cert_id.is_null() {
        return CELL_SSL_ERROR_INVALID_PARAM;
    }

    match crate::context::get_hle_context_mut().ssl.load_certificate(CellSslCertType::Pem) {
        Ok(id) => {
            unsafe {
                *cert_id = id;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellSslCertGetSerialNumber - Get certificate serial number
pub fn cell_ssl_cert_get_serial_number(
    cert_id: SslCertId,
    _serial: *mut u8,
    _length: *mut u32,
) -> i32 {
    trace!("cellSslCertGetSerialNumber called with cert_id: {}", cert_id);

    // Get serial number through global manager
    let _result = crate::context::get_hle_context().ssl.get_serial_number(cert_id);

    // Note: Writing serial number requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetPublicKey - Get certificate public key
pub fn cell_ssl_cert_get_public_key(
    cert_id: SslCertId,
    _key: *mut u8,
    _length: *mut u32,
) -> i32 {
    trace!("cellSslCertGetPublicKey called with cert_id: {}", cert_id);

    // Get public key through global manager
    let _result = crate::context::get_hle_context().ssl.get_public_key(cert_id);

    // Note: Writing public key requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetRsaPublicKeyModulus - Get RSA public key modulus
pub fn cell_ssl_cert_get_rsa_public_key_modulus(
    cert_id: SslCertId,
    _modulus: *mut u8,
    _length: *mut u32,
) -> i32 {
    trace!("cellSslCertGetRsaPublicKeyModulus called with cert_id: {}", cert_id);

    // Get public key through global manager (contains modulus)
    let _result = crate::context::get_hle_context().ssl.get_public_key(cert_id);

    // Note: Writing modulus requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetRsaPublicKeyExponent - Get RSA public key exponent
pub fn cell_ssl_cert_get_rsa_public_key_exponent(
    cert_id: SslCertId,
    _exponent: *mut u8,
    _length: *mut u32,
) -> i32 {
    trace!("cellSslCertGetRsaPublicKeyExponent called with cert_id: {}", cert_id);

    // Get public key through global manager (contains exponent)
    let _result = crate::context::get_hle_context().ssl.get_public_key(cert_id);

    // Note: Writing exponent requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetNotBefore - Get certificate validity start date
pub fn cell_ssl_cert_get_not_before(
    cert_id: SslCertId,
    _begin: *mut u64,
) -> i32 {
    trace!("cellSslCertGetNotBefore called with cert_id: {}", cert_id);

    // Get validity from global manager
    let _validity = crate::context::get_hle_context().ssl.get_validity(cert_id);

    // Note: Writing validity start date requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetNotAfter - Get certificate validity end date
pub fn cell_ssl_cert_get_not_after(
    cert_id: SslCertId,
    _limit: *mut u64,
) -> i32 {
    trace!("cellSslCertGetNotAfter called with cert_id: {}", cert_id);

    // Get validity from global manager
    let _validity = crate::context::get_hle_context().ssl.get_validity(cert_id);

    // Note: Writing validity end date requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetSubjectName - Get certificate subject name
pub fn cell_ssl_cert_get_subject_name(
    cert_id: SslCertId,
    _subject: *mut u8,
    _length: *mut u32,
) -> i32 {
    trace!("cellSslCertGetSubjectName called with cert_id: {}", cert_id);

    // Get subject name through global manager
    let _result = crate::context::get_hle_context().ssl.get_subject_name(cert_id);

    // Note: Writing subject name requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertGetIssuerName - Get certificate issuer name
pub fn cell_ssl_cert_get_issuer_name(
    cert_id: SslCertId,
    _issuer: *mut u8,
    _length: *mut u32,
) -> i32 {
    trace!("cellSslCertGetIssuerName called with cert_id: {}", cert_id);

    // Get issuer name through global manager
    let _result = crate::context::get_hle_context().ssl.get_issuer_name(cert_id);

    // Note: Writing issuer name requires memory subsystem integration

    0 // CELL_OK
}

/// cellSslCertUnload - Unload certificate
pub fn cell_ssl_cert_unload(cert_id: SslCertId) -> i32 {
    trace!("cellSslCertUnload called with cert_id: {}", cert_id);

    match crate::context::get_hle_context_mut().ssl.unload_certificate(cert_id) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssl_manager_new() {
        let manager = SslManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.certificate_count(), 0);
        assert_eq!(manager.context_count(), 0);
    }

    #[test]
    fn test_ssl_manager_init_end() {
        let mut manager = SslManager::new();

        manager.init(0x10000).unwrap();
        assert!(manager.is_initialized());

        manager.end().unwrap();
        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_ssl_manager_double_init() {
        let mut manager = SslManager::new();

        manager.init(0x10000).unwrap();
        assert_eq!(manager.init(0x10000), Err(CELL_SSL_ERROR_ALREADY_INITIALIZED));
    }

    #[test]
    fn test_ssl_manager_end_without_init() {
        let mut manager = SslManager::new();

        assert_eq!(manager.end(), Err(CELL_SSL_ERROR_NOT_INITIALIZED));
    }

    #[test]
    fn test_ssl_manager_load_certificate() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        assert!(cert_id > 0);
        assert_eq!(manager.certificate_count(), 1);
    }

    #[test]
    fn test_ssl_manager_unload_certificate() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.unload_certificate(cert_id).unwrap();
        assert_eq!(manager.certificate_count(), 0);
    }

    #[test]
    fn test_ssl_manager_unload_invalid_certificate() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        assert_eq!(manager.unload_certificate(999), Err(CELL_SSL_ERROR_CERT_NOT_FOUND));
    }

    #[test]
    fn test_ssl_manager_add_ca_certificate() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.add_ca_certificate(cert_id).unwrap();
        assert_eq!(manager.ca_certificate_count(), 1);
    }

    #[test]
    fn test_ssl_manager_add_invalid_ca_certificate() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        assert_eq!(manager.add_ca_certificate(999), Err(CELL_SSL_ERROR_CERT_NOT_FOUND));
    }

    #[test]
    fn test_ssl_manager_create_context() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let ctx_id = manager.create_context().unwrap();
        assert!(ctx_id > 0);
        assert_eq!(manager.context_count(), 1);
    }

    #[test]
    fn test_ssl_manager_destroy_context() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let ctx_id = manager.create_context().unwrap();
        manager.destroy_context(ctx_id).unwrap();
        assert_eq!(manager.context_count(), 0);
    }

    #[test]
    fn test_ssl_manager_set_certificate_data() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(cert_id, "CN=Test", "CN=Issuer", &[1, 2, 3, 4]).unwrap();

        assert_eq!(manager.get_subject_name(cert_id).unwrap(), "CN=Test");
        assert_eq!(manager.get_issuer_name(cert_id).unwrap(), "CN=Issuer");
        assert_eq!(manager.get_serial_number(cert_id).unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_ssl_manager_get_validity() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        let (not_before, not_after) = manager.get_validity(cert_id).unwrap();

        assert_eq!(not_before, 0);
        assert_eq!(not_after, u64::MAX);
    }

    #[test]
    fn test_ssl_manager_multiple_certificates() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert1 = manager.load_certificate(CellSslCertType::Pem).unwrap();
        let cert2 = manager.load_certificate(CellSslCertType::Der).unwrap();
        let cert3 = manager.load_certificate(CellSslCertType::Pkcs12).unwrap();

        assert_ne!(cert1, cert2);
        assert_ne!(cert2, cert3);
        assert_eq!(manager.certificate_count(), 3);
    }

    #[test]
    fn test_ssl_lifecycle() {
        // Reset HLE context first to ensure clean state
        crate::context::reset_hle_context();
        assert_eq!(cell_ssl_init(0x10000), 0);
        assert_eq!(cell_ssl_end(), 0);
    }

    #[test]
    fn test_ssl_cert_loader() {
        // Reset and initialize SSL first
        crate::context::reset_hle_context();
        assert_eq!(cell_ssl_init(0x10000), 0);
        
        let mut cert_id = 0;
        let cert_path = b"test.pem\0";
        let mut buffer = vec![0u8; 1024];

        let result = unsafe {
            cell_ssl_certificate_loader(
                &mut cert_id,
                cert_path.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        };

        assert_eq!(result, 0);
        assert!(cert_id > 0);
        
        // Clean up
        assert_eq!(cell_ssl_end(), 0);
    }

    #[test]
    fn test_ssl_cert_types() {
        assert_eq!(CellSslCertType::Pem as u32, 0);
        assert_eq!(CellSslCertType::Der as u32, 1);
        assert_eq!(CellSslCertType::Pkcs12 as u32, 2);
    }

    #[test]
    fn test_ssl_verify_results() {
        assert_eq!(CellSslVerifyResult::Ok as u32, 0);
        assert_eq!(CellSslVerifyResult::Expired as u32, 1);
        assert_eq!(CellSslVerifyResult::NotYetValid as u32, 2);
        assert_eq!(CellSslVerifyResult::IssuerNotFound as u32, 3);
    }

    #[test]
    fn test_ssl_error_codes() {
        assert_ne!(CELL_SSL_ERROR_NOT_INITIALIZED, 0);
        assert_ne!(CELL_SSL_ERROR_ALREADY_INITIALIZED, 0);
        assert_ne!(CELL_SSL_ERROR_INVALID_CERT, 0);
        assert_ne!(CELL_SSL_ERROR_CERT_NOT_FOUND, 0);
    }

    // --- TLS Session Tests ---

    #[test]
    fn test_ssl_create_session() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();
        let ctx_id = manager.create_context().unwrap();

        let session_id = manager.create_session(ctx_id, "example.com").unwrap();
        assert!(session_id > 0);
        assert_eq!(manager.session_count(), 1);
        assert_eq!(manager.get_session_state(session_id).unwrap(), TlsHandshakeState::Initial);
    }

    #[test]
    fn test_ssl_destroy_session() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();
        let ctx_id = manager.create_context().unwrap();

        let session_id = manager.create_session(ctx_id, "example.com").unwrap();
        manager.destroy_session(session_id).unwrap();
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_ssl_create_session_invalid_context() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        assert_eq!(manager.create_session(999, "example.com"), Err(CELL_SSL_ERROR_INVALID_PARAM));
    }

    #[test]
    fn test_ssl_handshake() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();
        let ctx_id = manager.create_context().unwrap();
        let session_id = manager.create_session(ctx_id, "example.com").unwrap();

        let result = manager.perform_handshake(session_id).unwrap();
        assert_eq!(result, TlsHandshakeState::Finished);
        assert_eq!(manager.get_session_state(session_id).unwrap(), TlsHandshakeState::Finished);
    }

    #[test]
    fn test_ssl_handshake_cipher_negotiation() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();
        let ctx_id = manager.create_context().unwrap();
        let session_id = manager.create_session(ctx_id, "secure.example.com").unwrap();

        manager.perform_handshake(session_id).unwrap();

        let cipher = manager.get_session_cipher(session_id).unwrap();
        assert_eq!(cipher, Some(TlsCipherSuite::RsaAes128CbcSha256));
    }

    #[test]
    fn test_ssl_handshake_double_fails() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();
        let ctx_id = manager.create_context().unwrap();
        let session_id = manager.create_session(ctx_id, "example.com").unwrap();

        manager.perform_handshake(session_id).unwrap();
        // Second handshake should fail (not in Initial state)
        assert_eq!(manager.perform_handshake(session_id), Err(CELL_SSL_ERROR_INVALID_PARAM));
    }

    // --- Certificate Validation Tests ---

    #[test]
    fn test_ssl_validate_certificate_ok() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(cert_id, "CN=Test", "CN=CA", &[1, 2, 3]).unwrap();

        // With empty CA store, certificate is accepted
        let result = manager.validate_certificate(cert_id).unwrap();
        assert_eq!(result, CellSslVerifyResult::Ok);
    }

    #[test]
    fn test_ssl_validate_certificate_issuer_not_found() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        // Create a CA certificate
        let ca_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(ca_id, "CN=RealCA", "CN=RealCA", &[1]).unwrap();
        manager.add_ca_certificate(ca_id).unwrap();

        // Create a certificate signed by a different issuer
        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(cert_id, "CN=Server", "CN=UnknownCA", &[2]).unwrap();

        let result = manager.validate_certificate(cert_id).unwrap();
        assert_eq!(result, CellSslVerifyResult::IssuerNotFound);
    }

    #[test]
    fn test_ssl_validate_self_signed_in_ca_store() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let cert_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(cert_id, "CN=SelfSigned", "CN=SelfSigned", &[1]).unwrap();
        manager.add_ca_certificate(cert_id).unwrap();

        let result = manager.validate_certificate(cert_id).unwrap();
        assert_eq!(result, CellSslVerifyResult::Ok);
    }

    #[test]
    fn test_ssl_validate_chain() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        // Root CA
        let root_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(root_id, "CN=Root", "CN=Root", &[1]).unwrap();
        manager.add_ca_certificate(root_id).unwrap();

        // Intermediate CA (signed by Root)
        let inter_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(inter_id, "CN=Intermediate", "CN=Root", &[2]).unwrap();

        // Server cert (signed by Intermediate)
        let server_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(server_id, "CN=Server", "CN=Intermediate", &[3]).unwrap();

        // Validate chain: server → intermediate → root
        let result = manager.validate_chain(&[server_id, inter_id, root_id]).unwrap();
        assert_eq!(result, CellSslVerifyResult::Ok);
    }

    #[test]
    fn test_ssl_validate_chain_broken() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        let root_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(root_id, "CN=Root", "CN=Root", &[1]).unwrap();
        manager.add_ca_certificate(root_id).unwrap();

        // Server cert signed by something not in chain
        let server_id = manager.load_certificate(CellSslCertType::Pem).unwrap();
        manager.set_certificate_data(server_id, "CN=Server", "CN=SomeOtherCA", &[2]).unwrap();

        let result = manager.validate_chain(&[server_id, root_id]).unwrap();
        assert_eq!(result, CellSslVerifyResult::IssuerNotFound);
    }

    #[test]
    fn test_ssl_validate_chain_empty() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();

        assert_eq!(manager.validate_chain(&[]), Err(CELL_SSL_ERROR_INVALID_PARAM));
    }

    // --- DER Parser Tests ---

    #[test]
    fn test_der_tlv_length_short() {
        let data = [0x05]; // length = 5
        let (len, consumed) = DerCertParser::parse_tlv_length(&data).unwrap();
        assert_eq!(len, 5);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_der_tlv_length_one_byte() {
        let data = [0x81, 0xFF]; // length = 255
        let (len, consumed) = DerCertParser::parse_tlv_length(&data).unwrap();
        assert_eq!(len, 255);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_der_tlv_length_two_bytes() {
        let data = [0x82, 0x01, 0x00]; // length = 256
        let (len, consumed) = DerCertParser::parse_tlv_length(&data).unwrap();
        assert_eq!(len, 256);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn test_der_oid_to_name() {
        assert_eq!(DerCertParser::oid_to_name(&[0x55, 0x04, 0x03]), "CN");
        assert_eq!(DerCertParser::oid_to_name(&[0x55, 0x04, 0x06]), "C");
        assert_eq!(DerCertParser::oid_to_name(&[0x55, 0x04, 0x0A]), "O");
        assert_eq!(DerCertParser::oid_to_name(&[0xFF]), "OID");
    }

    #[test]
    fn test_der_parse_utc_time() {
        // "230101000000Z" = Jan 1, 2023 00:00:00 UTC
        let ts = DerCertParser::parse_utc_time("230101000000", false);
        assert!(ts > 0); // Should be ~1672531200
        
        // Generalized time
        let ts2 = DerCertParser::parse_utc_time("20230101000000", true);
        assert!(ts2 > 0);
    }

    #[test]
    fn test_der_parse_invalid() {
        // Too small
        assert!(DerCertParser::parse(&[0x30, 0x01, 0x00]).is_err());
        
        // Wrong tag
        assert!(DerCertParser::parse(&[0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).is_err());
    }

    // --- Base64 / PEM Tests ---

    #[test]
    fn test_base64_decode() {
        let decoded = SslManager::base64_decode("SGVsbG8=").unwrap();
        assert_eq!(decoded, b"Hello");

        let decoded = SslManager::base64_decode("AQID").unwrap();
        assert_eq!(decoded, &[1, 2, 3]);
    }

    #[test]
    fn test_pem_to_der() {
        let pem = "-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----";
        let der = SslManager::pem_to_der(pem).unwrap();
        assert_eq!(der, &[1, 2, 3]);
    }

    #[test]
    fn test_pem_to_der_empty() {
        let pem = "not a certificate";
        assert_eq!(SslManager::pem_to_der(pem), Err(CELL_SSL_ERROR_INVALID_CERT));
    }

    // --- TLS Types Tests ---

    #[test]
    fn test_tls_cipher_suites() {
        assert_eq!(TlsCipherSuite::RsaAes128CbcSha as u16, 0x002F);
        assert_eq!(TlsCipherSuite::RsaAes256CbcSha as u16, 0x0035);
        assert_eq!(TlsCipherSuite::RsaAes128CbcSha256 as u16, 0x003C);
        assert_eq!(TlsCipherSuite::RsaAes256CbcSha256 as u16, 0x003D);
    }

    #[test]
    fn test_tls_handshake_states() {
        assert_ne!(TlsHandshakeState::Initial, TlsHandshakeState::Finished);
        assert_ne!(TlsHandshakeState::ClientHelloSent, TlsHandshakeState::ServerHelloReceived);
    }

    #[test]
    fn test_ssl_session_end_cleans_up() {
        let mut manager = SslManager::new();
        manager.init(0x10000).unwrap();
        let ctx_id = manager.create_context().unwrap();
        manager.create_session(ctx_id, "test.com").unwrap();
        assert_eq!(manager.session_count(), 1);

        manager.end().unwrap();
        assert_eq!(manager.session_count(), 0);
    }
}
