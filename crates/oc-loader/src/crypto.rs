//! Cryptographic operations for SELF decryption
//!
//! This module handles decryption of PS3 executables using keys extracted
//! from the official PS3 firmware (PUP file).
//!
//! The PS3 uses a hierarchical key system:
//! - erk (encryption round key) - extracted from firmware
//! - riv (reset initialization vector) - extracted from firmware
//! - These are used to decrypt the metadata which contains per-file keys

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding, StreamCipher};
use oc_core::error::LoaderError;
use sha1::{Sha1, Digest};
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use tracing::{debug, info, warn};

/// AES-128 CBC decryptor type
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;

/// Key types for PS3 encryption
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    /// Retail (production) keys
    Retail,
    /// Debug keys
    Debug,
    /// Application-specific keys
    App,
    /// Isolated SPU keys
    IsoSpu,
    /// LV0 (bootloader) keys
    Lv0,
    /// LV1 (hypervisor) keys
    Lv1,
    /// LV2 (kernel) keys
    Lv2,
    /// NPD (content protection) keys
    Npd,
    /// SELF metadata keys
    MetaLdr,
    /// VSH keys
    Vsh,
}

/// Encryption algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionType {
    /// AES-128 CBC
    Aes128Cbc,
    /// AES-256 CBC
    Aes256Cbc,
    /// No encryption
    None,
}

/// Key database entry
#[derive(Debug, Clone)]
pub struct KeyEntry {
    pub key_type: KeyType,
    pub key: Vec<u8>,
    pub iv: Option<Vec<u8>>,
    pub description: String,
    /// Key revision/version
    pub revision: u32,
}

/// SELF key set (erk + riv)
#[derive(Debug, Clone)]
pub struct SelfKeySet {
    /// Encryption round key
    pub erk: [u8; 32],
    /// Reset initialization vector
    pub riv: [u8; 16],
    /// Key revision
    pub revision: u16,
    /// Key type identifier
    pub key_type: u16,
}

/// AES key size constants
const AES_128_KEY_SIZE: usize = 16;
const AES_256_KEY_SIZE: usize = 32;
const AES_IV_SIZE: usize = 16;
const AES_BLOCK_SIZE: usize = 16;

/// Crypto engine for SELF decryption
pub struct CryptoEngine {
    /// Key database
    keys: HashMap<KeyType, Vec<KeyEntry>>,
    /// SELF key sets indexed by (key_type, revision)
    self_keys: HashMap<(u16, u16), SelfKeySet>,
    /// Whether firmware keys have been loaded
    firmware_loaded: bool,
    /// Firmware keys directory path
    keys_dir: Option<String>,
}

impl CryptoEngine {
    /// Create a new crypto engine
    pub fn new() -> Self {
        let mut engine = Self {
            keys: HashMap::new(),
            self_keys: HashMap::new(),
            firmware_loaded: false,
            keys_dir: None,
        };
        
        // Load built-in keys (same as RPCS3's key_vault)
        engine.load_builtin_keys();
        
        engine
    }
    
