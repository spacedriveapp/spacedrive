//! Spacedrive Core CLI Demo
//! 
//! Run with: cargo run --example cli_demo -- --help

use sd_core_new::infrastructure::cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}