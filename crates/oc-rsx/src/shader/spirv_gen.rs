//! SPIR-V code generator for RSX shaders
//!
//! Generates SPIR-V bytecode from decoded RSX vertex and fragment programs.

#![allow(dead_code)] // Many SPIR-V opcodes reserved for future shader features

use super::types::*;

/// SPIR-V magic number
const SPIRV_MAGIC: u32 = 0x07230203;

/// SPIR-V version (1.0)
const SPIRV_VERSION: u32 = 0x00010000;

/// SPIR-V generator ID (our tool)
const SPIRV_GENERATOR: u32 = 0x00080001;

// SPIR-V opcodes
const OP_CAPABILITY: u16 = 17;
const OP_EXT_INST_IMPORT: u16 = 11;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_NAME: u16 = 5;
const OP_DECORATE: u16 = 71;
const OP_TYPE_VOID: u16 = 19;
const OP_TYPE_BOOL: u16 = 20;
const OP_TYPE_FLOAT: u16 = 22;
const OP_TYPE_VECTOR: u16 = 23;
const OP_TYPE_POINTER: u16 = 32;
const OP_TYPE_FUNCTION: u16 = 33;
const OP_TYPE_IMAGE: u16 = 25;
const OP_TYPE_SAMPLED_IMAGE: u16 = 27;
const OP_TYPE_SAMPLER: u16 = 26;
const OP_CONSTANT: u16 = 43;
const OP_VARIABLE: u16 = 59;
const OP_LOAD: u16 = 61;
const OP_STORE: u16 = 62;
const OP_ACCESS_CHAIN: u16 = 65;
const OP_FUNCTION: u16 = 54;
const OP_FUNCTION_END: u16 = 56;
const OP_LABEL: u16 = 248;
const OP_RETURN: u16 = 253;
const OP_FNEGATE: u16 = 127;
const OP_FADD: u16 = 129;
const OP_FSUB: u16 = 131;
const OP_FMUL: u16 = 133;
const OP_FDIV: u16 = 136;
const OP_FMOD: u16 = 141;
const OP_EXT_INST: u16 = 12;
const OP_DOT: u16 = 148;
const OP_VECTOR_SHUFFLE: u16 = 79;
const OP_COMPOSITE_CONSTRUCT: u16 = 80;
const OP_COMPOSITE_EXTRACT: u16 = 81;
const OP_IMAGE_SAMPLE_IMPLICIT_LOD: u16 = 87;
const OP_IMAGE_SAMPLE_EXPLICIT_LOD: u16 = 88;
const OP_SAMPLED_IMAGE: u16 = 86;
const OP_SELECT: u16 = 169;
const OP_FLESS_THAN: u16 = 184;
const OP_FGREATER_THAN: u16 = 186;
const OP_FLESS_THAN_EQUAL: u16 = 188;
const OP_FGREATER_THAN_EQUAL: u16 = 190;
const OP_FORD_EQUAL: u16 = 180;
const OP_FORD_NOT_EQUAL: u16 = 182;
const OP_CONSTANT_TRUE: u16 = 41;
const OP_CONSTANT_FALSE: u16 = 42;
const OP_KILL: u16 = 252;
const OP_DPDX: u16 = 207;
const OP_DPDY: u16 = 208;

// Capability values
const CAP_DERIVATIVE_CONTROL: u32 = 51;
const CAP_SHADER: u32 = 1;

// Execution model values
const EXEC_MODEL_VERTEX: u32 = 0;
const EXEC_MODEL_FRAGMENT: u32 = 4;

// Execution mode values
const EXEC_MODE_ORIGIN_UPPER_LEFT: u32 = 7;

// Storage class values
const STORAGE_INPUT: u32 = 1;
const STORAGE_OUTPUT: u32 = 3;
const STORAGE_FUNCTION: u32 = 7;

// Decoration values  
const DECORATION_LOCATION: u32 = 30;
const DECORATION_BUILTIN: u32 = 11;
const BUILTIN_POSITION: u32 = 0;
const BUILTIN_FRAG_COORD: u32 = 15;

/// SPIR-V code builder
pub struct SpirVBuilder {
    /// Current ID bound
    id_bound: u32,
    /// Capabilities section
    capabilities: Vec<u32>,
    /// Extensions section
    extensions: Vec<u32>,
    /// Ext inst import section
    ext_inst_imports: Vec<u32>,
    /// Memory model
    memory_model: Vec<u32>,
    /// Entry points
    entry_points: Vec<u32>,
    /// Execution modes
    execution_modes: Vec<u32>,
    /// Debug names
    debug_names: Vec<u32>,
    /// Annotations (decorations)
    annotations: Vec<u32>,
    /// Types, constants, variables
    types_constants: Vec<u32>,
    /// Function definitions
    functions: Vec<u32>,
    /// GLSL.std.450 import ID
    glsl_ext_id: u32,
    /// Type IDs
    type_void: u32,
    type_bool: u32,
    type_bvec4: u32,
    type_float: u32,
    type_vec2: u32,
    type_vec3: u32,
    type_vec4: u32,
    type_func_void: u32,
}

impl SpirVBuilder {
    pub fn new() -> Self {
        Self {
            id_bound: 1,
            capabilities: Vec::new(),
            extensions: Vec::new(),
            ext_inst_imports: Vec::new(),
            memory_model: Vec::new(),
            entry_points: Vec::new(),
            execution_modes: Vec::new(),
            debug_names: Vec::new(),
            annotations: Vec::new(),
            types_constants: Vec::new(),
            functions: Vec::new(),
            glsl_ext_id: 0,
            type_void: 0,
            type_bool: 0,
            type_bvec4: 0,
            type_float: 0,
            type_vec2: 0,
            type_vec3: 0,
            type_vec4: 0,
            type_func_void: 0,
        }
    }

    /// Allocate a new ID
    fn alloc_id(&mut self) -> u32 {
        let id = self.id_bound;
        self.id_bound += 1;
        id
    }

    /// Encode an instruction word
    fn encode_word(opcode: u16, word_count: u16) -> u32 {
        ((word_count as u32) << 16) | (opcode as u32)
    }

    /// Add capability
    fn add_capability(&mut self, cap: u32) {
        self.capabilities.push(Self::encode_word(OP_CAPABILITY, 2));
        self.capabilities.push(cap);
    }

    /// Add GLSL.std.450 import
    fn add_glsl_import(&mut self) -> u32 {
        let id = self.alloc_id();
        self.glsl_ext_id = id;
        
        // "GLSL.std.450" = 12 chars + null, padded to 16 bytes = 4 words
        self.ext_inst_imports.push(Self::encode_word(OP_EXT_INST_IMPORT, 6));
        self.ext_inst_imports.push(id);
        // "GLSL.std.450\0" in little-endian u32s
        self.ext_inst_imports.push(0x534C_4C47); // "GLSL"
        self.ext_inst_imports.push(0x2E64_7473); // ".std"
        self.ext_inst_imports.push(0x3035_342E); // ".450"
        self.ext_inst_imports.push(0x0000_0030); // "0\0\0\0"
        
        id
    }

    /// Add memory model
    fn add_memory_model(&mut self) {
        self.memory_model.push(Self::encode_word(OP_MEMORY_MODEL, 3));
        self.memory_model.push(1); // Logical
        self.memory_model.push(1); // GLSL450
    }

