//! Spaceblock is a file transfer protocol that uses a block based system to transfer files.
//! This protocol is modelled after SyncThing's BEP protocol. A huge thanks to it's original authors!
//! You can read more about it here: https://docs.syncthing.net/specs/bep-v1.html
#![allow(unused)] // TODO: This module is still in heavy development!

use std::{
	marker::PhantomData,
	path::{Path, PathBuf},
};

use tokio::io::AsyncReadExt;

use crate::spacetime::{SpaceTimeStream, UnicastStream};

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSize(i64);

impl BlockSize {
	// TODO: Validating `BlockSize` are multiple of 2, i think

	pub fn from_size(size: u64) -> Self {
		// TODO: Something like: https://docs.syncthing.net/specs/bep-v1.html#selection-of-block-size
		Self(131072) // 128 KiB
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
		let name_len = stream.read_u8().await.map_err(|_| ())?; // TODO: Error handling
		let mut name = vec![0u8; name_len as usize];
		stream.read_exact(&mut name).await.map_err(|_| ())?; // TODO: Error handling
		let name = String::from_utf8(name).map_err(|_| ())?; // TODO: Error handling

		let size = stream.read_u8().await.map_err(|_| ())? as u64; // TODO: Error handling
		let block_size = BlockSize::from_size(size); // TODO: Get from stream: stream.read_u8().await.map_err(|_| ())?; // TODO: Error handling

		Ok(Self {
			name,
			size,
			block_size,
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();
		buf.push(self.name.len() as u8); // TODO: This being a `u8` isn't going to scale to a name bigger than 255 bytes lmao
		buf.extend(self.name.as_bytes());
		buf.push(self.size as u8); // TODO: This being a `u8` isn't going to scale to files bigger than 255 bytes lmao
						   // buf.push(&self.block_size.to_be_bytes()); // TODO: Do this as well
		buf
	}
}

/// TODO
pub struct Block<'a> {
	// TODO: File content, checksum, source location so it can be resent!
	pub offset: i64,
	pub size: i64,
	pub data: &'a [u8],
	// TODO: Checksum?
}

/// TODO
pub struct Transfer<'a> {
	// buf: &'a mut [u8],
	phantom: PhantomData<&'a ()>,
}

impl<'a> Transfer<'a> {
	// TODO: Allow the user to cancel a tranfer
	// TODO: Handle if the stream is dropped

	pub fn from_file(path: impl AsRef<Path>) -> Self {
		// let size = std::fs::metadata(path.as_ref()).unwrap().len();

		Self {
			// buf: &mut Vec::with_capacity(42069),
			phantom: PhantomData,
		}
	}
}
