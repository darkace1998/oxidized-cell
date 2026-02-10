//! RSX debugger for command buffer and graphics state inspection

use oc_rsx::state::RsxState;

/// RSX debug state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsxDebugState {
    /// Running normally
    Running,
    /// Paused
    Paused,
    /// Step one command
    StepCommand,
    /// Step one frame
    StepFrame,
}

/// RSX command entry for debugging
#[derive(Debug, Clone)]
pub struct RsxCommandEntry {
    /// Method register address
    pub method: u32,
    /// Data value
    pub data: u32,
    /// Human-readable method name
    pub method_name: String,
    /// Decoded description
    pub description: String,
}

/// Render target debug info
#[derive(Debug, Clone)]
pub struct RenderTargetDebugInfo {
    /// Render target index
    pub index: usize,
    /// Address offset
    pub offset: u32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Format
    pub format: String,
    /// Is active
    pub active: bool,
}

/// Texture debug info
#[derive(Debug, Clone)]
pub struct TextureDebugInfo {
    /// Texture unit index
    pub unit: usize,
    /// Texture offset
    pub offset: u32,
    /// Width
    pub width: u16,
    /// Height
    pub height: u16,
    /// Format
    pub format: String,
    /// Is enabled
    pub enabled: bool,
}

/// Vertex attribute debug info
#[derive(Debug, Clone)]
pub struct VertexAttributeDebugInfo {
    /// Attribute index
    pub index: usize,
    /// Offset
    pub offset: u32,
    /// Stride
    pub stride: u16,
    /// Size (number of components)
    pub size: u8,
    /// Type
    pub type_name: String,
    /// Is enabled
    pub enabled: bool,
}

/// Shader debug state for step-through debugging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderDebugState {
    /// Not debugging shaders
    Inactive,
    /// Paused at shader instruction
    Paused,
    /// Stepping through shader
    Stepping,
}

/// Shader debug info for vertex/fragment programs
#[derive(Debug, Clone)]
pub struct ShaderDebugInfo {
    /// Shader type
    pub shader_type: ShaderType,
    /// Program offset/address
    pub address: u32,
    /// Program size in bytes
    pub size: u32,
    /// Number of instructions
    pub instruction_count: usize,
    /// Current instruction index (for debugging)
    pub current_instruction: usize,
    /// Input registers used
    pub inputs_used: Vec<String>,
    /// Output registers written
    pub outputs_written: Vec<String>,
    /// Constant registers used
    pub constants_used: Vec<u32>,
    /// Texture samplers used
    pub samplers_used: Vec<u32>,
}

/// Shader type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderType {
    /// Vertex program
    Vertex,
    /// Fragment program
    Fragment,
}

/// Shader instruction debug info
#[derive(Debug, Clone)]
pub struct ShaderInstructionDebugInfo {
    /// Instruction index
    pub index: usize,
    /// Raw instruction data
    pub raw_data: Vec<u32>,
    /// Disassembled instruction
    pub disasm: String,
    /// Destination register
    pub dst: Option<String>,
    /// Source registers
    pub src: Vec<String>,
    /// Is this a texture operation
    pub is_texture_op: bool,
    /// Is this a flow control operation
    pub is_flow_control: bool,
}

/// Command buffer region for visualization
#[derive(Debug, Clone)]
pub struct CommandBufferRegion {
    /// Start address in command buffer
    pub start: u32,
    /// End address
    pub end: u32,
    /// Description of what this region contains
    pub description: String,
    /// Commands in this region
    pub commands: Vec<RsxCommandEntry>,
}

/// RSX debugger
pub struct RsxDebugger {
    /// Debug state
    pub state: RsxDebugState,
    /// Command history
    command_history: Vec<RsxCommandEntry>,
    /// Maximum command history size
    max_command_history: usize,
    /// Break on method (optional method register to break on)
    break_on_method: Option<u32>,
    /// Frame counter
    pub frame_count: u64,
    /// Commands in current frame
    pub commands_this_frame: u64,
    /// Shader debug state
    pub shader_debug_state: ShaderDebugState,
    /// Current vertex program debug info
    vertex_program_debug: Option<ShaderDebugInfo>,
    /// Current fragment program debug info
    fragment_program_debug: Option<ShaderDebugInfo>,
    /// Command buffer regions for visualization
    command_buffer_regions: Vec<CommandBufferRegion>,
}