    /// Add basic types
    fn add_basic_types(&mut self) {
        // void
        self.type_void = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_VOID, 2));
        self.types_constants.push(self.type_void);

        // bool
        self.type_bool = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_BOOL, 2));
        self.types_constants.push(self.type_bool);

        // float
        self.type_float = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_FLOAT, 3));
        self.types_constants.push(self.type_float);
        self.types_constants.push(32); // 32-bit

        // vec2
        self.type_vec2 = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_VECTOR, 4));
        self.types_constants.push(self.type_vec2);
        self.types_constants.push(self.type_float);
        self.types_constants.push(2);

        // vec3
        self.type_vec3 = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_VECTOR, 4));
        self.types_constants.push(self.type_vec3);
        self.types_constants.push(self.type_float);
        self.types_constants.push(3);

        // vec4
        self.type_vec4 = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_VECTOR, 4));
        self.types_constants.push(self.type_vec4);
        self.types_constants.push(self.type_float);
        self.types_constants.push(4);

        // bvec4 (for comparison results)
        self.type_bvec4 = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_VECTOR, 4));
        self.types_constants.push(self.type_bvec4);
        self.types_constants.push(self.type_bool);
        self.types_constants.push(4);

        // function void()
        self.type_func_void = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_FUNCTION, 3));
        self.types_constants.push(self.type_func_void);
        self.types_constants.push(self.type_void);
    }

    /// Add pointer type
    fn add_pointer_type(&mut self, storage: u32, base_type: u32) -> u32 {
        let id = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_TYPE_POINTER, 4));
        self.types_constants.push(id);
        self.types_constants.push(storage);
        self.types_constants.push(base_type);
        id
    }

    /// Add float constant
    fn add_float_constant(&mut self, value: f32) -> u32 {
        let id = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_CONSTANT, 4));
        self.types_constants.push(self.type_float);
        self.types_constants.push(id);
        self.types_constants.push(value.to_bits());
        id
    }

    /// Add vec4 constant
    fn add_vec4_constant(&mut self, x: u32, y: u32, z: u32, w: u32) -> u32 {
        let id = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
        self.types_constants.push(self.type_vec4);
        self.types_constants.push(id);
        self.types_constants.push(x);
        self.types_constants.push(y);
        self.types_constants.push(z);
        self.types_constants.push(w);
        id
    }

    /// Add variable
    fn add_variable(&mut self, ptr_type: u32, storage: u32) -> u32 {
        let id = self.alloc_id();
        self.types_constants.push(Self::encode_word(OP_VARIABLE, 4));
        self.types_constants.push(ptr_type);
        self.types_constants.push(id);
        self.types_constants.push(storage);
        id
    }

    /// Add location decoration
    fn add_location(&mut self, target: u32, location: u32) {
        self.annotations.push(Self::encode_word(OP_DECORATE, 4));
        self.annotations.push(target);
        self.annotations.push(DECORATION_LOCATION);
        self.annotations.push(location);
    }

    /// Add builtin decoration
    fn add_builtin(&mut self, target: u32, builtin: u32) {
        self.annotations.push(Self::encode_word(OP_DECORATE, 4));
        self.annotations.push(target);
        self.annotations.push(DECORATION_BUILTIN);
        self.annotations.push(builtin);
    }

    /// Build final SPIR-V bytecode
    pub fn build(self) -> Vec<u32> {
        let mut spirv = Vec::new();

        // Header
        spirv.push(SPIRV_MAGIC);
        spirv.push(SPIRV_VERSION);
        spirv.push(SPIRV_GENERATOR);
        spirv.push(self.id_bound);
        spirv.push(0); // Reserved

        // Sections in order
        spirv.extend(&self.capabilities);
        spirv.extend(&self.extensions);
        spirv.extend(&self.ext_inst_imports);
        spirv.extend(&self.memory_model);
        spirv.extend(&self.entry_points);
        spirv.extend(&self.execution_modes);
        spirv.extend(&self.debug_names);
        spirv.extend(&self.annotations);
        spirv.extend(&self.types_constants);
        spirv.extend(&self.functions);

        spirv
    }
}

/// SPIR-V generator for vertex programs
pub struct VpSpirVGen<'a> {
    program: &'a VertexProgram,
    builder: SpirVBuilder,
    // Temp register IDs (r0-r31)
    temp_regs: [u32; 32],
    // Output register IDs
    output_vars: [u32; 16],
    // Input variable IDs
    input_vars: [u32; 16],
    // Constant IDs (vec4 constants)
    constant_ids: Vec<u32>,
}

impl<'a> VpSpirVGen<'a> {
    pub fn new(program: &'a VertexProgram) -> Self {
        Self {
            program,
            builder: SpirVBuilder::new(),
            temp_regs: [0; 32],
            output_vars: [0; 16],
            input_vars: [0; 16],
            constant_ids: Vec::new(),
        }
    }

