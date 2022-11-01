//! This module contains all possible errors that this crate can return.
use thiserror::Error;

#[cfg(feature = "rspc")]
impl From<Error> for rspc::Error {
	fn from(err: Error) -> Self {
		rspc::Error::with_cause(
			rspc::ErrorCode::InternalServerError,
			"Internal cryptographic error occured".into(),
			err,
		)
	}
}

pub type Result<T> = std::result::Result<T, Error>;

/// This enum defines all possible errors that this crate can give
#[derive(Error, Debug)]
pub enum Error {
	#[error("not enough bytes were written to the output file")]
	WriteMismatch,
	#[error("there was an error hashing the password")]
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
	#[error("error while serializing/deserializing the metadata")]
	MetadataDeSerialization,
	#[error("no metadata found")]
	NoMetadata,
	#[error("tried adding too many keyslots to a header")]
	TooManyKeyslots,
	#[error("requested key wasn't found in the key manager")]
	KeyNotFound,
	#[error("no default key has been set")]
	NoDefaultKeySet,
	#[error("no master password has been provided to the keymanager")]
	NoMasterPassword,
	#[error("mismatch between supplied keys and the keystore")]
	KeystoreMismatch,
}
