use thiserror::Error;

pub type Result<T> = std::result::Result<T, CryptoError>;

#[derive(Debug, Error)]
pub enum CryptoError {
	#[error("crypto error: {0}")]
	Crypto(#[from] sd_crypto::Error),

	#[error("generic key manager error")]
	KeyManager,
}
