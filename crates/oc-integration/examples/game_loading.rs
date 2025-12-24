//! Example demonstrating the complete game loading pipeline
//!
//! This example shows how to:
//! 1. Initialize the emulator
//! 2. Load a PS3 executable (ELF/SELF)
//! 3. Load PRX modules (shared libraries)
//! 4. Set up thread state with TLS
//! 5. Run the game

use oc_core::Config;
use oc_integration::{EmulatorRunner, LoadedGame};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("=== PS3 Game Loading Pipeline Example ===\n");

    // Step 1: Create emulator with default configuration
    println!("Step 1: Initializing emulator...");
    let config = Config::default();
    let mut runner = EmulatorRunner::new(config)?;
    println!("  ✓ Emulator initialized\n");

    // Step 2: Load a game
    println!("Step 2: Loading game...");
    println!("  Note: This example shows the API usage.");
    println!("  To actually load a game, provide a valid PS3 ELF/SELF file path.\n");

    // Example of how to load a game:
    // let game_path = PathBuf::from("/path/to/game.elf");
    // let game = runner.load_game(&game_path)?;
    
    // For this example, we'll create a mock LoadedGame structure
    let game = create_mock_loaded_game();
    
    println!("  Game Information:");
    println!("    Entry Point: 0x{:x}", game.entry_point);
    println!("    Base Address: 0x{:08x}", game.base_addr);
    println!("    Stack Address: 0x{:08x}", game.stack_addr);
    println!("    Stack Size: 0x{:x} ({} KB)", game.stack_size, game.stack_size / 1024);
    println!("    TOC Pointer: 0x{:x}", game.toc);
    println!("    TLS Address: 0x{:08x}", game.tls_addr);
    println!("    TLS Size: 0x{:x} ({} KB)", game.tls_size, game.tls_size / 1024);
    println!("    PRX Modules: {}", game.prx_modules.len());
    for (i, module) in game.prx_modules.iter().enumerate() {
        println!("      {}. {}", i + 1, module);
    }
    println!();

    // Step 3: Demonstrate PRX loading
    println!("Step 3: PRX Module Loading");
    println!("  PRX modules provide shared library functionality.");
    println!("  Example modules that might be loaded:");
    println!("    - libgcm_sys.prx (Graphics system)");
    println!("    - libsysutil.prx (System utilities)");
    println!("    - libspurs.prx (SPURS task scheduler)");
    println!("    - libfs.prx (File system)");
    println!();
    
    // Example of how to load PRX modules:
    // let prx_paths = vec![
    //     PathBuf::from("/path/to/libgcm_sys.prx"),
    //     PathBuf::from("/path/to/libsysutil.prx"),
    // ];
    // let mut game_loader = GameLoader::new(runner.memory().clone());
    // game_loader.load_prx_modules(&mut game, &prx_paths)?;

    // Step 4: Thread initialization with TLS
    println!("Step 4: Thread State Initialization");
    println!("  When loading a game, the main PPU thread is created with:");
    println!("    R1  (Stack Pointer) = 0x{:08x}", game.stack_addr);
    println!("    R2  (TOC)           = 0x{:x}", game.toc);
    println!("    R3  (argc)          = 0 (no arguments)");
    println!("    R13 (TLS)           = 0x{:08x}", game.tls_addr);
    println!("    PC  (Entry Point)   = 0x{:x}", game.entry_point);
    println!();

    // Step 5: Execution
    println!("Step 5: Starting Execution");
    println!("  Call runner.start() to begin execution");
    println!("  Call runner.run_frame() in your main loop");
    println!("  The emulator will:");
    println!("    - Schedule PPU/SPU threads");
    println!("    - Execute instructions");
    println!("    - Handle syscalls");
    println!("    - Process graphics commands");
    println!("    - Maintain 60 FPS target");
    println!();

    // Example execution loop (not actually running):
    // runner.start()?;
    // loop {
    //     runner.run_frame()?;
    //     
    //     if runner.is_stopped() {
    //         break;
    //     }
    // }

    println!("=== Example Complete ===");
    println!("\nPhase 14: Game Loading Features");
    println!("  ✓ ELF/SELF loading");
    println!("  ✓ PRX module support");
    println!("  ✓ Thread-Local Storage (TLS)");
    println!("  ✓ Symbol resolution");
    println!("  ✓ Dynamic relocations");
    println!("  ✓ Complete thread initialization");

    Ok(())
}

/// Create a mock LoadedGame for demonstration
fn create_mock_loaded_game() -> LoadedGame {
    LoadedGame {
        entry_point: 0x10000,
        base_addr: 0x10000000,
        stack_addr: 0xD0100000,
        stack_size: 0x100000,  // 1 MB
        toc: 0x10008000,
        tls_addr: 0xE0000000,
        tls_size: 0x10000,     // 64 KB
        path: "/dev_hdd0/game/EXAMPLE001/USRDIR/EBOOT.BIN".to_string(),
        is_self: false,
        prx_modules: vec![
            "libgcm_sys.prx".to_string(),
            "libsysutil.prx".to_string(),
            "libspurs.prx".to_string(),
        ],
    }
}
