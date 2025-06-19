//! Spacedrive Core CLI
//! 
//! The main CLI binary for Spacedrive Core operations.
//! 
//! Usage:
//!   spacedrive-cli --help
//!   spacedrive-cli library create "My Library"
//!   spacedrive-cli location add /path/to/folder
//!   spacedrive-cli tui

use sd_core_new::infrastructure::cli;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cli::run().await
}