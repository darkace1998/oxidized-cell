//! HLE module registry

use std::collections::HashMap;

/// HLE function signature
pub type HleFunction = fn(args: &[u64]) -> i64;

/// HLE module
pub struct HleModule {
    /// Module name
    pub name: String,
    /// Exported functions (NID -> function)
    pub functions: HashMap<u32, HleFunction>,
}

impl HleModule {
    /// Create a new HLE module
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            functions: HashMap::new(),
        }
    }

    /// Register a function
    pub fn register(&mut self, nid: u32, func: HleFunction) {
        self.functions.insert(nid, func);
    }

    /// Get a function by NID
    pub fn get_function(&self, nid: u32) -> Option<&HleFunction> {
        self.functions.get(&nid)
    }
}

/// Module registry
pub struct ModuleRegistry {
    modules: HashMap<String, HleModule>,
}

impl ModuleRegistry {
    /// Create a new module registry
    pub fn new() -> Self {
        let mut registry = Self {
            modules: HashMap::new(),
        };
        registry.register_default_modules();
        registry
    }

    /// Register default HLE modules
    fn register_default_modules(&mut self) {
        // Graphics Modules
        
        // cellGcmSys - RSX management
        let mut gcm = HleModule::new("cellGcmSys");
        gcm.register(0x055BD74D, |_| 0); // cellGcmGetTiledPitchSize
        gcm.register(0x21AC3697, |_| 0); // cellGcmInit
        gcm.register(0x9BA451E4, |_| 0); // cellGcmSetFlipMode
        gcm.register(0xD01B570D, |_| 0); // cellGcmGetConfiguration
        self.modules.insert("cellGcmSys".to_string(), gcm);

        // cellGifDec - GIF decoding
        let mut gif_dec = HleModule::new("cellGifDec");
        gif_dec.register(0x48436B53, |_| 0); // cellGifDecCreate
        gif_dec.register(0x7D61A31F, |_| 0); // cellGifDecOpen
        gif_dec.register(0x8251B8ED, |_| 0); // cellGifDecClose
        gif_dec.register(0x97D7C656, |_| 0); // cellGifDecDestroy
        self.modules.insert("cellGifDec".to_string(), gif_dec);

        // cellPngDec - PNG decoding
        let mut png_dec = HleModule::new("cellPngDec");
        png_dec.register(0x157D30C5, |_| 0); // cellPngDecCreate
        png_dec.register(0x820DAE1F, |_| 0); // cellPngDecOpen
        png_dec.register(0x9E9D7D42, |_| 0); // cellPngDecClose
        png_dec.register(0x5B3D1FF1, |_| 0); // cellPngDecDestroy
        self.modules.insert("cellPngDec".to_string(), png_dec);

        // cellJpgDec - JPEG decoding
        let mut jpg_dec = HleModule::new("cellJpgDec");
        jpg_dec.register(0x50A7C5ED, |_| 0); // cellJpgDecCreate
        jpg_dec.register(0x8B300F66, |_| 0); // cellJpgDecOpen
        jpg_dec.register(0x6C9CC258, |_| 0); // cellJpgDecClose
        jpg_dec.register(0x9338A07A, |_| 0); // cellJpgDecDestroy
        self.modules.insert("cellJpgDec".to_string(), jpg_dec);

        // System Modules
        
        // cellSysutil - System utilities
        let mut sysutil = HleModule::new("cellSysutil");
        sysutil.register(0x0BAE8772, |_| 0); // cellSysutilCheckCallback
        sysutil.register(0x40E34A7A, |_| 0); // cellSysutilRegisterCallback
        sysutil.register(0xA5768D6B, |_| 0); // cellSysutilUnregisterCallback
        self.modules.insert("cellSysutil".to_string(), sysutil);

        // cellGame - Game data access
        let mut game = HleModule::new("cellGame");
        game.register(0xDB9819F3, |_| 0); // cellGameBootCheck
        game.register(0x70ACEC67, |_| 0); // cellGameDataCheck
        game.register(0x42A2E133, |_| 0); // cellGameContentErrorDialog
        self.modules.insert("cellGame".to_string(), game);

        // cellSaveData - Save data management
        let mut save_data = HleModule::new("cellSaveData");
        save_data.register(0x1DFBFDD6, |_| 0); // cellSaveDataListLoad2
        save_data.register(0x2A8EFC31, |_| 0); // cellSaveDataListSave2
        save_data.register(0x2DE0D663, |_| 0); // cellSaveDataDelete2
        self.modules.insert("cellSaveData".to_string(), save_data);

        // Multimedia Modules
        
        // cellDmux - Demuxer
        let mut dmux = HleModule::new("cellDmux");
        dmux.register(0x04E7CFAB, |_| 0); // cellDmuxOpen
        dmux.register(0x87E8C5CB, |_| 0); // cellDmuxClose
        dmux.register(0x6FF7FCC8, |_| 0); // cellDmuxSetStream
        dmux.register(0xE340569F, |_| 0); // cellDmuxEnableEs
        self.modules.insert("cellDmux".to_string(), dmux);

        // cellVdec - Video decoder
        let mut vdec = HleModule::new("cellVdec");
        vdec.register(0xC982A84A, |_| 0); // cellVdecOpen
        vdec.register(0x8CDCFBA5, |_| 0); // cellVdecClose
        vdec.register(0xD8C4B54B, |_| 0); // cellVdecStartSeq
        vdec.register(0x9F4B2B40, |_| 0); // cellVdecEndSeq
        self.modules.insert("cellVdec".to_string(), vdec);

        // cellAdec - Audio decoder
        let mut adec = HleModule::new("cellAdec");
        adec.register(0x2CFFC4C9, |_| 0); // cellAdecOpen
        adec.register(0x2FB5C730, |_| 0); // cellAdecClose
        adec.register(0x26DB9555, |_| 0); // cellAdecStartSeq
        adec.register(0x68B6BB05, |_| 0); // cellAdecEndSeq
        self.modules.insert("cellAdec".to_string(), adec);

        // cellVpost - Video post-processing
        let mut vpost = HleModule::new("cellVpost");
        vpost.register(0xAFCE7E2A, |_| 0); // cellVpostOpen
        vpost.register(0x22C9F9DC, |_| 0); // cellVpostClose
        vpost.register(0xE4DC0E5D, |_| 0); // cellVpostExec
        self.modules.insert("cellVpost".to_string(), vpost);

        // Network Modules
        
        // cellNetCtl - Network control
        let mut net_ctl = HleModule::new("cellNetCtl");
        net_ctl.register(0xBD5A59FC, |_| 0); // cellNetCtlInit
        net_ctl.register(0x6D5044C4, |_| 0); // cellNetCtlTerm
        net_ctl.register(0x899337F1, |_| 0); // cellNetCtlGetState
        self.modules.insert("cellNetCtl".to_string(), net_ctl);

        // cellHttp - HTTP client
        let mut http = HleModule::new("cellHttp");
        http.register(0x77F93D09, |_| 0); // cellHttpInit
        http.register(0xF1FC9429, |_| 0); // cellHttpEnd
        http.register(0x03863A9F, |_| 0); // cellHttpCreateClient
        self.modules.insert("cellHttp".to_string(), http);

        // cellSsl - SSL/TLS
        let mut ssl = HleModule::new("cellSsl");
        ssl.register(0x0C34B7A5, |_| 0); // cellSslInit
        ssl.register(0x7B37EC3F, |_| 0); // cellSslEnd
        ssl.register(0x5BFD9DA1, |_| 0); // cellSslCertificateLoader
        self.modules.insert("cellSsl".to_string(), ssl);

        // Utilities Modules
        
        // cellFont - Font rendering
        let mut font = HleModule::new("cellFont");
        font.register(0x25C107E6, |_| 0); // cellFontInit
        font.register(0xEC89A187, |_| 0); // cellFontEnd
        font.register(0x042E74E3, |_| 0); // cellFontOpenFontMemory
        self.modules.insert("cellFont".to_string(), font);

        // cellSpurs - SPURS task scheduler
        let mut spurs = HleModule::new("cellSpurs");
        spurs.register(0x1CFCE711, |_| 0); // cellSpursInitialize
        spurs.register(0x8BE30633, |_| 0); // cellSpursFinalize
        spurs.register(0x9C939DBF, |_| 0); // cellSpursAttachLv2EventQueue
        self.modules.insert("cellSpurs".to_string(), spurs);

        // libsre - Regular expressions
        let mut sre = HleModule::new("libsre");
        sre.register(0x53E24EC1, |_| 0); // cellSreCompile
        sre.register(0x8F79CFD8, |_| 0); // cellSreFree
        sre.register(0x7B27D657, |_| 0); // cellSreMatch
        self.modules.insert("libsre".to_string(), sre);

        // Other System Modules
        
        // cellPad - Controller input
        let mut pad = HleModule::new("cellPad");
        pad.register(0x578E3C98, |_| 0); // cellPadInit
        pad.register(0x3733EA3C, |_| 0); // cellPadEnd
        pad.register(0x1CF98800, |_| 0); // cellPadGetData
        pad.register(0x6BC09C61, |_| 0); // cellPadGetInfo2
        self.modules.insert("cellPad".to_string(), pad);
    }