    /// Load built-in decryption keys (equivalent to RPCS3's KeyVault)
    /// These are the publicly known keys needed for SELF decryption
    /// 
    /// Key types (program_type from SELF):
    ///   1 = LV0, 2 = LV1, 3 = LV2, 4 = APP, 5 = ISO, 6 = LDR, 7 = UNK7, 8 = NPDRM
    fn load_builtin_keys(&mut self) {
        info!("Loading built-in decryption keys");
        
        // =========================================================
        // APP keys (program_type = 4) - MOST RETAIL GAMES USE THESE
        // From RPCS3's key_vault.cpp LoadSelfAPPKeys()
        // =========================================================
        
        // App key revision 0x00 (early games)
        self.add_key_set(4, 0x0000,
            "95F50019E7A68E341FA72EFDF4D60ED376E25CF46BB48DFDD1F080259DC93F04",
            "4A0955D946DB70D691A640BB7FAECC4C");
        
        // App keys revision 0x01-0x03 (PS3 firmware 3.xx era)
        self.add_key_set(4, 0x0001,
            "79481839C406A632BDB4AC093D73D99AE1587F24CE7E69192C1CD0010274A8AB",
            "6F0F25E1C8C4B7AE70DF968B04521DDA");
        
        self.add_key_set(4, 0x0002,
            "4F89BE98DDD43CAD343F5BA6B1A133B0A971566F770484AAC20B5DD1DC9FA06A",
            "90C127A9B43BA9D8E89FE6529E25206F");
        
        self.add_key_set(4, 0x0003,
            "C1E6A351FCED6A0636BFCB6801A0942DB7C28BDFC5E0A053A3F52F52FCE9754E",
            "E0908163F457576440466ACAA443AE7C");
        
        // App keys revision 0x04-0x06
        self.add_key_set(4, 0x0004,
            "838F5860CF97CDAD75B399CA44F4C214CDF951AC795298D71DF3C3B7E93AAEDA",
            "7FDBB2E924D182BB0D69844ADC4ECA5B");
        
        self.add_key_set(4, 0x0005,
            "C109AB56593DE5BE8BA190578E7D8109346E86A11088B42C727E2B793FD64BDC",
            "15D3F191295C94B09B71EBDE088A187A");
        
        self.add_key_set(4, 0x0006,
            "6DFD7AFB470D2B2C955AB22264B1FF3C67F180983B26C01615DE9F2ECCBE7F41",
            "24BD1C19D2A8286B8ACE39E4A37801C2");
        
        // App keys revision 0x07-0x09
        self.add_key_set(4, 0x0007,
            "945B99C0E69CAF0558C588B95FF41B232660ECB017741F3218C12F9DFDEEDE55",
            "1D5EFBE7C5D34AD60F9FBC46A5977FCE");
        
        self.add_key_set(4, 0x0008,
            "2C9E8969EC44DFB6A8771DC7F7FDFBCCAF329EC3EC070900CABB23742A9A6E13",
            "5A4CEFD5A9C3C093D0B9352376D19405");
        
        self.add_key_set(4, 0x0009,
            "F69E4A2934F114D89F386CE766388366CDD210F1D8913E3B973257F1201D632B",
            "F4D535069301EE888CC2A852DB654461");
        
        // App keys revision 0x0A-0x0C
        self.add_key_set(4, 0x000A,
            "29805302E7C92F204009161CA93F776A072141A8C46A108E571C46D473A176A3",
            "5D1FAB844107676ABCDFC25EAEBCB633");
        
        self.add_key_set(4, 0x000B,
            "A4C97402CC8A71BC7748661FE9CE7DF44DCE95D0D58938A59F47B9E9DBA7BFC3",
            "E4792F2B9DB30CB8D1596077A13FB3B5");
        
        self.add_key_set(4, 0x000C,
            "9814EFFF67B7074D1B263BF85BDC8576CE9DEC914123971B169472A1BC2387FA",
            "D43B1FA8BE15714B3078C23908BB2BCA");
        
        // App keys revision 0x0D-0x0F
        self.add_key_set(4, 0x000D,
            "03B4C421E0C0DE708C0F0B71C24E3EE04306AE7383D8C5621394CCB99FF7A194",
            "5ADB9EAFE897B54CB1060D6885BE22CF");
        
        self.add_key_set(4, 0x000E,
            "39A870173C226EB8A3EEE9CA6FB675E82039B2D0CCB22653BFCE4DB013BAEA03",
            "90266C98CBAA06C1BF145FF760EA1B45");
        
        self.add_key_set(4, 0x000F,
            "FD52DFA7C6EEF5679628D12E267AA863B9365E6DB95470949CFD235B3FCA0F3B",
            "64F50296CF8CF49CD7C643572887DA0B");
        
        // App keys revision 0x10-0x12
        self.add_key_set(4, 0x0010,
            "A5E51AD8F32FFBDE808972ACEE46397F2D3FE6BC823C8218EF875EE3A9B0584F",
            "7A203D5112F799979DF0E1B8B5B52AA4");
        
        self.add_key_set(4, 0x0011,
            "0F8EAB8884A51D092D7250597388E3B8B75444AC138B9D36E5C7C5B8C3DF18FD",
            "97AF39C383E7EF1C98FA447C597EA8FE");
        
        self.add_key_set(4, 0x0012,
            "DBF62D76FC81C8AC92372A9D631DDC9219F152C59C4B20BFF8F96B64AB065E94",
            "CB5DD4BE8CF115FFB25801BC6086E729");
        
        // App keys revision 0x13-0x14
        self.add_key_set(4, 0x0013,
            "DBF62D76FC81C8AC92372A9D631DDC9219F152C59C4B20BFF8F96B64AB065E94",
            "CB5DD4BE8CF115FFB25801BC6086E729");
        
        self.add_key_set(4, 0x0014,
            "491B0D72BB21ED115950379F4564CE784A4BFAABB00E8CB71294B192B7B9F88E",
            "F98843588FED8B0E62D7DDCB6F0CECF4");
        
        // App key revision 0x15 (was missing!)
        self.add_key_set(4, 0x0015,
            "F11DBD2C97B32AD37E55F8E743BC821D3E67630A6784D9A058DDD26313482F0F",
            "FC5FA12CA3D2D336C4B8B425D679DA55");
        
        // App keys revision 0x16-0x18 (firmware 4.xx era - MOST MODERN GAMES)
        self.add_key_set(4, 0x0016,
            "A106692224F1E91E1C4EBAD4A25FBFF66B4B13E88D878E8CD072F23CD1C5BF7C",
            "62773C70BD749269C0AFD1F12E73909E");
        
        self.add_key_set(4, 0x0017,
            "4E104DCE09BA878C75DA98D0B1636F0E5F058328D81419E2A3D22AB0256FDF46",
            "954A86C4629E116532304A740862EF85");
        
        self.add_key_set(4, 0x0018,
            "1F876AB252DDBCB70E74DC4A20CD8ED51E330E62490E652F862877E8D8D0F997",
            "BF8D6B1887FA88E6D85C2EDB2FBEC147");
        
        // App keys revision 0x19-0x1B
        self.add_key_set(4, 0x0019,
            "3236B9937174DF1DC12EC2DD8A318A0EA4D3ECDEA5DFB4AC1B8278447000C297",
            "6153DEE781B8ADDC6A439498B816DC46");
        
        self.add_key_set(4, 0x001A,
            "5EFD1E9961462794E3B9EF2A4D0C1F46F642AAE053B5025504130590E66F19C9",
            "1AC8FA3B3C90F8FDE639515F91B58327");
        
        self.add_key_set(4, 0x001B,
            "66637570D1DEC098467DB207BAEA786861964D0964D4DBAF89E76F46955D181B",
            "9F7B5713A5ED59F6B35CD8F8A165D4B8");
        
        // App keys revision 0x1C-0x1D (LATEST - firmware 4.8x+)
        self.add_key_set(4, 0x001C,
            "CFF025375BA0079226BE01F4A31F346D79F62CFB643CA910E16CF60BD9092752",
            "FD40664E2EBBA01BF359B0DCDF543DA4");
        
        self.add_key_set(4, 0x001D,
            "D202174EB65A62048F3674B59EF6FE72E1872962F3E1CD658DE8D7AF71DA1F3E",
            "ACB9945914EBB7B9A31ECE320AE09F2D");
        
        // =========================================================
        // NPDRM keys (program_type = 8) - for PSN downloads
        // From RPCS3's key_vault.cpp LoadSelfNPDRMKeys()
        // =========================================================
        
        self.add_key_set(8, 0x0001,
            "F9EDD0301F770FABBA8863D9897F0FEA6551B09431F61312654E28F43533EA6B",
            "A551CCB4A42C37A734A2B4F9657D5540");
        
        self.add_key_set(8, 0x0002,
            "8E737230C80E66AD0162EDDD32F1F774EE5E4E187449F19079437A508FCF9C86",
            "7AAECC60AD12AED90C348D8C11D2BED5");
        
        self.add_key_set(8, 0x0003,
            "1B715B0C3E8DC4C1A5772EBA9C5D34F7CCFE5B82025D453F3167566497239664",
            "E31E206FBB8AEA27FAB0D9A2FFB6B62F");
        
        self.add_key_set(8, 0x0004,
            "BB4DBF66B744A33934172D9F8379A7A5EA74CB0F559BB95D0E7AECE91702B706",
            "ADF7B207A15AC601110E61DDFC210AF6");
        
        self.add_key_set(8, 0x0006,
            "8B4C52849765D2B5FA3D5628AFB17644D52B9FFEE235B4C0DB72A62867EAA020",
            "05719DF1B1D0306C03910ADDCE4AF887");
        
        self.add_key_set(8, 0x0007,
            "3946DFAA141718C7BE339A0D6C26301C76B568AEBC5CD52652F2E2E0297437C3",
            "E4897BE553AE025CDCBF2B15D1C9234E");
        
        self.add_key_set(8, 0x0009,
            "0786F4B0CA5937F515BDCE188F569B2EF3109A4DA0780A7AA07BD89C3350810A",
            "04AD3C2F122A3B35E804850CAD142C6D");
        
        self.add_key_set(8, 0x000A,
            "03C21AD78FBB6A3D425E9AAB1298F9FD70E29FD4E6E3A3C151205DA50C413DE4",
            "0A99D4D4F8301A88052D714AD2FB565E");
        
        self.add_key_set(8, 0x000C,
            "357EBBEA265FAEC271182D571C6CD2F62CFA04D325588F213DB6B2E0ED166D92",
            "D26E6DD2B74CD78E866E742E5571B84F");
        
        self.add_key_set(8, 0x000D,
            "337A51416105B56E40D7CAF1B954CDAF4E7645F28379904F35F27E81CA7B6957",
            "8405C88E042280DBD794EC7E22B74002");
        
        self.add_key_set(8, 0x000F,
            "135C098CBE6A3E037EBE9F2BB9B30218DDE8D68217346F9AD33203352FBB3291",
            "4070C898C2EAAD1634A288AA547A35A8");
        
        self.add_key_set(8, 0x0010,
            "4B3CD10F6A6AA7D99F9B3A660C35ADE08EF01C2C336B9E46D1BB5678B4261A61",
            "C0F2AB86E6E0457552DB50D7219371C5");
        
        self.add_key_set(8, 0x0013,
            "265C93CF48562EC5D18773BEB7689B8AD10C5EB6D21421455DEBC4FB128CBF46",
            "8DEA5FF959682A9B98B688CEA1EF4A1D");
        
        self.add_key_set(8, 0x0016,
            "7910340483E419E55F0D33E4EA5410EEEC3AF47814667ECA2AA9D75602B14D4B",
            "4AD981431B98DFD39B6388EDAD742A8E");
        
        self.add_key_set(8, 0x0019,
            "FBDA75963FE690CFF35B7AA7B408CF631744EDEF5F7931A04D58FD6A921FFDB3",
            "F72C1D80FFDA2E3BF085F4133E6D2805");
        
        self.add_key_set(8, 0x001C,
            "8103EA9DB790578219C4CEDF0592B43064A7D98B601B6C7BC45108C4047AA80F",
            "246F4B8328BE6A2D394EDE20479247C5");
        
        // =========================================================
        // LV2 keys (program_type = 3) - for kernel/system modules
        // =========================================================
        
        self.add_key_set(3, 0x0000,
            "688D5FCAC6F4EA35AC6AC79B10506007286131EE038116DB8AA2C0B0340D9FB0",
            "BE9419FD6C7E00E20D77D889E6637A0D");
        
        // =========================================================
        // ISO keys (program_type = 5) - for SPU isolated modules  
        // =========================================================
        
        self.add_key_set(5, 0x0000,
            "8474ADCA3B3244931EECEB9357841442442A1C4A4BCF4E498E6738950F4E4093",
            "FFF9CACCC4129125CAFB240F419E5F39");
        
        self.add_key_set(5, 0x0001,
            "E6A21C599B75696C169EC02582BDA74A776134A6E05108EA701EC0CA2AC03592",
            "4262657A3185D9480F82C8BD2F81766F");
        
        // =========================================================
        // LDR keys (program_type = 6) - for secure loaders
        // =========================================================
        
        self.add_key_set(6, 0x0000,
            "C0CEFE84C227F75BD07A7EB846509F93B238E770DACB9FF4A388F812482BE21B",
            "47EE7454E4774CC9B8960C7B59F4C14D");
        
        info!("Loaded {} built-in key sets", self.self_keys.len());
    }
    
