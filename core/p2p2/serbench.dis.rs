// mkfile -n 1g ./demo.file
// mkfile -n 100m ./demo.file

use std::{
	fmt,
	fs::File,
	io::{BufReader, Cursor, Read, Write},
	time::Instant,
};

use brotli::CompressorWriter;
use rmp::{
	decode::{read_bin_len, read_ext_meta, RmpRead},
	encode::{write_bin, write_ext_meta},
};
use serde::{Deserialize, Serialize};

/// TODO: Replace this with the type coming from @brendonovich's sync layer.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SyncPayload {
	id: String,
}

const DEFAULT_BUF_SIZE: usize = 32 * 1024; // 32 KiB // TODO: Benchmark and decide on good buffer size.
const SPACEDRIVE_MSG_TYPE_MSGPACK_EXT: i8 = 0x73; // 0x73 is 's' in ASCII.

/// TODO
pub enum Message<'a> {
	Ping,
	Pong,
	File(File),
	SyncPayload(SyncPayload),
	Phantom(&'a ()), // TODO: Remove this variant
}

impl<'a> fmt::Debug for Message<'a> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Message::Ping => write!(f, "Ping"),
			Message::Pong => write!(f, "Pong"),
			Message::File(_) => write!(f, "File(...)"),
			Message::SyncPayload(payload) => write!(f, "SyncPayload({:?})", payload),
			Message::Phantom(_) => unreachable!(),
		}
	}
}

impl<'a> Message<'a> {
	// TODO: Make private
	pub(crate) const fn to_byte_identifier(&self) -> u8 {
		match self {
			Message::Ping => 0x01,
			Message::Pong => 0x02,
			Message::File(_) => 0x03,
			Message::SyncPayload(_) => 0x04,
			Message::Phantom(_) => unreachable!(),
		}
	}

	pub fn write<W>(self, wr: &mut W) -> Result<(), ()>
	where
		W: Write,
	{
		write_ext_meta(wr, 1, SPACEDRIVE_MSG_TYPE_MSGPACK_EXT).unwrap();
		wr.write_all(&[self.to_byte_identifier()]).unwrap();

		match self {
			Message::Ping => {}
			Message::Pong => {}
			Message::SyncPayload(payload) => {
				let msg = rmp_serde::encode::to_vec_named(&payload).unwrap();
				write_bin(wr, &msg).unwrap();
			}
			Message::File(file) => {
				// TODO: Send file metadata before file -> including name, etc

				let mut reader = BufReader::with_capacity(DEFAULT_BUF_SIZE, file);
				let mut incoming_buf = [0; DEFAULT_BUF_SIZE];
				let mut outgoing_buf: &mut [u8] = &mut [0; DEFAULT_BUF_SIZE]; // TODO: Remove this in favor of fixed sized chunks so we can tell Msgpak before hand and then write from brotli directly to the network.

				while reader.read(&mut incoming_buf).unwrap() == DEFAULT_BUF_SIZE {
					{
						let mut writer = CompressorWriter::new(
							&mut outgoing_buf,
							DEFAULT_BUF_SIZE,
							3,  // BROTLI_PARAM_QUALITY
							22, // BROTLI_PARAM_LGWIN
						);
						writer.write_all(&mut incoming_buf).unwrap();
					}

					// TODO: Include integrity checksum with each chunk and system to ask for chunk again if it's corrupted.
					write_bin(wr, &mut outgoing_buf).unwrap(); // TODO: Is this the way to allow decoding on the other end.
				}
			}
			_ => unimplemented!(),
		}

		Ok(())
	}

	pub fn read<R>(rd: &mut R) -> Result<Self, ()>
	where
		R: Read,
	{
		let ext_meta = read_ext_meta(rd).unwrap();
		if ext_meta.typeid != SPACEDRIVE_MSG_TYPE_MSGPACK_EXT {
			return Err(());
		} else if ext_meta.size != 1 {
			return Err(());
		}
		let byte_identifier = {
			let mut buf = [0; 1];
			rd.read_exact(&mut buf).unwrap();
			buf[0]
		};

		match byte_identifier {
            0x01 /* Message::Ping */ => Ok(Message::Ping),
            0x02 /* Message::Pong */ => Ok(Message::Pong),
            0x03 /* Message::File */ => {
                unimplemented!(); // TODO: Allow reading files back from message.
            },
            0x04 /* Message::SyncPayload */ => {
                let len = read_bin_len(rd).unwrap();
                let mut buf = Vec::with_capacity(len as usize);
                rd.take(len as u64).read_to_end(&mut buf).unwrap();
                Ok(Message::SyncPayload(rmp_serde::decode::from_slice(&buf).unwrap()))
            }
            _ => Err(()),
        }
	}
}

fn main() {
	let mut buf = Vec::with_capacity(1024 * 1024 * 1024 * 5 /* 5 GB */);

	// Test 1 - Ping
	let msg = Message::Ping;
	msg.write(&mut buf).unwrap();
	let msg = Message::read(&mut &buf[..]).unwrap();
	println!("{:?}", msg);
	buf.clear();

	let msg = Message::Pong;
	msg.write(&mut buf).unwrap();
	println!("{} {:X?}", buf.len(), buf);
	let msg = Message::read(&mut &buf[..]).unwrap();
	println!("{:?}", msg);
	buf.clear();

	// Test 2 - SyncPayload
	let msg = Message::SyncPayload(SyncPayload {
		id: "123".to_string(),
		// This would come from Brendan's sync layer and all it needs is to be a serde compatible type!
	});
	msg.write(&mut buf).unwrap();
	println!("{} {:X?}", buf.len(), buf);

	let msg = Message::read(&mut &buf[..]).unwrap();
	println!("{:?}", msg);

	buf.clear();

	// Test 3 - File
	let now = Instant::now();
	let file = File::open("./demo.file").unwrap();
	let msg = Message::File(file);
	msg.write(&mut buf).unwrap();
	println!("{:?} {}", now.elapsed(), buf.len());

	// TODO: Make this work.
	// let msg = Message::read(&mut &buf[..]).unwrap();
	// println!("{:?}", msg);

	buf.clear();
}

// TODO: All of this needs to be unit tested!!!!!