impl Default for RsxDebugger {
    fn default() -> Self {
        Self::new()
    }
}

impl RsxDebugger {
    /// Create a new RSX debugger
    pub fn new() -> Self {
        Self {
            state: RsxDebugState::Running,
            command_history: Vec::new(),
            max_command_history: 10000,
            break_on_method: None,
            frame_count: 0,
            commands_this_frame: 0,
            shader_debug_state: ShaderDebugState::Inactive,
            vertex_program_debug: None,
            fragment_program_debug: None,
            command_buffer_regions: Vec::new(),
        }
    }

    /// Pause RSX execution
    pub fn pause(&mut self) {
        self.state = RsxDebugState::Paused;
        tracing::info!("RSX debugger: paused");
    }

    /// Resume RSX execution
    pub fn resume(&mut self) {
        self.state = RsxDebugState::Running;
        tracing::info!("RSX debugger: resumed");
    }

    /// Step one command
    pub fn step_command(&mut self) {
        self.state = RsxDebugState::StepCommand;
        tracing::debug!("RSX debugger: step command");
    }

    /// Step one frame
    pub fn step_frame(&mut self) {
        self.state = RsxDebugState::StepFrame;
        tracing::debug!("RSX debugger: step frame");
    }

    /// Set breakpoint on method register
    pub fn break_on_method(&mut self, method: u32) {
        self.break_on_method = Some(method);
        tracing::info!("RSX debugger: break on method 0x{:04x}", method);
    }

    /// Clear method breakpoint
    pub fn clear_method_breakpoint(&mut self) {
        self.break_on_method = None;
    }

    /// Check if should break before executing command
    pub fn check_before_command(&mut self, method: u32) -> bool {
        match self.state {
            RsxDebugState::Running => {
                // Check for method breakpoint
                if self.break_on_method == Some(method) {
                    tracing::info!("RSX debugger: method breakpoint hit at 0x{:04x}", method);
                    self.state = RsxDebugState::Paused;
                    return true;
                }
                false
            }
            RsxDebugState::Paused => true,
            RsxDebugState::StepCommand => {
                self.state = RsxDebugState::Paused;
                true
            }
            RsxDebugState::StepFrame => false,
        }
    }

    /// Record end of frame
    pub fn record_frame_end(&mut self) {
        self.frame_count += 1;
        
        if self.state == RsxDebugState::StepFrame {
            self.state = RsxDebugState::Paused;
            tracing::info!("RSX debugger: frame {} complete, pausing", self.frame_count);
        }
        
        self.commands_this_frame = 0;
    }

    /// Record a command for history
    pub fn record_command(&mut self, method: u32, data: u32) {
        let method_name = Self::method_name(method);
        let description = Self::describe_command(method, data);
        
        let entry = RsxCommandEntry {
            method,
            data,
            method_name,
            description,
        };
        
        self.command_history.push(entry);
        self.commands_this_frame += 1;
        
        // Limit history size
        if self.command_history.len() > self.max_command_history {
            self.command_history.remove(0);
        }
    }

    /// Get command history
    pub fn get_command_history(&self, count: usize) -> &[RsxCommandEntry] {
        let start = self.command_history.len().saturating_sub(count);
        &self.command_history[start..]
    }

    /// Clear command history
    pub fn clear_history(&mut self) {
        self.command_history.clear();
    }

