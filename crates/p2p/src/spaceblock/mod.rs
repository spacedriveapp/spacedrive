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
	reqs: &'a SpaceblockRequests,
	on_progress: F,
	total_offset: u64,
	total_bytes: u64,
	// TODO: Remove `i` plz
	i: usize,
	cancelled: &'a AtomicBool,
}

impl<'a, F> Transfer<'a, F>
where
	F: Fn(u8) + 'a,
{
	// TODO: Handle `req.range` correctly in this code

	pub fn new(req: &'a SpaceblockRequests, on_progress: F, cancelled: &'a AtomicBool) -> Self {
		Self {
			reqs: req,
			on_progress,
			total_offset: 0,
			total_bytes: req.requests.iter().map(|req| req.size).sum(),
			i: 0,
			cancelled,
		}
	}

	// TODO: Should `new` take in the streams too cause this means we `Stream` `SpaceblockRequest` could get outta sync.
	pub async fn send(
		&mut self,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		mut file: (impl AsyncBufRead + Unpin),
	) {
		// We manually implement what is basically a `BufReader` so we have more control
		let mut buf = vec![0u8; self.reqs.block_size.size() as usize];
		let mut offset: u64 = 0;

		loop {
			if self.cancelled.load(Ordering::Relaxed) {
				stream.write_all(&Msg::Cancelled.to_bytes()).await.unwrap(); // TODO: Error handling
				stream.flush().await.unwrap(); // TODO: Error handling
				return;
			}

			let read = file.read(&mut buf[..]).await.unwrap(); // TODO: Error handling
			self.total_offset += read as u64;
			(self.on_progress)(
				((self.total_offset as f64 / self.total_bytes as f64) * 100.0) as u8,
			); // SAFETY: Percent must be between 0 and 100

			if read == 0 {
				if (offset + read as u64) != self.reqs.requests[self.i].size {
					// The file may have been modified during sender on the sender and we don't account for that.
					panic!("File sending has stopped but it doesn't match the expected length!"); // TODO: Error handling + send error to remote
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
			offset += read as u64;

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
		&mut self,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		mut file: (impl AsyncWrite + Unpin),
	) {
		// We manually implement what is basically a `BufReader` so we have more control
		let mut data_buf = vec![0u8; self.reqs.block_size.size() as usize];
		let mut offset: u64 = 0;

		if self.reqs.requests[self.i].size == 0 {
			self.i += 1;
			return;
		}

		// TODO: Prevent loop being a DOS vector
		loop {
			if self.cancelled.load(Ordering::Relaxed) {
				stream.write_u8(1).await.unwrap(); // TODO: Error handling
				stream.flush().await.unwrap(); // TODO: Error handling
				return;
			}

			// TODO: Timeout if nothing is being received
			let msg = Msg::from_stream(stream, &mut data_buf).await.unwrap(); // TODO: Error handling
			match msg {
				Msg::Block(block) => {
					self.total_offset += block.size;
					(self.on_progress)(
						((self.total_offset as f64 / self.total_bytes as f64) * 100.0) as u8,
					); // SAFETY: Percent must be between 0 and 100

					debug!(
						"Received block at offset {} of size {}",
						block.offset, block.size
					);
					offset += block.size;

					file.write_all(&data_buf[..block.size as usize])
						.await
						.unwrap(); // TODO: Error handling

					// TODO: Should this be `read == 0`
					// TODO: Out of range protection on indexed access
					if offset == self.reqs.requests[self.i].size {
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

		stream.write_u8(2).await.unwrap(); // TODO: Error handling
		stream.flush().await.unwrap(); // TODO: Error handling
		file.flush().await.unwrap(); // TODO: Error handling
		self.i += 1;
	}
}

#[cfg(test)]
mod tests {
	use std::{io::Cursor, mem};

	use tokio::sync::oneshot;
	use uuid::Uuid;

	use super::*;

	#[tokio::test]
	async fn test_spaceblock_single_block() {
		let (mut client, mut server) = tokio::io::duplex(64);

		// This is sent out of band of Spaceblock
		let data = b"Spacedrive".to_vec();
		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size: BlockSize::from_size(data.len() as u64),
			requests: vec![SpaceblockRequest {
				name: "Demo".to_string(),
				size: data.len() as u64,
				range: Range::Full,
			}],
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

		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size,
			requests: vec![SpaceblockRequest {
				name: "Demo".to_string(),
				size: data.len() as u64,
				range: Range::Full,
			}],
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

	#[tokio::test]
	async fn test_transfer_receiver_cancelled() {
		let (mut client, mut server) = tokio::io::duplex(64);

		// This is sent out of band of Spaceblock
		let block_size = 25u32;
		let data = vec![0u8; block_size as usize];
		let block_size = BlockSize::dangerously_new(block_size); // TODO: Determine it using proper algo instead of harcoding it

		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size,
			requests: vec![SpaceblockRequest {
				name: "Demo".to_string(),
				size: data.len() as u64,
				range: Range::Full,
			}],
		};

		let (tx, rx) = oneshot::channel();
		tokio::spawn({
			let req = req.clone();
			let data = data.clone();
			async move {
				let file = BufReader::new(Cursor::new(data));
				tx.send(()).unwrap();

				Transfer::new(&req, |_| {}, &Arc::new(AtomicBool::new(true)))
					.send(&mut client, file)
					.await;
			}
		});

		rx.await.unwrap();

		let mut result = Vec::new();
		Transfer::new(&req, |_| {}, &Default::default())
			.receive(&mut server, &mut result)
			.await;
		assert_eq!(result, Vec::<u8>::new()); // Cancelled by sender so no data
	}

	#[tokio::test]
	async fn test_transfer_sender_cancelled() {
		let (mut client, mut server) = tokio::io::duplex(64);

		// This is sent out of band of Spaceblock
		let block_size = 25u32;
		let data = vec![0u8; block_size as usize];
		let block_size = BlockSize::dangerously_new(block_size); // TODO: Determine it using proper algo instead of harcoding it

		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size,
			requests: vec![SpaceblockRequest {
				name: "Demo".to_string(),
				size: data.len() as u64,
				range: Range::Full,
			}],
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
		Transfer::new(&req, |_| {}, &Arc::new(AtomicBool::new(true)))
			.receive(&mut server, &mut result)
			.await;
		assert_eq!(result, Vec::<u8>::new()); // Cancelled by sender so no data
	}

	// https://linear.app/spacedriveapp/issue/ENG-1300/spaceblock-doesnt-like-zero-sized-files
	#[tokio::test]
	async fn test_spaceblock_zero_sized_file() {
		let (mut client, mut server) = tokio::io::duplex(64);

		// This is sent out of band of Spaceblock
		let block_size = 25u32;
		let data = vec![0u8; 0]; // Zero sized file
		let block_size = BlockSize::dangerously_new(block_size); // TODO: Determine it using proper algo instead of harcoding it

		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size,
			requests: vec![SpaceblockRequest {
				name: "Demo".to_string(),
				size: data.len() as u64,
				range: Range::Full,
			}],
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
		assert_eq!(result, Vec::<u8>::new()); // Cancelled by sender so no data
	}

	#[tokio::test]
	async fn test_msg() {
		let block = Block {
			offset: 0,
			size: 10,
			data: b"Spacedrive".as_ref(),
		};
		let data_len = block.data.len();
		let mut msg = Msg::Block(block);
		let bytes = msg.to_bytes();
		let mut data2 = vec![0; data_len];
		let msg2 = Msg::from_stream(&mut Cursor::new(bytes), &mut data2)
			.await
			.unwrap();
		let data = mem::take(match &mut msg {
			Msg::Block(block) => &mut block.data,
			_ => unreachable!(),
		}); // We decode the data into
		assert_eq!(msg, msg2);
		assert_eq!(data, data2);

		let msg = Msg::Cancelled;
		let bytes = msg.to_bytes();
		let msg2 = Msg::from_stream(&mut Cursor::new(bytes), &mut [0u8; 64])
			.await
			.unwrap();
		assert_eq!(msg, msg2);
	}
}
