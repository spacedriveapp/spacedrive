//! Spaceblock is a file transfer protocol that uses a block based system to transfer files.
//! This protocol is modelled after SyncThing's BEP protocol. A huge thanks to it's original authors!
//! You can read more about it here: <https://docs.syncthing.net/specs/bep-v1.html>
#![allow(unused)] // TODO: This module is still in heavy development!

use std::{
	marker::PhantomData,
	path::{Path, PathBuf},
	string::FromUtf8Error,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use thiserror::Error;
use tokio::{
	fs::File,
	io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
};
use tracing::debug;

use crate::{
	proto::{decode, encode},
	spacetime::UnicastStream,
};

mod block;
mod block_size;
mod sb_request;

pub use block::*;
pub use block_size::*;
pub use sb_request::*;

#[derive(Debug, PartialEq, Eq)]
pub enum Msg<'a> {
	Block(Block<'a>),
	Cancelled,
}

impl<'a> Msg<'a> {
	pub async fn from_stream<'b>(
		stream: &mut (impl AsyncReadExt + Unpin),
		data_buf: &'b mut [u8],
	) -> Result<Msg<'a>, ()> {
		let discriminator = stream.read_u8().await.unwrap(); // TODO: Error handling
		match discriminator {
			0 => {
				Ok(Msg::Block(
					Block::from_stream(stream, data_buf).await.unwrap(),
				)) // TODO: Error handling
			}
			1 => Ok(Msg::Cancelled),
			_ => panic!("Invalid message type"),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Msg::Block(block) => {
				let mut bytes = Vec::new();
				bytes.push(0);
				bytes.extend(block.to_bytes());
				bytes
			}
			Msg::Cancelled => vec![1],
		}
	}
}

/// TODO
pub struct Transfer<'a, F> {
	req: &'a SpaceblockRequest,
	on_progress: F,
	cancelled: &'a AtomicBool,
}

