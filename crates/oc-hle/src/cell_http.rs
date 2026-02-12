//! cellHttp HLE - HTTP Client
//!
//! This module provides HLE implementations for the PS3's HTTP client library.
//! Supports HTTP/1.0 and HTTP/1.1 with transaction-based request/response handling.

use std::collections::HashMap;
use tracing::{debug, trace};
use crate::memory::write_be32;

// Error codes
pub const CELL_HTTP_ERROR_NOT_INITIALIZED: i32 = 0x80710001u32 as i32;
pub const CELL_HTTP_ERROR_NOT_INITIALIZED_INITIALIZED: i32 = 0x80710002u32 as i32;
pub const CELL_HTTP_ERROR_INVALID_PARAM: i32 = 0x80710003u32 as i32;
pub const CELL_HTTP_ERROR_NO_MEMORY: i32 = 0x80710004u32 as i32;
pub const CELL_HTTP_ERROR_INVALID_CLIENT: i32 = 0x80710005u32 as i32;
pub const CELL_HTTP_ERROR_INVALID_TRANSACTION: i32 = 0x80710006u32 as i32;
pub const CELL_HTTP_ERROR_NOT_CONNECTED: i32 = 0x80710007u32 as i32;
pub const CELL_HTTP_ERROR_BUSY: i32 = 0x80710008u32 as i32;

/// HTTP client handle
pub type HttpClientId = u32;

/// HTTP transaction handle
pub type HttpTransactionId = u32;

/// HTTP method
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellHttpMethod {
    Get = 0,
    Post = 1,
    Head = 2,
    Put = 3,
    Delete = 4,
    Options = 5,
    Trace = 6,
    Connect = 7,
}

/// HTTP version
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellHttpVersion {
    Http10 = 0,
    Http11 = 1,
}

/// Transaction state
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionState {
    Created,
    RequestSent,
    ResponseReceived,
    Completed,
    Error,
}

/// HTTP status code
pub type CellHttpStatusCode = u32;

/// HTTP header
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CellHttpHeader {
    pub name: [u8; 256],
    pub value: [u8; 1024],
}

impl Default for CellHttpHeader {
    fn default() -> Self {
        Self {
            name: [0; 256],
            value: [0; 1024],
        }
    }
}

/// Transaction entry
#[derive(Debug, Clone)]
struct TransactionEntry {
    method: CellHttpMethod,
    url: String,
    state: TransactionState,
    request_headers: Vec<(String, String)>,
    response_headers: Vec<(String, String)>,
    request_body: Vec<u8>,
    response_body: Vec<u8>,
    status_code: u32,
    content_length: u64,
    bytes_sent: u64,
    bytes_received: u64,
}

