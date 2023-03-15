use anyhow::{Context, Result};
use clap::Parser;
use indoc::printdoc;
use sd_crypto::header::file::FileHeader;
use std::path::PathBuf;
use tokio::fs::File;

#[derive(Parser)]
struct Args {
	#[arg(help = "the file path to get details for")]
	path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
	let args = Args::parse();

	let mut reader = File::open(args.path).await.context("unable to open file")?;
	let header = FileHeader::from_reader(&mut reader).await?;
	print_crypto_details(&header, &header.get_aad());

	Ok(())
}

fn print_crypto_details(header: &FileHeader, aad: &[u8]) {
	printdoc! {"
        Header version: {version}
        Encryption algorithm: {algorithm}
		Nonce (hex): {nonce}
        AAD (hex): {hex}
    ",
		version = header.get_version(),
		algorithm = header.get_algorithm(),
		nonce = hex::encode(header.get_nonce()),
		hex = hex::encode(aad)
	};
}