    /// Get a module by name
    pub fn get_module(&self, name: &str) -> Option<&HleModule> {
        self.modules.get(name)
    }

    /// Register a module
    pub fn register_module(&mut self, module: HleModule) {
        self.modules.insert(module.name.clone(), module);
    }

    /// Get a function from any module
    pub fn find_function(&self, module: &str, nid: u32) -> Option<&HleFunction> {
        self.modules.get(module)?.get_function(nid)
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_registry() {
        let registry = ModuleRegistry::new();
        
        // Test graphics modules
        assert!(registry.get_module("cellGcmSys").is_some());
        assert!(registry.get_module("cellGifDec").is_some());
        assert!(registry.get_module("cellPngDec").is_some());
        assert!(registry.get_module("cellJpgDec").is_some());
        
        // Test system modules
        assert!(registry.get_module("cellSysutil").is_some());
        assert!(registry.get_module("cellGame").is_some());
        assert!(registry.get_module("cellSaveData").is_some());
        
        // Test multimedia modules
        assert!(registry.get_module("cellDmux").is_some());
        assert!(registry.get_module("cellVdec").is_some());
        assert!(registry.get_module("cellAdec").is_some());
        assert!(registry.get_module("cellVpost").is_some());
        
        // Test network modules
        assert!(registry.get_module("cellNetCtl").is_some());
        assert!(registry.get_module("cellHttp").is_some());
        assert!(registry.get_module("cellSsl").is_some());
        
        // Test utilities modules
        assert!(registry.get_module("cellFont").is_some());
        assert!(registry.get_module("cellSpurs").is_some());
        assert!(registry.get_module("libsre").is_some());
        
        // Test other system modules
        assert!(registry.get_module("cellPad").is_some());
        
        // Test function lookup
        let func = registry.find_function("cellGcmSys", 0x21AC3697);
        assert!(func.is_some());
    }

    #[test]
    fn test_module_functions() {
        let registry = ModuleRegistry::new();
        
        // Test that we can find registered functions
        assert!(registry.find_function("cellGifDec", 0x48436B53).is_some());
        assert!(registry.find_function("cellJpgDec", 0x50A7C5ED).is_some());
        assert!(registry.find_function("cellDmux", 0x04E7CFAB).is_some());
        assert!(registry.find_function("cellVdec", 0xC982A84A).is_some());
        assert!(registry.find_function("cellAdec", 0x2CFFC4C9).is_some());
        assert!(registry.find_function("cellSsl", 0x0C34B7A5).is_some());
        
        // Test that non-existent functions return None
        assert!(registry.find_function("cellGcmSys", 0xFFFFFFFF).is_none());
        assert!(registry.find_function("NonExistentModule", 0x12345678).is_none());
    }
}