    /// Generate SPIR-V for the vertex program
    pub fn generate(mut self) -> Result<SpirVModule, String> {
        // Setup common stuff
        self.builder.add_capability(CAP_SHADER);
        self.builder.add_glsl_import();
        self.builder.add_memory_model();
        self.builder.add_basic_types();

        // Create input/output variables
        let ptr_input_vec4 = self.builder.add_pointer_type(STORAGE_INPUT, self.builder.type_vec4);
        let ptr_output_vec4 = self.builder.add_pointer_type(STORAGE_OUTPUT, self.builder.type_vec4);

        // Add input variables based on input_mask
        let mut interface_vars = Vec::new();
        for i in 0..16 {
            if self.program.input_mask & (1 << i) != 0 {
                let var = self.builder.add_variable(ptr_input_vec4, STORAGE_INPUT);
                self.input_vars[i] = var;
                self.builder.add_location(var, i as u32);
                interface_vars.push(var);
            }
        }

        // Add output position (builtin)
        let pos_out = self.builder.add_variable(ptr_output_vec4, STORAGE_OUTPUT);
        self.output_vars[0] = pos_out;
        self.builder.add_builtin(pos_out, BUILTIN_POSITION);
        interface_vars.push(pos_out);

        // Add other outputs based on output_mask
        for i in 1..16 {
            if self.program.output_mask & (1 << i) != 0 {
                let var = self.builder.add_variable(ptr_output_vec4, STORAGE_OUTPUT);
                self.output_vars[i] = var;
                self.builder.add_location(var, i as u32);
                interface_vars.push(var);
            }
        }
        
        // Add constants as SPIR-V constants
        self.add_vp_constants();

        // Create main function
        let main_id = self.builder.alloc_id();
        
        // Entry point - needs variable word count based on interface vars
        let ep_word_count = 4 + interface_vars.len() as u16;
        self.builder.entry_points.push(SpirVBuilder::encode_word(OP_ENTRY_POINT, ep_word_count));
        self.builder.entry_points.push(EXEC_MODEL_VERTEX);
        self.builder.entry_points.push(main_id);
        self.builder.entry_points.push(0x6E69616D); // "main" in little-endian
        for var in &interface_vars {
            self.builder.entry_points.push(*var);
        }

        // Function definition
        self.builder.functions.push(SpirVBuilder::encode_word(OP_FUNCTION, 5));
        self.builder.functions.push(self.builder.type_void);
        self.builder.functions.push(main_id);
        self.builder.functions.push(0); // None
        self.builder.functions.push(self.builder.type_func_void);

        // Label
        let label_id = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_LABEL, 2));
        self.builder.functions.push(label_id);

        // Generate code for each instruction
        self.generate_instructions()?;

        // Return
        self.builder.functions.push(SpirVBuilder::encode_word(OP_RETURN, 1));
        
        // End function
        self.builder.functions.push(SpirVBuilder::encode_word(OP_FUNCTION_END, 1));

        Ok(SpirVModule {
            bytecode: self.builder.build(),
            stage: ShaderStage::VERTEX,
        })
    }

    fn generate_instructions(&mut self) -> Result<(), String> {
        // Initialize temp registers to zero
        let c0 = self.builder.add_float_constant(0.0);
        let c1 = self.builder.add_float_constant(1.0);
        
        for i in 0..32 {
            let reg = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(reg);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.temp_regs[i] = reg;
        }

        // Process each decoded instruction
        for instr in &self.program.decoded {
            self.emit_vp_instruction(instr)?;
        }

        // If no decoded instructions, generate passthrough
        if self.program.decoded.is_empty() {
            if self.input_vars[0] != 0 && self.output_vars[0] != 0 {
                let loaded = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_LOAD, 4));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(loaded);
                self.builder.functions.push(self.input_vars[0]);

                self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
                self.builder.functions.push(self.output_vars[0]);
                self.builder.functions.push(loaded);
            } else if self.output_vars[0] != 0 {
                let default_pos = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(default_pos);
                self.builder.functions.push(c0);
                self.builder.functions.push(c0);
                self.builder.functions.push(c0);
                self.builder.functions.push(c1);

                self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
                self.builder.functions.push(self.output_vars[0]);
                self.builder.functions.push(default_pos);
            }
        }

        Ok(())
    }

    /// Emit SPIR-V for a single vertex program instruction
    fn emit_vp_instruction(&mut self, instr: &DecodedVpInstruction) -> Result<(), String> {
        // Handle vector operation
        if instr.vec_opcode != VpVecOpcode::Nop {
            let result = self.emit_vec_op(instr)?;
            if result != 0 && instr.vec_writemask != 0 {
                self.write_vp_result(result, instr.vec_dst, instr.vec_writemask, false)?;
            }
        }

        // Handle scalar operation
        if instr.sca_opcode != VpScaOpcode::Nop {
            let result = self.emit_sca_op(instr)?;
            if result != 0 && instr.sca_writemask != 0 {
                self.write_vp_result(result, instr.sca_dst, instr.sca_writemask, true)?;
            }
        }

        Ok(())
    }

    /// Add vertex program constants as SPIR-V constants
    fn add_vp_constants(&mut self) {
        self.constant_ids.clear();
        for constant in &self.program.constants {
            let cx = self.builder.add_float_constant(constant[0]);
            let cy = self.builder.add_float_constant(constant[1]);
            let cz = self.builder.add_float_constant(constant[2]);
            let cw = self.builder.add_float_constant(constant[3]);
            
            let vec_id = self.builder.alloc_id();
            self.builder.types_constants.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.types_constants.push(self.builder.type_vec4);
            self.builder.types_constants.push(vec_id);
            self.builder.types_constants.push(cx);
            self.builder.types_constants.push(cy);
            self.builder.types_constants.push(cz);
            self.builder.types_constants.push(cw);
            
            self.constant_ids.push(vec_id);
        }
    }

    /// Load a source operand
    fn load_source(&mut self, src: &VpSource, d1: &VpD1) -> u32 {
        let value = match src.reg_type {
            VpRegType::Temp => self.temp_regs[src.tmp_src as usize],
            VpRegType::Input => {
                let input_idx = d1.input_src as usize;
                if self.input_vars[input_idx] != 0 {
                    // Load from input variable
                    let loaded = self.builder.alloc_id();
                    self.builder.functions.push(SpirVBuilder::encode_word(OP_LOAD, 4));
                    self.builder.functions.push(self.builder.type_vec4);
                    self.builder.functions.push(loaded);
                    self.builder.functions.push(self.input_vars[input_idx]);
                    loaded
                } else {
                    self.make_zero_vec4()
                }
            }
            VpRegType::Constant => {
                // Load constant from program constants
                let const_idx = d1.const_src as usize;
                let start = self.program.constant_range.0 as usize;
                let local_idx = const_idx.saturating_sub(start);
                
                if local_idx < self.constant_ids.len() {
                    self.constant_ids[local_idx]
                } else {
                    // Constant not loaded, return zero
                    self.make_zero_vec4()
                }
            }
            _ => self.make_zero_vec4(),
        };

        // Apply swizzle if not identity (xyzw = 0123)
        let needs_swizzle = src.swz_x != 0 || src.swz_y != 1 || src.swz_z != 2 || src.swz_w != 3;
        let swizzled = if needs_swizzle {
            let result = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_VECTOR_SHUFFLE, 9));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(value);
            self.builder.functions.push(value);
            self.builder.functions.push(src.swz_x as u32);
            self.builder.functions.push(src.swz_y as u32);
            self.builder.functions.push(src.swz_z as u32);
            self.builder.functions.push(src.swz_w as u32);
            result
        } else {
            value
        };

        // Apply negate if needed
        if src.neg {
            let result = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_FNEGATE, 4));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(swizzled);
            result
        } else {
            swizzled
        }
    }

    /// Emit vector operation
    fn emit_vec_op(&mut self, instr: &DecodedVpInstruction) -> Result<u32, String> {
        let src0 = self.load_source(&instr.sources[0], &instr.d1);
        let src1 = self.load_source(&instr.sources[1], &instr.d1);
        let src2 = self.load_source(&instr.sources[2], &instr.d1);

        let result = match instr.vec_opcode {
            VpVecOpcode::Nop => return Ok(0),
            
            VpVecOpcode::Mov => src0,
            
            VpVecOpcode::Mul => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            VpVecOpcode::Add => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            VpVecOpcode::Mad => {
                // src0 * src1 + src2
                let mul = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(mul);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(mul);
                self.builder.functions.push(src2);
                r
            }
            
            VpVecOpcode::Dp3 => {
                // Extract xyz, dot product
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_DOT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                // Splat to vec4
                let vec = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(vec);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                vec
            }
            
            VpVecOpcode::Dp4 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_DOT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let vec = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(vec);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                vec
            }
            
            VpVecOpcode::Min => {
                // Use GLSL.std.450 FMin
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(37); // FMin
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            VpVecOpcode::Max => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(40); // FMax
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            VpVecOpcode::Frc => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(10); // Fract
                self.builder.functions.push(src0);
                r
            }
            
            VpVecOpcode::Flr => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(8); // Floor
                self.builder.functions.push(src0);
                r
            }
            
            VpVecOpcode::Slt => {
                // Set Less Than: result = src0 < src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FLESS_THAN, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            VpVecOpcode::Sge => {
                // Set Greater or Equal: result = src0 >= src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FGREATER_THAN_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            VpVecOpcode::Seq => {
                // Set Equal: result = src0 == src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FORD_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            VpVecOpcode::Sne => {
                // Set Not Equal: result = src0 != src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FORD_NOT_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            VpVecOpcode::Sgt => {
                // Set Greater Than: result = src0 > src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FGREATER_THAN, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            VpVecOpcode::Sle => {
                // Set Less or Equal: result = src0 <= src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FLESS_THAN_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            VpVecOpcode::Sfl => {
                // Set False (all zeros)
                let c0 = self.builder.add_float_constant(0.0);
                self.make_vec4_from_scalar(c0)
            }
            
            VpVecOpcode::Str => {
                // Set True (all ones)
                let c1 = self.builder.add_float_constant(1.0);
                self.make_vec4_from_scalar(c1)
            }
            
            VpVecOpcode::Ssg => {
                // Sign of source: -1 if < 0, 0 if == 0, 1 if > 0
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(6); // FSign
                self.builder.functions.push(src0);
                r
            }
            
            VpVecOpcode::Dph => {
                // Dot product homogeneous: (src0.x*src1.x + src0.y*src1.y + src0.z*src1.z + src1.w)
                // DPH = DP3(src0.xyz, src1.xyz) + src1.w
                // Need to compute manually since OP_DOT on vec4 gives 4-component result
                
                // Extract xyz components
                let src0_x = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_x);
                self.builder.functions.push(src0);
                self.builder.functions.push(0);
                
                let src0_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_y);
                self.builder.functions.push(src0);
                self.builder.functions.push(1);
                
                let src0_z = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_z);
                self.builder.functions.push(src0);
                self.builder.functions.push(2);
                
                let src1_x = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_x);
                self.builder.functions.push(src1);
                self.builder.functions.push(0);
                
                let src1_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_y);
                self.builder.functions.push(src1);
                self.builder.functions.push(1);
                
                let src1_z = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_z);
                self.builder.functions.push(src1);
                self.builder.functions.push(2);
                
                let src1_w = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_w);
                self.builder.functions.push(src1);
                self.builder.functions.push(3);
                
                // Compute x*x + y*y + z*z
                let mul_x = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_x);
                self.builder.functions.push(src0_x);
                self.builder.functions.push(src1_x);
                
                let mul_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(src0_y);
                self.builder.functions.push(src1_y);
                
                let mul_z = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_z);
                self.builder.functions.push(src0_z);
                self.builder.functions.push(src1_z);
                
                let sum_xy = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(sum_xy);
                self.builder.functions.push(mul_x);
                self.builder.functions.push(mul_y);
                
                let dp3 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(dp3);
                self.builder.functions.push(sum_xy);
                self.builder.functions.push(mul_z);
                
                // Add src1.w
                let sum = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(sum);
                self.builder.functions.push(dp3);
                self.builder.functions.push(src1_w);
                
                // Splat to vec4
                self.make_vec4_from_scalar(sum)
            }
            
            VpVecOpcode::Dst => {
                // Distance vector: result = (1, src0.y*src1.y, src0.z, src1.w)
                let c1 = self.builder.add_float_constant(1.0);
                
                // Extract components
                let src0_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_y);
                self.builder.functions.push(src0);
                self.builder.functions.push(1);
                
                let src1_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_y);
                self.builder.functions.push(src1);
                self.builder.functions.push(1);
                
                let src0_z = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_z);
                self.builder.functions.push(src0);
                self.builder.functions.push(2);
                
                let src1_w = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_w);
                self.builder.functions.push(src1);
                self.builder.functions.push(3);
                
                // y * y
                let mul_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(src0_y);
                self.builder.functions.push(src1_y);
                
                // Construct result vec4
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(c1);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(src0_z);
                self.builder.functions.push(src1_w);
                r
            }
            
            VpVecOpcode::Arl => {
                // Address register load - truncate to integer
                // For now, just pass through (ARL result is used for indexing)
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(9); // Trunc
                self.builder.functions.push(src0);
                r
            }
            
            VpVecOpcode::Txl => {
                // Texture lookup with explicit LOD (in vertex shader)
                // src0 is texture coordinate with LOD in w
                // For now, just return a default (white) since VP textures are rare
                let c1 = self.builder.add_float_constant(1.0);
                self.make_vec4_from_scalar(c1)
            }
            
            _ => {
                // Unhandled - return src0 as fallback
                src0
            }
        };

        Ok(result)
    }

    /// Emit scalar operation
    fn emit_sca_op(&mut self, instr: &DecodedVpInstruction) -> Result<u32, String> {
        let src0 = self.load_source(&instr.sources[0], &instr.d1);
        
        // Extract x component for scalar ops
        let x = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
        self.builder.functions.push(self.builder.type_float);
        self.builder.functions.push(x);
        self.builder.functions.push(src0);
        self.builder.functions.push(0);

        let scalar = match instr.sca_opcode {
            VpScaOpcode::Nop => return Ok(0),
            
            VpScaOpcode::Mov => x,
            
            VpScaOpcode::Rcp => {
                let r = self.builder.alloc_id();
                let one = self.builder.add_float_constant(1.0);
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FDIV, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(one);
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Rsq => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(32); // InverseSqrt
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Exp => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(27); // Exp
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Log => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(28); // Log
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Ex2 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(29); // Exp2
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Lg2 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(30); // Log2
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Sin => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(13); // Sin
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Cos => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(14); // Cos
                self.builder.functions.push(x);
                r
            }
            
            VpScaOpcode::Rcc => {
                // Reciprocal clamped: clamp(1/x, 5.42101e-36, 1.884467e+19)
                // For simplicity, just do reciprocal with max/min clamp
                let one = self.builder.add_float_constant(1.0);
                let rcp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FDIV, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(rcp);
                self.builder.functions.push(one);
                self.builder.functions.push(x);
                
                // Clamp result
                let min_val = self.builder.add_float_constant(5.42101e-36);
                let max_val = self.builder.add_float_constant(1.884467e+19);
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 8));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(43); // FClamp
                self.builder.functions.push(rcp);
                self.builder.functions.push(min_val);
                self.builder.functions.push(max_val);
                r
            }
            
            VpScaOpcode::Lit => {
                // Lit: result.x = 1.0, result.y = max(0, src.x), result.z = special, result.w = 1.0
                // For scalar, we return the y component calculation
                let c0 = self.builder.add_float_constant(0.0);
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(40); // FMax
                self.builder.functions.push(x);
                self.builder.functions.push(c0);
                r
            }
            
            VpScaOpcode::Bra | VpScaOpcode::Bri | VpScaOpcode::Cal | VpScaOpcode::Cli |
            VpScaOpcode::Ret | VpScaOpcode::Brb | VpScaOpcode::Clb => {
                // Flow control ops - these affect control flow, not register output
                // In SPIR-V, these would need structured control flow
                // For now, return x as no-op (flow control handled separately)
                x
            }
            
            VpScaOpcode::Psh | VpScaOpcode::Pop => {
                // Address stack operations - not directly representable in SPIR-V
                // These would need to be handled by the control flow analysis
                x
            }
            
            _ => {
                // Unhandled - return x as fallback
                x
            }
        };

        // Splat scalar to vec4
        let result = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(result);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);

        Ok(result)
    }

    /// Write result to destination register
    fn write_vp_result(&mut self, value: u32, dst: u8, mask: u8, _is_scalar: bool) -> Result<(), String> {
        // Check if dst is output register (bit 6)
        let is_output = dst & 0x20 != 0;
        let reg_idx = (dst & 0x1F) as usize;

        if is_output {
            // Write to output variable
            if reg_idx < 16 && self.output_vars[reg_idx] != 0 {
                // Apply write mask if partial
                if mask == 0xF {
                    // Full write
                    self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
                    self.builder.functions.push(self.output_vars[reg_idx]);
                    self.builder.functions.push(value);
                } else {
                    // Partial write - load current, blend, store
                    let current = self.builder.alloc_id();
                    self.builder.functions.push(SpirVBuilder::encode_word(OP_LOAD, 4));
                    self.builder.functions.push(self.builder.type_vec4);
                    self.builder.functions.push(current);
                    self.builder.functions.push(self.output_vars[reg_idx]);

                    // Blend based on mask
                    let blended = self.blend_by_mask(current, value, mask);
                    
                    self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
                    self.builder.functions.push(self.output_vars[reg_idx]);
                    self.builder.functions.push(blended);
                }
            }
        } else if reg_idx < 32 {
            // Write to temp register
            if mask == 0xF {
                self.temp_regs[reg_idx] = value;
            } else {
                // Partial write
                let current = self.temp_regs[reg_idx];
                let blended = self.blend_by_mask(current, value, mask);
                self.temp_regs[reg_idx] = blended;
            }
        }

        Ok(())
    }

    /// Blend two vectors based on write mask
    fn blend_by_mask(&mut self, old: u32, new: u32, mask: u8) -> u32 {
        // Use VectorShuffle to select components
        let result = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_VECTOR_SHUFFLE, 9));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(result);
        self.builder.functions.push(old);
        self.builder.functions.push(new);
        // Select from new (indices 4-7) if mask bit set, else old (indices 0-3)
        self.builder.functions.push(if mask & 0x8 != 0 { 4 } else { 0 }); // x
        self.builder.functions.push(if mask & 0x4 != 0 { 5 } else { 1 }); // y
        self.builder.functions.push(if mask & 0x2 != 0 { 6 } else { 2 }); // z
        self.builder.functions.push(if mask & 0x1 != 0 { 7 } else { 3 }); // w
        result
    }

    /// Create a zero vec4
    fn make_zero_vec4(&mut self) -> u32 {
        let c0 = self.builder.add_float_constant(0.0);
        let zero = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(zero);
        self.builder.functions.push(c0);
        self.builder.functions.push(c0);
        self.builder.functions.push(c0);
        self.builder.functions.push(c0);
        zero
    }

    /// Create a vec4 from a scalar value (splat)
    fn make_vec4_from_scalar(&mut self, scalar: u32) -> u32 {
        let vec = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(vec);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        vec
    }
}

