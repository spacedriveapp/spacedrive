//! Spaceblock is a file transfer protocol that uses a block based system to transfer files.
//! This protocol is modelled after SyncThing's BEP protocol. A huge thanks to it's original authors!
//! You can read more about it here: https://docs.syncthing.net/specs/bep-v1.html
#![allow(unused)] // TODO: This module is still in heavy development!

use std::{
	marker::PhantomData,
	path::{Path, PathBuf},
};

use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWriteExt, BufReader},
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

	pub fn to_size(&self) -> u32 {
		self.0
	}
}

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacedropRequest {
	pub name: String,
	pub size: u64,
	// TODO: Include file permissions
	pub block_size: BlockSize,
}

impl SpacedropRequest {
	pub async fn from_stream(stream: &mut UnicastStream) -> Result<Self, ()> {
		let mut name_len = [0; 2];
		stream.read_exact(&mut name_len).await.map_err(|_| ())?; // TODO: Error handling
		let name_len = u16::from_le_bytes(name_len);

		let mut name = vec![0u8; name_len as usize];
		stream.read_exact(&mut name).await.map_err(|_| ())?; // TODO: Error handling
		let name = String::from_utf8(name).map_err(|_| ())?; // TODO: Error handling

		let mut size = [0; 8];
		stream.read_exact(&mut size).await.map_err(|_| ())?; // TODO: Error handling
		let size = u64::from_le_bytes(size);

		// TODO: If we change `BlockSize` and both clients are running a different version this will not match up and everything will explode
		let block_size = BlockSize::from_size(size); // TODO: Error handling

		Ok(Self {
			name,
			size,
			block_size,
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();

		let len_buf = self.name.len().to_le_bytes();
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
		buf.extend_from_slice(&self.data);
		buf
	}

	pub async fn from_stream(
		stream: &mut UnicastStream,
		data_buf: &mut [u8],
	) -> Result<Block<'a>, ()> {
		let mut offset = [0; 8];
		stream.read_exact(&mut offset).await.map_err(|_| ())?; // TODO: Error handling
		let offset = u64::from_le_bytes(offset);

		let mut size = [0; 8];
		stream.read_exact(&mut size).await.map_err(|_| ())?; // TODO: Error handling
		let size = u64::from_le_bytes(size);

		// TODO: Handle overflow of `data_buf`
		// TODO: Prevent this being used as a DoS cause I think it can
		let mut read_offset = 0u64;
		loop {
			let read = stream.read(data_buf).await.map_err(|_| ())?; // TODO: Error handling
			read_offset += read as u64;

			if read_offset == size {
				break;
			}
		}

		Ok(Self {
			offset,
			size,
			data: &[], // TODO: This is super cringe. Data should be decoded here but lifetimes and extra allocations become a major concern.
		})
	}
}

pub async fn send(stream: &mut UnicastStream, mut file: File, req: &SpacedropRequest) {
	// We manually implement what is basically a `BufReader` so we have more control
	let mut buf = vec![0u8; req.block_size.to_size() as usize];
	let mut offset: u64 = 0;

	loop {
		let read = file.read(&mut buf[..]).await.unwrap(); // TODO: Error handling
		offset += read as u64;

		let block = Block {
			offset: offset,
			size: read as u64,
			data: &buf[..read],
		};
		debug!(
			"Sending block at offset {} of size {}",
			block.offset, block.size
		);
		stream.write_all(&block.to_bytes()).await.unwrap(); // TODO: Error handling

		if read == 0 {
			if offset != req.size {
				panic!("U dun goofed"); // TODO: Error handling
			}

			break;
		}
	}
}

pub async fn receive(stream: &mut UnicastStream, mut file: File, req: &SpacedropRequest) {
	// We manually implement what is basically a `BufReader` so we have more control
	let mut data_buf = vec![0u8; req.block_size.to_size() as usize];
	let mut offset: u64 = 0;

	loop {
		let block = Block::from_stream(stream, &mut data_buf).await.unwrap(); // TODO: Error handling

		debug!(
			"Received block at offset {} of size {}",
			block.offset, block.size
		);
		file.write_all(&data_buf).await.unwrap(); // TODO: Error handling

		// TODO: Should this be `read == 0`
		if offset == req.size {
			break;
		}
	}
}
