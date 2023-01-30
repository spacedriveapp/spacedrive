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
	let (header, aad) = FileHeader::from_reader(&mut reader).await?;
	print_crypto_details(&header, &aad);

	Ok(())
}

fn print_crypto_details(header: &FileHeader, aad: &[u8]) {
	printdoc! {"
        Header version: {version}
        Encryption algorithm: {algorithm}
        AAD (hex): {hex}
    ",
		version = header.version,
		algorithm = header.algorithm,
		hex = hex::encode(aad)
	};

	header.keyslots.iter().enumerate().for_each(|(i, k)| {
		printdoc! {"
            Keyslot {index}:
              Version: {version}
              Algorithm: {algorithm}
              Hashing algorithm: {hashing_algorithm}
              Salt (hex): {salt}
              Master Key (hex, encrypted): {master}
              Master key nonce (hex): {nonce}
        ",
			index = i + i,
			version = k.version,
			algorithm = k.algorithm,
			hashing_algorithm = k.hashing_algorithm,
			salt = hex::encode(k.salt),
			master = hex::encode(k.master_key),
			nonce = hex::encode(k.nonce.clone())
		};
	});

	header.metadata.iter().for_each(|m| {
		printdoc! {"
            Metadata:
              Version: {version}
              Algorithm: {algorithm}
              Encrypted size: {size}
              Nonce (hex): {nonce}
        ",
			version = m.version,
			algorithm = m.algorithm,
			size = m.metadata.len(),
			nonce = hex::encode(m.metadata_nonce.clone())
		}
	});

	header.preview_media.iter().for_each(|p| {
		printdoc! {"
            Preview Media:
              Version: {version}
              Algorithm: {algorithm}
              Encrypted size: {size}
              Nonce (hex): {nonce}
        ",
			version = p.version,
			algorithm = p.algorithm,
			size = p.media.len(),
			nonce = hex::encode(p.media_nonce.clone())
		};
	});
}