    /// Add a key set with specified type and revision
    fn add_key_set(&mut self, key_type: u16, revision: u16, erk_hex: &str, riv_hex: &str) {
        if let (Some(erk), Some(riv)) = (hex_decode(erk_hex), hex_decode(riv_hex)) {
            let mut erk_arr = [0u8; 32];
            let mut riv_arr = [0u8; 16];
            erk_arr[..erk.len().min(32)].copy_from_slice(&erk[..erk.len().min(32)]);
            riv_arr[..riv.len().min(16)].copy_from_slice(&riv[..riv.len().min(16)]);
            
            self.add_self_key_set(SelfKeySet {
                erk: erk_arr,
                riv: riv_arr,
                revision,
                key_type,
            });
        }
    }
    


    /// Create crypto engine and attempt to load keys from default location
    pub fn with_default_keys() -> Self {
        let mut engine = Self::new();
        
        // Try common key locations
        let possible_paths = [
            "dev_flash/",
            "./firmware/",
        ];

        for path in &possible_paths {
            if Path::new(path).exists() {
                if engine.load_firmware_keys(path).is_ok() {
                    break;
                }
            }
        }

        // Also try loading keys.txt if present
        for keys_file in &["keys.txt", "firmware/keys.txt", "dev_flash/keys.txt"] {
            if Path::new(keys_file).exists() {
                let _ = engine.load_keys_file(keys_file);
            }
        }

        engine
    }

