use thiserror::Error;

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
	#[error("error initialising in-memory encryption/decryption")]
	MemoryModeInit,
	#[error("wrong password provided")]
	IncorrectPassword,
	#[error("no keyslots available")]
	NoKeyslots,
}
