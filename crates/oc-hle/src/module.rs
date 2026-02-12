//! HLE module registry
//!
//! Maps PS3 firmware NID (numeric identifier) values to their HLE
//! handler functions.  When a NID has a real implementation in one of
//! the `cell_*` modules the registry dispatches directly to it.
//! Unimplemented NIDs are routed through a logging stub so that
//! callers see a single debug message the first time the function is
//! invoked.

use std::collections::HashMap;
use tracing::{debug, warn, trace};

/// HLE function signature
pub type HleFunction = fn(args: &[u64]) -> i64;

/// NID metadata for logging/debugging
#[derive(Debug, Clone)]
pub struct NidInfo {
    /// NID value
    pub nid: u32,
    /// Symbolic function name (e.g. "cellGcmInit")
    pub name: &'static str,
    /// Whether a real HLE handler is registered (vs. stub)
    pub implemented: bool,
}

/// HLE module
pub struct HleModule {
    /// Module name
    pub name: String,
    /// Exported functions (NID -> function)
    pub functions: HashMap<u32, HleFunction>,
    /// NID metadata for all registered functions
    pub nid_info: HashMap<u32, NidInfo>,
}

impl HleModule {
    /// Create a new HLE module
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            functions: HashMap::new(),
            nid_info: HashMap::new(),
        }
    }

    /// Register a function
    pub fn register(&mut self, nid: u32, func: HleFunction) {
        self.functions.insert(nid, func);
    }

    /// Register a function with metadata
    pub fn register_named(&mut self, nid: u32, name: &'static str, func: HleFunction, implemented: bool) {
        self.functions.insert(nid, func);
        self.nid_info.insert(nid, NidInfo { nid, name, implemented });
    }

    /// Get a function by NID
    pub fn get_function(&self, nid: u32) -> Option<&HleFunction> {
        self.functions.get(&nid)
    }

    /// Get NID info
    pub fn get_nid_info(&self, nid: u32) -> Option<&NidInfo> {
        self.nid_info.get(&nid)
    }

    /// Get count of implemented (non-stub) functions
    pub fn implemented_count(&self) -> usize {
        self.nid_info.values().filter(|n| n.implemented).count()
    }

    /// Get total function count
    pub fn total_count(&self) -> usize {
        self.functions.len()
    }
}

/// PRX import entry — describes a function imported by a loaded PRX
#[derive(Debug, Clone)]
pub struct PrxImport {
    /// Module name the import belongs to
    pub module_name: String,
    /// Function NID
    pub nid: u32,
    /// Address in guest memory where the stub was written
    pub stub_addr: u32,
    /// Whether the import has been resolved to an HLE handler
    pub resolved: bool,
}

/// Module registry
pub struct ModuleRegistry {
    modules: HashMap<String, HleModule>,
    /// PRX import table (stub_addr -> import)
    prx_imports: HashMap<u32, PrxImport>,
    /// Set of NIDs that have already been logged as unimplemented
    logged_unimplemented: std::collections::HashSet<u32>,
}

impl ModuleRegistry {
    /// Create a new module registry
    pub fn new() -> Self {
        let mut registry = Self {
            modules: HashMap::new(),
            prx_imports: HashMap::new(),
            logged_unimplemented: std::collections::HashSet::new(),
        };
        registry.register_default_modules();
        registry
    }