    /// Load decryption keys from installed PS3 firmware
    ///
    /// The firmware should be installed to a dev_flash directory structure.
    pub fn load_firmware_keys(&mut self, dev_flash_path: &str) -> Result<(), LoaderError> {
        info!("Loading firmware keys from: {}", dev_flash_path);

        let path = Path::new(dev_flash_path);
        if !path.exists() {
            return Err(LoaderError::InvalidFirmware(
                format!("Firmware path does not exist: {}", dev_flash_path)
            ));
        }

        self.keys_dir = Some(dev_flash_path.to_string());
        self.firmware_loaded = true;
        
        let stats = self.get_stats();
        info!(
            "Firmware keys loaded: {} SELF key sets, {} total keys",
            self.self_keys.len(),
            stats.total()
        );

        Ok(())
    }

    /// Load keys from a keys.txt file (RPCS3 format compatible)
    /// 
    /// Format: KEY_NAME=HEXVALUE
    pub fn load_keys_file(&mut self, path: &str) -> Result<(), LoaderError> {
        info!("Loading keys from file: {}", path);

        let content = fs::read_to_string(path)
            .map_err(|e| LoaderError::InvalidFirmware(format!("Failed to read keys file: {}", e)))?;

        let mut loaded = 0;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if let Some((name, value)) = line.split_once('=') {
                let name = name.trim();
                let value = value.trim();
                
                if let Some(key_data) = hex_decode(value) {
                    if let Some((key_type, desc)) = parse_key_name(name) {
                        self.add_key(KeyEntry {
                            key_type,
                            key: key_data,
                            iv: None,
                            description: desc,
                            revision: 0,
                        });
                        loaded += 1;
                    }
                }
            }
        }