    /// Get graphics state snapshot
    pub fn get_state_snapshot(&self, state: &RsxState) -> RsxStateSnapshot {
        RsxStateSnapshot {
            // Viewport
            viewport_x: state.viewport_x,
            viewport_y: state.viewport_y,
            viewport_width: state.viewport_width as u32,
            viewport_height: state.viewport_height as u32,
            
            // Scissor
            scissor_x: state.surface_clip_x,
            scissor_y: state.surface_clip_y,
            scissor_width: state.surface_clip_width,
            scissor_height: state.surface_clip_height,
            
            // Blend state
            blend_enabled: state.blend_enable,
            blend_src_rgb: state.blend_src_factor as u16,
            blend_dst_rgb: state.blend_dst_factor as u16,
            blend_src_alpha: state.blend_src_factor as u16,
            blend_dst_alpha: state.blend_dst_factor as u16,
            
            // Depth state
            depth_test_enabled: state.depth_test_enable,
            depth_write_enabled: state.depth_write_enable,
            depth_func: state.depth_func as u16,
            
            // Stencil state
            stencil_test_enabled: state.stencil_test_enable,
            stencil_func: state.stencil_func as u8,
            stencil_ref: state.stencil_ref,
            stencil_mask: state.stencil_mask,
            
            // Other state
            cull_face: state.cull_face_mode as u16,
            primitive_type: state.primitive_type,
            clear_color: state.clear_color,
            clear_depth: state.clear_depth,
        }
    }

    /// Get render target info
    pub fn get_render_targets(&self, state: &RsxState) -> Vec<RenderTargetDebugInfo> {
        let mut targets = Vec::new();
        
        for i in 0..4 {
            targets.push(RenderTargetDebugInfo {
                index: i,
                offset: state.surface_offset_color[i],
                width: state.surface_clip_width as u32,
                height: state.surface_clip_height as u32,
                format: Self::surface_format_name(state.surface_format as u8),
                active: state.surface_offset_color[i] != 0,
            });
        }
        
        targets
    }

    /// Get texture unit info
    pub fn get_textures(&self, state: &RsxState) -> Vec<TextureDebugInfo> {
        let mut textures = Vec::new();
        
        for i in 0..16 {
            textures.push(TextureDebugInfo {
                unit: i,
                offset: state.texture_offset[i],
                width: ((state.texture_format[i] >> 16) & 0xFFFF) as u16,
                height: (state.texture_format[i] & 0xFFFF) as u16,
                format: Self::texture_format_name(((state.texture_format[i] >> 8) & 0xFF) as u8),
                enabled: state.texture_offset[i] != 0,
            });
        }
        
        textures
    }

    /// Get vertex attribute info
    pub fn get_vertex_attributes(&self, state: &RsxState) -> Vec<VertexAttributeDebugInfo> {
        let mut attrs = Vec::new();
        
        for i in 0..16 {
            let format = state.vertex_attrib_format[i];
            let stride = ((format >> 8) & 0xFF) as u16;
            let size = ((format >> 4) & 0xF) as u8;
            let type_id = (format & 0xF) as u8;
            attrs.push(VertexAttributeDebugInfo {
                index: i,
                offset: state.vertex_attrib_offset[i],
                stride,
                size,
                type_name: Self::vertex_type_name(type_id),
                enabled: stride != 0,
            });
        }
        
        attrs
    }

    /// Check if RSX is paused
    pub fn is_paused(&self) -> bool {
        self.state == RsxDebugState::Paused
    }

    /// Check if RSX is running
    pub fn is_running(&self) -> bool {
        self.state == RsxDebugState::Running
    }

    // === Shader Debugging Methods ===

    /// Start shader debugging
    pub fn start_shader_debug(&mut self) {
        self.shader_debug_state = ShaderDebugState::Paused;
        tracing::info!("RSX shader debugging started");
    }

    /// Stop shader debugging
    pub fn stop_shader_debug(&mut self) {
        self.shader_debug_state = ShaderDebugState::Inactive;
        self.vertex_program_debug = None;
        self.fragment_program_debug = None;
        tracing::info!("RSX shader debugging stopped");
    }

    /// Step to next shader instruction
    pub fn step_shader(&mut self) {
        if self.shader_debug_state == ShaderDebugState::Paused {
            self.shader_debug_state = ShaderDebugState::Stepping;
        }
    }

    /// Set vertex program debug info
    pub fn set_vertex_program_debug(&mut self, info: ShaderDebugInfo) {
        self.vertex_program_debug = Some(info);
    }

    /// Set fragment program debug info
    pub fn set_fragment_program_debug(&mut self, info: ShaderDebugInfo) {
        self.fragment_program_debug = Some(info);
    }

