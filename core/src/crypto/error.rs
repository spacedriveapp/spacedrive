use thiserror::Error;

pub type Result<T> = std::result::Result<T, CryptoError>;

impl From<CryptoError> for rspc::Error {
	fn from(value: CryptoError) -> Self {
		Self::new(rspc::ErrorCode::InternalServerError, value.to_string())
	}
}

#[derive(Debug, Error)]
pub enum CryptoError {
	#[error("crypto error: {0}")]
	Crypto(#[from] sd_crypto::Error),

	#[error("generic key manager error")]
	KeyManager,

	#[error("the key specified was not found")]
	KeyNotFound,
	#[error("the key manager is locked")]
	Locked,
	#[error("the key is already mounted")]
	AlreadyMounted,
	#[error("key not mounted")]
	NotMounted,

	#[error("there was an error during a conversion")]
	Conversion,

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

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	#[error("async IO error: {0}")]
	IoAsync(#[from] tokio::io::Error),
}