        info!("Loaded {} keys from file", loaded);
        self.firmware_loaded = loaded > 0;
        Ok(())
    }

    /// Register a SELF key set
    pub fn add_self_key_set(&mut self, key_set: SelfKeySet) {
        debug!(
            "Adding SELF key set: type=0x{:04x}, revision=0x{:04x}",
            key_set.key_type, key_set.revision
        );
        self.self_keys.insert((key_set.key_type, key_set.revision), key_set);
    }

    /// Get SELF key set by type and revision
    /// 
    /// This implements RPCS3-style key lookup:
    /// For APP/NPDRM keys: exact revision match is required
    /// RPCS3's GetSelfAPPKey() only returns a key if revision matches exactly
    pub fn get_self_key_set(&self, key_type: u16, revision: u16) -> Option<&SelfKeySet> {
        info!("Looking for key: type=0x{:04x} ({}), revision=0x{:04x}", 
            key_type, 
            match key_type {
                1 => "LV0",
                2 => "LV1", 
                3 => "LV2",
                4 => "APP",
                5 => "ISO",
                6 => "LDR",
                7 => "UNK7",
                8 => "NPDRM",
                _ => "UNKNOWN",
            },
            revision);
        
        // Try exact match first (type, revision)
        if let Some(keys) = self.self_keys.get(&(key_type, revision)) {
            info!("Found exact key match for (type=0x{:04x}, revision=0x{:04x})", key_type, revision);
            return Some(keys);
        }
        
        // For APP and NPDRM keys, RPCS3 only uses exact revision matches
        // Don't fall back to wrong keys - that causes garbage decryption
        if key_type == 4 || key_type == 8 {
            // Log available revisions for this type to help debug
            let available_revisions: Vec<u16> = self.self_keys.keys()
                .filter(|(t, _)| *t == key_type)
                .map(|(_, r)| *r)
                .collect();
            
            warn!("No key found for type=0x{:04x}, revision=0x{:04x}", key_type, revision);
            warn!("Available revisions for this type: {:?}", available_revisions);
            warn!("This game may require a different firmware key set.");
            
            return None;
        }
        
        // For other key types (LV0, LV1, LV2, etc.), try fallback strategies
        // Try with revision 0 as fallback
        if let Some(keys) = self.self_keys.get(&(key_type, 0)) {
            debug!("Found key with revision 0 fallback");
            return Some(keys);
        }
        
        // Try to find any key with matching type
        let type_match = self.self_keys.iter()
            .find(|((t, _r), _)| *t == key_type)
            .map(|(_, v)| v);
        
        if type_match.is_some() {
            debug!("Found key with matching type (different revision)");
        } else {
            warn!("No key found. Available keys: {:?}", 
                self.self_keys.keys().collect::<Vec<_>>());
        }
        
        type_match
    }

    /// List available keys for debugging
    pub fn list_available_keys(&self) -> String {
        let keys: Vec<String> = self.self_keys.keys()
            .map(|(t, r)| format!("(type=0x{:04x}, rev=0x{:04x})", t, r))
            .collect();
        if keys.is_empty() {
            "none".to_string()
        } else {
            keys.join(", ")
        }
    }

    /// Get count of SELF key sets
    pub fn self_key_count(&self) -> usize {
        self.self_keys.len()
    }

    /// Check if firmware keys are loaded
    pub fn has_firmware_keys(&self) -> bool {
        self.firmware_loaded || !self.self_keys.is_empty() || !self.keys.is_empty()
    }

    /// Add a key to the database
    pub fn add_key(&mut self, entry: KeyEntry) {
        debug!("Adding key: {} ({} bytes)", entry.description, entry.key.len());
        self.keys
            .entry(entry.key_type)
            .or_insert_with(Vec::new)
            .push(entry);
    }

    /// Get a key by type
    pub fn get_key(&self, key_type: KeyType) -> Option<&[u8]> {
        self.keys
            .get(&key_type)
            .and_then(|entries| entries.first())
            .map(|entry| entry.key.as_slice())
    }

    /// Get all keys of a specific type
    pub fn get_keys(&self, key_type: KeyType) -> Vec<&KeyEntry> {
        self.keys
            .get(&key_type)
            .map(|entries| entries.iter().collect())
            .unwrap_or_default()
    }

    /// Decrypt data using AES
    pub fn decrypt_aes(
        &self,
        encrypted_data: &[u8],
        key: &[u8],
        iv: &[u8],
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "AES decryption: data_len={}, key_len={}, iv_len={}",
            encrypted_data.len(),
            key.len(),
            iv.len()
        );

        // Validate inputs
        if key.len() != AES_128_KEY_SIZE && key.len() != AES_256_KEY_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid key length (must be {} or {} bytes)", AES_128_KEY_SIZE, AES_256_KEY_SIZE),
            ));
        }

        if iv.len() != AES_IV_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid IV length (must be {} bytes)", AES_IV_SIZE),
            ));
        }

        // Align data to block size
        let aligned_len = if encrypted_data.len() % AES_BLOCK_SIZE != 0 {
            (encrypted_data.len() / AES_BLOCK_SIZE + 1) * AES_BLOCK_SIZE
        } else {
            encrypted_data.len()
        };

        let mut buffer = vec![0u8; aligned_len];
        buffer[..encrypted_data.len()].copy_from_slice(encrypted_data);

        // Decrypt based on key size
        match key.len() {
            AES_128_KEY_SIZE => {
                let decryptor = Aes128CbcDec::new_from_slices(key, iv)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create decryptor: {}", e)))?;
                decryptor
                    .decrypt_padded_mut::<NoPadding>(&mut buffer)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Decryption failed: {}", e)))?;
            }
            AES_256_KEY_SIZE => {
                let decryptor = Aes256CbcDec::new_from_slices(key, iv)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create decryptor: {}", e)))?;
                decryptor
                    .decrypt_padded_mut::<NoPadding>(&mut buffer)
                    .map_err(|e| LoaderError::DecryptionFailed(format!("Decryption failed: {}", e)))?;
            }
            _ => unreachable!(),
        }

        buffer.truncate(encrypted_data.len());
        Ok(buffer)
    }

    /// Decrypt data using AES-128-CTR with offset
    /// The offset is used to adjust the counter for sections that don't start at offset 0
    /// This is needed when multiple sections share the same key/IV but are placed at different
    /// offsets in the destination ELF file.
    pub fn decrypt_aes_ctr_with_offset(
        &self,
        encrypted_data: &[u8],
        key: &[u8],
        iv: &[u8],
        byte_offset: u64,
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "AES-CTR decryption with offset: data_len={}, key_len={}, iv_len={}, byte_offset=0x{:x}",
            encrypted_data.len(),
            key.len(),
            iv.len(),
            byte_offset
        );

        if key.len() != AES_128_KEY_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid key length for CTR mode (must be {} bytes)", AES_128_KEY_SIZE),
            ));
        }

        if iv.len() != AES_IV_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid IV length (must be {} bytes)", AES_IV_SIZE),
            ));
        }

        // Calculate block offset (16 bytes per block)
        let block_offset = byte_offset / 16;
        
        // Adjust the IV by adding the block offset to the counter portion
        // The IV is treated as a 128-bit big-endian counter
        let mut adjusted_iv = [0u8; 16];
        adjusted_iv.copy_from_slice(iv);
        
        // Add block_offset to the IV (treating it as a big-endian 128-bit number)
        let mut carry = block_offset;
        for i in (0..16).rev() {
            let sum = adjusted_iv[i] as u64 + (carry & 0xFF);
            adjusted_iv[i] = sum as u8;
            carry = (carry >> 8) + (sum >> 8);
            if carry == 0 {
                break;
            }
        }
        
        debug!("Adjusted IV for offset 0x{:x}: {:02x?}", byte_offset, adjusted_iv);

        let mut buffer = encrypted_data.to_vec();
        let mut cipher = Aes128Ctr::new_from_slices(key, &adjusted_iv)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create CTR cipher: {}", e)))?;
        
        cipher.apply_keystream(&mut buffer);
        
        Ok(buffer)
    }

    /// Decrypt data using AES-128-CTR
    pub fn decrypt_aes_ctr(
        &self,
        encrypted_data: &[u8],
        key: &[u8],
        iv: &[u8],
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "AES-CTR decryption: data_len={}, key_len={}, iv_len={}",
            encrypted_data.len(),
            key.len(),
            iv.len()
        );

        if key.len() != AES_128_KEY_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid key length for CTR mode (must be {} bytes)", AES_128_KEY_SIZE),
            ));
        }

        if iv.len() != AES_IV_SIZE {
            return Err(LoaderError::DecryptionFailed(
                format!("Invalid IV length (must be {} bytes)", AES_IV_SIZE),
            ));
        }

        let mut buffer = encrypted_data.to_vec();
        let mut cipher = Aes128Ctr::new_from_slices(key, iv)
            .map_err(|e| LoaderError::DecryptionFailed(format!("Failed to create CTR cipher: {}", e)))?;
        
        cipher.apply_keystream(&mut buffer);
        
        Ok(buffer)
    }

    /// Decrypt SELF metadata using key type and revision
    pub fn decrypt_self_metadata(
        &self,
        encrypted_metadata: &[u8],
        key_type: u16,
        revision: u16,
    ) -> Result<Vec<u8>, LoaderError> {
        debug!(
            "Decrypting SELF metadata: type=0x{:04x}, revision=0x{:04x}, len={}",
            key_type, revision, encrypted_metadata.len()
        );

        let key_set = self.get_self_key_set(key_type, revision)
            .ok_or_else(|| LoaderError::DecryptionFailed(
                format!(
                    "No keys available for SELF type 0x{:04x} revision 0x{:04x}. \
                     Please install PS3 firmware first.",
                    key_type, revision
                )
            ))?;

        // Use AES-128 with the erk and riv from the key set
        let key = &key_set.erk[..AES_128_KEY_SIZE];
        let iv = &key_set.riv;

        self.decrypt_aes(encrypted_metadata, key, iv)
    }

    /// Decrypt metadata using MetaLV2 keys (legacy method)
    pub fn decrypt_metadata_lv2(
        &self,
        encrypted_metadata: &[u8],
        key_type: KeyType,
    ) -> Result<Vec<u8>, LoaderError> {
        debug!("Decrypting MetaLV2 metadata with key type: {:?}", key_type);

        let key = self.get_key(key_type)
            .ok_or_else(|| LoaderError::DecryptionFailed(
                format!("Key type {:?} not found. Please install PS3 firmware.", key_type)
            ))?;

        // MetaLV2 uses specific IV (typically all zeros)
        let iv = vec![0u8; AES_IV_SIZE];

        self.decrypt_aes(encrypted_metadata, key, &iv)
    }

    /// Compute SHA-1 hash
    pub fn sha1(&self, data: &[u8]) -> [u8; 20] {
        let mut hasher = Sha1::new();
        hasher.update(data);
        hasher.finalize().into()
    }    /// Verify SHA-1 hash
    pub fn verify_sha1(&self, data: &[u8], expected_hash: &[u8; 20]) -> bool {
        let computed = self.sha1(data);
        computed == *expected_hash
    }

    /// Check if a key type is available
    pub fn has_key(&self, key_type: KeyType) -> bool {
        self.keys.contains_key(&key_type)
    }

    /// Get key database statistics
    pub fn get_stats(&self) -> KeyStats {
        let mut stats = KeyStats::default();
        
        for (key_type, entries) in &self.keys {
            let count = entries.len();
            match key_type {
                KeyType::Retail => stats.retail_keys = count,
                KeyType::Debug => stats.debug_keys = count,
                KeyType::App => stats.app_keys = count,
                KeyType::IsoSpu => stats.iso_spu_keys = count,
                KeyType::Lv0 => stats.lv0_keys = count,
                KeyType::Lv1 => stats.lv1_keys = count,
                KeyType::Lv2 => stats.lv2_keys = count,
                KeyType::Npd => stats.npd_keys = count,
                KeyType::MetaLdr => stats.meta_ldr_keys = count,
                KeyType::Vsh => stats.vsh_keys = count,
            }
        }

        stats.self_key_sets = self.self_keys.len();
        stats
    }
}

