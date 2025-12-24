//! Oxidized-Cell - PS3 Emulator
//!
//! Main entry point for the emulator application.

use oc_ui::app;

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Oxidized-Cell PS3 Emulator");

    // Run the application
    app::run()
}