/// SPIR-V generator for fragment programs
/// Fragment program SPIR-V generator
pub struct FpSpirVGen<'a> {
    program: &'a FragmentProgram,
    builder: SpirVBuilder,
    /// Temp register IDs (r0-r63)
    temp_regs: [u32; 64],
    /// Input variable IDs (varyings from vertex shader)
    input_vars: [u32; 16],
    /// Output color variable
    color_out: u32,
    /// Texture sampler IDs (combined image sampler)
    sampler_vars: [u32; 16],
    /// Sampled image type
    sampled_image_type: u32,
}

impl<'a> FpSpirVGen<'a> {
    pub fn new(program: &'a FragmentProgram) -> Self {
        Self {
            program,
            builder: SpirVBuilder::new(),
            temp_regs: [0; 64],
            input_vars: [0; 16],
            color_out: 0,
            sampler_vars: [0; 16],
            sampled_image_type: 0,
        }
    }

    /// Generate SPIR-V for the fragment program
    pub fn generate(mut self) -> Result<SpirVModule, String> {
        // Setup common stuff
        self.builder.add_capability(CAP_SHADER);
        self.builder.add_glsl_import();
        self.builder.add_memory_model();
        self.builder.add_basic_types();

        // Create pointer types
        let ptr_input_vec4 = self.builder.add_pointer_type(STORAGE_INPUT, self.builder.type_vec4);
        let ptr_output_vec4 = self.builder.add_pointer_type(STORAGE_OUTPUT, self.builder.type_vec4);
        
        // Create input variables (varyings)
        let mut interface_vars = Vec::new();
        for i in 0..8 {
            // Common inputs: color0, color1, texcoord0-5
            let var = self.builder.add_variable(ptr_input_vec4, STORAGE_INPUT);
            self.input_vars[i] = var;
            self.builder.add_location(var, i as u32);
            interface_vars.push(var);
        }
        
        // Create texture samplers based on texture_mask
        self.create_texture_samplers();

        // Create output variable (color)
        self.color_out = self.builder.add_variable(ptr_output_vec4, STORAGE_OUTPUT);
        self.builder.add_location(self.color_out, 0);
        interface_vars.push(self.color_out);

        // Create main function
        let main_id = self.builder.alloc_id();
        
        // Entry point with all interface variables
        let ep_word_count = 4 + interface_vars.len() as u16;
        self.builder.entry_points.push(SpirVBuilder::encode_word(OP_ENTRY_POINT, ep_word_count));
        self.builder.entry_points.push(EXEC_MODEL_FRAGMENT);
        self.builder.entry_points.push(main_id);
        self.builder.entry_points.push(0x6E69616D); // "main"
        for var in &interface_vars {
            self.builder.entry_points.push(*var);
        }

        // Execution mode - origin upper left
        self.builder.execution_modes.push(SpirVBuilder::encode_word(OP_EXECUTION_MODE, 3));
        self.builder.execution_modes.push(main_id);
        self.builder.execution_modes.push(EXEC_MODE_ORIGIN_UPPER_LEFT);

        // Function definition
        self.builder.functions.push(SpirVBuilder::encode_word(OP_FUNCTION, 5));
        self.builder.functions.push(self.builder.type_void);
        self.builder.functions.push(main_id);
        self.builder.functions.push(0); // None
        self.builder.functions.push(self.builder.type_func_void);

        // Label
        let label_id = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_LABEL, 2));
        self.builder.functions.push(label_id);

        // Initialize temp registers to zero
        let c0 = self.builder.add_float_constant(0.0);
        for i in 0..64 {
            let reg = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(reg);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.temp_regs[i] = reg;
        }

        // Process decoded instructions
        self.generate_fp_instructions()?;

        // If no decoded instructions, output default color
        if self.program.decoded.is_empty() {
            let c1 = self.builder.add_float_constant(1.0);
            let white = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(white);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);

            self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
            self.builder.functions.push(self.color_out);
            self.builder.functions.push(white);
        }

        // Return
        self.builder.functions.push(SpirVBuilder::encode_word(OP_RETURN, 1));
        
        // End function
        self.builder.functions.push(SpirVBuilder::encode_word(OP_FUNCTION_END, 1));

        Ok(SpirVModule {
            bytecode: self.builder.build(),
            stage: ShaderStage::FRAGMENT,
        })
    }

    fn generate_fp_instructions(&mut self) -> Result<(), String> {
        for instr in &self.program.decoded.clone() {
            self.emit_fp_instruction(instr)?;
        }
        Ok(())
    }

    fn emit_fp_instruction(&mut self, instr: &DecodedFpInstruction) -> Result<(), String> {
        let result = self.emit_fp_op(instr)?;
        
        if result != 0 && !instr.dest.no_dest {
            let dest_reg = instr.dest.dest_reg as usize;
            let writemask = instr.dest.writemask;
            
            // Check if writing to output color (r0 in FP is typically output)
            if dest_reg == 0 {
                self.write_fp_output(result, writemask)?;
            } else if dest_reg < 64 {
                self.write_fp_temp(result, dest_reg, writemask)?;
            }
        }

        Ok(())
    }

    fn load_fp_source(&mut self, src: &FpSource) -> u32 {
        let value = match src.reg_type {
            FpRegType::Temp => {
                let idx = src.reg_index as usize;
                if idx < 64 { self.temp_regs[idx] } else { self.temp_regs[0] }
            }
            FpRegType::Input => {
                let idx = src.reg_index as usize;
                if idx < 16 && self.input_vars[idx] != 0 {
                    let loaded = self.builder.alloc_id();
                    self.builder.functions.push(SpirVBuilder::encode_word(OP_LOAD, 4));
                    self.builder.functions.push(self.builder.type_vec4);
                    self.builder.functions.push(loaded);
                    self.builder.functions.push(self.input_vars[idx]);
                    loaded
                } else {
                    self.make_zero_vec4()
                }
            }
            FpRegType::Constant => {
                // Would need constant buffer
                self.make_zero_vec4()
            }
            _ => self.make_zero_vec4(),
        };

        // Apply swizzle
        let needs_swizzle = src.swz_x != 0 || src.swz_y != 1 || src.swz_z != 2 || src.swz_w != 3;
        let swizzled = if needs_swizzle {
            let result = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_VECTOR_SHUFFLE, 9));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(value);
            self.builder.functions.push(value);
            self.builder.functions.push(src.swz_x as u32);
            self.builder.functions.push(src.swz_y as u32);
            self.builder.functions.push(src.swz_z as u32);
            self.builder.functions.push(src.swz_w as u32);
            result
        } else {
            value
        };

        // Apply abs
        let absed = if src.abs {
            let result = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(self.builder.glsl_ext_id);
            self.builder.functions.push(4); // FAbs
            self.builder.functions.push(swizzled);
            result
        } else {
            swizzled
        };

        // Apply negate
        if src.neg {
            let result = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_FNEGATE, 4));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(absed);
            result
        } else {
            absed
        }
    }

    fn make_zero_vec4(&mut self) -> u32 {
        let c0 = self.builder.add_float_constant(0.0);
        let zero = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(zero);
        self.builder.functions.push(c0);
        self.builder.functions.push(c0);
        self.builder.functions.push(c0);
        self.builder.functions.push(c0);
        zero
    }

    /// Create a vec4 from a scalar value (splat)
    fn make_vec4_from_scalar(&mut self, scalar: u32) -> u32 {
        let vec = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(vec);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        self.builder.functions.push(scalar);
        vec
    }

    fn emit_fp_op(&mut self, instr: &DecodedFpInstruction) -> Result<u32, String> {
        let src0 = self.load_fp_source(&instr.sources[0]);
        let src1 = self.load_fp_source(&instr.sources[1]);
        let src2 = self.load_fp_source(&instr.sources[2]);

        let result = match instr.opcode {
            FpOpcode::Nop => return Ok(0),
            
            FpOpcode::Mov => src0,
            
            FpOpcode::Mul => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            FpOpcode::Add => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            FpOpcode::Mad => {
                // src0 * src1 + src2
                let mul = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(mul);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(mul);
                self.builder.functions.push(src2);
                r
            }
            
            FpOpcode::Dp3 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_DOT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                // Splat to vec4
                let vec = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(vec);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                vec
            }
            
            FpOpcode::Dp4 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_DOT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let vec = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(vec);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                self.builder.functions.push(r);
                vec
            }
            
            FpOpcode::Min => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(37); // FMin
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            FpOpcode::Max => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(40); // FMax
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            FpOpcode::Frc => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(10); // Fract
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Flr => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(8); // Floor
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Rcp => {
                // 1.0 / src0
                let c1 = self.builder.add_float_constant(1.0);
                let one = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(one);
                self.builder.functions.push(c1);
                self.builder.functions.push(c1);
                self.builder.functions.push(c1);
                self.builder.functions.push(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FDIV, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(one);
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Rsq => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(32); // InverseSqrt
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Ex2 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(29); // Exp2
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Lg2 => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(30); // Log2
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Sin => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(13); // Sin
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Cos => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(14); // Cos
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Pow => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(26); // Pow
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            FpOpcode::Lrp => {
                // mix(src2, src1, src0)
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 8));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(46); // FMix
                self.builder.functions.push(src2);
                self.builder.functions.push(src1);
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Nrm => {
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(69); // Normalize
                self.builder.functions.push(src0);
                r
            }
            
            // Texture sampling instructions
            FpOpcode::Tex => {
                // Sample texture with implicit LOD
                self.sample_texture(instr.tex_unit, src0, false, false)
            }
            
            FpOpcode::Txp => {
                // Sample texture with projective divide (divide by w)
                self.sample_texture(instr.tex_unit, src0, true, false)
            }
            
            FpOpcode::Txl => {
                // Sample texture with explicit LOD (LOD in src0.w)
                self.sample_texture(instr.tex_unit, src0, false, true)
            }
            
            FpOpcode::Txb => {
                // Sample texture with bias (bias in src0.w)
                // For now, treat like TEX
                self.sample_texture(instr.tex_unit, src0, false, false)
            }
            
            FpOpcode::Txd => {
                // Sample texture with derivatives - treat like TEX for now
                // Full implementation would need OpImageSampleExplicitLod with Grad
                self.sample_texture(instr.tex_unit, src0, false, false)
            }
            
            FpOpcode::Slt => {
                // Set Less Than: result = src0 < src1 ? 1.0 : 0.0
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FLESS_THAN, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            FpOpcode::Sge => {
                // Set Greater or Equal
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FGREATER_THAN_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            FpOpcode::Sle => {
                // Set Less or Equal
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FLESS_THAN_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            FpOpcode::Sgt => {
                // Set Greater Than
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FGREATER_THAN, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            FpOpcode::Sne => {
                // Set Not Equal
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FORD_NOT_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            FpOpcode::Seq => {
                // Set Equal
                let cmp = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FORD_EQUAL, 5));
                self.builder.functions.push(self.builder.type_bvec4);
                self.builder.functions.push(cmp);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                let zero = self.make_vec4_from_scalar(c0);
                let one = self.make_vec4_from_scalar(c1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_SELECT, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(cmp);
                self.builder.functions.push(one);
                self.builder.functions.push(zero);
                r
            }
            
            FpOpcode::Str => {
                // Set True (all 1.0s)
                let c1 = self.builder.add_float_constant(1.0);
                self.make_vec4_from_scalar(c1)
            }
            
            FpOpcode::Sfl => {
                // Set False (all 0.0s)
                let c0 = self.builder.add_float_constant(0.0);
                self.make_vec4_from_scalar(c0)
            }
            
            FpOpcode::Dst => {
                // Distance vector: result = (1, src0.y*src1.y, src0.z, src1.w)
                let c1 = self.builder.add_float_constant(1.0);
                
                // Extract components
                let src0_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_y);
                self.builder.functions.push(src0);
                self.builder.functions.push(1);
                
                let src1_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_y);
                self.builder.functions.push(src1);
                self.builder.functions.push(1);
                
                let src0_z = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_z);
                self.builder.functions.push(src0);
                self.builder.functions.push(2);
                
                let src1_w = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src1_w);
                self.builder.functions.push(src1);
                self.builder.functions.push(3);
                
                // y * y
                let mul_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(src0_y);
                self.builder.functions.push(src1_y);
                
                // Construct result vec4
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(c1);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(src0_z);
                self.builder.functions.push(src1_w);
                r
            }
            
            FpOpcode::Lit => {
                // Lit: compute lighting coefficients
                // result.x = 1.0, result.y = max(0, src0.x), 
                // result.z = src0.x > 0 ? pow(max(0, src0.y), clamp(src0.w, -128, 128)) : 0
                // result.w = 1.0
                let c0 = self.builder.add_float_constant(0.0);
                let c1 = self.builder.add_float_constant(1.0);
                
                // Extract src0.x
                let src0_x = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(src0_x);
                self.builder.functions.push(src0);
                self.builder.functions.push(0);
                
                // max(0, src0.x)
                let y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(y);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(40); // FMax
                self.builder.functions.push(c0);
                self.builder.functions.push(src0_x);
                
                // For z, just use 1.0 for simplicity (full LIT is complex)
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(c1);
                self.builder.functions.push(y);
                self.builder.functions.push(c1);
                self.builder.functions.push(c1);
                r
            }
            
            FpOpcode::Ddx => {
                // Derivative with respect to x (screen space)
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_DPDX, 4));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Ddy => {
                // Derivative with respect to y (screen space)
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_DPDY, 4));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                r
            }
            
            FpOpcode::Kil => {
                // Kill/discard fragment - check if src0 < 0
                // In SPIR-V, this requires checking condition then OpKill
                // For now, we'll emit unconditional kill (conservative)
                // Real implementation would need control flow for conditional kill
                self.builder.functions.push(SpirVBuilder::encode_word(OP_KILL, 1));
                self.make_zero_vec4() // Return dummy value (execution terminates)
            }
            
            FpOpcode::Div => {
                // Component-wise division
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FDIV, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            FpOpcode::Divsq => {
                // Division by sqrt: src0 / sqrt(src1)
                let sqrt = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 6));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(sqrt);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(31); // Sqrt
                self.builder.functions.push(src1);
                
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FDIV, 5));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(src0);
                self.builder.functions.push(sqrt);
                r
            }
            
            FpOpcode::Dp2 => {
                // 2-component dot product
                // Extract x and y from both sources, compute x0*x1 + y0*y1
                let x0 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(x0);
                self.builder.functions.push(src0);
                self.builder.functions.push(0);
                
                let y0 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(y0);
                self.builder.functions.push(src0);
                self.builder.functions.push(1);
                
                let x1 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(x1);
                self.builder.functions.push(src1);
                self.builder.functions.push(0);
                
                let y1 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(y1);
                self.builder.functions.push(src1);
                self.builder.functions.push(1);
                
                let mul_x = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_x);
                self.builder.functions.push(x0);
                self.builder.functions.push(x1);
                
                let mul_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(y0);
                self.builder.functions.push(y1);
                
                let sum = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(sum);
                self.builder.functions.push(mul_x);
                self.builder.functions.push(mul_y);
                
                // Splat to vec4
                self.make_vec4_from_scalar(sum)
            }
            
            FpOpcode::Dp2a => {
                // DP2A: x0*x1 + y0*y1 + z2
                let x0 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(x0);
                self.builder.functions.push(src0);
                self.builder.functions.push(0);
                
                let y0 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(y0);
                self.builder.functions.push(src0);
                self.builder.functions.push(1);
                
                let x1 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(x1);
                self.builder.functions.push(src1);
                self.builder.functions.push(0);
                
                let y1 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(y1);
                self.builder.functions.push(src1);
                self.builder.functions.push(1);
                
                let z2 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(z2);
                self.builder.functions.push(src2);
                self.builder.functions.push(2);
                
                let mul_x = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_x);
                self.builder.functions.push(x0);
                self.builder.functions.push(x1);
                
                let mul_y = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FMUL, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(mul_y);
                self.builder.functions.push(y0);
                self.builder.functions.push(y1);
                
                let sum1 = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(sum1);
                self.builder.functions.push(mul_x);
                self.builder.functions.push(mul_y);
                
                let sum = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_FADD, 5));
                self.builder.functions.push(self.builder.type_float);
                self.builder.functions.push(sum);
                self.builder.functions.push(sum1);
                self.builder.functions.push(z2);
                
                self.make_vec4_from_scalar(sum)
            }
            
            FpOpcode::Refl => {
                // Reflect: src0 - 2 * dot(src0, src1) * src1
                let r = self.builder.alloc_id();
                self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 7));
                self.builder.functions.push(self.builder.type_vec4);
                self.builder.functions.push(r);
                self.builder.functions.push(self.builder.glsl_ext_id);
                self.builder.functions.push(71); // Reflect
                self.builder.functions.push(src0);
                self.builder.functions.push(src1);
                r
            }
            
            // Flow control ops - these would need structured control flow
            FpOpcode::Brk | FpOpcode::Cal | FpOpcode::Ife | FpOpcode::Loop | 
            FpOpcode::Rep | FpOpcode::Ret => {
                // Flow control operations are handled at a higher level
                // They affect program structure, not register values
                self.make_zero_vec4()
            }
            
            FpOpcode::Lif => {
                // Load immediate float - typically embedded in instruction
                // For now, return src0
                src0
            }
            
            FpOpcode::Fenct | FpOpcode::Fencb => {
                // Fence operations for texture/buffer access ordering
                // No register result needed
                src0
            }
            
            _ => {
                // Unhandled - return src0
                src0
            }
        };

        // Apply saturation if needed
        let final_result = if instr.dest.saturate && result != 0 {
            let c0 = self.builder.add_float_constant(0.0);
            let c1 = self.builder.add_float_constant(1.0);
            let zero = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(zero);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            self.builder.functions.push(c0);
            
            let one = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(one);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            
            let r = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_EXT_INST, 8));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(r);
            self.builder.functions.push(self.builder.glsl_ext_id);
            self.builder.functions.push(43); // FClamp
            self.builder.functions.push(result);
            self.builder.functions.push(zero);
            self.builder.functions.push(one);
            r
        } else {
            result
        };

        Ok(final_result)
    }

    fn write_fp_output(&mut self, value: u32, mask: u8) -> Result<(), String> {
        if mask == 0xF {
            self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
            self.builder.functions.push(self.color_out);
            self.builder.functions.push(value);
        } else {
            // Partial write - load current, blend, store
            let current = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_LOAD, 4));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(current);
            self.builder.functions.push(self.color_out);

            let blended = self.blend_by_mask(current, value, mask);
            
            self.builder.functions.push(SpirVBuilder::encode_word(OP_STORE, 3));
            self.builder.functions.push(self.color_out);
            self.builder.functions.push(blended);
        }
        Ok(())
    }

    fn write_fp_temp(&mut self, value: u32, idx: usize, mask: u8) -> Result<(), String> {
        if mask == 0xF {
            self.temp_regs[idx] = value;
        } else {
            let current = self.temp_regs[idx];
            let blended = self.blend_by_mask(current, value, mask);
            self.temp_regs[idx] = blended;
        }
        Ok(())
    }

    fn blend_by_mask(&mut self, old: u32, new: u32, mask: u8) -> u32 {
        let result = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_VECTOR_SHUFFLE, 9));
        self.builder.functions.push(self.builder.type_vec4);
        self.builder.functions.push(result);
        self.builder.functions.push(old);
        self.builder.functions.push(new);
        self.builder.functions.push(if mask & 0x8 != 0 { 4 } else { 0 }); // x
        self.builder.functions.push(if mask & 0x4 != 0 { 5 } else { 1 }); // y
        self.builder.functions.push(if mask & 0x2 != 0 { 6 } else { 2 }); // z
        self.builder.functions.push(if mask & 0x1 != 0 { 7 } else { 3 }); // w
        result
    }

    /// Create texture sampler types and variables
    fn create_texture_samplers(&mut self) {
        // Create image type: OpTypeImage float Dim2D 0 0 0 1 Unknown
        let image_type = self.builder.alloc_id();
        self.builder.types_constants.push(SpirVBuilder::encode_word(OP_TYPE_IMAGE, 9));
        self.builder.types_constants.push(image_type);
        self.builder.types_constants.push(self.builder.type_float);
        self.builder.types_constants.push(1); // Dim = 2D
        self.builder.types_constants.push(0); // Depth = not depth
        self.builder.types_constants.push(0); // Arrayed = no
        self.builder.types_constants.push(0); // MS = no
        self.builder.types_constants.push(1); // Sampled = used with sampler
        self.builder.types_constants.push(0); // Image format = Unknown

        // Create sampled image type
        self.sampled_image_type = self.builder.alloc_id();
        self.builder.types_constants.push(SpirVBuilder::encode_word(OP_TYPE_SAMPLED_IMAGE, 3));
        self.builder.types_constants.push(self.sampled_image_type);
        self.builder.types_constants.push(image_type);

        // Create pointer type for UniformConstant samplers
        let ptr_sampled_image = self.builder.alloc_id();
        self.builder.types_constants.push(SpirVBuilder::encode_word(OP_TYPE_POINTER, 4));
        self.builder.types_constants.push(ptr_sampled_image);
        self.builder.types_constants.push(0); // StorageClass = UniformConstant
        self.builder.types_constants.push(self.sampled_image_type);

        // Create sampler variables for each used texture unit
        for i in 0..16 {
            if self.program.texture_mask & (1 << i) != 0 {
                let var = self.builder.alloc_id();
                self.builder.types_constants.push(SpirVBuilder::encode_word(OP_VARIABLE, 4));
                self.builder.types_constants.push(ptr_sampled_image);
                self.builder.types_constants.push(var);
                self.builder.types_constants.push(0); // StorageClass = UniformConstant
                
                // Add binding decoration
                self.builder.annotations.push(SpirVBuilder::encode_word(OP_DECORATE, 4));
                self.builder.annotations.push(var);
                self.builder.annotations.push(33); // Decoration = Binding
                self.builder.annotations.push(i as u32);
                
                // Add descriptor set decoration
                self.builder.annotations.push(SpirVBuilder::encode_word(OP_DECORATE, 4));
                self.builder.annotations.push(var);
                self.builder.annotations.push(34); // Decoration = DescriptorSet
                self.builder.annotations.push(0);
                
                self.sampler_vars[i] = var;
            }
        }
    }

    /// Sample a texture
    fn sample_texture(&mut self, tex_unit: u8, coord: u32, projective: bool, explicit_lod: bool) -> u32 {
        let unit = tex_unit as usize;
        if unit >= 16 || self.sampler_vars[unit] == 0 {
            // No sampler for this unit, return opaque white
            let c1 = self.builder.add_float_constant(1.0);
            let white = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(white);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            self.builder.functions.push(c1);
            return white;
        }

        // Load the sampler
        let sampler = self.builder.alloc_id();
        self.builder.functions.push(SpirVBuilder::encode_word(OP_LOAD, 4));
        self.builder.functions.push(self.sampled_image_type);
        self.builder.functions.push(sampler);
        self.builder.functions.push(self.sampler_vars[unit]);

        // Prepare texture coordinate
        let tex_coord = if projective {
            // Divide xy by w for projective texturing
            // Extract w component
            let w = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
            self.builder.functions.push(self.builder.type_float);
            self.builder.functions.push(w);
            self.builder.functions.push(coord);
            self.builder.functions.push(3); // w component

            // Create vec4 with w in all components
            let w_splat = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_CONSTRUCT, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(w_splat);
            self.builder.functions.push(w);
            self.builder.functions.push(w);
            self.builder.functions.push(w);
            self.builder.functions.push(w);

            // Divide coord by w
            let divided = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_FDIV, 5));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(divided);
            self.builder.functions.push(coord);
            self.builder.functions.push(w_splat);
            divided
        } else {
            coord
        };

        // Sample the texture
        let result = self.builder.alloc_id();
        if explicit_lod {
            // OpImageSampleExplicitLod with Lod operand (from coord.w)
            let lod = self.builder.alloc_id();
            self.builder.functions.push(SpirVBuilder::encode_word(OP_COMPOSITE_EXTRACT, 5));
            self.builder.functions.push(self.builder.type_float);
            self.builder.functions.push(lod);
            self.builder.functions.push(coord);
            self.builder.functions.push(3); // w component is LOD

            self.builder.functions.push(SpirVBuilder::encode_word(OP_IMAGE_SAMPLE_EXPLICIT_LOD, 7));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(sampler);
            self.builder.functions.push(tex_coord);
            self.builder.functions.push(2); // Image operands: Lod
            self.builder.functions.push(lod);
        } else {
            // OpImageSampleImplicitLod
            self.builder.functions.push(SpirVBuilder::encode_word(OP_IMAGE_SAMPLE_IMPLICIT_LOD, 5));
            self.builder.functions.push(self.builder.type_vec4);
            self.builder.functions.push(result);
            self.builder.functions.push(sampler);
            self.builder.functions.push(tex_coord);
        }

        result
    }
}

