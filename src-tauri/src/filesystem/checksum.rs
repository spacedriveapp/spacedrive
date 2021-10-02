use data_encoding::HEXLOWER;
use ring::digest::{Context, Digest, SHA256};
use sha256::digest;
use std::io;
use std::io::{BufReader, Read};
use std::time::Instant;

fn sha256_digest<R: Read>(mut reader: R) -> io::Result<Digest> {
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

pub async fn create_buffer_checksum(path: &str) -> io::Result<String> {
  let start = Instant::now();
  // read file as buffer and convert to digest
  let digest = sha256_digest(BufReader::new(std::fs::File::open(path)?))?;
  // create a lowercase hash from
  let hex = HEXLOWER.encode(digest.as_ref());
  println!("hashing complete in {:?} {}", start.elapsed(), hex);
  Ok(hex)
}

pub fn create_meta_checksum(uri: String, size_in_bytes: u64) -> io::Result<String> {
  Ok(digest(format!("{}{}", uri, size_in_bytes.to_string())))
}
