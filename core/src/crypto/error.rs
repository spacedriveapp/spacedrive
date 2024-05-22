use std::num::TryFromIntError;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, KeyManagerError>;

impl From<KeyManagerError> for rspc::Error {
	fn from(value: KeyManagerError) -> Self {
		Self::new(rspc::ErrorCode::InternalServerError, value.to_string())
	}
}

#[derive(Debug, Error)]
pub enum KeyManagerError {
	// #[error("crypto error: {0}")]
	// Crypto(#[from] sd_crypto::Error),
	#[error("the key specified was not found")]
	KeyNotFound,
	#[error("the key manager is locked")]
	Locked,
	#[error("the key is already mounted")]
	AlreadyMounted,
	#[error("key not mounted")]
	NotMounted,
	#[error("the key is already queued")]
	AlreadyQueued,

	#[error("there was an error during a conversion")]
	Conversion,
	#[error("there was an error converting ints")]
	IntConversion(#[from] TryFromIntError),

	#[error("the test vector failed (password is likely incorrect)")]
	IncorrectPassword,
	#[error("there was an issue while unlocking the key manager")]
	Unlock,

	#[error("an unsupported operation was attempted")]
	Unsupported,

	#[error("the word provided is too short")]
	WordTooShort,

	#[error("the specified file already exists and would be overwritten")]
	FileAlreadyExists,
	#[error("the specified file doesn't exist")]
	FileDoesntExist,
	#[error("the specified file is too large")]
	FileTooLarge,

	#[error("this action would delete the last root key (and make the key manager unusable)")]
	LastRootKey,

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	#[error("async IO error: {0}")]
	IoAsync(#[from] tokio::io::Error),

	#[error("error while converting a UUID: {0}")]
	Uuid(#[from] uuid::Error),
}
