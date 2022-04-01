use anyhow::Result;
use data_encoding::HEXLOWER;
use ring::digest::{Context, Digest, SHA256};
use std::convert::TryInto;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::os::unix::prelude::FileExt;

static SAMPLE_COUNT: u64 = 4;
static SAMPLE_SIZE: u64 = 10000;

pub fn partial_checksum(path: &str, size: u64) -> Result<String> {
	// open file reference
	let file = File::open(path)?;

	let mut context = Context::new(&SHA256);

	// include the file size in the checksum
	context.update(&size.to_le_bytes());

	// if size is small enough, just read the whole thing
	if SAMPLE_COUNT * SAMPLE_SIZE > size {
		let mut buf = vec![0u8; size.try_into()?];
		file.read_exact_at(&mut buf, 0)?;
		context.update(&buf);
	} else {
		// loop over samples
		for i in 0..SAMPLE_COUNT {
			let start_point = (size / SAMPLE_COUNT) * i;
			let mut buf = vec![0u8; SAMPLE_SIZE.try_into()?];
			file.read_exact_at(&mut buf, start_point)?;
			context.update(&buf);
		}
		// sample end of file
		let mut buf = vec![0u8; SAMPLE_SIZE.try_into()?];
		file.read_exact_at(&mut buf, size - SAMPLE_SIZE)?;
		context.update(&buf);
	}

	let digest = context.finish();
	let hex = HEXLOWER.encode(digest.as_ref());

	Ok(hex)
}

pub fn full_checksum(path: &str) -> Result<String> {
	// read file as buffer and convert to digest
	let mut reader = BufReader::new(File::open(path).unwrap());
	let mut context = Context::new(&SHA256);
	let mut buffer = [0; 1024];
	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		context.update(&buffer[..count]);
	}
	let digest = context.finish();
	// create a lowercase hash from
	let hex = HEXLOWER.encode(digest.as_ref());

	Ok(hex)
}

pub fn sha256_digest<R: Read>(mut reader: R) -> io::Result<Digest> {
	let mut context = Context::new(&SHA256);
	let mut buffer = [0; 1024];
	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		context.update(&buffer[..count]);
	}
	Ok(context.finish())
}
