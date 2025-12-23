# Build script for oxidized-cell (Windows PowerShell)
# This script automates the build process for different configurations

param(
    [switch]$Release,
    [switch]$WithLLVM,
    [switch]$Clean,
    [switch]$Test,
    [string]$Target = "",
    [switch]$Help
)

# Colors for output
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Green
}

function Write-Warning-Custom {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

function Write-Error-Custom {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

# Show usage
function Show-Usage {
    Write-Host @"
Usage: .\build.ps1 [OPTIONS]

Build oxidized-cell emulator

OPTIONS:
    -Release        Build in release mode (default: debug)
    -WithLLVM       Build with LLVM JIT support
    -Clean          Clean before building
    -Test           Run tests after building
    -Target TARGET  Cross-compile for target triple
    -Help           Show this help message

EXAMPLES:
    .\build.ps1                                 # Debug build without LLVM
    .\build.ps1 -Release -WithLLVM              # Release build with LLVM
    .\build.ps1 -Clean -Test                    # Clean build and run tests
    .\build.ps1 -Target x86_64-pc-windows-gnu -Release  # Cross-compile with MinGW

"@
}

# Show help if requested
if ($Help) {
    Show-Usage
    exit 0
}

# Print build configuration
Write-Info "Build configuration:"
Write-Host "  Build type: $(if ($Release) { 'release' } else { 'debug' })"
Write-Host "  With LLVM: $WithLLVM"
Write-Host "  Clean build: $Clean"
Write-Host "  Run tests: $Test"
if ($Target) {
    Write-Host "  Target: $Target"
}
Write-Host ""

# Check for required tools
Write-Info "Checking for required tools..."

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error-Custom "cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
}

if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    Write-Warning-Custom "CMake not found. C++ components will not be built."
    Write-Warning-Custom "Install CMake from https://cmake.org/download/"
}

# Check for LLVM if requested
if ($WithLLVM) {
    if (-not $env:LLVM_DIR) {
        Write-Warning-Custom "LLVM_DIR not set. Attempting to auto-detect..."
        
        # Common LLVM installation paths on Windows
        $llvmPaths = @(
            "C:\Program Files\LLVM\lib\cmake\llvm",
            "C:\Program Files (x86)\LLVM\lib\cmake\llvm",
            "$env:ProgramFiles\LLVM\lib\cmake\llvm",
            "${env:ProgramFiles(x86)}\LLVM\lib\cmake\llvm"
        )
        
        foreach ($path in $llvmPaths) {
            if (Test-Path $path) {
                $env:LLVM_DIR = $path
                Write-Info "Found LLVM at $path"
                break
            }
        }
        
        if (-not $env:LLVM_DIR) {
            Write-Error-Custom "Could not find LLVM. Please install LLVM 17+ or set LLVM_DIR."
            exit 1
        }
    } else {
        Write-Info "Using LLVM from: $env:LLVM_DIR"
    }
}

# Clean if requested
if ($Clean) {
    Write-Info "Cleaning build artifacts..."
    cargo clean
    if (Test-Path "cpp\build") {
        Remove-Item -Recurse -Force "cpp\build"
    }
}

# Build cargo arguments
$cargoArgs = @("--workspace")

if ($Release) {
    $cargoArgs += "--release"
    $buildType = "release"
} else {
    $buildType = "debug"
}

if ($Target) {
    $cargoArgs += "--target", $Target
    
    # Add target if not already installed
    Write-Info "Adding Rust target: $Target"
    rustup target add $Target 2>$null
}

# Build
Write-Info "Building oxidized-cell..."
$buildCmd = "cargo build $($cargoArgs -join ' ')"
Write-Host "Running: $buildCmd"

cargo build @cargoArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error-Custom "Build failed!"
    exit 1
}

Write-Info "Build completed successfully!"

# Run tests if requested
if ($Test) {
    Write-Info "Running tests..."
    cargo test @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error-Custom "Some tests failed!"
        exit 1
    }
    Write-Info "All tests passed!"
}

# Print build output location
if ($Target) {
    $buildDir = "target\$Target\$buildType"
} else {
    $buildDir = "target\$buildType"
}

Write-Info "Build artifacts are in: $buildDir"

# List executables
if (Test-Path $buildDir) {
    $executables = Get-ChildItem $buildDir -Filter "*.exe" | Select-Object -First 5
    if ($executables) {
        Write-Info "Executables:"
        foreach ($exe in $executables) {
            Write-Host "  - $($exe.Name)"
        }
    }
}

Write-Info "Done!"
