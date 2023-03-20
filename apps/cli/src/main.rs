use anyhow::{Context, Result};
use clap::Parser;
use indoc::printdoc;
use sd_crypto::{header::FileHeader, types::MagicBytes};
use std::path::PathBuf;
use tokio::fs::File;

#[derive(Parser)]
struct Args {
	#[arg(help = "the file path to get details for")]
	path: PathBuf,
}

/// Should be sourced from the core
pub const ENCRYPTED_FILE_MAGIC_BYTES: MagicBytes<8> =
	MagicBytes::new([0x62, 0x61, 0x6C, 0x6C, 0x61, 0x70, 0x70, 0x03]);

#[tokio::main]
async fn main() -> Result<()> {
	let args = Args::parse();

	let mut reader = File::open(args.path).await.context("unable to open file")?;
	let header = FileHeader::from_reader_async(&mut reader, ENCRYPTED_FILE_MAGIC_BYTES).await?;
	print_crypto_details(&header);

	Ok(())
}

fn print_crypto_details(header: &FileHeader) {
	printdoc! {"
        Header version: {version}
        Encryption algorithm: {algorithm}
		Nonce (hex): {nonce}
        AAD (hex): {hex}
    ",
		version = header.get_version(),
		algorithm = header.get_algorithm(),
		nonce = hex::encode(header.get_nonce().inner()),
		hex = hex::encode(header.get_aad().inner())
	};
}
