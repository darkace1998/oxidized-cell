//! PARAM.SFO file format

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

/// SFO file entry
#[derive(Debug, Clone)]
pub enum SfoValue {
    Utf8(String),
    Utf8S(String),
    Integer(u32),
}

/// PARAM.SFO parser
pub struct Sfo {
    entries: HashMap<String, SfoValue>,
}

impl Sfo {
    /// Parse SFO from reader
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self, std::io::Error> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        if &magic != b"\x00PSF" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid SFO magic",
            ));
        }

        let mut header = [0u8; 16];
        reader.read_exact(&mut header)?;

        let _version = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let key_table_start = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        let data_table_start = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
        let entries_count = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);

        let mut entries = HashMap::new();

        for i in 0..entries_count {
            let entry_offset = 20 + i * 16;
            reader.seek(SeekFrom::Start(entry_offset as u64))?;

            let mut entry_data = [0u8; 16];
            reader.read_exact(&mut entry_data)?;

            let key_offset = u16::from_le_bytes([entry_data[0], entry_data[1]]);
            let data_fmt = u16::from_le_bytes([entry_data[2], entry_data[3]]);
            let data_len = u32::from_le_bytes([entry_data[4], entry_data[5], entry_data[6], entry_data[7]]);
            let _data_max_len = u32::from_le_bytes([entry_data[8], entry_data[9], entry_data[10], entry_data[11]]);
            let data_offset = u32::from_le_bytes([entry_data[12], entry_data[13], entry_data[14], entry_data[15]]);

            // Read key
            reader.seek(SeekFrom::Start((key_table_start + key_offset as u32) as u64))?;
            let mut key = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                reader.read_exact(&mut byte)?;
                if byte[0] == 0 {
                    break;
                }
                key.push(byte[0]);
            }
            let key = String::from_utf8_lossy(&key).to_string();

            // Read value
            reader.seek(SeekFrom::Start((data_table_start + data_offset) as u64))?;
            let value = match data_fmt {
                0x0404 => {
                    let mut buf = [0u8; 4];
                    reader.read_exact(&mut buf)?;
                    SfoValue::Integer(u32::from_le_bytes(buf))
                }
                0x0004 | 0x0204 => {
                    let mut buf = vec![0u8; data_len as usize];
                    reader.read_exact(&mut buf)?;
                    // Remove null terminator if present
                    while buf.last() == Some(&0) {
                        buf.pop();
                    }
                    let s = String::from_utf8_lossy(&buf).to_string();
                    if data_fmt == 0x0004 {
                        SfoValue::Utf8S(s)
                    } else {
                        SfoValue::Utf8(s)
                    }
                }
                _ => continue,
            };

            entries.insert(key, value);
        }

        Ok(Self { entries })
    }

    /// Get a string value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.entries.get(key)? {
            SfoValue::Utf8(s) | SfoValue::Utf8S(s) => Some(s),
            _ => None,
        }
    }

    /// Get an integer value
    pub fn get_integer(&self, key: &str) -> Option<u32> {
        match self.entries.get(key)? {
            SfoValue::Integer(v) => Some(*v),
            _ => None,
        }
    }

    /// Get title
    pub fn title(&self) -> Option<&str> {
        self.get_string("TITLE")
    }

    /// Get title ID
    pub fn title_id(&self) -> Option<&str> {
        self.get_string("TITLE_ID")
    }

    /// Get version
    pub fn version(&self) -> Option<&str> {
        self.get_string("VERSION")
    }
}

impl Sfo {
    /// Create a new empty SFO
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a string entry
    pub fn add_string(&mut self, key: &str, value: &str) {
        self.entries.insert(key.to_string(), SfoValue::Utf8(value.to_string()));
    }

    /// Add an integer entry
    pub fn add_integer(&mut self, key: &str, value: u32) {
        self.entries.insert(key.to_string(), SfoValue::Integer(value));
    }

    /// Get all entries
    pub fn entries(&self) -> &HashMap<String, SfoValue> {
        &self.entries
    }

