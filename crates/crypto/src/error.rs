//! This module contains all possible errors that this crate can return.

use std::string::FromUtf8Error;

impl From<Error> for bincode::error::EncodeError {
	fn from(value: Error) -> Self {
		Self::OtherString(value.to_string())
	}
}

pub type Result<T> = std::result::Result<T, Error>;

/// This enum defines all possible errors that this crate can give
#[allow(deprecated)]
#[derive(thiserror::Error, Debug)]
pub enum Error {
	// crypto primitive errors (STREAM, hashing)
	#[error("there was an error while password hashing")]
	Hashing,
	#[error("error while encrypting")]
	Encrypt,
	#[error("error while decrypting (could be: wrong key, wrong data, wrong aad, etc)")]
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

	#[error("error while encoding with bincode: {0}")]
	BincodeEncode(#[from] bincode::error::EncodeError),

	#[error("error while decoding with bincode: {0}")]
	BincodeDecode(#[from] bincode::error::DecodeError),

	// #[cfg(feature = "serde")]
	// #[error("error while encoding with serde")]
	// Serde,
	#[error("keystore error")]
	Keystore,

	#[error("redb error: {0}")]
	Redb(#[from] redb::Error),
	#[error("redb error: {0}")]
	RedbDatabase(#[from] redb::DatabaseError),
	#[error("redb error: {0}")]
	RedbTransaction(#[from] redb::TransactionError),
	#[error("redb error: {0}")]
	RedbTable(#[from] redb::TableError),
	#[error("redb error: {0}")]
	RedbStorage(#[from] redb::StorageError),
	#[error("redb error: {0}")]
	RedbCommit(#[from] redb::CommitError),

	#[error("vault root key already exists")]
	RootKeyAlreadyExists,

	// general errors
	#[error("expected length differs from provided length")]
	LengthMismatch,

	// TODO(brxken128): remove this, and add appropriate/correct errors
	#[error("expected type/value differs from provided")]
	Validity,
	#[error("string parse error")]
	StringParse(#[from] FromUtf8Error),

	// i/o
	#[cfg(not(feature = "tokio"))]
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[cfg(feature = "tokio")]
	#[error("I/O error: {0}")]
	AsyncIo(#[from] tokio::io::Error),
	#[cfg(feature = "tokio")]
	#[error("Async task join error: {0}")]
	JoinError(#[from] tokio::task::JoinError),

	#[error("hex error: {0}")]
	Hex(#[from] hex::FromHexError),

	// keyring
	#[cfg(all(target_os = "linux", feature = "keyring"))]
	#[error("error with the keyutils keyring: {0}")]
	KeyUtils(#[from] linux_keyutils::KeyError),
	#[cfg(all(target_os = "linux", feature = "keyring", feature = "secret-service"))]
	#[error("error with the secret service keyring: {0}")]
	SecretService(#[from] secret_service::Error),
	#[cfg(all(any(target_os = "macos", target_os = "ios"), feature = "keyring"))]
	#[error("error with the apple keyring: {0}")]
	AppleKeyring(#[from] security_framework::base::Error),
	#[cfg(feature = "keyring")]
	#[error("generic keyring error")]
	Keyring,
}
