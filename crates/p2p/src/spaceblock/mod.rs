//! Spaceblock is a file transfer protocol that uses a block based system to transfer files.
//! This protocol is modelled after SyncThing's BEP protocol. A huge thanks to it's original authors!
//! You can read more about it here: <https://docs.syncthing.net/specs/bep-v1.html>
#![allow(unused)] // TODO: This module is still in heavy development!

use std::{
	marker::PhantomData,
	path::{Path, PathBuf},
	string::FromUtf8Error,
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

/// TODO
pub struct Transfer<'a, F> {
	req: &'a SpaceblockRequest,
	on_progress: F,
}

impl<'a, F> Transfer<'a, F>
where
	F: Fn(u8) + 'a,
{
	pub fn new(req: &'a SpaceblockRequest, on_progress: F) -> Self {
		Self { req, on_progress }
	}

	pub async fn send(
		&self,
		stream: &mut (impl AsyncWrite + Unpin),
		mut file: (impl AsyncBufRead + Unpin),
	) {
		// We manually implement what is basically a `BufReader` so we have more control
		let mut buf = vec![0u8; self.req.block_size.size() as usize];
		let mut offset: u64 = 0;

		loop {
			let read = file.read(&mut buf[..]).await.unwrap(); // TODO: Error handling
			offset += read as u64;
			(self.on_progress)(((self.req.size / offset) * 100) as u8); // SAFETY: Percent must be between 0 and 100

			if read == 0 {
				if offset != self.req.size {
					panic!("U dun goofed"); // TODO: Error handling
				}

				break;
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
			stream.write_all(&block.to_bytes()).await.unwrap(); // TODO: Error handling
		}
	}

	pub async fn receive(
		&self,
		stream: &mut (impl AsyncReadExt + Unpin),
		mut file: (impl AsyncWrite + Unpin),
	) {
		// We manually implement what is basically a `BufReader` so we have more control
		let mut data_buf = vec![0u8; self.req.block_size.size() as usize];
		let mut offset: u64 = 0;

		// TODO: Prevent loop being a DOS vector
		loop {
			// TODO: Timeout if nothing is being received
			let block = Block::from_stream(stream, &mut data_buf).await.unwrap(); // TODO: Error handling
			offset += block.size;
			(self.on_progress)(((self.req.size / offset) * 100) as u8); // SAFETY: Percent must be between 0 and 100

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
		}
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
		};

		let (tx, rx) = oneshot::channel();
		tokio::spawn({
			let req = req.clone();
			let data = data.clone();
			async move {
				let file = BufReader::new(Cursor::new(data));
				tx.send(()).unwrap();
				Transfer::new(&req, |_| {}).send(&mut client, file).await;
			}
		});

		rx.await.unwrap();

		let mut result = Vec::new();
		Transfer::new(&req, |_| {})
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
		};

		let (tx, rx) = oneshot::channel();
		tokio::spawn({
			let req = req.clone();
			let data = data.clone();
			async move {
				let file = BufReader::new(Cursor::new(data));
				tx.send(()).unwrap();
				Transfer::new(&req, |_| {}).send(&mut client, file).await;
			}
		});

		rx.await.unwrap();

		let mut result = Vec::new();
		Transfer::new(&req, |_| {})
			.receive(&mut server, &mut result)
			.await;
		assert_eq!(result, data);
	}
}