    /// Get vertex program debug info
    pub fn get_vertex_program_debug(&self) -> Option<&ShaderDebugInfo> {
        self.vertex_program_debug.as_ref()
    }

    /// Get fragment program debug info
    pub fn get_fragment_program_debug(&self) -> Option<&ShaderDebugInfo> {
        self.fragment_program_debug.as_ref()
    }

    /// Check if shader debugging is active
    pub fn is_shader_debugging(&self) -> bool {
        self.shader_debug_state != ShaderDebugState::Inactive
    }

    // === Command Buffer Visualization Methods ===

    /// Add a command buffer region for visualization
    pub fn add_command_buffer_region(&mut self, start: u32, end: u32, description: &str) {
        self.command_buffer_regions.push(CommandBufferRegion {
            start,
            end,
            description: description.to_string(),
            commands: Vec::new(),
        });
    }

    /// Get command buffer regions
    pub fn get_command_buffer_regions(&self) -> &[CommandBufferRegion] {
        &self.command_buffer_regions
    }

    /// Clear command buffer regions
    pub fn clear_command_buffer_regions(&mut self) {
        self.command_buffer_regions.clear();
    }

    /// Analyze command history and group by draw calls
    pub fn analyze_command_history(&self) -> Vec<CommandBufferRegion> {
        let mut regions = Vec::new();
        let mut current_region_start = 0;
        let mut current_region_cmds = Vec::new();
        let mut in_draw = false;
        
        for (i, cmd) in self.command_history.iter().enumerate() {
            current_region_cmds.push(cmd.clone());
            
            // Check for draw start (BEGIN)
            if cmd.method == 0x1808 && cmd.data != 0 {
                in_draw = true;
            }
            
            // Check for draw end (END)
            if cmd.method == 0x1808 && cmd.data == 0 && in_draw {
                in_draw = false;
                regions.push(CommandBufferRegion {
                    start: current_region_start as u32,
                    end: i as u32,
                    description: format!("Draw call {}", regions.len()),
                    commands: current_region_cmds.clone(),
                });
                current_region_start = i + 1;
                current_region_cmds.clear();
            }
            
            // Check for clear
            if cmd.method == 0x1D94 {
                regions.push(CommandBufferRegion {
                    start: current_region_start as u32,
                    end: i as u32,
                    description: "Clear".to_string(),
                    commands: current_region_cmds.clone(),
                });
                current_region_start = i + 1;
                current_region_cmds.clear();
            }
        }
        
        // Add remaining commands
        if !current_region_cmds.is_empty() {
            regions.push(CommandBufferRegion {
                start: current_region_start as u32,
                end: self.command_history.len() as u32,
                description: "Pending commands".to_string(),
                commands: current_region_cmds,
            });
        }
        
        regions
    }