impl TransactionEntry {
    fn new(method: CellHttpMethod, url: String) -> Self {
        Self {
            method,
            url,
            state: TransactionState::Created,
            request_headers: Vec::new(),
            response_headers: Vec::new(),
            request_body: Vec::new(),
            response_body: Vec::new(),
            status_code: 0,
            content_length: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    /// Set request body
    fn set_request_body(&mut self, body: Vec<u8>) {
        self.request_body = body;
    }

    /// Get request body
    fn get_request_body(&self) -> &[u8] {
        &self.request_body
    }
}

/// HTTP backend for actual network requests
#[derive(Debug, Clone)]
struct HttpBackend {
    /// Whether to use actual networking or simulation
    use_real_network: bool,
}

/// Response body streamer for handling large downloads incrementally
#[derive(Debug, Clone)]
pub struct ResponseStreamer {
    /// Full response body data
    data: Vec<u8>,
    /// Current read position
    position: usize,
    /// Total content length
    total_length: usize,
    /// Chunk size for streaming delivery (default 8KB)
    chunk_size: usize,
    /// Whether streaming is complete
    is_complete: bool,
}

impl ResponseStreamer {
    /// Create a new response streamer from response data
    pub fn new(data: Vec<u8>) -> Self {
        let total_length = data.len();
        Self {
            data,
            position: 0,
            total_length,
            chunk_size: 8192, // 8KB default chunks
            is_complete: false,
        }
    }

    /// Set the chunk size for streaming delivery
    pub fn set_chunk_size(&mut self, size: usize) {
        self.chunk_size = size.max(1); // Minimum 1 byte
    }

    /// Read the next chunk of data
    pub fn read_chunk(&mut self, max_size: usize) -> Vec<u8> {
        if self.is_complete || self.position >= self.total_length {
            self.is_complete = true;
            return Vec::new();
        }

        let read_size = max_size.min(self.chunk_size).min(self.total_length - self.position);
        let end = self.position + read_size;
        let chunk = self.data[self.position..end].to_vec();
        self.position = end;

        if self.position >= self.total_length {
            self.is_complete = true;
        }

        chunk
    }

    /// Get total content length
    pub fn total_length(&self) -> usize {
        self.total_length
    }

    /// Get bytes already read
    pub fn bytes_read(&self) -> usize {
        self.position
    }

    /// Get remaining bytes
    pub fn remaining(&self) -> usize {
        self.total_length.saturating_sub(self.position)
    }

    /// Check if streaming is complete
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Reset position to beginning for re-reading
    pub fn reset(&mut self) {
        self.position = 0;
        self.is_complete = false;
    }
}

impl HttpBackend {
    fn new() -> Self {
        Self {
            // Use simulation by default for safety
            use_real_network: false,
        }
    }

    /// Enable or disable real networking
    fn set_real_network(&mut self, enable: bool) {
        self.use_real_network = enable;
    }

    /// Send HTTP request — dispatches to real or simulated backend
    fn send_request(
        &self,
        method: &CellHttpMethod,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<HttpResponse, i32> {
        trace!("HttpBackend::send_request: {:?} {} (body: {} bytes)", method, url, body.len());

        if self.use_real_network {
            return self.send_real_request(method, url, headers, body);
        }

        // Simulate HTTP response based on method and URL
        self.send_simulated_request(method, url, body)
    }

    /// Send a real HTTP/1.1 request via std::net::TcpStream
    fn send_real_request(
        &self,
        method: &CellHttpMethod,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<HttpResponse, i32> {
        use std::io::{Read, Write, BufRead, BufReader};
        use std::net::TcpStream;
        use std::time::Duration;

        // Parse URL into host, port, path
        let (host, port, path) = Self::parse_url(url)?;
        
        trace!("HttpBackend::send_real_request: connecting to {}:{}", host, port);

        // Connect with timeout
        let addr = format!("{}:{}", host, port);
        
        // Resolve address using ToSocketAddrs (handles both IP and hostname)
        use std::net::ToSocketAddrs;
        let socket_addr = addr.to_socket_addrs()
            .map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?
            .next()
            .ok_or(CELL_HTTP_ERROR_NOT_CONNECTED)?;
        
        let stream = TcpStream::connect_timeout(
            &socket_addr,
            Duration::from_secs(30),
        ).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;

        stream.set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;

        let mut stream = stream;

        // Build HTTP/1.1 request
        let method_str = match method {
            CellHttpMethod::Get => "GET",
            CellHttpMethod::Post => "POST",
            CellHttpMethod::Head => "HEAD",
            CellHttpMethod::Put => "PUT",
            CellHttpMethod::Delete => "DELETE",
            CellHttpMethod::Options => "OPTIONS",
            CellHttpMethod::Trace => "TRACE",
            CellHttpMethod::Connect => "CONNECT",
        };

        let mut request = format!("{} {} HTTP/1.1\r\nHost: {}\r\n", method_str, path, host);

        // Add user headers
        for (name, value) in headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }

        // Add Content-Length for body
        if !body.is_empty() {
            request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }

        // Add Connection: close
        request.push_str("Connection: close\r\n\r\n");

        // Write request
        stream.write_all(request.as_bytes()).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
        if !body.is_empty() {
            stream.write_all(body).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
        }
        stream.flush().map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;

        // Read response
        let mut reader = BufReader::new(&stream);

        // Parse status line: "HTTP/1.1 200 OK\r\n"
        let mut status_line = String::new();
        reader.read_line(&mut status_line).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;

        let (status_code, reason) = Self::parse_status_line(&status_line)?;

        // Parse response headers
        let mut response_headers = Vec::new();
        let mut content_length: Option<usize> = None;
        let mut is_chunked = false;

        loop {
            let mut header_line = String::new();
            reader.read_line(&mut header_line).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
            let trimmed = header_line.trim();
            if trimmed.is_empty() {
                break; // End of headers
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                let name = name.trim().to_string();
                let value = value.trim().to_string();
                if name.eq_ignore_ascii_case("Content-Length") {
                    content_length = value.parse().ok();
                }
                if name.eq_ignore_ascii_case("Transfer-Encoding") && value.eq_ignore_ascii_case("chunked") {
                    is_chunked = true;
                }
                response_headers.push((name, value));
            }
        }

        // Read response body
        let response_body = if is_chunked {
            Self::read_chunked_body(&mut reader)?
        } else if let Some(len) = content_length {
            let mut body_buf = vec![0u8; len];
            reader.read_exact(&mut body_buf).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
            body_buf
        } else {
            // Read until EOF
            let mut body_buf = Vec::new();
            let _ = reader.read_to_end(&mut body_buf);
            body_buf
        };

        trace!("HttpBackend::send_real_request: status={}, body={} bytes", status_code, response_body.len());

        Ok(HttpResponse {
            status_code,
            reason,
            headers: response_headers,
            body: response_body,
        })
    }

    /// Parse URL into (host, port, path)
    fn parse_url(url: &str) -> Result<(String, u16, String), i32> {
        // Strip protocol
        let (is_https, remainder) = if let Some(rest) = url.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            (false, rest)
        } else {
            (false, url)
        };

        let default_port: u16 = if is_https { 443 } else { 80 };

        // Split host from path
        let (host_port, path) = match remainder.find('/') {
            Some(idx) => (&remainder[..idx], &remainder[idx..]),
            None => (remainder, "/"),
        };

        // Split host from port
        let (host, port) = match host_port.rfind(':') {
            Some(idx) => {
                let port_str = &host_port[idx + 1..];
                let port = port_str.parse::<u16>().unwrap_or(default_port);
                (&host_port[..idx], port)
            }
            None => (host_port, default_port),
        };

        Ok((host.to_string(), port, path.to_string()))
    }

    /// Parse HTTP status line: "HTTP/1.1 200 OK"
    fn parse_status_line(line: &str) -> Result<(u32, String), i32> {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 2 {
            return Err(CELL_HTTP_ERROR_NOT_CONNECTED);
        }

        let status_code = parts[1].parse::<u32>().map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
        let reason = if parts.len() >= 3 {
            parts[2].trim().to_string()
        } else {
            String::new()
        };

        Ok((status_code, reason))
    }

    /// Read chunked transfer-encoded body
    fn read_chunked_body(reader: &mut impl std::io::BufRead) -> Result<Vec<u8>, i32> {
        let mut body = Vec::new();

        loop {
            let mut size_line = String::new();
            reader.read_line(&mut size_line).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
            // Strip chunk extensions (everything after ';') per RFC 7230 §4.1
            let size_str = size_line.trim().split(';').next().unwrap_or("0").trim();
            let chunk_size = usize::from_str_radix(size_str, 16)
                .map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
            
            if chunk_size == 0 {
                // Read trailing \r\n
                let mut trailing = String::new();
                let _ = reader.read_line(&mut trailing);
                break;
            }

            let mut chunk = vec![0u8; chunk_size];
            reader.read_exact(&mut chunk).map_err(|_| CELL_HTTP_ERROR_NOT_CONNECTED)?;
            body.extend_from_slice(&chunk);

            // Read trailing \r\n after chunk data
            let mut trailing = String::new();
            let _ = reader.read_line(&mut trailing);
        }

        Ok(body)
    }

    /// Send simulated HTTP response
    fn send_simulated_request(
        &self,
        method: &CellHttpMethod,
        _url: &str,
        body: &[u8],
    ) -> Result<HttpResponse, i32> {
        let (status_code, reason, content_type, response_body) = match *method {
            CellHttpMethod::Get => {
                let body = b"<html><body>Hello, World!</body></html>".to_vec();
                (200, "OK", "text/html; charset=UTF-8", body)
            }
            CellHttpMethod::Post => {
                let msg = format!("{{\"received\": {} }}", body.len());
                (200, "OK", "application/json", msg.into_bytes())
            }
            CellHttpMethod::Head => {
                (200, "OK", "text/html; charset=UTF-8", vec![])
            }
            CellHttpMethod::Put => {
                let msg = format!("{{\"updated\": true, \"size\": {} }}", body.len());
                (200, "OK", "application/json", msg.into_bytes())
            }
            CellHttpMethod::Delete => {
                (204, "No Content", "application/json", vec![])
            }
            CellHttpMethod::Options => {
                (200, "OK", "text/plain", b"GET,POST,PUT,DELETE,HEAD,OPTIONS".to_vec())
            }
            _ => {
                (200, "OK", "text/plain", b"OK".to_vec())
            }
        };
        
        Ok(HttpResponse {
            status_code,
            reason: String::from(reason),
            headers: vec![
                (String::from("Content-Type"), String::from(content_type)),
                (String::from("Content-Length"), response_body.len().to_string()),
                (String::from("Connection"), String::from("close")),
            ],
            body: response_body,
        })
    }

    /// Send request with proxy
    fn send_request_with_proxy(
        &self,
        method: &CellHttpMethod,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
        proxy_host: &str,
        proxy_port: u16,
    ) -> Result<HttpResponse, i32> {
        trace!("HttpBackend::send_request_with_proxy: {:?} {} via {}:{}", 
               method, url, proxy_host, proxy_port);

        if self.use_real_network {
            trace!("HttpBackend: proxy not implemented, using direct request");
        }

        // Fall back to regular request
        self.send_request(method, url, headers, body)
    }
}

/// HTTP response
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct HttpResponse {
    status_code: u32,
    reason: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// Client entry
#[allow(dead_code)]
#[derive(Debug)]
struct ClientEntry {
    transactions: HashMap<HttpTransactionId, TransactionEntry>,
    next_transaction_id: HttpTransactionId,
    proxy_host: Option<String>,
    proxy_port: u16,
    timeout: u32,
    version: CellHttpVersion,
    /// HTTP backend
    backend: HttpBackend,
}

impl ClientEntry {
    fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            next_transaction_id: 1,
            proxy_host: None,
            proxy_port: 0,
            timeout: 30000, // 30 seconds default
            version: CellHttpVersion::Http11,
            backend: HttpBackend::new(),
        }
    }
}

/// HTTP manager
pub struct HttpManager {
    is_initialized: bool,
    pool_size: u32,
    clients: HashMap<HttpClientId, ClientEntry>,
    next_client_id: HttpClientId,
}

impl HttpManager {
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            pool_size: 0,
            clients: HashMap::new(),
            next_client_id: 1,
        }
    }

    /// Initialize HTTP library
    pub fn init(&mut self, pool_size: u32) -> Result<(), i32> {
        if self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED_INITIALIZED);
        }

        if pool_size == 0 {
            return Err(CELL_HTTP_ERROR_INVALID_PARAM);
        }

        self.is_initialized = true;
        self.pool_size = pool_size;

        Ok(())
    }

    /// Shutdown HTTP library
    pub fn end(&mut self) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        self.clients.clear();
        self.is_initialized = false;

        Ok(())
    }

    /// Create HTTP client
    pub fn create_client(&mut self) -> Result<HttpClientId, i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let id = self.next_client_id;
        self.next_client_id += 1;

        self.clients.insert(id, ClientEntry::new());

        Ok(id)
    }

    /// Destroy HTTP client
    pub fn destroy_client(&mut self, client_id: HttpClientId) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        self.clients.remove(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        Ok(())
    }

    /// Create HTTP transaction
    pub fn create_transaction(&mut self, client_id: HttpClientId, method: CellHttpMethod, url: &str) -> Result<HttpTransactionId, i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;

        let transaction_id = client.next_transaction_id;
        client.next_transaction_id += 1;

        let transaction = TransactionEntry::new(method, url.to_string());
        client.transactions.insert(transaction_id, transaction);

        Ok(transaction_id)
    }

    /// Destroy HTTP transaction
    pub fn destroy_transaction(&mut self, client_id: HttpClientId, transaction_id: HttpTransactionId) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        client.transactions.remove(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        Ok(())
    }

    /// Add request header
    pub fn add_request_header(&mut self, client_id: HttpClientId, transaction_id: HttpTransactionId, name: &str, value: &str) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get_mut(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        if transaction.state != TransactionState::Created {
            return Err(CELL_HTTP_ERROR_BUSY);
        }

        transaction.request_headers.push((name.to_string(), value.to_string()));

        Ok(())
    }

    /// Send HTTP request
    pub fn send_request(&mut self, client_id: HttpClientId, transaction_id: HttpTransactionId, data_size: u64) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get_mut(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        if transaction.state != TransactionState::Created {
            return Err(CELL_HTTP_ERROR_BUSY);
        }

        // Get request body from transaction
        let request_body = transaction.get_request_body().to_vec();
        
        // Send request through backend with actual body
        let response = if let (Some(proxy_host), proxy_port) = (&client.proxy_host, client.proxy_port) {
            client.backend.send_request_with_proxy(
                &transaction.method,
                &transaction.url,
                &transaction.request_headers,
                &request_body,
                proxy_host,
                proxy_port,
            )?
        } else {
            client.backend.send_request(
                &transaction.method,
                &transaction.url,
                &transaction.request_headers,
                &request_body,
            )?
        };

        // Update transaction with response
        transaction.bytes_sent = data_size;
        transaction.state = TransactionState::RequestSent;
        transaction.status_code = response.status_code;
        transaction.response_headers = response.headers;
        transaction.response_body = response.body;
        transaction.content_length = transaction.response_body.len() as u64;
        transaction.state = TransactionState::ResponseReceived;

        trace!("HttpManager::send_request: {} {} -> status {} (response: {} bytes)", 
               transaction.method as u32, transaction.url, transaction.status_code, transaction.content_length);

        Ok(())
    }

    /// Receive HTTP response
    pub fn recv_response(&mut self, client_id: HttpClientId, transaction_id: HttpTransactionId, buffer_size: u64) -> Result<(u64, Vec<u8>), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get_mut(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        if transaction.state != TransactionState::ResponseReceived {
            return Err(CELL_HTTP_ERROR_NOT_CONNECTED);
        }

        // Calculate how many bytes we can return
        let remaining = transaction.content_length.saturating_sub(transaction.bytes_received);
        let bytes_to_read = std::cmp::min(buffer_size, remaining);
        
        // Get the response body slice
        let start = transaction.bytes_received as usize;
        let end = start + bytes_to_read as usize;
        let data = if end <= transaction.response_body.len() {
            transaction.response_body[start..end].to_vec()
        } else {
            Vec::new()
        };
        
        transaction.bytes_received += bytes_to_read;

        if transaction.bytes_received >= transaction.content_length {
            transaction.state = TransactionState::Completed;
        }

        trace!("HttpManager::recv_response: {} bytes (total: {}/{})", 
               bytes_to_read, transaction.bytes_received, transaction.content_length);

        Ok((bytes_to_read, data))
    }

    /// Get response status code
    pub fn get_status_code(&self, client_id: HttpClientId, transaction_id: HttpTransactionId) -> Result<u32, i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        Ok(transaction.status_code)
    }

    /// Set proxy
    pub fn set_proxy(&mut self, client_id: HttpClientId, host: &str, port: u16) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        client.proxy_host = Some(host.to_string());
        client.proxy_port = port;

        Ok(())
    }

    /// Set timeout
    pub fn set_timeout(&mut self, client_id: HttpClientId, timeout: u32) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        client.timeout = timeout;

        Ok(())
    }

    /// Get client count
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get transaction count for a client
    pub fn transaction_count(&self, client_id: HttpClientId) -> Result<usize, i32> {
        let client = self.clients.get(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        Ok(client.transactions.len())
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Check if client is valid
    pub fn is_client_valid(&self, client_id: HttpClientId) -> bool {
        self.is_initialized && self.clients.contains_key(&client_id)
    }

    /// Set request body for a transaction
    pub fn set_request_body(&mut self, client_id: HttpClientId, transaction_id: HttpTransactionId, body: Vec<u8>) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get_mut(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        if transaction.state != TransactionState::Created {
            return Err(CELL_HTTP_ERROR_BUSY);
        }

        transaction.set_request_body(body);
        Ok(())
    }

    /// Get response content length for a transaction
    pub fn get_content_length(&self, client_id: HttpClientId, transaction_id: HttpTransactionId) -> Result<u64, i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get(&transaction_id).ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        Ok(transaction.content_length)
    }

    /// Enable or disable real networking for a client
    pub fn enable_real_network(&mut self, client_id: HttpClientId, enable: bool) -> Result<(), i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get_mut(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        client.backend.set_real_network(enable);
        
        debug!("HttpManager::enable_real_network: client={}, enable={}", client_id, enable);
        Ok(())
    }

    /// Create a response streamer for a completed transaction
    ///
    /// The streamer provides chunked access to the response body, useful for
    /// large downloads where the full response shouldn't be buffered at once.
    pub fn create_response_streamer(
        &self,
        client_id: HttpClientId,
        transaction_id: HttpTransactionId,
    ) -> Result<ResponseStreamer, i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get(&transaction_id)
            .ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        if transaction.state != TransactionState::ResponseReceived && 
           transaction.state != TransactionState::Completed {
            return Err(CELL_HTTP_ERROR_NOT_CONNECTED);
        }

        Ok(ResponseStreamer::new(transaction.response_body.clone()))
    }

    /// Get a response header value by name for a transaction
    pub fn get_response_header(
        &self,
        client_id: HttpClientId,
        transaction_id: HttpTransactionId,
        name: &str,
    ) -> Result<Option<String>, i32> {
        if !self.is_initialized {
            return Err(CELL_HTTP_ERROR_NOT_INITIALIZED);
        }

        let client = self.clients.get(&client_id).ok_or(CELL_HTTP_ERROR_INVALID_CLIENT)?;
        let transaction = client.transactions.get(&transaction_id)
            .ok_or(CELL_HTTP_ERROR_INVALID_TRANSACTION)?;

        let value = transaction.response_headers.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone());

        Ok(value)
    }
}

