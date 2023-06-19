//! Spaceblock is a file transfer protocol that uses a block based system to transfer files.
//! This protocol is modelled after SyncThing's BEP protocol. A huge thanks to it's original authors!
//! You can read more about it here: https://docs.syncthing.net/specs/bep-v1.html
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

use crate::spacetime::{SpaceTimeStream, UnicastStream};

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSize(u32); // Max block size is gonna be 3.9GB which is stupidly overkill

impl BlockSize {
	// TODO: Validating `BlockSize` are multiple of 2, i think. Idk why but BEP does it.

	pub fn from_size(size: u64) -> Self {
		// TODO: Something like: https://docs.syncthing.net/specs/bep-v1.html#selection-of-block-size
		Self(131072) // 128 KiB
	}

	/// This is super dangerous as it doesn't enforce any assumptions of the protocol and is designed just for tests.
	#[cfg(test)]
	pub fn dangerously_new(size: u32) -> Self {
		Self(size)
	}

	pub fn size(&self) -> u32 {
		self.0
	}
}

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceblockRequest {
	pub name: String,
	pub size: u64,
	// TODO: Include file permissions
	pub block_size: BlockSize,
}

#[derive(Debug, Error)]
pub enum SpacedropRequestError {
	#[error("io error reading name len: {0}")]
	NameLenIoError(std::io::Error),
	#[error("io error reading name: {0}")]
	NameIoError(std::io::Error),
	#[error("error utf-8 decoding name: {0}")]
	NameFormatError(FromUtf8Error),
	#[error("io error reading file size: {0}")]
	SizeIoError(std::io::Error),
}

impl SpaceblockRequest {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, SpacedropRequestError> {
		let name = {
			let len = stream
				.read_u16_le()
				.await
				.map_err(SpacedropRequestError::NameLenIoError)?;

			let mut buf = vec![0u8; len as usize];
			stream
				.read_exact(&mut buf)
				.await
				.map_err(SpacedropRequestError::NameIoError)?;

			String::from_utf8(buf).map_err(SpacedropRequestError::NameFormatError)?
		};

		let size = stream
			.read_u64_le()
			.await
			.map_err(SpacedropRequestError::SizeIoError)?;
		let block_size = BlockSize::from_size(size); // TODO: Get from stream: stream.read_u8().await.map_err(|_| ())?; // TODO: Error handling

		Ok(Self {
			name,
			size,
			block_size,
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();

		let len_buf = (self.name.len() as u16).to_le_bytes();
		if self.name.len() > u16::MAX as usize {
			panic!("Name is too long!"); // TODO: Error handling
		}
		buf.extend_from_slice(&len_buf);
		buf.extend(self.name.as_bytes());

		buf.extend_from_slice(&self.size.to_le_bytes());

		buf
	}
}

/// TODO
pub struct Block<'a> {
	// TODO: File content, checksum, source location so it can be resent!
	pub offset: u64,
	pub size: u64,
	pub data: &'a [u8],
	// TODO: Checksum?
}

impl<'a> Block<'a> {
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();
		buf.extend_from_slice(&self.offset.to_le_bytes());
		buf.extend_from_slice(&self.size.to_le_bytes());
		buf.extend_from_slice(self.data);
		buf
	}

	pub async fn from_stream(
		stream: &mut (impl AsyncReadExt + Unpin),
		data_buf: &mut [u8],
	) -> Result<Block<'a>, ()> {
		let mut offset = [0; 8];
		stream.read_exact(&mut offset).await.map_err(|_| ())?; // TODO: Error handling
		let offset = u64::from_le_bytes(offset);

		let mut size = [0; 8];
		stream.read_exact(&mut size).await.map_err(|_| ())?; // TODO: Error handling
		let size = u64::from_le_bytes(size);

		// TODO: Ensure `size` is `block_size` or smaller else buffer overflow

		stream
			.read_exact(&mut data_buf[..size as usize])
			.await
			.map_err(|_| ())?; // TODO: Error handling

		Ok(Self {
			offset,
			size,
			data: &[], // TODO: This is super cringe. Data should be decoded here but lifetimes and extra allocations become a major concern.
		})
	}
}

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
