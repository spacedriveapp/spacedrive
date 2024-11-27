//! This module contains all possible errors that this crate can return.

use tokio::io;

/// This enum defines all possible errors that this crate can give
#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("Block too big for oneshot encryption: size in bytes = {0}")]
	BlockTooBig(usize),
	#[error("Invalid key size: expected 32 bytes, got {0}")]
	InvalidKeySize(usize),

	/// Encrypt and decrypt errors, AEAD crate doesn't provide any error context for these
	/// as it can be a security hazard to leak information about the error.
	#[error("Encryption error")]
	Encrypt,
	#[error("Decryption error")]
	Decrypt,

	/// I/O error while encrypting
	#[error("I/O error while encrypting: {{context: {context}, source: {source}}}")]
	EncryptIo {
		context: &'static str,
		#[source]
		source: io::Error,
	},
	#[error("I/O error while decrypting: {{context: {context}, source: {source}}}")]
	DecryptIo {
		context: &'static str,
		#[source]
		source: io::Error,
	},

	/// I/O error while erasing a file
	#[error("I/O error while erasing: {{context: {context}, source: {source}}}")]
	EraseIo {
		context: &'static str,
		#[source]
		source: io::Error,
	},

	#[error("hex error: {0}")]
	Hex(#[from] hex::FromHexError),

	#[error("Entropy source error: {0}")]
	EntropySource(#[from] rand_core::getrandom::Error),
}