impl Default for HttpManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellHttpInit - Initialize HTTP library
///
/// # Arguments
/// * `poolSize` - Memory pool size
///
/// # Returns
/// * 0 on success
pub fn cell_http_init(pool_size: u32) -> i32 {
    debug!("cellHttpInit(poolSize={})", pool_size);

    match crate::context::get_hle_context_mut().http.init(pool_size) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellHttpEnd - Shutdown HTTP library
///
/// # Returns
/// * 0 on success
pub fn cell_http_end() -> i32 {
    debug!("cellHttpEnd()");

    match crate::context::get_hle_context_mut().http.end() {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellHttpCreateClient - Create HTTP client
///
/// # Arguments
/// * `client` - Client handle address
///
/// # Returns
/// * 0 on success
pub fn cell_http_create_client(client_addr: u32) -> i32 {
    debug!("cellHttpCreateClient(client_addr=0x{:08X})", client_addr);

    match crate::context::get_hle_context_mut().http.create_client() {
        Ok(client_id) => {
            // Write client handle to memory
            if client_addr != 0 {
                if let Err(e) = write_be32(client_addr, client_id) {
                    return e;
                }
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellHttpDestroyClient - Destroy HTTP client
///
/// # Arguments
/// * `client` - Client handle
///
/// # Returns
/// * 0 on success
pub fn cell_http_destroy_client(client: u32) -> i32 {
    debug!("cellHttpDestroyClient(client={})", client);

    match crate::context::get_hle_context_mut().http.destroy_client(client) {
        Ok(_) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellHttpCreateTransaction - Create HTTP transaction
///
/// # Arguments
/// * `client` - Client handle
/// * `method` - HTTP method
/// * `url` - URL address
/// * `transaction` - Transaction handle address
///
/// # Returns
/// * 0 on success
pub fn cell_http_create_transaction(
    client: u32,
    method: u32,
    _url_addr: u32,
    _transaction_addr: u32,
) -> i32 {
    debug!("cellHttpCreateTransaction(client={}, method={})", client, method);

    // Validate client exists through global manager
    if !crate::context::get_hle_context().http.is_client_valid(client) {
        return CELL_HTTP_ERROR_INVALID_CLIENT;
    }

    // Note: URL parsing and transaction creation requires memory subsystem integration

    0 // CELL_OK
}

/// cellHttpDestroyTransaction - Destroy HTTP transaction
///
/// # Arguments
/// * `transaction` - Transaction handle
///
/// # Returns
/// * 0 on success
pub fn cell_http_destroy_transaction(transaction: u32) -> i32 {
    debug!("cellHttpDestroyTransaction(transaction={})", transaction);

    // Validate transaction through global manager
    // Note: Transaction tracking requires full implementation

    0 // CELL_OK
}

/// cellHttpSendRequest - Send HTTP request
///
/// # Arguments
/// * `transaction` - Transaction handle
/// * `data` - Request body data address
/// * `size` - Request body size
///
/// # Returns
/// * 0 on success
pub fn cell_http_send_request(transaction: u32, _data_addr: u32, size: u64) -> i32 {
    trace!("cellHttpSendRequest(transaction={}, size={})", transaction, size);

    // Verify HTTP manager is initialized
    if !crate::context::get_hle_context().http.is_initialized() {
        return CELL_HTTP_ERROR_NOT_INITIALIZED;
    }

    // Note: Actual HTTP request sending requires network backend integration

    0 // CELL_OK
}

/// cellHttpRecvResponse - Receive HTTP response
///
/// # Arguments
/// * `transaction` - Transaction handle
/// * `data` - Response buffer address
/// * `size` - Buffer size
///
/// # Returns
/// * Number of bytes received on success
pub fn cell_http_recv_response(transaction: u32, data_addr: u32, size: u64) -> i64 {
    trace!("cellHttpRecvResponse(transaction={}, data_addr=0x{:08X}, size={})", transaction, data_addr, size);

    // Verify HTTP manager is initialized
    if !crate::context::get_hle_context().http.is_initialized() {
        return 0;
    }

    // For now, we need to find the client that owns this transaction
    // In a full implementation, we would track transaction-to-client mapping
    // For simulation, just return 0 bytes if data can't be written
    
    // If we have valid data address, we could write response data here
    // The actual integration would require knowing which client the transaction belongs to
    
    if data_addr != 0 && size > 0 {
        // Write empty response for now - actual data would come from transaction
        trace!("cellHttpRecvResponse: returning 0 bytes (transaction lookup not implemented)");
    }

    0 // Return 0 bytes - would need full transaction-to-client mapping for actual data
}

/// cellHttpAddRequestHeader - Add request header
///
/// # Arguments
/// * `transaction` - Transaction handle
/// * `name` - Header name address
/// * `value` - Header value address
///
/// # Returns
/// * 0 on success
pub fn cell_http_add_request_header(
    transaction: u32,
    _name_addr: u32,
    _value_addr: u32,
) -> i32 {
    trace!("cellHttpAddRequestHeader(transaction={})", transaction);

    // Verify HTTP manager is initialized
    if !crate::context::get_hle_context().http.is_initialized() {
        return CELL_HTTP_ERROR_NOT_INITIALIZED;
    }

    // Note: Header reading requires memory subsystem integration

    0 // CELL_OK
}

/// cellHttpGetStatusCode - Get response status code
///
/// # Arguments
/// * `transaction` - Transaction handle
/// * `statusCode` - Status code address
///
/// # Returns
/// * 0 on success
pub fn cell_http_get_status_code(transaction: u32, _status_code_addr: u32) -> i32 {
    trace!("cellHttpGetStatusCode(transaction={})", transaction);

    // Verify HTTP manager is initialized
    if !crate::context::get_hle_context().http.is_initialized() {
        return CELL_HTTP_ERROR_NOT_INITIALIZED;
    }

    // Note: Writing status code requires memory subsystem integration

    0 // CELL_OK
}

/// cellHttpGetResponseHeader - Get response header
///
/// # Arguments
/// * `transaction` - Transaction handle
/// * `name` - Header name address
/// * `value` - Header value buffer address
/// * `valueLen` - Buffer length address
///
/// # Returns
/// * 0 on success
pub fn cell_http_get_response_header(
    transaction: u32,
    _name_addr: u32,
    _value_addr: u32,
    _value_len_addr: u32,
) -> i32 {
    trace!("cellHttpGetResponseHeader(transaction={})", transaction);

    // Verify HTTP manager is initialized
    if !crate::context::get_hle_context().http.is_initialized() {
        return CELL_HTTP_ERROR_NOT_INITIALIZED;
    }

    // Note: Header reading requires memory subsystem integration

    0 // CELL_OK
}

/// cellHttpSetProxy - Set HTTP proxy
///
/// # Arguments
/// * `client` - Client handle
/// * `host` - Proxy host address
/// * `port` - Proxy port
///
/// # Returns
/// * 0 on success
pub fn cell_http_set_proxy(client: u32, _host_addr: u32, port: u16) -> i32 {
    debug!("cellHttpSetProxy(client={}, port={})", client, port);

    // Validate client exists through global manager
    if !crate::context::get_hle_context().http.is_client_valid(client) {
        return CELL_HTTP_ERROR_INVALID_CLIENT;
    }

    // Note: Proxy configuration requires network backend integration

    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_manager_new() {
        let manager = HttpManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.client_count(), 0);
    }

    #[test]
    fn test_http_manager_init_end() {
        let mut manager = HttpManager::new();

        manager.init(1024 * 1024).unwrap();
        assert!(manager.is_initialized());

        manager.end().unwrap();
        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_http_manager_double_init() {
        let mut manager = HttpManager::new();

        manager.init(1024 * 1024).unwrap();
        assert_eq!(manager.init(1024 * 1024), Err(CELL_HTTP_ERROR_NOT_INITIALIZED_INITIALIZED));
    }

    #[test]
    fn test_http_manager_end_without_init() {
        let mut manager = HttpManager::new();

        assert_eq!(manager.end(), Err(CELL_HTTP_ERROR_NOT_INITIALIZED));
    }

    #[test]
    fn test_http_manager_init_zero_pool() {
        let mut manager = HttpManager::new();

        assert_eq!(manager.init(0), Err(CELL_HTTP_ERROR_INVALID_PARAM));
    }

    #[test]
    fn test_http_manager_create_client() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();

        let client_id = manager.create_client().unwrap();
        assert!(client_id > 0);
        assert_eq!(manager.client_count(), 1);
    }

    #[test]
    fn test_http_manager_destroy_client() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();

        let client_id = manager.create_client().unwrap();
        manager.destroy_client(client_id).unwrap();
        assert_eq!(manager.client_count(), 0);
    }

    #[test]
    fn test_http_manager_destroy_invalid_client() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();

        assert_eq!(manager.destroy_client(999), Err(CELL_HTTP_ERROR_INVALID_CLIENT));
    }

    #[test]
    fn test_http_manager_create_transaction() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();

        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();
        assert!(transaction_id > 0);
        assert_eq!(manager.transaction_count(client_id).unwrap(), 1);
    }

    #[test]
    fn test_http_manager_destroy_transaction() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        manager.destroy_transaction(client_id, transaction_id).unwrap();
        assert_eq!(manager.transaction_count(client_id).unwrap(), 0);
    }

    #[test]
    fn test_http_manager_add_header() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        manager.add_request_header(client_id, transaction_id, "Content-Type", "application/json").unwrap();
    }

    #[test]
    fn test_http_manager_send_request() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        manager.send_request(client_id, transaction_id, 0).unwrap();
        assert_eq!(manager.get_status_code(client_id, transaction_id).unwrap(), 200);
    }

    #[test]
    fn test_http_manager_set_proxy() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();

        manager.set_proxy(client_id, "proxy.example.com", 8080).unwrap();
    }

    #[test]
    fn test_http_manager_set_timeout() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();

        manager.set_timeout(client_id, 60000).unwrap();
    }

    #[test]
    fn test_http_init() {
        let result = cell_http_init(1024 * 1024);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_http_method() {
        assert_eq!(CellHttpMethod::Get as u32, 0);
        assert_eq!(CellHttpMethod::Post as u32, 1);
        assert_eq!(CellHttpMethod::Head as u32, 2);
        assert_eq!(CellHttpMethod::Put as u32, 3);
        assert_eq!(CellHttpMethod::Delete as u32, 4);
    }

    #[test]
    fn test_http_version() {
        assert_eq!(CellHttpVersion::Http10 as u32, 0);
        assert_eq!(CellHttpVersion::Http11 as u32, 1);
    }

    #[test]
    fn test_http_error_codes() {
        assert_ne!(CELL_HTTP_ERROR_NOT_INITIALIZED, 0);
        assert_ne!(CELL_HTTP_ERROR_INVALID_CLIENT, 0);
        assert_ne!(CELL_HTTP_ERROR_INVALID_TRANSACTION, 0);
        assert_ne!(CELL_HTTP_ERROR_BUSY, 0);
    }

    #[test]
    fn test_http_manager_request_body() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Post, "http://example.com/api").unwrap();

        // Set request body
        let body = b"{'key': 'value'}".to_vec();
        manager.set_request_body(client_id, transaction_id, body.clone()).unwrap();
    }

    #[test]
    fn test_http_manager_recv_response_with_body() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        // Send request
        manager.send_request(client_id, transaction_id, 0).unwrap();
        
        // Verify content length is set (simulated response has content)
        let content_length = manager.get_content_length(client_id, transaction_id).unwrap();
        assert!(content_length > 0, "Content length should be greater than 0");
        
        // Receive response data
        let (bytes, data) = manager.recv_response(client_id, transaction_id, 1024).unwrap();
        assert_eq!(bytes as usize, data.len());
        assert!(bytes > 0, "Should receive some bytes");
    }

    #[test]
    fn test_http_manager_post_with_body() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Post, "http://example.com/api").unwrap();

        // Set request body
        let request_body = b"test data".to_vec();
        manager.set_request_body(client_id, transaction_id, request_body).unwrap();

        // Send request with body
        manager.send_request(client_id, transaction_id, 9).unwrap();
        
        // Check status
        assert_eq!(manager.get_status_code(client_id, transaction_id).unwrap(), 200);
        
        // Receive response
        let (bytes, _data) = manager.recv_response(client_id, transaction_id, 1024).unwrap();
        assert!(bytes > 0);
    }

    #[test]
    fn test_http_manager_head_request() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Head, "http://example.com").unwrap();

        manager.send_request(client_id, transaction_id, 0).unwrap();
        assert_eq!(manager.get_status_code(client_id, transaction_id).unwrap(), 200);
        
        // HEAD returns no body
        let content_length = manager.get_content_length(client_id, transaction_id).unwrap();
        assert_eq!(content_length, 0);
    }

    #[test]
    fn test_http_manager_delete_request() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Delete, "http://example.com/resource/1").unwrap();

        manager.send_request(client_id, transaction_id, 0).unwrap();
        assert_eq!(manager.get_status_code(client_id, transaction_id).unwrap(), 204);
    }

    #[test]
    fn test_response_streamer_basic() {
        let data = b"Hello, World! This is a test response body.".to_vec();
        let streamer = ResponseStreamer::new(data.clone());
        
        assert_eq!(streamer.total_length(), data.len());
        assert_eq!(streamer.bytes_read(), 0);
        assert_eq!(streamer.remaining(), data.len());
        assert!(!streamer.is_complete());
    }

    #[test]
    fn test_response_streamer_read_chunks() {
        let data = b"ABCDEFGHIJKLMNOP".to_vec(); // 16 bytes
        let mut streamer = ResponseStreamer::new(data);
        streamer.set_chunk_size(4);

        let chunk1 = streamer.read_chunk(1024);
        assert_eq!(chunk1, b"ABCD");
        assert_eq!(streamer.bytes_read(), 4);

        let chunk2 = streamer.read_chunk(1024);
        assert_eq!(chunk2, b"EFGH");
        assert_eq!(streamer.bytes_read(), 8);

        let chunk3 = streamer.read_chunk(1024);
        assert_eq!(chunk3, b"IJKL");

        let chunk4 = streamer.read_chunk(1024);
        assert_eq!(chunk4, b"MNOP");
        assert!(streamer.is_complete());

        // No more data
        let chunk5 = streamer.read_chunk(1024);
        assert!(chunk5.is_empty());
    }

    #[test]
    fn test_response_streamer_max_size_limit() {
        let data = b"ABCDEFGHIJKLMNOP".to_vec(); // 16 bytes
        let mut streamer = ResponseStreamer::new(data);
        streamer.set_chunk_size(100);  // Big chunk size

        // But max_size is small
        let chunk = streamer.read_chunk(3);
        assert_eq!(chunk, b"ABC");
    }

    #[test]
    fn test_response_streamer_reset() {
        let data = b"Hello".to_vec();
        let mut streamer = ResponseStreamer::new(data);
        
        let _ = streamer.read_chunk(5);
        assert!(streamer.is_complete());
        
        streamer.reset();
        assert!(!streamer.is_complete());
        assert_eq!(streamer.bytes_read(), 0);
        
        let chunk = streamer.read_chunk(5);
        assert_eq!(chunk, b"Hello");
    }

    #[test]
    fn test_response_streamer_empty() {
        let mut streamer = ResponseStreamer::new(Vec::new());
        assert_eq!(streamer.total_length(), 0);
        assert!(streamer.read_chunk(1024).is_empty());
        assert!(streamer.is_complete());
    }

    #[test]
    fn test_http_manager_create_response_streamer() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        manager.send_request(client_id, transaction_id, 0).unwrap();

        let mut streamer = manager.create_response_streamer(client_id, transaction_id).unwrap();
        assert!(streamer.total_length() > 0);
        
        let chunk = streamer.read_chunk(4096);
        assert!(!chunk.is_empty());
    }

    #[test]
    fn test_http_manager_create_response_streamer_before_send() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        // Should fail — request not sent yet
        assert!(manager.create_response_streamer(client_id, transaction_id).is_err());
    }

    #[test]
    fn test_http_manager_enable_real_network() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();

        // Should succeed
        manager.enable_real_network(client_id, true).unwrap();
        manager.enable_real_network(client_id, false).unwrap();
    }

    #[test]
    fn test_http_manager_enable_real_network_invalid_client() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();

        assert_eq!(manager.enable_real_network(999, true), Err(CELL_HTTP_ERROR_INVALID_CLIENT));
    }

    #[test]
    fn test_http_manager_get_response_header() {
        let mut manager = HttpManager::new();
        manager.init(1024 * 1024).unwrap();
        let client_id = manager.create_client().unwrap();
        let transaction_id = manager.create_transaction(client_id, CellHttpMethod::Get, "http://example.com").unwrap();

        manager.send_request(client_id, transaction_id, 0).unwrap();

        let content_type = manager.get_response_header(client_id, transaction_id, "Content-Type").unwrap();
        assert!(content_type.is_some());
        assert!(content_type.unwrap().contains("text/html"));

        let missing = manager.get_response_header(client_id, transaction_id, "X-Nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_http_url_parsing() {
        let (host, port, path) = HttpBackend::parse_url("http://example.com/path").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        assert_eq!(path, "/path");

        let (host, port, path) = HttpBackend::parse_url("https://secure.example.com:8443/api").unwrap();
        assert_eq!(host, "secure.example.com");
        assert_eq!(port, 8443);
        assert_eq!(path, "/api");

        let (host, port, path) = HttpBackend::parse_url("http://localhost").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 80);
        assert_eq!(path, "/");
    }

    #[test]
    fn test_http_status_line_parsing() {
        let (code, reason) = HttpBackend::parse_status_line("HTTP/1.1 200 OK\r\n").unwrap();
        assert_eq!(code, 200);
        assert_eq!(reason, "OK");

        let (code, reason) = HttpBackend::parse_status_line("HTTP/1.1 404 Not Found\r\n").unwrap();
        assert_eq!(code, 404);
        assert_eq!(reason, "Not Found");

        let (code, _) = HttpBackend::parse_status_line("HTTP/1.0 301 Moved Permanently").unwrap();
        assert_eq!(code, 301);
    }
}