/// Main shader translator
pub struct ShaderTranslator {
    vertex_cache: Vec<(u64, SpirVModule)>,
    fragment_cache: Vec<(u64, SpirVModule)>,
}

impl ShaderTranslator {
    pub fn new() -> Self {
        Self {
            vertex_cache: Vec::new(),
            fragment_cache: Vec::new(),
        }
    }

    /// Compute hash for a program
    fn hash_program(data: &[u32]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    /// Translate vertex program to SPIR-V
    pub fn translate_vertex(&mut self, program: &mut VertexProgram) -> Result<SpirVModule, String> {
        let hash = Self::hash_program(&program.instructions);

        // Check cache
        if let Some((_, module)) = self.vertex_cache.iter().find(|(h, _)| *h == hash) {
            return Ok(module.clone());
        }

        // Decode instructions
        super::vp_decode::VpDecoder::decode(program)?;

        // Generate SPIR-V
        let gen = VpSpirVGen::new(program);
        let module = gen.generate()?;

        // Cache result
        self.vertex_cache.push((hash, module.clone()));

        Ok(module)
    }

    /// Translate fragment program to SPIR-V
    pub fn translate_fragment(&mut self, program: &mut FragmentProgram) -> Result<SpirVModule, String> {
        let hash = Self::hash_program(&program.instructions);

        // Check cache
        if let Some((_, module)) = self.fragment_cache.iter().find(|(h, _)| *h == hash) {
            return Ok(module.clone());
        }

        // Decode instructions
        super::fp_decode::FpDecoder::decode(program)?;

        // Generate SPIR-V
        let gen = FpSpirVGen::new(program);
        let module = gen.generate()?;

        // Cache result
        self.fragment_cache.push((hash, module.clone()));

        Ok(module)
    }

    /// Clear shader caches
    pub fn clear_cache(&mut self) {
        self.vertex_cache.clear();
        self.fragment_cache.clear();
    }
}

impl Default for ShaderTranslator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spirv_header() {
        let builder = SpirVBuilder::new();
        let spirv = builder.build();
        assert_eq!(spirv[0], SPIRV_MAGIC);
    }