    /// Register default HLE modules
    fn register_default_modules(&mut self) {
        // =================================================================
        // Graphics Modules
        // =================================================================

        let mut gcm = HleModule::new("cellGcmSys");
        gcm.register_named(0x055BD74D, "cellGcmGetTiledPitchSize", |_| 0, false);
        gcm.register_named(0x21AC3697, "cellGcmInit", |args| {
            let cmd_size = if args.is_empty() { 0x100000 } else { args[0] as u32 };
            crate::context::get_hle_context_mut().gcm.init(0x10000000, cmd_size);
            0
        }, true);
        gcm.register_named(0x9BA451E4, "cellGcmSetFlipMode", |args| {
            let mode = if args.is_empty() { 0 } else { args[0] as u32 };
            let flip_mode = if mode == 2 {
                crate::cell_gcm_sys::CellGcmFlipMode::Hsync
            } else {
                crate::cell_gcm_sys::CellGcmFlipMode::Vsync
            };
            crate::context::get_hle_context_mut().gcm.set_flip_mode(flip_mode) as i64
        }, true);
        gcm.register_named(0xD01B570D, "cellGcmGetConfiguration", |_| 0, false);
        self.modules.insert("cellGcmSys".to_string(), gcm);

        let mut gif_dec = HleModule::new("cellGifDec");
        gif_dec.register_named(0x48436B53, "cellGifDecCreate", |_| 0, false);
        gif_dec.register_named(0x7D61A31F, "cellGifDecOpen", |_| 0, false);
        gif_dec.register_named(0x8251B8ED, "cellGifDecClose", |_| 0, false);
        gif_dec.register_named(0x97D7C656, "cellGifDecDestroy", |_| 0, false);
        self.modules.insert("cellGifDec".to_string(), gif_dec);

        let mut png_dec = HleModule::new("cellPngDec");
        png_dec.register_named(0x157D30C5, "cellPngDecCreate", |_| 0, false);
        png_dec.register_named(0x820DAE1F, "cellPngDecOpen", |_| 0, false);
        png_dec.register_named(0x9E9D7D42, "cellPngDecClose", |_| 0, false);
        png_dec.register_named(0x5B3D1FF1, "cellPngDecDestroy", |_| 0, false);
        self.modules.insert("cellPngDec".to_string(), png_dec);

        let mut jpg_dec = HleModule::new("cellJpgDec");
        jpg_dec.register_named(0x50A7C5ED, "cellJpgDecCreate", |_| 0, false);
        jpg_dec.register_named(0x8B300F66, "cellJpgDecOpen", |_| 0, false);
        jpg_dec.register_named(0x6C9CC258, "cellJpgDecClose", |_| 0, false);
        jpg_dec.register_named(0x9338A07A, "cellJpgDecDestroy", |_| 0, false);
        self.modules.insert("cellJpgDec".to_string(), jpg_dec);

        // =================================================================
        // System Modules
        // =================================================================

        let mut sysutil = HleModule::new("cellSysutil");
        sysutil.register_named(0x0BAE8772, "cellSysutilCheckCallback", |_| {
            crate::cell_sysutil::cell_sysutil_check_callback() as i64
        }, true);
        sysutil.register_named(0x40E34A7A, "cellSysutilRegisterCallback", |args| {
            let (slot, func, userdata) = (
                args.first().copied().unwrap_or(0) as u32,
                args.get(1).copied().unwrap_or(0) as u32,
                args.get(2).copied().unwrap_or(0) as u32,
            );
            crate::cell_sysutil::cell_sysutil_register_callback(slot, func, userdata) as i64
        }, true);
        sysutil.register_named(0xA5768D6B, "cellSysutilUnregisterCallback", |args| {
            let slot = args.first().copied().unwrap_or(0) as u32;
            crate::cell_sysutil::cell_sysutil_unregister_callback(slot) as i64
        }, true);
        self.modules.insert("cellSysutil".to_string(), sysutil);

        let mut game = HleModule::new("cellGame");
        game.register_named(0xDB9819F3, "cellGameBootCheck", |_| 0, false);
        game.register_named(0x70ACEC67, "cellGameDataCheck", |_| 0, false);
        game.register_named(0x42A2E133, "cellGameContentErrorDialog", |_| 0, false);
        self.modules.insert("cellGame".to_string(), game);

        let mut save_data = HleModule::new("cellSaveData");
        save_data.register_named(0x1DFBFDD6, "cellSaveDataListLoad2", |args| {
            let version = args.first().copied().unwrap_or(0) as u32;
            crate::cell_save_data::cell_save_data_list_load2(version, 0, 0, 0, 0, 0, 0, 0) as i64
        }, true);
        save_data.register_named(0x2A8EFC31, "cellSaveDataListSave2", |args| {
            let version = args.first().copied().unwrap_or(0) as u32;
            crate::cell_save_data::cell_save_data_list_save2(version, 0, 0, 0, 0, 0, 0, 0) as i64
        }, true);
        save_data.register_named(0x2DE0D663, "cellSaveDataDelete2", |args| {
            let version = args.first().copied().unwrap_or(0) as u32;
            crate::cell_save_data::cell_save_data_delete2(version, 0, 0, 0, 0, 0, 0) as i64
        }, true);
        self.modules.insert("cellSaveData".to_string(), save_data);

        // =================================================================
        // Multimedia Modules
        // =================================================================

        let mut dmux = HleModule::new("cellDmux");
        dmux.register_named(0x04E7CFAB, "cellDmuxOpen", |_| 0, false);
        dmux.register_named(0x87E8C5CB, "cellDmuxClose", |_| 0, false);
        dmux.register_named(0x6FF7FCC8, "cellDmuxSetStream", |_| 0, false);
        dmux.register_named(0xE340569F, "cellDmuxEnableEs", |_| 0, false);
        self.modules.insert("cellDmux".to_string(), dmux);

        let mut vdec = HleModule::new("cellVdec");
        vdec.register_named(0xC982A84A, "cellVdecOpen", |_| 0, false);
        vdec.register_named(0x8CDCFBA5, "cellVdecClose", |_| 0, false);
        vdec.register_named(0xD8C4B54B, "cellVdecStartSeq", |_| 0, false);
        vdec.register_named(0x9F4B2B40, "cellVdecEndSeq", |_| 0, false);
        self.modules.insert("cellVdec".to_string(), vdec);

        let mut adec = HleModule::new("cellAdec");
        adec.register_named(0x2CFFC4C9, "cellAdecOpen", |_| 0, false);
        adec.register_named(0x2FB5C730, "cellAdecClose", |_| 0, false);
        adec.register_named(0x26DB9555, "cellAdecStartSeq", |_| 0, false);
        adec.register_named(0x68B6BB05, "cellAdecEndSeq", |_| 0, false);
        self.modules.insert("cellAdec".to_string(), adec);

        let mut vpost = HleModule::new("cellVpost");
        vpost.register_named(0xAFCE7E2A, "cellVpostOpen", |_| 0, false);
        vpost.register_named(0x22C9F9DC, "cellVpostClose", |_| 0, false);
        vpost.register_named(0xE4DC0E5D, "cellVpostExec", |_| 0, false);
        self.modules.insert("cellVpost".to_string(), vpost);

        // =================================================================
        // Network Modules
        // =================================================================

        let mut net_ctl = HleModule::new("cellNetCtl");
        net_ctl.register_named(0xBD5A59FC, "cellNetCtlInit", |_| 0, false);
        net_ctl.register_named(0x6D5044C4, "cellNetCtlTerm", |_| 0, false);
        net_ctl.register_named(0x899337F1, "cellNetCtlGetState", |_| 0, false);
        self.modules.insert("cellNetCtl".to_string(), net_ctl);

        let mut http = HleModule::new("cellHttp");
        http.register_named(0x77F93D09, "cellHttpInit", |_| 0, false);
        http.register_named(0xF1FC9429, "cellHttpEnd", |_| 0, false);
        http.register_named(0x03863A9F, "cellHttpCreateClient", |_| 0, false);
        self.modules.insert("cellHttp".to_string(), http);

        let mut ssl = HleModule::new("cellSsl");
        ssl.register_named(0x0C34B7A5, "cellSslInit", |_| 0, false);
        ssl.register_named(0x7B37EC3F, "cellSslEnd", |_| 0, false);
        ssl.register_named(0x5BFD9DA1, "cellSslCertificateLoader", |_| 0, false);
        self.modules.insert("cellSsl".to_string(), ssl);

        // =================================================================
        // Utilities Modules
        // =================================================================

        let mut font = HleModule::new("cellFont");
        font.register_named(0x25C107E6, "cellFontInit", |args| {
            let config_addr = args.first().copied().unwrap_or(0) as u32;
            crate::cell_font::cell_font_init(config_addr) as i64
        }, true);
        font.register_named(0xEC89A187, "cellFontEnd", |_| {
            crate::cell_font::cell_font_end() as i64
        }, true);
        font.register_named(0x042E74E3, "cellFontOpenFontMemory", |_| 0, false);
        self.modules.insert("cellFont".to_string(), font);

        let mut spurs = HleModule::new("cellSpurs");
        spurs.register_named(0x1CFCE711, "cellSpursInitialize", |_| 0, false);
        spurs.register_named(0x8BE30633, "cellSpursFinalize", |_| 0, false);
        spurs.register_named(0x9C939DBF, "cellSpursAttachLv2EventQueue", |_| 0, false);
        self.modules.insert("cellSpurs".to_string(), spurs);

        let mut sre = HleModule::new("libsre");
        sre.register_named(0x53E24EC1, "cellSreCompile", |_| 0, false);
        sre.register_named(0x8F79CFD8, "cellSreFree", |_| 0, false);
        sre.register_named(0x7B27D657, "cellSreMatch", |_| 0, false);
        self.modules.insert("libsre".to_string(), sre);

        // =================================================================
        // Other System Modules
        // =================================================================

        let mut pad = HleModule::new("cellPad");
        pad.register_named(0x578E3C98, "cellPadInit", |_| 0, false);
        pad.register_named(0x3733EA3C, "cellPadEnd", |_| 0, false);
        pad.register_named(0x1CF98800, "cellPadGetData", |_| 0, false);
        pad.register_named(0x6BC09C61, "cellPadGetInfo2", |_| 0, false);
        self.modules.insert("cellPad".to_string(), pad);

        let mut audio = HleModule::new("cellAudio");
        audio.register_named(0x56DFE179, "cellAudioInit", |_| 0, false);
        audio.register_named(0x04AF134E, "cellAudioQuit", |_| 0, false);
        audio.register_named(0xCA5AC370, "cellAudioPortOpen", |_| 0, false);
        audio.register_named(0x5B1E2C73, "cellAudioPortClose", |_| 0, false);
        audio.register_named(0x74A66AF0, "cellAudioPortStart", |_| 0, false);
        audio.register_named(0x8C628DDE, "cellAudioPortStop", |_| 0, false);
        audio.register_named(0x4109D08C, "cellAudioGetPortConfig", |_| 0, false);
        audio.register_named(0x377E0CD9, "cellAudioCreateNotifyEventQueue", |_| 0, false);
        audio.register_named(0x0D831209, "cellAudioSetNotifyEventQueue", |_| 0, false);
        audio.register_named(0xF9CD769B, "cellAudioRemoveNotifyEventQueue", |_| 0, false);
        self.modules.insert("cellAudio".to_string(), audio);

        let mut fs = HleModule::new("cellFs");
        fs.register_named(0x718BF5F8, "cellFsOpen", |_| 0, false);
        fs.register_named(0x2CB51F0D, "cellFsClose", |_| 0, false);
        fs.register_named(0xB1840098, "cellFsRead", |_| 0, false);
        fs.register_named(0x5B0E89A3, "cellFsWrite", |_| 0, false);
        fs.register_named(0x4D5FF8E2, "cellFsLseek", |_| 0, false);
        fs.register_named(0xCBB83B20, "cellFsFstat", |_| 0, false);
        fs.register_named(0xAADD4A40, "cellFsStat", |_| 0, false);
        fs.register_named(0xB37F693E, "cellFsOpendir", |_| 0, false);
        fs.register_named(0x5C74903D, "cellFsReaddir", |_| 0, false);
        fs.register_named(0xA3EBFA2B, "cellFsClosedir", |_| 0, false);
        self.modules.insert("cellFs".to_string(), fs);

        let mut resc = HleModule::new("cellResc");
        resc.register_named(0x23134710, "cellRescInit", |_| 0, false);
        resc.register_named(0x09FB6A6D, "cellRescExit", |_| 0, false);
        resc.register_named(0x24C35E65, "cellRescSetDisplayMode", |_| 0, false);
        resc.register_named(0x6CD0F95F, "cellRescSetSrc", |_| 0, false);
        resc.register_named(0x516EE89E, "cellRescSetDsts", |_| 0, false);
        resc.register_named(0x69A41AA6, "cellRescSetConvertAndFlip", |_| 0, false);
        self.modules.insert("cellResc".to_string(), resc);

        let mut spurs_jq = HleModule::new("cellSpursJq");
        spurs_jq.register_named(0x15625280, "cellSpursJobQueueCreate", |_| 0, false);
        spurs_jq.register_named(0x4B9837DA, "cellSpursJobQueueDestroy", |_| 0, false);
        spurs_jq.register_named(0x6A5B3005, "cellSpursJobQueuePushJob", |_| 0, false);
        spurs_jq.register_named(0x2FF2A154, "cellSpursJobQueueSync", |_| 0, false);
        self.modules.insert("cellSpursJq".to_string(), spurs_jq);

        let mut kb = HleModule::new("cellKb");
        kb.register_named(0x43E5E12C, "cellKbInit", |_| 0, false);
        kb.register_named(0xBFC32557, "cellKbEnd", |_| 0, false);
        kb.register_named(0x2073B7F6, "cellKbGetInfo", |_| 0, false);
        kb.register_named(0xFF0A21B7, "cellKbRead", |_| 0, false);
        kb.register_named(0xA5F85E4D, "cellKbSetReadMode", |_| 0, false);
        kb.register_named(0x3F72C56E, "cellKbSetCodeType", |_| 0, false);
        self.modules.insert("cellKb".to_string(), kb);

        let mut mouse = HleModule::new("cellMouse");
        mouse.register_named(0xC9030138, "cellMouseInit", |_| 0, false);
        mouse.register_named(0xE10183CE, "cellMouseEnd", |_| 0, false);
        mouse.register_named(0x3EF66B95, "cellMouseGetInfo", |_| 0, false);
        mouse.register_named(0xDBDABD6A, "cellMouseGetData", |_| 0, false);
        mouse.register_named(0x4D0B3B1F, "cellMouseGetDataList", |_| 0, false);
        mouse.register_named(0x21A62E9B, "cellMouseClearBuf", |_| 0, false);
        self.modules.insert("cellMouse".to_string(), mouse);

        let mut mic = HleModule::new("cellMic");
        mic.register_named(0x06FBE9D6, "cellMicInit", |_| 0, false);
        mic.register_named(0x9FF53C1A, "cellMicEnd", |_| 0, false);
        mic.register_named(0x186CB69A, "cellMicOpen", |_| 0, false);
        mic.register_named(0xC6B88C77, "cellMicClose", |_| 0, false);
        mic.register_named(0xF8E25EB4, "cellMicStart", |_| 0, false);
        mic.register_named(0x87A08D29, "cellMicStop", |_| 0, false);
        mic.register_named(0x75DA7A97, "cellMicRead", |_| 0, false);
        self.modules.insert("cellMic".to_string(), mic);

        let mut font_ft = HleModule::new("cellFontFT");
        font_ft.register_named(0x1387C6C4, "cellFontFTInit", |args| {
            let config_addr = args.first().copied().unwrap_or(0) as u32;
            crate::cell_font_ft::cell_font_ft_init(config_addr) as i64
        }, true);
        font_ft.register_named(0x41C80CB8, "cellFontFTEnd", |_| {
            crate::cell_font_ft::cell_font_ft_end() as i64
        }, true);
        font_ft.register_named(0xA885CC9B, "cellFontFTOpenFontMemory", |_| 0, false);
        font_ft.register_named(0xE5D8F51E, "cellFontFTOpenFontFile", |_| 0, false);
        font_ft.register_named(0x2E936C08, "cellFontFTCloseFont", |_| 0, false);
        font_ft.register_named(0xB276F1F6, "cellFontFTLoadGlyph", |_| 0, false);
        self.modules.insert("cellFontFT".to_string(), font_ft);
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

    // ====================================================================
    // Function-Level Logging for Unimplemented NIDs
    // ====================================================================

    /// Call a function by module name and NID, with automatic logging
    ///
    /// This is the primary dispatch entry point.  It logs a warning the
    /// first time an unimplemented NID is called, and silently returns
    /// `0` on subsequent calls to avoid flooding the log.
    pub fn call_function(&mut self, module: &str, nid: u32, args: &[u64]) -> i64 {
        if let Some(m) = self.modules.get(module) {
            if let Some(info) = m.nid_info.get(&nid) {
                if !info.implemented {
                    if !self.logged_unimplemented.contains(&nid) {
                        warn!(
                            "UNIMPLEMENTED: {}::{}  NID=0x{:08X}",
                            module, info.name, nid
                        );
                        self.logged_unimplemented.insert(nid);
                    } else {
                        trace!("STUB: {}::{}  NID=0x{:08X}", module, info.name, nid);
                    }
                } else {
                    trace!("HLE: {}::{}  NID=0x{:08X}", module, info.name, nid);
                }
            } else if !self.logged_unimplemented.contains(&nid) {
                warn!(
                    "UNKNOWN NID: module={} NID=0x{:08X}",
                    module, nid
                );
                self.logged_unimplemented.insert(nid);
            }

            if let Some(func) = m.get_function(nid) {
                return func(args);
            }
        } else {
            debug!("Module '{}' not registered in HLE registry", module);
        }

        0 // Default stub return value
    }

    // ====================================================================
    // Dynamic PRX Import Resolution
    // ====================================================================

    /// Register a PRX import that needs to be resolved
    pub fn register_prx_import(&mut self, module_name: &str, nid: u32, stub_addr: u32) {
        debug!(
            "ModuleRegistry::register_prx_import: module={} NID=0x{:08X} stub=0x{:08X}",
            module_name, nid, stub_addr
        );
        self.prx_imports.insert(stub_addr, PrxImport {
            module_name: module_name.to_string(),
            nid,
            stub_addr,
            resolved: false,
        });
    }

    /// Resolve all pending PRX imports against the current module registry
    ///
    /// Returns the number of successfully resolved imports.
    pub fn resolve_prx_imports(&mut self) -> u32 {
        let mut resolved = 0u32;
        let module_map = &self.modules;

        for import in self.prx_imports.values_mut() {
            if import.resolved {
                continue;
            }
            if let Some(m) = module_map.get(&import.module_name) {
                if m.get_function(import.nid).is_some() {
                    import.resolved = true;
                    resolved += 1;
                    debug!(
                        "Resolved PRX import: {}::0x{:08X} at stub 0x{:08X}",
                        import.module_name, import.nid, import.stub_addr
                    );
                } else {
                    trace!(
                        "Unresolved PRX import: {}::0x{:08X} (NID not found)",
                        import.module_name, import.nid
                    );
                }
            } else {
                trace!(
                    "Unresolved PRX import: {}::0x{:08X} (module not found)",
                    import.module_name, import.nid
                );
            }
        }

        if resolved > 0 {
            debug!("ModuleRegistry::resolve_prx_imports: resolved {}/{} imports",
                resolved, self.prx_imports.len());
        }

        resolved
    }

    /// Check if a PRX import is resolved
    pub fn is_import_resolved(&self, stub_addr: u32) -> bool {
        self.prx_imports.get(&stub_addr).map(|i| i.resolved).unwrap_or(false)
    }

    /// Get PRX import info by stub address
    pub fn get_prx_import(&self, stub_addr: u32) -> Option<&PrxImport> {
        self.prx_imports.get(&stub_addr)
    }

    /// Get total count of PRX imports
    pub fn prx_import_count(&self) -> usize {
        self.prx_imports.len()
    }

    /// Get count of resolved PRX imports
    pub fn resolved_import_count(&self) -> usize {
        self.prx_imports.values().filter(|i| i.resolved).count()
    }

    // ====================================================================
    // Introspection
    // ====================================================================

    /// Get a summary of implementation status across all modules
    pub fn get_implementation_summary(&self) -> Vec<(&str, usize, usize)> {
        let mut summary: Vec<(&str, usize, usize)> = self.modules.values()
            .map(|m| (m.name.as_str(), m.implemented_count(), m.total_count()))
            .collect();
        summary.sort_by_key(|(name, _, _)| *name);
        summary
    }

    /// Get list of all registered module names
    pub fn module_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.modules.keys().map(|k| k.as_str()).collect();
        names.sort();
        names
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
        assert!(registry.get_module("cellAudio").is_some());
        assert!(registry.get_module("cellFs").is_some());
        
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
        assert!(registry.find_function("cellAudio", 0x56DFE179).is_some());
        assert!(registry.find_function("cellFs", 0x718BF5F8).is_some());
        
        // Test that non-existent functions return None
        assert!(registry.find_function("cellGcmSys", 0xFFFFFFFF).is_none());
        assert!(registry.find_function("NonExistentModule", 0x12345678).is_none());
    }

