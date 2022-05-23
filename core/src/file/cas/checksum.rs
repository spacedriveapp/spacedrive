use anyhow::Result;
use data_encoding::HEXLOWER;

use ring::digest::{Context, SHA256};
use std::convert::TryInto;
use std::fs::File;
use std::io::{BufReader, Read};

#[cfg(target_family = "unix")]
use std::os::unix::prelude::FileExt;

#[cfg(target_family = "windows")]
use std::os::windows::prelude::*;

static SAMPLE_COUNT: u64 = 4;
static SAMPLE_SIZE: u64 = 10000;

fn read_at(file: &File, offset: u64, size: u64) -> Result<Vec<u8>> {
	let mut buf = vec![0u8; size as usize];

	#[cfg(target_family = "unix")]
	file.read_exact_at(&mut buf, offset)?;

	#[cfg(target_family = "windows")]
	file.seek_read(&mut buf, offset)?;

	Ok(buf)
}

pub fn generate_cas_id(path: &str, size: u64) -> Result<String> {
	// open file reference
	let file = File::open(path)?;

	let mut context = Context::new(&SHA256);

	// include the file size in the checksum
	context.update(&size.to_le_bytes());

	// if size is small enough, just read the whole thing
	if SAMPLE_COUNT * SAMPLE_SIZE > size {
		let buf = read_at(&file, 0, size.try_into()?)?;
		context.update(&buf);
	} else {
		// loop over samples
		for i in 0..SAMPLE_COUNT {
			let buf = read_at(&file, (size / SAMPLE_COUNT) * i, SAMPLE_SIZE.try_into()?)?;
			context.update(&buf);
		}
		// sample end of file
		let buf = read_at(&file, size - SAMPLE_SIZE, SAMPLE_SIZE.try_into()?)?;
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
