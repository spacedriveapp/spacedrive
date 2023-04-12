//! This module contains all possible errors that this crate can return.

use std::string::FromUtf8Error;
use thiserror::Error;

#[cfg(feature = "bincode")]
impl From<Error> for bincode::error::EncodeError {
	fn from(value: Error) -> Self {
		Self::OtherString(value.to_string())
	}
}

pub type Result<T> = std::result::Result<T, Error>;

/// This enum defines all possible errors that this crate can give
#[derive(Error, Debug)]
pub enum Error {
	// crypto primitive errors (STREAM, hashing)
	#[error("there was an error while password hashing")]
	Hashing,
	#[error("error while encrypting")]
	Encrypt,
	#[error("error while decrypting (could be: wrong password, wrong data, wrong aad, etc)")]
	Decrypt,

	// header errors
	#[error("no keyslots available")]
	NoKeyslots,
	#[error("tried adding too many keyslots to a header")]
	TooManyKeyslots,
	#[error("no header objects available (or none that match)")]
	NoObjects,
	#[error("tried adding too many objects to a header (or too many with the same name)")]
	TooManyObjects,
	#[error("read magic bytes aren't equal to the expected bytes")]
	MagicByteMismatch,

	#[cfg(feature = "bincode")]
	#[error("error while encoding with bincode: {0}")]
	BincodeEncode(#[from] bincode::error::EncodeError),
	#[cfg(feature = "bincode")]
	#[error("error while decoding with bincode: {0}")]
	BincodeDecode(#[from] bincode::error::DecodeError),

	#[error("keystore error")]
	Keystore,

	// general errors
	#[error("expected length differs from provided length")]
	LengthMismatch,
	#[error("expected type/value differs from provided")]
	Validity,
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("string parse error")]
	StringParse(#[from] FromUtf8Error),

	// keyring
	#[cfg(all(target_os = "linux", feature = "sys"))]
	#[error("error with the linux keyring: {0}")]
	LinuxKeyring(#[from] linux_keyutils::KeyError),
	#[cfg(all(any(target_os = "macos", target_os = "ios"), feature = "sys"))]
	#[error("error with the apple keyring: {0}")]
	AppleKeyring(#[from] security_framework::base::Error),
	#[cfg(feature = "sys")]
	#[error("generic keyring error")]
	Keyring,
}