    /// Generate PARAM.SFO binary data
    /// 
    /// PARAM.SFO format:
    /// - Header (20 bytes): Magic (4), Version (4), Key table offset (4), Data table offset (4), Entry count (4)
    /// - Index table: 16 bytes per entry
    /// - Key table: null-terminated strings
    /// - Data table: actual values
    pub fn generate(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Collect and sort entries by key for deterministic output
        let mut sorted_entries: Vec<_> = self.entries.iter().collect();
        sorted_entries.sort_by(|a, b| a.0.cmp(b.0));

        let entry_count = sorted_entries.len() as u32;
        let index_table_size = entry_count * 16;
        let header_size: u32 = 20;

        // Calculate key table and data table offsets
        let key_table_offset = header_size + index_table_size;

        // Calculate key table size (sum of all keys + null terminators)
        let mut key_table_size: u32 = 0;
        for (key, _) in &sorted_entries {
            key_table_size += key.len() as u32 + 1; // +1 for null terminator
        }

        // Align key table to 4 bytes
        let key_table_size_aligned = (key_table_size + 3) & !3;

        let data_table_offset = key_table_offset + key_table_size_aligned;

        // Write header
        data.extend_from_slice(b"\x00PSF"); // Magic
        data.extend_from_slice(&1u32.to_le_bytes()); // Version 1.1 (0x00000101) -> simplified to 1
        data.extend_from_slice(&key_table_offset.to_le_bytes());
        data.extend_from_slice(&data_table_offset.to_le_bytes());
        data.extend_from_slice(&entry_count.to_le_bytes());

        // Build key table and calculate offsets
        let mut key_table = Vec::new();
        let mut key_offsets = Vec::new();

        for (key, _) in &sorted_entries {
            key_offsets.push(key_table.len() as u16);
            key_table.extend_from_slice(key.as_bytes());
            key_table.push(0); // null terminator
        }

        // Pad key table to 4-byte alignment
        while key_table.len() < key_table_size_aligned as usize {
            key_table.push(0);
        }

        // Build data table and index entries
        let mut data_table = Vec::new();
        let mut index_entries = Vec::new();

        for (i, (_, value)) in sorted_entries.iter().enumerate() {
            let key_offset = key_offsets[i];
            let data_offset = data_table.len() as u32;

            let (data_fmt, data_len, data_max_len, value_bytes) = match value {
                SfoValue::Integer(v) => {
                    (0x0404u16, 4u32, 4u32, v.to_le_bytes().to_vec())
                }
                SfoValue::Utf8(s) => {
                    let bytes = s.as_bytes();
                    let len = (bytes.len() + 1) as u32; // +1 for null
                    let max_len = ((len + 3) & !3) as u32; // align to 4
                    let mut value_bytes = bytes.to_vec();
                    value_bytes.push(0); // null terminator
                    while value_bytes.len() < max_len as usize {
                        value_bytes.push(0);
                    }
                    (0x0204u16, len, max_len, value_bytes)
                }
                SfoValue::Utf8S(s) => {
                    let bytes = s.as_bytes();
                    let len = (bytes.len() + 1) as u32;
                    let max_len = ((len + 3) & !3) as u32;
                    let mut value_bytes = bytes.to_vec();
                    value_bytes.push(0);
                    while value_bytes.len() < max_len as usize {
                        value_bytes.push(0);
                    }
                    (0x0004u16, len, max_len, value_bytes)
                }
            };

            // Build index entry (16 bytes)
            let mut entry = Vec::new();
            entry.extend_from_slice(&key_offset.to_le_bytes());
            entry.extend_from_slice(&data_fmt.to_le_bytes());
            entry.extend_from_slice(&data_len.to_le_bytes());
            entry.extend_from_slice(&data_max_len.to_le_bytes());
            entry.extend_from_slice(&data_offset.to_le_bytes());
            index_entries.push(entry);

            data_table.extend(value_bytes);
        }

        // Assemble final data
        for entry in index_entries {
            data.extend(entry);
        }
        data.extend(key_table);
        data.extend(data_table);

        data
    }
}

impl Default for Sfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating PARAM.SFO files
pub struct SfoBuilder {
    sfo: Sfo,
}

impl SfoBuilder {
    /// Create a new SFO builder
    pub fn new() -> Self {
        Self { sfo: Sfo::new() }
    }

    /// Set the game title
    pub fn title(mut self, title: &str) -> Self {
        self.sfo.add_string("TITLE", title);
        self
    }

    /// Set the title ID (e.g., "BLES00000")
    pub fn title_id(mut self, title_id: &str) -> Self {
        self.sfo.add_string("TITLE_ID", title_id);
        self
    }

    /// Set the category (e.g., "SD" for save data, "DG" for disc game)
    pub fn category(mut self, category: &str) -> Self {
        self.sfo.add_string("CATEGORY", category);
        self
    }

    /// Set the savedata directory name
    pub fn savedata_directory(mut self, dir: &str) -> Self {
        self.sfo.add_string("SAVEDATA_DIRECTORY", dir);
        self
    }

    /// Set the detail/description
    pub fn detail(mut self, detail: &str) -> Self {
        self.sfo.add_string("DETAIL", detail);
        self
    }

    /// Set the subtitle
    pub fn subtitle(mut self, subtitle: &str) -> Self {
        self.sfo.add_string("SUB_TITLE", subtitle);
        self
    }

    /// Set the version string
    pub fn version(mut self, version: &str) -> Self {
        self.sfo.add_string("VERSION", version);
        self
    }