    #[test]
    fn test_vertex_program_passthrough() {
        // Create a simple vertex program with position input
        let mut program = VertexProgram::new();
        program.input_mask = 0x1; // Position input
        program.output_mask = 0x1; // Position output
        // Empty instructions - will generate passthrough
        
        let gen = VpSpirVGen::new(&program);
        let result = gen.generate();
        
        assert!(result.is_ok(), "VP generation should succeed");
        let module = result.unwrap();
        assert!(!module.bytecode.is_empty(), "Should generate SPIR-V bytecode");
        assert_eq!(module.bytecode[0], SPIRV_MAGIC, "Should have valid SPIR-V magic");
        assert!(module.stage.contains(ShaderStage::VERTEX));
    }

    #[test]
    fn test_fragment_program_passthrough() {
        // Create a simple fragment program
        let program = FragmentProgram::new();
        // Empty instructions - will generate white output
        
        let gen = FpSpirVGen::new(&program);
        let result = gen.generate();
        
        assert!(result.is_ok(), "FP generation should succeed");
        let module = result.unwrap();
        assert!(!module.bytecode.is_empty(), "Should generate SPIR-V bytecode");
        assert_eq!(module.bytecode[0], SPIRV_MAGIC, "Should have valid SPIR-V magic");
        assert!(module.stage.contains(ShaderStage::FRAGMENT));
    }