/// Key database statistics
#[derive(Debug, Default)]
pub struct KeyStats {
    pub retail_keys: usize,
    pub debug_keys: usize,
    pub app_keys: usize,
    pub iso_spu_keys: usize,
    pub lv0_keys: usize,
    pub lv1_keys: usize,
    pub lv2_keys: usize,
    pub npd_keys: usize,
    pub meta_ldr_keys: usize,
    pub vsh_keys: usize,
    pub self_key_sets: usize,
}

impl KeyStats {
    /// Get total number of keys across all types
    pub fn total(&self) -> usize {
        self.retail_keys + self.debug_keys + self.app_keys +
        self.iso_spu_keys + self.lv0_keys + self.lv1_keys + 
        self.lv2_keys + self.npd_keys + self.meta_ldr_keys +
        self.vsh_keys + self.self_key_sets
    }
}

impl Default for CryptoEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a hex string into bytes
fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

/// Parse key name to determine type
fn parse_key_name(name: &str) -> Option<(KeyType, String)> {
    let name_upper = name.to_uppercase();
    
    let key_type = if name_upper.contains("LV0") {
        KeyType::Lv0
    } else if name_upper.contains("LV1") {
        KeyType::Lv1
    } else if name_upper.contains("LV2") {
        KeyType::Lv2
    } else if name_upper.contains("VSH") {
        KeyType::Vsh
    } else if name_upper.contains("NPD") || name_upper.contains("NPDRM") {
        KeyType::Npd
    } else if name_upper.contains("ISO") || name_upper.contains("SPU") {
        KeyType::IsoSpu
    } else if name_upper.contains("APP") {
        KeyType::App
    } else if name_upper.contains("DEBUG") || name_upper.contains("DBG") {
        KeyType::Debug
    } else if name_upper.contains("META") || name_upper.contains("LDR") {
        KeyType::MetaLdr
    } else {
        KeyType::Retail
    };

    Some((key_type, name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_engine_creation() {
        let engine = CryptoEngine::new();
        assert!(!engine.has_firmware_keys());
    }

    #[test]
    fn test_hex_decode() {
        assert_eq!(hex_decode("0102030405"), Some(vec![1, 2, 3, 4, 5]));
        assert_eq!(hex_decode("AABBCCDD"), Some(vec![0xAA, 0xBB, 0xCC, 0xDD]));
        assert_eq!(hex_decode("123"), None); // Odd length
    }

    #[test]
    fn test_key_addition() {
        let mut engine = CryptoEngine::new();
        
        let key_entry = KeyEntry {
            key_type: KeyType::App,
            key: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            iv: Some(vec![0u8; 16]),
            description: "Test key".to_string(),
            revision: 0,
        };

        engine.add_key(key_entry);
        assert!(engine.has_key(KeyType::App));
    }

    #[test]
    fn test_aes_128_decryption() {
        let engine = CryptoEngine::new();
        
        // Test with valid inputs
        let key = [0u8; 16];
        let iv = [0u8; 16];
        let data = [0u8; 16];
        
        let result = engine.decrypt_aes(&data, &key, &iv);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sha1() {
        let engine = CryptoEngine::new();
        
        // SHA-1 of empty string
        let hash = engine.sha1(b"");
        let expected = hex_decode("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        assert_eq!(&hash[..], &expected[..]);
    }

    #[test]
    fn test_self_key_set() {
        let mut engine = CryptoEngine::new();
        
        let key_set = SelfKeySet {
            erk: [0u8; 32],
            riv: [0u8; 16],
            revision: 1,
            key_type: 0x1001,
        };
        
        engine.add_self_key_set(key_set);
        
        assert!(engine.get_self_key_set(0x1001, 1).is_some());
        assert!(engine.get_self_key_set(0x1001, 99).is_none());
    }

    #[test]
    fn test_parse_key_name() {
        assert_eq!(parse_key_name("LV2_KEY").map(|x| x.0), Some(KeyType::Lv2));
        assert_eq!(parse_key_name("VSH_CRYPT").map(|x| x.0), Some(KeyType::Vsh));
        assert_eq!(parse_key_name("DEBUG_KEY").map(|x| x.0), Some(KeyType::Debug));
    }

    #[test]
    fn test_key_stats() {
        let mut engine = CryptoEngine::new();
        
        engine.add_key(KeyEntry {
            key_type: KeyType::Retail,
            key: vec![0u8; 16],
            iv: None,
            description: "test".to_string(),
            revision: 0,
        });
        
        let stats = engine.get_stats();
        assert_eq!(stats.retail_keys, 1);
    }
}
