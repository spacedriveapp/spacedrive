use data_encoding::HEXUPPER;
use ring::digest::{Context, Digest, SHA256};
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};

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

pub fn create_hash(path: &str) -> io::Result<String> {
  let input = File::open(path)?;
  let reader = BufReader::new(input);
  let digest = sha256_digest(reader)?;
  Ok(HEXUPPER.encode(digest.as_ref()))
}