    /// Get method name from register address
    fn method_name(method: u32) -> String {
        match method {
            0x0000 => "NV4097_NO_OPERATION".to_string(),
            0x0100 => "NV4097_SET_OBJECT".to_string(),
            0x0180 => "NV4097_SET_CONTEXT_DMA_NOTIFIES".to_string(),
            0x0184 => "NV4097_SET_CONTEXT_DMA_A".to_string(),
            0x0188 => "NV4097_SET_CONTEXT_DMA_B".to_string(),
            0x018C => "NV4097_SET_CONTEXT_DMA_COLOR_A".to_string(),
            0x0190 => "NV4097_SET_CONTEXT_DMA_ZETA".to_string(),
            0x0194 => "NV4097_SET_CONTEXT_DMA_COLOR_B".to_string(),
            0x0200..=0x023C => format!("NV4097_SET_SURFACE_COLOR_TARGET[{}]", (method - 0x0200) / 4),
            0x0300 => "NV4097_SET_SURFACE_PITCH_A".to_string(),
            0x0304 => "NV4097_SET_SURFACE_COLOR_OFFSET".to_string(),
            0x0308 => "NV4097_SET_SURFACE_PITCH_B".to_string(),
            0x030C => "NV4097_SET_SURFACE_COLOR_OFFSET_B".to_string(),
            0x0A00 => "NV4097_SET_VIEWPORT_HORIZONTAL".to_string(),
            0x0A04 => "NV4097_SET_VIEWPORT_VERTICAL".to_string(),
            0x0A20 => "NV4097_SET_VIEWPORT_OFFSET".to_string(),
            0x0A30 => "NV4097_SET_VIEWPORT_SCALE".to_string(),
            0x08E4 => "NV4097_SET_SCISSOR_HORIZONTAL".to_string(),
            0x08E8 => "NV4097_SET_SCISSOR_VERTICAL".to_string(),
            0x0310 => "NV4097_SET_BLEND_ENABLE".to_string(),
            0x0320 => "NV4097_SET_BLEND_FUNC_SRC".to_string(),
            0x0324 => "NV4097_SET_BLEND_FUNC_DST".to_string(),
            0x0350 => "NV4097_SET_DEPTH_TEST_ENABLE".to_string(),
            0x0354 => "NV4097_SET_DEPTH_FUNC".to_string(),
            0x0358 => "NV4097_SET_DEPTH_MASK".to_string(),
            0x08C0 => "NV4097_SET_CULL_FACE_ENABLE".to_string(),
            0x08C4 => "NV4097_SET_CULL_FACE".to_string(),
            0x1808 => "NV4097_SET_BEGIN_END".to_string(),
            0x1810 => "NV4097_DRAW_ARRAYS".to_string(),
            0x1814 => "NV4097_DRAW_INDEX_ARRAY".to_string(),
            0x1D94 => "NV4097_CLEAR_SURFACE".to_string(),
            0x1D98 => "NV4097_SET_CLEAR_RECT_HORIZONTAL".to_string(),
            0x1D9C => "NV4097_SET_CLEAR_RECT_VERTICAL".to_string(),
            0x1D80 => "NV4097_SET_COLOR_CLEAR_VALUE".to_string(),
            0x1D8C => "NV4097_SET_ZSTENCIL_CLEAR_VALUE".to_string(),
            _ => format!("NV4097_METHOD_0x{:04X}", method),
        }
    }

    /// Describe a command
    fn describe_command(method: u32, data: u32) -> String {
        match method {
            0x0310 => format!("Blend: {}", if data != 0 { "enabled" } else { "disabled" }),
            0x0350 => format!("Depth test: {}", if data != 0 { "enabled" } else { "disabled" }),
            0x0358 => format!("Depth write: {}", if data != 0 { "enabled" } else { "disabled" }),
            0x1808 => {
                if data == 0 {
                    "End primitive".to_string()
                } else {
                    format!("Begin primitive type {}", data)
                }
            }
            0x1810 => {
                let first = data & 0xFFFFFF;
                let count = (data >> 24) & 0xFF;
                format!("Draw arrays: first={}, count={}", first, count)
            }
            0x1814 => {
                let first = data & 0xFFFFFF;
                let count = (data >> 24) & 0xFF;
                format!("Draw indexed: first={}, count={}", first, count)
            }
            0x1D94 => format!("Clear surface: mask=0x{:08X}", data),
            0x1D80 => format!("Clear color: 0x{:08X}", data),
            0x08E4 => {
                let x = data & 0xFFFF;
                let w = (data >> 16) & 0xFFFF;
                format!("Scissor horizontal: x={}, width={}", x, w)
            }
            0x08E8 => {
                let y = data & 0xFFFF;
                let h = (data >> 16) & 0xFFFF;
                format!("Scissor vertical: y={}, height={}", y, h)
            }
            _ => format!("data=0x{:08X}", data),
        }
    }

    /// Get surface format name
    fn surface_format_name(format: u8) -> String {
        match format {
            1 => "A8R8G8B8".to_string(),
            3 => "A16R16G16B16".to_string(),
            4 => "A16R16G16B16_FLOAT".to_string(),
            5 => "A32R32G32B32_FLOAT".to_string(),
            _ => format!("Format_{}", format),
        }
    }

