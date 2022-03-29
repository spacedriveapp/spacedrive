#![allow(dead_code)]

use anyhow::Result;
use data_encoding::HEXLOWER;
use ring::digest::{Context, SHA256};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::os::unix::prelude::FileExt;
use std::time::Instant;

static BIG_FILE: &str = "/Users/jamie/Movies/2022-03-08 06-08-35.mkv";
static LITTLE_FILE: &str = "/Users/jamie/Movies/client_state.json";

fn main() {
    println!("Generating hash from file {:?}", BIG_FILE);

    let start = Instant::now();
    let checksum = sampled_checksum(BIG_FILE).unwrap();
    println!(
        "Sampled checksum completed in {:?} {}",
        start.elapsed(),
        checksum
    );

    let start = Instant::now();
    let checksum = full_checksum(BIG_FILE).unwrap();
    println!(
        "Full checksum completed in {:?} {}",
        start.elapsed(),
        checksum
    );
}

static SAMPLE_COUNT: u64 = 6;
static SAMPLE_SIZE: u64 = 10000;

pub fn sampled_checksum(path: &str) -> Result<String> {
    // get file size
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    // open file reference
    let file = File::open(path)?;

    let mut context = Context::new(&SHA256);
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
