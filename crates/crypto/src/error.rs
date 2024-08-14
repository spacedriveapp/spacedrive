//! This module contains all possible errors that this crate can return.

use tokio::io;

/// This enum defines all possible errors that this crate can give
#[derive(thiserror::Error, Debug)]
pub enum Error {
	// crypto errors
	#[error("Block too big for oneshot encryption: size in bytes = {0}")]
	BlockTooBig(usize),

	/// Encrypt and decrypt errors, AEAD crate doesn't provide any error context for these
	/// as it can be a security hazard to leak information about the error.
	#[error("Encryption error")]
	Encrypt,
	#[error("Decryption error")]
	Decrypt,

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

	#[error("I/O error while erasing: {{context: {context}, source: {source}}}")]
	EraseIo {
		context: &'static str,
		#[source]
		source: io::Error,
	},

	#[error("hex error: {0}")]
	Hex(#[from] hex::FromHexError),
}
