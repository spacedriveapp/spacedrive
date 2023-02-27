//! This module contains all possible errors that this crate can return.

use std::string::FromUtf8Error;

use thiserror::Error;

#[cfg(feature = "rspc")]
impl From<Error> for rspc::Error {
	fn from(err: Error) -> Self {
		Self::new(rspc::ErrorCode::InternalServerError, err.to_string())
	}
}

pub type Result<T> = std::result::Result<T, Error>;

/// This enum defines all possible errors that this crate can give
#[derive(Error, Debug)]
pub enum Error {
	// crypto primitive errors (STREAM, hashing)
	#[error("there was an error while password hashing")]
	PasswordHash,
	#[error("error while encrypting")]
	Encrypt,
	#[error("error while decrypting")]
	Decrypt,
	#[error("nonce length mismatch")]
	NonceLengthMismatch,
	#[error("error initialising stream encryption/decryption")]
	StreamModeInit,

	// header errors
	#[error("no keyslots available")]
	NoKeyslots,
	#[error("no preview media found")]
	NoPreviewMedia,
	#[error("no metadata found")]
	NoMetadata,
	#[error("tried adding too many keyslots to a header")]
	TooManyKeyslots,

	// key manager
	#[error("requested key wasn't found in the key manager")]
	KeyNotFound,
	#[error("key is already mounted")]
	KeyAlreadyMounted,
	#[error("key not mounted")]
	KeyNotMounted,
	#[error("key isn't in the queue")]
	KeyNotQueued,
	#[error("key is already in the queue")]
	KeyAlreadyQueued,
	#[error("no default key has been set")]
	NoDefaultKeySet,
	#[error("keymanager is not unlocked")]
	NotUnlocked,
	#[error("no verification key")]
	NoVerificationKey,
	#[error("key isn't flagged as memory only")]
	KeyNotMemoryOnly,

	// general errors
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("mismatched data length while converting vec to array")]
	VecArrSizeMismatch,
	#[error("incorrect password/details were provided")]
	IncorrectPassword,
	#[error("error while serializing/deserializing an item")]
	Serialization,
	#[error("string parse error")]
	StringParse(#[from] FromUtf8Error),

	// keyring
	#[cfg(target_os = "linux")]
	#[error("error with the linux keyring: {0}")]
	LinuxKeyringError(#[from] secret_service::Error),
	#[cfg(any(target_os = "macos", target_os = "ios"))]
	#[error("error with the apple keyring: {0}")]
	AppleKeyringError(#[from] security_framework::base::Error),
	#[error("generic keyring error")]
	KeyringError,
	#[error("keyring not available on this platform")]
	KeyringNotSupported,
}
