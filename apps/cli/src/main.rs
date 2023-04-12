use anyhow::{Context, Result};
use clap::Parser;
use indoc::printdoc;
use sd_crypto::{
	encoding::Header,
	types::{Aad, MagicBytes},
};
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
	let (header, aad) = Header::from_reader_async(&mut reader, ENCRYPTED_FILE_MAGIC_BYTES).await?;
	print_crypto_details(&header, &aad);

	Ok(())
}

fn print_crypto_details(header: &Header, aad: &Aad) {
	printdoc! {"
        Header version: {version}
        Encryption algorithm: {algorithm}
		Nonce (hex): {nonce}
        Expected AAD (hex): {exp_aad}
		Found AAD (hex): {read_aad}
    ",
		version = header.version,
		algorithm = header.algorithm,
		nonce = hex::encode(header.nonce.inner()),
		exp_aad = hex::encode(header.generate_aad().inner()),
		read_aad = hex::encode(aad.inner())
	};
}