    #[test]
    fn test_module_nid_metadata() {
        let registry = ModuleRegistry::new();

        let gcm = registry.get_module("cellGcmSys").unwrap();
        let info = gcm.get_nid_info(0x21AC3697).unwrap();
        assert_eq!(info.name, "cellGcmInit");
        assert!(info.implemented);

        // A stub function should be marked as not implemented
        let gif = registry.get_module("cellGifDec").unwrap();
        let info = gif.get_nid_info(0x48436B53).unwrap();
        assert_eq!(info.name, "cellGifDecCreate");
        assert!(!info.implemented);
    }

    #[test]
    fn test_module_call_function_logging() {
        let mut registry = ModuleRegistry::new();

        // Calling a real implementation should work
        let result = registry.call_function("cellSysutil", 0x0BAE8772, &[]);
        assert_eq!(result, 0);

        // Calling a stub should also return 0
        let result = registry.call_function("cellGifDec", 0x48436B53, &[]);
        assert_eq!(result, 0);

        // Calling an unknown module returns 0
        let result = registry.call_function("NonExistent", 0x12345678, &[]);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_module_prx_import_resolution() {
        let mut registry = ModuleRegistry::new();

        // Register a PRX import for a known NID
        registry.register_prx_import("cellGcmSys", 0x21AC3697, 0x00100000);
        registry.register_prx_import("cellGcmSys", 0xFFFFFFFF, 0x00100004);

        assert_eq!(registry.prx_import_count(), 2);
        assert_eq!(registry.resolved_import_count(), 0);

        // Resolve
        let count = registry.resolve_prx_imports();
        assert_eq!(count, 1);

        assert!(registry.is_import_resolved(0x00100000));
        assert!(!registry.is_import_resolved(0x00100004));
        assert_eq!(registry.resolved_import_count(), 1);
    }

    #[test]
    fn test_module_implementation_summary() {
        let registry = ModuleRegistry::new();
        let summary = registry.get_implementation_summary();

        assert!(!summary.is_empty());

        let sysutil = summary.iter().find(|(name, _, _)| *name == "cellSysutil");
        assert!(sysutil.is_some());
        let (_, implemented, total) = sysutil.unwrap();
        assert!(*implemented > 0);
        assert!(*total > 0);
    }

    #[test]
    fn test_module_names() {
        let registry = ModuleRegistry::new();
        let names = registry.module_names();
        assert!(names.contains(&"cellGcmSys"));
        assert!(names.contains(&"cellSysutil"));
        assert!(names.contains(&"cellFont"));
    }
}
