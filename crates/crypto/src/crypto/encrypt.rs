use std::io::Cursor;

use crate::{
	primitives::{
		types::{Key, Nonce},
		BLOCK_LEN,
	},
	Error, Result,
};
use aead::{stream::EncryptorLE31, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{exhaustive_read, Algorithm};

pub enum StreamEncryption {
	XChaCha20Poly1305(Box<EncryptorLE31<XChaCha20Poly1305>>),
	Aes256Gcm(Box<EncryptorLE31<Aes256Gcm>>),
}

impl StreamEncryption {
	/// This should be used to initialize a stream encryption object.
	///
	/// The master key, a suitable nonce, and a specific algorithm should be provided.
	#[allow(clippy::needless_pass_by_value)]
	pub fn new(key: Key, nonce: Nonce, algorithm: Algorithm) -> Result<Self> {
		if nonce.len() != algorithm.nonce_len() {
			return Err(Error::NonceLengthMismatch);
		}

		let encryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
					.map_err(|_| Error::StreamModeInit)?;

				let stream = EncryptorLE31::from_aead(cipher, (&*nonce).into());
				Self::XChaCha20Poly1305(Box::new(stream))
			}
			Algorithm::Aes256Gcm => {
				let cipher =
					Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::StreamModeInit)?;

				let stream = EncryptorLE31::from_aead(cipher, (&*nonce).into());
				Self::Aes256Gcm(Box::new(stream))
			}
		};

		Ok(encryption_object)
	}

	fn encrypt_next<'msg, 'aad>(
		&mut self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.encrypt_next(payload),
			Self::Aes256Gcm(s) => s.encrypt_next(payload),
		}
	}

	fn encrypt_last<'msg, 'aad>(
		self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.encrypt_last(payload),
			Self::Aes256Gcm(s) => s.encrypt_last(payload),
		}
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
		let mut read_buffer = vec![0u8; BLOCK_LEN].into_boxed_slice();

		loop {
			let read_count = exhaustive_read(&mut reader, &mut read_buffer).await?;

			if read_count == BLOCK_LEN {
				let payload = Payload {
					aad,
					msg: &read_buffer,
				};

				let encrypted_data = self.encrypt_next(payload).map_err(|_| Error::Encrypt)?;
				writer.write_all(&encrypted_data).await?;
			} else {
				// we use `..read_count` in order to only use the read data, and not zeroes also
				let payload = Payload {
					aad,
					msg: &read_buffer[..read_count],
				};

				let encrypted_data = self.encrypt_last(payload).map_err(|_| Error::Encrypt)?;
				writer.write_all(&encrypted_data).await?;
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
		let mut writer = Cursor::new(Vec::<u8>::new());
		let encryptor = Self::new(key, nonce, algorithm)?;

		encryptor
			.encrypt_streams(bytes, &mut writer, aad)
			.await
			.map_or_else(Err, |_| Ok(writer.into_inner()))
	}
}
