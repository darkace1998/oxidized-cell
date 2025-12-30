use std::env;
use std::path::PathBuf;

fn main() {
    // Get the workspace root
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR environment variable not set")
    );
    let workspace_root = manifest_dir
        .parent()
        .expect("Failed to get parent directory")
        .parent()
        .expect("Failed to get workspace root");
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
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("CARGO_CFG_TARGET_ARCH environment variable not set");
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
    
    // Cross-compilation settings for Windows
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();
    
    if target_os == "windows" && target.contains("gnu") {
        // For MinGW, use win32 threading to match Rust's expectations
        // Note: The __gthr_win32_* symbols come from libgcc when using win32 threads
        // We need to ensure these are properly linked
        build.flag("-mthreads");
    }
    
    // Compile
    build.compile("oc_cpp");
    
    // Link required libraries for MinGW Windows builds
    if target_os == "windows" && target.contains("gnu") {
        // Try to find and add the gcc lib path for threading support
        if let Ok(libgcc_path) = std::process::Command::new(
            env::var("CXX_x86_64_pc_windows_gnu").unwrap_or_else(|_| "x86_64-w64-mingw32-g++".to_string())
        )
            .arg("-print-file-name=libgcc_s.a")
            .output()
        {
            let libgcc = String::from_utf8_lossy(&libgcc_path.stdout).trim().to_string();
            if let Some(dir) = std::path::Path::new(&libgcc).parent() {
                if dir.exists() {
                    println!("cargo:rustc-link-search=native={}", dir.display());
                }
            }
        }
        
        // Add MinGW base library path
        let mingw_lib = std::path::Path::new("/usr/x86_64-w64-mingw32/lib");
        if mingw_lib.exists() {
            println!("cargo:rustc-link-search=native={}", mingw_lib.display());
        }
        
        // Link stdc++ statically for C++ runtime to avoid DLL dependencies
        // This ensures the Windows executable is self-contained
        println!("cargo:rustc-link-lib=static=stdc++");
        println!("cargo:rustc-link-lib=static=gcc");
    }
    
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", cpp_src.display());
    println!("cargo:rerun-if-changed={}", cpp_include.display());
}
