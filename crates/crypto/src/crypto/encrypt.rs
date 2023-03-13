use std::io::Cursor;

use crate::{
	primitives::{
		types::{Key, Nonce},
		BLOCK_LEN,
	},
	Error, Result,
};
use aead::{stream::EncryptorLE31, Payload};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{exhaustive_read, new_cipher, Algorithm};

pub enum StreamEncryptor {
	XChaCha20Poly1305(Box<EncryptorLE31<XChaCha20Poly1305>>),
	Aes256Gcm(Box<EncryptorLE31<Aes256Gcm>>),
}

impl StreamEncryptor {
	/// This should be used to initialize a stream encryption object.
	///
	/// The master key, a suitable nonce, and a specific algorithm should be provided.
	#[allow(clippy::needless_pass_by_value)]
	pub fn new(key: Key, nonce: Nonce, algorithm: Algorithm) -> Result<Self> {
		if nonce.len() != algorithm.nonce_len() {
			return Err(Error::NonceLengthMismatch);
		}

		let stream = match algorithm {
			Algorithm::XChaCha20Poly1305 => Self::XChaCha20Poly1305(Box::new(
				EncryptorLE31::from_aead(new_cipher(key)?, (&*nonce).into()),
			)),
			Algorithm::Aes256Gcm => Self::Aes256Gcm(Box::new(EncryptorLE31::from_aead(
				new_cipher(key)?,
				(&*nonce).into(),
			))),
		};

		Ok(stream)
	}

	fn encrypt_next<'msg, 'aad>(
		&mut self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.encrypt_next(payload),
			Self::Aes256Gcm(s) => s.encrypt_next(payload),
		}
		.map_err(|_| Error::Encrypt)
	}

	fn encrypt_last<'msg, 'aad>(self, payload: impl Into<Payload<'msg, 'aad>>) -> Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.encrypt_last(payload),
			Self::Aes256Gcm(s) => s.encrypt_last(payload),
		}
		.map_err(|_| Error::Encrypt)
	}

	/// This function should be used for encrypting large amounts of data.
	///
	/// The streaming implementation reads blocks of data in `BLOCK_LEN`, encrypts, and writes to the writer.
	///
	/// It requires a reader, a writer, and any AAD to go with it.
	///
	/// The AAD will be authenticated with each block of data.
	pub async fn encrypt_streams<R, W>(
		mut self,
		mut reader: R,
		mut writer: W,
		aad: &[u8],
	) -> Result<()>
	where
		R: AsyncReadExt + Unpin + Send,
		W: AsyncWriteExt + Unpin + Send,
	{
		let mut buffer = vec![0u8; BLOCK_LEN].into_boxed_slice();

		loop {
			let count = exhaustive_read(&mut reader, &mut buffer).await?;

			let payload = Payload {
				aad,
				msg: &buffer[..count],
			};

			if count == BLOCK_LEN {
				let ciphertext = self.encrypt_next(payload)?;
				writer.write_all(&ciphertext).await?;
			} else {
				let ciphertext = self.encrypt_last(payload)?;
				writer.write_all(&ciphertext).await?;
				break;
			}
		}

		writer.flush().await?;

		Ok(())
	}

	/// This should ideally only be used for small amounts of data
	///
	/// It is just a thin wrapper around `encrypt_streams()`, but reduces the amount of code needed elsewhere.
	#[allow(unused_mut)]
	pub async fn encrypt_bytes(
		key: Key,
		nonce: Nonce,
		algorithm: Algorithm,
		bytes: &[u8],
		aad: &[u8],
	) -> Result<Vec<u8>> {
		let mut writer = Cursor::new(Vec::new());
		let encryptor = Self::new(key, nonce, algorithm)?;

		encryptor
			.encrypt_streams(bytes, &mut writer, aad)
			.await
			.map_or_else(Err, |_| Ok(writer.into_inner()))
	}
}