impl<'a, F> Transfer<'a, F>
where
	F: Fn(u8) + 'a,
{
	// TODO: Handle `req.range` correctly in this code

	pub fn new(req: &'a SpaceblockRequest, on_progress: F, cancelled: &'a AtomicBool) -> Self {
		Self {
			req,
			on_progress,
			cancelled,
		}
	}

	pub async fn send(
		&self,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		mut file: (impl AsyncBufRead + Unpin),
	) {
		// We manually implement what is basically a `BufReader` so we have more control
		let mut buf = vec![0u8; self.req.block_size.size() as usize];
		let mut offset: u64 = 0;

		loop {
			if self.cancelled.load(Ordering::Relaxed) {
				stream.write_all(&Msg::Cancelled.to_bytes()).await.unwrap(); // TODO: Error handling
				stream.flush().await.unwrap(); // TODO: Error handling
				return;
			}

			let read = file.read(&mut buf[..]).await.unwrap(); // TODO: Error handling
			offset += read as u64;
			(self.on_progress)(((offset as f64 / self.req.size as f64) * 100.0) as u8); // SAFETY: Percent must be between 0 and 100

			if read == 0 {
				if offset != self.req.size {
					panic!("U dun goofed"); // TODO: Error handling
				}

				return;
			}

			let block = Block {
				offset,
				size: read as u64,
				data: &buf[..read],
			};
			debug!(
				"Sending block at offset {} of size {}",
				block.offset, block.size
			);
			stream
				.write_all(&Msg::Block(block).to_bytes())
				.await
				.unwrap(); // TODO: Error handling
			stream.flush().await.unwrap(); // TODO: Error handling

			match stream.read_u8().await.unwrap() {
				// Continue sending
				0 => {}
				// Cancelled by user
				1 => {
					debug!("Receiver cancelled Spacedrop transfer!");
					return;
				}
				// Transfer complete
				2 => {
					return;
				}
				_ => todo!(),
			}
		}
	}

	pub async fn receive(
		&self,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		mut file: (impl AsyncWrite + Unpin),
	) {
		// We manually implement what is basically a `BufReader` so we have more control
		let mut data_buf = vec![0u8; self.req.block_size.size() as usize];
		let mut offset: u64 = 0;

		// TODO: Prevent loop being a DOS vector
		loop {
			// TODO: Timeout if nothing is being received
			let msg = Msg::from_stream(stream, &mut data_buf).await.unwrap(); // TODO: Error handling
			match msg {
				Msg::Block(block) => {
					offset += block.size;
					(self.on_progress)(((offset as f64 / self.req.size as f64) * 100.0) as u8); // SAFETY: Percent must be between 0 and 100

					debug!(
						"Received block at offset {} of size {}",
						block.offset, block.size
					);
					file.write_all(&data_buf[..block.size as usize])
						.await
						.unwrap(); // TODO: Error handling

					// TODO: Should this be `read == 0`
					if offset == self.req.size {
						break;
					}

					stream
						.write_u8(self.cancelled.load(Ordering::Relaxed) as u8)
						.await
						.unwrap();
					stream.flush().await.unwrap(); // TODO: Error handling
				}
				Msg::Cancelled => {
					debug!("Sender cancelled Spacedrop transfer!");
					return;
				}
			}
		}

		stream.write_u8(2).await.unwrap();
		stream.flush().await.unwrap(); // TODO: Error handling
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use tokio::sync::oneshot;

	use super::*;

	#[tokio::test]
	async fn test_spaceblock_request() {
		let req = SpaceblockRequest {
			name: "Demo".to_string(),
			size: 42069,
			block_size: BlockSize::from_size(42069),
			range: Range::Full,
		};

		let bytes = req.to_bytes();
		let req2 = SpaceblockRequest::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);

		let req = SpaceblockRequest {
			name: "Demo".to_string(),
			size: 42069,
			block_size: BlockSize::from_size(42069),
			range: Range::Partial(0..420),
		};

		let bytes = req.to_bytes();
		let req2 = SpaceblockRequest::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);
	}

	#[tokio::test]
	async fn test_spaceblock_single_block() {
		let (mut client, mut server) = tokio::io::duplex(64);

		// This is sent out of band of Spaceblock
		let data = b"Spacedrive".to_vec();
		let req = SpaceblockRequest {
			name: "Demo".to_string(),
			size: data.len() as u64,
			block_size: BlockSize::from_size(data.len() as u64),
			range: Range::Full,
		};

		let (tx, rx) = oneshot::channel();
		tokio::spawn({
			let req = req.clone();
			let data = data.clone();
			async move {
				let file = BufReader::new(Cursor::new(data));
				tx.send(()).unwrap();
				Transfer::new(&req, |_| {}, &Default::default())
					.send(&mut client, file)
					.await;
			}
		});

		rx.await.unwrap();

		let mut result = Vec::new();
		Transfer::new(&req, |_| {}, &Default::default())
			.receive(&mut server, &mut result)
			.await;
		assert_eq!(result, data);
	}

	// https://github.com/spacedriveapp/spacedrive/pull/942
	#[tokio::test]
	async fn test_spaceblock_multiple_blocks() {
		let (mut client, mut server) = tokio::io::duplex(64);

		// This is sent out of band of Spaceblock
		let block_size = 131072u32;
		let data = vec![0u8; block_size as usize * 4]; // Let's pacman some RAM
		let block_size = BlockSize::dangerously_new(block_size);

		let req = SpaceblockRequest {
			name: "Demo".to_string(),
			size: data.len() as u64,
			block_size,
			range: Range::Full,
		};

		let (tx, rx) = oneshot::channel();
		tokio::spawn({
			let req = req.clone();
			let data = data.clone();
			async move {
				let file = BufReader::new(Cursor::new(data));
				tx.send(()).unwrap();
				Transfer::new(&req, |_| {}, &Default::default())
					.send(&mut client, file)
					.await;
			}
		});

		rx.await.unwrap();

		let mut result = Vec::new();
		Transfer::new(&req, |_| {}, &Default::default())
			.receive(&mut server, &mut result)
			.await;
		assert_eq!(result, data);
	}

	// TODO: Unit test the condition when the receiver sets the `cancelled` flag

	// TODO: Unit test the condition when the sender sets the `cancelled` flag

	#[tokio::test]
	async fn test_msg() {
		let block = Block {
			offset: 0,
			size: 10,
			data: b"Spacedrive".as_ref(),
		};
		let msg = Msg::Block(block);
		let bytes = msg.to_bytes();
		let msg2 = Msg::from_stream(&mut Cursor::new(bytes), &mut [0u8; 64])
			.await
			.unwrap();
		assert_eq!(msg, msg2);

		let msg = Msg::Cancelled;
		let bytes = msg.to_bytes();
		let msg2 = Msg::from_stream(&mut Cursor::new(bytes), &mut [0u8; 64])
			.await
			.unwrap();
		assert_eq!(msg, msg2);
	}
}
