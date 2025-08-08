use anyhow::Result;
use clap::Parser;
use sd_bench::cli::commands::{self as cli, Cli};

#[tokio::main]
async fn main() -> Result<()> {
	init_tracing();
	let cli = Cli::parse();
	cli::run(cli).await
}

fn init_tracing() {
	let _ = tracing_subscriber::fmt()
		.with_max_level(tracing::Level::INFO)
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.try_init();
}
