use thiserror::Error;

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
}
