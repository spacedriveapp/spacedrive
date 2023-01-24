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
	#[error("there was an error while password hashing")]
	PasswordHash,
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("error while encrypting")]
	Encrypt,
	#[error("error while decrypting")]
	Decrypt,
	#[error("nonce length mismatch")]
	NonceLengthMismatch,
	#[error("invalid file header")]
	FileHeader,
	#[error("error initialising stream encryption/decryption")]
	StreamModeInit,
	#[error("wrong password provided")]
	IncorrectPassword,
	#[error("no keyslots available")]
	NoKeyslots,
	#[error("mismatched data length while converting vec to array")]
	VecArrSizeMismatch,
	#[error("error while parsing preview media length")]
	MediaLengthParse,
	#[error("no preview media found")]
	NoPreviewMedia,
	#[error("error while serializing/deserializing an item")]
	Serialization,
	#[error("no metadata found")]
	NoMetadata,
	#[error("tried adding too many keyslots to a header")]
	TooManyKeyslots,
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
	#[error("no master password has been provided to the keymanager")]
	NoMasterPassword,
	#[error("mismatch between supplied keys and the keystore")]
	KeystoreMismatch,
	#[error("mutex lock error")]
	MutexLock,
	#[error("no verification key")]
	NoVerificationKey,
	#[error("key isn't flagged as memory only")]
	KeyNotMemoryOnly,
	#[error("wrong information provided to the key manager")]
	IncorrectKeymanagerDetails,
	#[error("string parse error")]
	StringParse(#[from] FromUtf8Error),
}

impl<T> From<std::sync::PoisonError<T>> for Error {
	fn from(_: std::sync::PoisonError<T>) -> Self {
		Self::MutexLock
	}
}
