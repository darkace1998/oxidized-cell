//! RSX vertex processing

use bitflags::bitflags;

bitflags! {
    /// Vertex attribute type flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VertexAttributeType: u8 {
        const FLOAT = 1;
        const SHORT = 2;
        const BYTE = 3;
        const HALF_FLOAT = 4;
        const COMPRESSED = 5;
    }
}

/// Vertex attribute descriptor
#[derive(Debug, Clone, Copy)]
pub struct VertexAttribute {
    /// Attribute index (0-15)
    pub index: u8,
    /// Number of components (1-4)
    pub size: u8,
    /// Data type
    pub type_: VertexAttributeType,
    /// Stride between vertices
    pub stride: u16,
    /// Offset into vertex data
    pub offset: u32,
    /// Whether attribute is normalized
    pub normalized: bool,
}

impl VertexAttribute {
    /// Create a new vertex attribute
    pub fn new(index: u8) -> Self {
        Self {
            index,
            size: 4,
            type_: VertexAttributeType::FLOAT,
            stride: 0,
            offset: 0,
            normalized: false,
        }
    }

    /// Get size in bytes for this attribute
    pub fn byte_size(&self) -> u32 {
        let type_size = match self.type_ {
            VertexAttributeType::FLOAT => 4,
            VertexAttributeType::SHORT => 2,
            VertexAttributeType::BYTE => 1,
            VertexAttributeType::HALF_FLOAT => 2,
            VertexAttributeType::COMPRESSED => 4,
            _ => {
                tracing::warn!("Unknown vertex attribute type, defaulting to 4 bytes");
                4
            }
        };
        (self.size as u32) * type_size
    }
}

/// Vertex buffer descriptor
#[derive(Debug, Clone)]
pub struct VertexBuffer {
    /// GPU memory address
    pub address: u32,
    /// Buffer size in bytes
    pub size: u32,
    /// Vertex stride
    pub stride: u16,
}

impl VertexBuffer {
    /// Create a new vertex buffer
    pub fn new(address: u32, size: u32, stride: u16) -> Self {
        Self { address, size, stride }
    }
}

/// Vertex cache for storing processed vertex data
pub struct VertexCache {
    /// Cached vertex buffers
    buffers: Vec<(u32, Vec<u8>)>, // (address, data)
    /// Maximum cache size in bytes
    max_size: usize,
    /// Current cache size
    current_size: usize,
}

impl VertexCache {
    /// Create a new vertex cache
    pub fn new(max_size: usize) -> Self {
        Self {
            buffers: Vec::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Get cached vertex data
    pub fn get(&self, address: u32) -> Option<&[u8]> {
        self.buffers
            .iter()
            .find(|(addr, _)| *addr == address)
            .map(|(_, data)| data.as_slice())
    }

    /// Insert vertex data into cache
    pub fn insert(&mut self, address: u32, data: Vec<u8>) {
        // Remove existing entry if present
        if let Some(pos) = self.buffers.iter().position(|(addr, _)| *addr == address) {
            let (_, old_data) = self.buffers.remove(pos);
            self.current_size -= old_data.len();
        }

        self.current_size += data.len();
        self.buffers.push((address, data));

        // Evict oldest entries if cache is full
        while self.current_size > self.max_size && !self.buffers.is_empty() {
            let (_, old_data) = self.buffers.remove(0);
            self.current_size -= old_data.len();
        }
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.current_size = 0;
    }

    /// Invalidate entries at or after address
    pub fn invalidate(&mut self, address: u32) {
        self.buffers.retain(|(addr, data)| {
            if *addr >= address {
                self.current_size -= data.len();
                false
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_attribute_byte_size() {
        let mut attr = VertexAttribute::new(0);
        attr.type_ = VertexAttributeType::FLOAT;
        attr.size = 3;
        assert_eq!(attr.byte_size(), 12); // 3 floats * 4 bytes

        attr.type_ = VertexAttributeType::SHORT;
        attr.size = 2;
        assert_eq!(attr.byte_size(), 4); // 2 shorts * 2 bytes
    }

    #[test]
    fn test_vertex_cache() {
        let mut cache = VertexCache::new(100);

        let data1 = vec![1, 2, 3, 4];
        cache.insert(0x1000, data1.clone());
        
        assert_eq!(cache.get(0x1000), Some(data1.as_slice()));
        assert_eq!(cache.get(0x2000), None);

        cache.clear();
        assert_eq!(cache.get(0x1000), None);
    }

    #[test]
    fn test_vertex_cache_eviction() {
        let mut cache = VertexCache::new(8);

        cache.insert(0x1000, vec![1, 2, 3, 4, 5]);
        cache.insert(0x2000, vec![6, 7, 8, 9, 10]);
        
        // Should evict first entry since 5 + 5 > 8
        assert_eq!(cache.get(0x1000), None);
        assert!(cache.get(0x2000).is_some());
    }
}
