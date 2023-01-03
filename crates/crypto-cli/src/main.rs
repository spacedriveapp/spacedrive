use anyhow::{Context, Result};
use clap::Parser;
use sd_crypto::header::file::FileHeader;
use std::{fs::File, path::PathBuf};

#[derive(Parser)]
struct Args {
	#[arg(help = "the file path to get details for")]
	path: PathBuf,
}

fn main() -> Result<()> {
	let args = Args::parse();

	let mut reader = File::open(args.path).context("unable to open file")?;
	let (header, aad) = FileHeader::deserialize(&mut reader)?;
	print_details(&header, &aad)?;

	Ok(())
}

fn print_details(header: &FileHeader, aad: &[u8]) -> Result<()> {
	println!("Header version: {}", header.version);
	println!("Encryption algorithm: {}", header.algorithm);
	println!("AAD (hex): {}", hex::encode(aad));
	header.keyslots.iter().enumerate().for_each(|(i, k)| {
		println!("Keyslot {}:", i + 1);
		println!("	Version: {}", k.version);
		println!("	Algorithm: {}", k.algorithm);
		println!("	Hashing algorithm: {}", k.hashing_algorithm);
		println!("	Salt (hex): {}", hex::encode(k.salt));
		println!("	Master Key (hex, encrypted): {}", hex::encode(k.master_key));
		println!("	Master key nonce (hex): {}", hex::encode(k.nonce.clone()));
	});

	header.metadata.clone().map(|m| {
		println!("Metadata:");
		println!("	Version: {}", m.version);
		println!("	Algorithm: {}", m.algorithm);
		println!("	Encrypted size: {}", m.metadata.len());
		println!("	Nonce (hex): {}", hex::encode(m.metadata_nonce));
	});

	header.preview_media.clone().map(|p| {
		println!("Preview Media:");
		println!("	Version: {}", p.version);
		println!("	Algorithm: {}", p.algorithm);
		println!("	Encrypted size: {}", p.media.len());
		println!("	Nonce (hex): {}", hex::encode(p.media_nonce))
	});

	Ok(())
}
