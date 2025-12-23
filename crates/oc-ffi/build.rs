use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap();
    
    // Get the workspace root (two levels up from oc-ffi)
    let workspace_root = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    
    let cpp_dir = workspace_root.join("cpp");
    let build_dir = PathBuf::from(&out_dir).join("cpp_build");
    
    // Create build directory
    std::fs::create_dir_all(&build_dir).unwrap();
    
    // Determine build type for CMake
    let cmake_build_type = if profile == "release" {
        "Release"
    } else {
        "Debug"
    };
    
    // Run CMake configuration
    let mut cmake_config = Command::new("cmake");
    cmake_config
        .current_dir(&build_dir)
        .arg(&cpp_dir)
        .arg(format!("-DCMAKE_BUILD_TYPE={}", cmake_build_type))
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", out_dir));
    
    // Add LLVM path if available
    if let Ok(llvm_dir) = env::var("LLVM_DIR") {
        cmake_config.arg(format!("-DLLVM_DIR={}", llvm_dir));
    }
    
    let status = cmake_config.status();
    
    match status {
        Ok(status) if status.success() => {
            // Build the C++ library
            let build_status = Command::new("cmake")
                .current_dir(&build_dir)
                .arg("--build")
                .arg(".")
                .arg("--config")
                .arg(cmake_build_type)
                .arg("--target")
                .arg("oc_cpp")
                .status()
                .expect("Failed to build C++ library");
            
            if !build_status.success() {
                panic!("C++ library build failed");
            }
            
            // Install to OUT_DIR
            let install_status = Command::new("cmake")
                .current_dir(&build_dir)
                .arg("--install")
                .arg(".")
                .status()
                .expect("Failed to install C++ library");
            
            if !install_status.success() {
                panic!("C++ library installation failed");
            }
            
            // Link the C++ library
            println!("cargo:rustc-link-search=native={}/lib", out_dir);
            println!("cargo:rustc-link-lib=static=oc_cpp");
            
            // Link standard C++ library
            #[cfg(target_os = "linux")]
            println!("cargo:rustc-link-lib=dylib=stdc++");
            
            #[cfg(target_os = "macos")]
            println!("cargo:rustc-link-lib=dylib=c++");
            
            #[cfg(target_os = "windows")]
            println!("cargo:rustc-link-lib=dylib=c++");
        }
        Ok(_) => {
            eprintln!("Warning: CMake configuration failed. C++ components will not be built.");
            eprintln!("This is expected if CMake or C++ toolchain is not available.");
        }
        Err(e) => {
            eprintln!("Warning: Failed to run CMake: {}. C++ components will not be built.", e);
            eprintln!("This is expected if CMake is not installed.");
        }
    }
    
    // Re-run if build script or C++ sources change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../cpp/");
}
