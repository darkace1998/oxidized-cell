//! Example demonstrating the integrated emulator runner
//!
//! This example shows how to create and use the EmulatorRunner
//! to coordinate all emulator subsystems.

use oc_core::Config;
use oc_integration::EmulatorRunner;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("oxidized-cell PS3 Emulator - Core Integration Demo");
    tracing::info!("====================================================");

    // Create configuration
    let config = Config::default();
    
    // Create emulator runner
    let mut runner = EmulatorRunner::new(config)?;
    tracing::info!("✓ Created emulator runner");

    // Initialize graphics backend
    runner.init_graphics()?;
    tracing::info!("✓ Initialized graphics backend");

    // Create a PPU thread (main thread)
    let ppu_thread_id = runner.create_ppu_thread(1000)?;
    tracing::info!("✓ Created PPU thread {}", ppu_thread_id);

    // Create SPU threads (6 SPUs available on PS3)
    for i in 0..6 {
        let spu_thread_id = runner.create_spu_thread(2000)?;
        tracing::info!("✓ Created SPU thread {}", spu_thread_id);
    }

    tracing::info!("\nEmulator State:");
    tracing::info!("  PPU threads: {}", runner.ppu_thread_count());
    tracing::info!("  SPU threads: {}", runner.spu_thread_count());
    tracing::info!("  Frame count: {}", runner.frame_count());
    tracing::info!("  Total cycles: {}", runner.total_cycles());

    // Start the emulator
    runner.start()?;
    tracing::info!("\n✓ Emulator started (state: {:?})", runner.state());

    // Run a few frames to demonstrate the frame loop
    tracing::info!("\nRunning 10 frames...");
    for frame in 0..10 {
        runner.run_frame()?;
        if frame % 5 == 4 {
            tracing::info!("  Frame {}: {} cycles executed", 
                runner.frame_count(), 
                runner.total_cycles()
            );
        }
    }

    // Pause the emulator
    runner.pause()?;
    tracing::info!("\n✓ Emulator paused (state: {:?})", runner.state());

    // Resume
    runner.resume()?;
    tracing::info!("✓ Emulator resumed (state: {:?})", runner.state());

    // Run a few more frames
    for _ in 0..5 {
        runner.run_frame()?;
    }

    // Stop the emulator
    runner.stop()?;
    tracing::info!("\n✓ Emulator stopped (state: {:?})", runner.state());

    tracing::info!("\nFinal Statistics:");
    tracing::info!("  Total frames: {}", runner.frame_count());
    tracing::info!("  Total cycles: {}", runner.total_cycles());
    tracing::info!("  PPU threads: {}", runner.ppu_thread_count());
    tracing::info!("  SPU threads: {}", runner.spu_thread_count());

    tracing::info!("\n✓ Demo completed successfully!");
    tracing::info!("\nNote: In a real scenario, you would:");
    tracing::info!("  1. Load game executable into memory");
    tracing::info!("  2. Set thread entry points and initial register values");
    tracing::info!("  3. Start thread execution");
    tracing::info!("  4. Run the frame loop continuously");

    Ok(())
}