    /// Set the app version
    pub fn app_ver(mut self, app_ver: &str) -> Self {
        self.sfo.add_string("APP_VER", app_ver);
        self
    }

    /// Set the parental level (0-11)
    pub fn parental_level(mut self, level: u32) -> Self {
        self.sfo.add_integer("PARENTAL_LEVEL", level);
        self
    }

    /// Set the resolution (bitmask)
    pub fn resolution(mut self, resolution: u32) -> Self {
        self.sfo.add_integer("RESOLUTION", resolution);
        self
    }

    /// Set the sound format (bitmask)
    pub fn sound_format(mut self, format: u32) -> Self {
        self.sfo.add_integer("SOUND_FORMAT", format);
        self
    }

    /// Set the attribute (bitmask)
    pub fn attribute(mut self, attr: u32) -> Self {
        self.sfo.add_integer("ATTRIBUTE", attr);
        self
    }

    /// Add a custom string entry
    pub fn add_string(mut self, key: &str, value: &str) -> Self {
        self.sfo.add_string(key, value);
        self
    }

    /// Add a custom integer entry
    pub fn add_integer(mut self, key: &str, value: u32) -> Self {
        self.sfo.add_integer(key, value);
        self
    }

    /// Build the SFO
    pub fn build(self) -> Sfo {
        self.sfo
    }

    /// Generate PARAM.SFO binary data
    pub fn generate(self) -> Vec<u8> {
        self.sfo.generate()
    }
}

impl Default for SfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_sfo_struct() {
        // Test would require actual SFO data
    }

    #[test]
    fn test_sfo_new() {
        let sfo = Sfo::new();
        assert!(sfo.entries().is_empty());
    }

    #[test]
    fn test_sfo_add_entries() {
        let mut sfo = Sfo::new();
        sfo.add_string("TITLE", "Test Game");
        sfo.add_integer("PARENTAL_LEVEL", 5);

        assert_eq!(sfo.title(), Some("Test Game"));
        assert_eq!(sfo.get_integer("PARENTAL_LEVEL"), Some(5));
    }

    #[test]
    fn test_sfo_builder() {
        let sfo = SfoBuilder::new()
            .title("My Save")
            .title_id("BLES00000")
            .category("SD")
            .savedata_directory("BLES00000-SAVE01")
            .detail("Chapter 5 - 50% Complete")
            .parental_level(3)
            .build();

        assert_eq!(sfo.title(), Some("My Save"));
        assert_eq!(sfo.title_id(), Some("BLES00000"));
        assert_eq!(sfo.get_string("CATEGORY"), Some("SD"));
        assert_eq!(sfo.get_string("SAVEDATA_DIRECTORY"), Some("BLES00000-SAVE01"));
        assert_eq!(sfo.get_string("DETAIL"), Some("Chapter 5 - 50% Complete"));
        assert_eq!(sfo.get_integer("PARENTAL_LEVEL"), Some(3));
    }

    #[test]
    fn test_sfo_generate_and_parse() {
        // Create an SFO with entries
        let original = SfoBuilder::new()
            .title("Test Save Data")
            .title_id("NPUB00001")
            .category("SD")
            .savedata_directory("NPUB00001-SAVE01")
            .parental_level(1)
            .build();

        // Generate binary data
        let data = original.generate();

        // Verify magic bytes
        assert_eq!(&data[0..4], b"\x00PSF");

        // Parse it back
        let mut cursor = Cursor::new(&data);
        let parsed = Sfo::parse(&mut cursor).expect("Failed to parse generated SFO");

        // Verify all fields match
        assert_eq!(parsed.title(), Some("Test Save Data"));
        assert_eq!(parsed.title_id(), Some("NPUB00001"));
        assert_eq!(parsed.get_string("CATEGORY"), Some("SD"));
        assert_eq!(parsed.get_string("SAVEDATA_DIRECTORY"), Some("NPUB00001-SAVE01"));
        assert_eq!(parsed.get_integer("PARENTAL_LEVEL"), Some(1));
    }

    #[test]
    fn test_sfo_generate_empty() {
        let sfo = Sfo::new();
        let data = sfo.generate();

        // Should have valid header even with no entries
        assert_eq!(&data[0..4], b"\x00PSF");
    }

    #[test]
    fn test_sfo_builder_direct_generate() {
        let data = SfoBuilder::new()
            .title("Quick Test")
            .title_id("TEST00001")
            .generate();

        // Parse back to verify
        let mut cursor = Cursor::new(&data);
        let parsed = Sfo::parse(&mut cursor).expect("Failed to parse");
        assert_eq!(parsed.title(), Some("Quick Test"));
        assert_eq!(parsed.title_id(), Some("TEST00001"));
    }
}