    /// Get texture format name
    fn texture_format_name(format: u8) -> String {
        match format {
            0x81 => "A8R8G8B8".to_string(),
            0x82 => "R5G6B5".to_string(),
            0x83 => "A1R5G5B5".to_string(),
            0x84 => "A4R4G4B4".to_string(),
            0x85 => "L8".to_string(),
            0x86 => "DXT1".to_string(),
            0x87 => "DXT3".to_string(),
            0x88 => "DXT5".to_string(),
            _ => format!("Format_0x{:02X}", format),
        }
    }

    /// Get vertex type name
    fn vertex_type_name(type_id: u8) -> String {
        match type_id {
            0 => "Disabled".to_string(),
            1 => "S1".to_string(),
            2 => "F32".to_string(),
            3 => "S16".to_string(),
            4 => "U8".to_string(),
            5 => "S16N".to_string(),
            6 => "F16".to_string(),
            _ => format!("Type_{}", type_id),
        }
    }
}

/// Snapshot of RSX graphics state
#[derive(Debug, Clone)]
pub struct RsxStateSnapshot {
    // Viewport
    pub viewport_x: f32,
    pub viewport_y: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
    
    // Scissor
    pub scissor_x: u16,
    pub scissor_y: u16,
    pub scissor_width: u16,
    pub scissor_height: u16,
    
    // Blend
    pub blend_enabled: bool,
    pub blend_src_rgb: u16,
    pub blend_dst_rgb: u16,
    pub blend_src_alpha: u16,
    pub blend_dst_alpha: u16,
    
    // Depth
    pub depth_test_enabled: bool,
    pub depth_write_enabled: bool,
    pub depth_func: u16,
    
    // Stencil
    pub stencil_test_enabled: bool,
    pub stencil_func: u8,
    pub stencil_ref: u8,
    pub stencil_mask: u8,
    
    // Other
    pub cull_face: u16,
    pub primitive_type: u32,
    pub clear_color: u32,
    pub clear_depth: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsx_debugger_creation() {
        let debugger = RsxDebugger::new();
        assert_eq!(debugger.state, RsxDebugState::Running);
        assert_eq!(debugger.frame_count, 0);
    }

    #[test]
    fn test_rsx_pause_resume() {
        let mut debugger = RsxDebugger::new();
        
        debugger.pause();
        assert_eq!(debugger.state, RsxDebugState::Paused);
        
        debugger.resume();
        assert_eq!(debugger.state, RsxDebugState::Running);
    }

    #[test]
    fn test_rsx_step_command() {
        let mut debugger = RsxDebugger::new();
        
        debugger.step_command();
        assert_eq!(debugger.state, RsxDebugState::StepCommand);
        
        // After check, should be paused
        assert!(debugger.check_before_command(0x1808));
        assert_eq!(debugger.state, RsxDebugState::Paused);
    }

    #[test]
    fn test_rsx_step_frame() {
        let mut debugger = RsxDebugger::new();
        
        debugger.step_frame();
        assert_eq!(debugger.state, RsxDebugState::StepFrame);
        
        // Should not pause on commands
        assert!(!debugger.check_before_command(0x1808));
        
        // Should pause at frame end
        debugger.record_frame_end();
        assert_eq!(debugger.state, RsxDebugState::Paused);
    }

    #[test]
    fn test_rsx_method_breakpoint() {
        let mut debugger = RsxDebugger::new();
        
        debugger.break_on_method(0x1D94); // Clear surface
        
        // Should not break on other methods
        assert!(!debugger.check_before_command(0x1808));
        
        // Should break on target method
        assert!(debugger.check_before_command(0x1D94));
        assert_eq!(debugger.state, RsxDebugState::Paused);
    }

    #[test]
    fn test_rsx_command_history() {
        let mut debugger = RsxDebugger::new();
        
        debugger.record_command(0x1808, 5); // Begin triangles
        debugger.record_command(0x1810, 0x03000000); // Draw 3 vertices
        debugger.record_command(0x1808, 0); // End
        
        let history = debugger.get_command_history(10);
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].method, 0x1808);
        assert_eq!(history[1].method, 0x1810);
        assert_eq!(history[2].method, 0x1808);
    }
}