    #[test]
    fn test_shader_translator_caching() {
        let mut translator = ShaderTranslator::new();
        
        let mut vp = VertexProgram::new();
        vp.input_mask = 0x1;
        vp.output_mask = 0x1;
        
        // First translation
        let result1 = translator.translate_vertex(&mut vp);
        assert!(result1.is_ok());
        
        // Second translation should hit cache
        let result2 = translator.translate_vertex(&mut vp);
        assert!(result2.is_ok());
        
        // Both should produce same bytecode
        let module1 = result1.unwrap();
        let module2 = result2.unwrap();
        assert_eq!(module1.bytecode, module2.bytecode);
    }

    #[test]
    fn test_spirv_basic_types() {
        let mut builder = SpirVBuilder::new();
        builder.add_capability(CAP_SHADER);
        builder.add_memory_model();
        builder.add_basic_types();
        
        // Verify types were allocated before consuming builder
        assert!(builder.type_void != 0);
        assert!(builder.type_float != 0);
        assert!(builder.type_vec4 != 0);
        
        let spirv = builder.build();
        
        // Should have header (5 words) + capability + memory model + types
        assert!(spirv.len() > 10, "Should have generated type definitions");
    }

    #[test]
    fn test_vertex_program_with_constants() {
        let mut program = VertexProgram::new();
        program.input_mask = 0x1;
        program.output_mask = 0x1;
        program.constant_range = (0, 4);
        program.constants = vec![
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        ];
        
        let gen = VpSpirVGen::new(&program);
        let result = gen.generate();
        
        assert!(result.is_ok(), "VP with constants should generate successfully");
        let module = result.unwrap();
        assert!(!module.bytecode.is_empty());
    }

    #[test]
    fn test_fragment_program_with_texture() {
        let mut program = FragmentProgram::new();
        program.texture_mask = 0x1; // Texture unit 0 enabled
        
        let gen = FpSpirVGen::new(&program);
        let result = gen.generate();
        
        assert!(result.is_ok(), "FP with texture should generate successfully");
        let module = result.unwrap();
        assert!(!module.bytecode.is_empty());
    }
}
