use std::io::Cursor;

use crate::{
	primitives::{
		types::{Key, Nonce},
		AEAD_TAG_LEN, BLOCK_LEN,
	},
	Error, Protected, Result,
};
use aead::{stream::DecryptorLE31, Payload};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{exhaustive_read, new_cipher, Algorithm};

pub enum StreamDecryptor {
	Aes256Gcm(Box<DecryptorLE31<Aes256Gcm>>),
	XChaCha20Poly1305(Box<DecryptorLE31<XChaCha20Poly1305>>),
}

impl StreamDecryptor {
	/// This should be used to initialize a stream decryption object.
	///
	/// The master key, nonce and algorithm that were used for encryption should be provided.
	#[allow(clippy::needless_pass_by_value)]
	pub fn new(key: Key, nonce: Nonce, algorithm: Algorithm) -> Result<Self> {
		if nonce.len() != algorithm.nonce_len() {
			return Err(Error::NonceLengthMismatch);
		}

		let stream = match algorithm {
			Algorithm::XChaCha20Poly1305 => Self::XChaCha20Poly1305(Box::new(
				DecryptorLE31::from_aead(new_cipher(key)?, (&*nonce).into()),
			)),
			Algorithm::Aes256Gcm => Self::Aes256Gcm(Box::new(DecryptorLE31::from_aead(
				new_cipher(key)?,
				(&*nonce).into(),
			))),
		};

		Ok(stream)
	}

	fn decrypt_next<'msg, 'aad>(
		&mut self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.decrypt_next(payload),
			Self::Aes256Gcm(s) => s.decrypt_next(payload),
		}
		.map_err(|_| Error::Decrypt)
	}

	fn decrypt_last<'msg, 'aad>(self, payload: impl Into<Payload<'msg, 'aad>>) -> Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.decrypt_last(payload),
			Self::Aes256Gcm(s) => s.decrypt_last(payload),
		}
		.map_err(|_| Error::Decrypt)
	}

	/// This function should be used for decrypting large amounts of data.
	///
	/// The streaming implementation reads blocks of data in `BLOCK_LEN`, decrypts, and writes to the writer.
	///
	/// It requires a reader, a writer, and any AAD that was used.
	///
	/// The AAD will be authenticated with each block of data - if the AAD doesn't match what was used during encryption, an error will be returned.
	pub async fn decrypt_streams<R, W>(
		mut self,
		mut reader: R,
		mut writer: W,
		aad: &[u8],
	) -> Result<()>
	where
		R: AsyncReadExt + Unpin + Send,
		W: AsyncWriteExt + Unpin + Send,
	{
		let mut read_buffer = vec![0u8; BLOCK_LEN + AEAD_TAG_LEN].into_boxed_slice();

		loop {
			let read_count = exhaustive_read(&mut reader, &mut read_buffer).await?;

			if read_count == (BLOCK_LEN + AEAD_TAG_LEN) {
				let payload = Payload {
					aad,
					msg: &read_buffer,
				};

				let decrypted_data = self.decrypt_next(payload)?;
				writer.write_all(&decrypted_data).await?;
			} else {
				let payload = Payload {
					aad,
					msg: &read_buffer[..read_count],
				};

				let decrypted_data = self.decrypt_last(payload)?;
				writer.write_all(&decrypted_data).await?;
				break;
			}
		}

		writer.flush().await?;

		Ok(())
	}

	/// This should ideally only be used for small amounts of data
	///
	/// It is just a thin wrapper around `decrypt_streams()`, but reduces the amount of code needed elsewhere.
	#[allow(unused_mut)]
	pub async fn decrypt_bytes(
		key: Key,
		nonce: Nonce,
		algorithm: Algorithm,
		bytes: &[u8],
		aad: &[u8],
	) -> Result<Protected<Vec<u8>>> {
		let mut writer = Cursor::new(Vec::new());
		let decryptor = Self::new(key, nonce, algorithm)?;

		decryptor
			.decrypt_streams(bytes, &mut writer, aad)
			.await
			.map_or_else(Err, |_| Ok(Protected::new(writer.into_inner())))
	}
}
