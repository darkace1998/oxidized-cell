use std::env;
use std::path::PathBuf;

fn main() {
    // Get the workspace root
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let cpp_src = workspace_root.join("cpp").join("src");
    let cpp_include = workspace_root.join("cpp").join("include");
    
    // Check if C++ sources exist
    if !cpp_src.exists() {
        eprintln!("Warning: C++ source directory not found at {:?}", cpp_src);
        println!("cargo:warning=C++ JIT sources not found, JIT functionality will be limited");
        return;
    }
    
    // Build C++ components
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .flag_if_supported("-std=c++20")
        .flag_if_supported("/std:c++20") // MSVC
        .include(&cpp_include)
        .file(cpp_src.join("ffi.cpp"))
        .file(cpp_src.join("ppu_jit.cpp"))
        .file(cpp_src.join("spu_jit.cpp"))
        .file(cpp_src.join("atomics.cpp"))
        .file(cpp_src.join("dma.cpp"));
    
    // Platform-specific settings
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_arch == "x86_64" {
        build.flag_if_supported("-mavx2");
        build.flag_if_supported("-mbmi2");
        build.flag_if_supported("/arch:AVX2"); // MSVC
        
        // Add SIMD source if it exists
        let simd_file = cpp_src.join("simd_avx.cpp");
        if simd_file.exists() {
            build.file(simd_file);
        }
    }
    
    // Add RSX shaders if it exists
    let rsx_file = cpp_src.join("rsx_shaders.cpp");
    if rsx_file.exists() {
        build.file(rsx_file);
    }
    
    // Compile
    build.compile("oc_cpp");
    
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", cpp_src.display());
    println!("cargo:rerun-if-changed={}", cpp_include.display());
}
