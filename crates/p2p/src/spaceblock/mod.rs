//! Spaceblock is a file transfer protocol that uses a block based system to transfer files.
//! This protocol is modelled after SyncThing's BEP protocol. A huge thanks to it's original authors!
//! You can read more about it here: https://docs.syncthing.net/specs/bep-v1.html
#![allow(unused)] // TODO: This module is still in heavy development!

use std::{
	marker::PhantomData,
	path::{Path, PathBuf},
};

/// TODO
pub struct BlockSize(i64);

impl BlockSize {
	// TODO: Validating `BlockSize` are multiple of 2, i think

	pub fn from_size(size: u64) -> Self {
		// TODO: Something like: https://docs.syncthing.net/specs/bep-v1.html#selection-of-block-size
		Self(131072) // 128 KiB
	}
}

/// TODO
pub struct TransferRequest<'a> {
	name: &'a str,
	size: u64,
	// TODO: Include file permissions
	block_size: u64,
}

/// TODO
pub struct Block<'a> {
	// TODO: File content, checksum, source location so it can be resent!
	offset: i64,
	size: i64,
	data: &'a [u8],
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
