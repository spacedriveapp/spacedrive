//! This module contains the crate's STREAM implementation, and wrappers that allow us to support multiple AEADs.
#![allow(clippy::use_self)] // I think: https://github.com/rust-lang/rust-clippy/issues/3909

use std::io::Cursor;

use crate::{
	primitives::{
		types::{Key, Nonce},
		AEAD_TAG_SIZE, BLOCK_SIZE,
	},
	Error, Protected, Result,
};
use aead::{
	stream::{DecryptorLE31, EncryptorLE31},
	KeyInit, Payload,
};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// These are all possible algorithms that can be used for encryption and decryption
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(
	feature = "serde",
	derive(serde::Serialize),
	derive(serde::Deserialize)
)]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub enum Algorithm {
	XChaCha20Poly1305,
	Aes256Gcm,
}

impl Algorithm {
	/// This function allows us to calculate the nonce length for a given algorithm
	#[must_use]
	pub const fn nonce_len(&self) -> usize {
		match self {
			Self::XChaCha20Poly1305 => 20,
			Self::Aes256Gcm => 8,
		}
	}
}

pub enum StreamEncryption {
	XChaCha20Poly1305(Box<EncryptorLE31<XChaCha20Poly1305>>),
	Aes256Gcm(Box<EncryptorLE31<Aes256Gcm>>),
}

pub enum StreamDecryption {
	Aes256Gcm(Box<DecryptorLE31<Aes256Gcm>>),
	XChaCha20Poly1305(Box<DecryptorLE31<XChaCha20Poly1305>>),
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
	/// The streaming implementation reads blocks of data in `BLOCK_SIZE`, encrypts, and writes to the writer.
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
		let mut read_buffer = vec![0u8; BLOCK_SIZE].into_boxed_slice();

		loop {
			let mut read_count = 0;
			loop {
				let i = reader.read(&mut read_buffer[read_count..]).await?;
				read_count += i;
				if i == 0 || read_count == BLOCK_SIZE {
					// if we're EOF or the buffer is filled
					break;
				}
			}

			if read_count == BLOCK_SIZE {
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

impl StreamDecryption {
	/// This should be used to initialize a stream decryption object.
	///
	/// The master key, nonce and algorithm that were used for encryption should be provided.
	#[allow(clippy::needless_pass_by_value)]
	pub fn new(key: Key, nonce: Nonce, algorithm: Algorithm) -> Result<Self> {
		if nonce.len() != algorithm.nonce_len() {
			return Err(Error::NonceLengthMismatch);
		}

		let decryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
					.map_err(|_| Error::StreamModeInit)?;

				let stream = DecryptorLE31::from_aead(cipher, (&*nonce).into());
				Self::XChaCha20Poly1305(Box::new(stream))
			}
			Algorithm::Aes256Gcm => {
				let cipher =
					Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::StreamModeInit)?;

				let stream = DecryptorLE31::from_aead(cipher, (&*nonce).into());
				Self::Aes256Gcm(Box::new(stream))
			}
		};

		Ok(decryption_object)
	}

	fn decrypt_next<'msg, 'aad>(
		&mut self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.decrypt_next(payload),
			Self::Aes256Gcm(s) => s.decrypt_next(payload),
		}
	}

	fn decrypt_last<'msg, 'aad>(
		self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.decrypt_last(payload),
			Self::Aes256Gcm(s) => s.decrypt_last(payload),
		}
	}

	/// This function should be used for decrypting large amounts of data.
	///
	/// The streaming implementation reads blocks of data in `BLOCK_SIZE`, decrypts, and writes to the writer.
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
		let mut read_buffer = vec![0u8; BLOCK_SIZE + AEAD_TAG_SIZE].into_boxed_slice();

		loop {
			let mut read_count = 0;
			loop {
				let i = reader.read(&mut read_buffer[read_count..]).await?;
				read_count += i;
				if i == 0 || read_count == (BLOCK_SIZE + AEAD_TAG_SIZE) {
					// if we're EOF or the buffer is filled
					break;
				}
			}

			if read_count == (BLOCK_SIZE + AEAD_TAG_SIZE) {
				let payload = Payload {
					aad,
					msg: &read_buffer,
				};

				let decrypted_data = self.decrypt_next(payload).map_err(|_| Error::Decrypt)?;
				writer.write_all(&decrypted_data).await?;
			} else {
				let payload = Payload {
					aad,
					msg: &read_buffer[..read_count],
				};

				let decrypted_data = self.decrypt_last(payload).map_err(|_| Error::Decrypt)?;
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
		let mut writer = Cursor::new(Vec::<u8>::new());
		let decryptor = Self::new(key, nonce, algorithm)?;

		decryptor
			.decrypt_streams(bytes, &mut writer, aad)
			.await
			.map_or_else(Err, |_| Ok(Protected::new(writer.into_inner())))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const KEY: [u8; 32] = [
		0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23,
		0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23,
		0x23, 0x23,
	];

	const AES_NONCE: [u8; 8] = [0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9];
	const XCHACHA_NONCE: [u8; 20] = [
		0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9,
		0xE9, 0xE9, 0xE9, 0xE9, 0xE9,
	];

	const PLAINTEXT: [u8; 32] = [
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A,
	];

	const AAD: [u8; 16] = [
		0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92,
		0x92,
	];

	const AES_ENCRYPT_BYTES_EXPECTED: [u8; 48] = [
		38, 96, 235, 51, 131, 187, 162, 152, 183, 13, 174, 87, 108, 113, 198, 88, 106, 121, 208,
		37, 20, 10, 2, 107, 69, 147, 171, 141, 46, 255, 181, 123, 24, 150, 104, 25, 70, 198, 169,
		232, 124, 99, 151, 226, 84, 113, 184, 134,
	];

	const AES_ENCRYPT_BYTES_WITH_AAD_EXPECTED: [u8; 48] = [
		38, 96, 235, 51, 131, 187, 162, 152, 183, 13, 174, 87, 108, 113, 198, 88, 106, 121, 208,
		37, 20, 10, 2, 107, 69, 147, 171, 141, 46, 255, 181, 123, 172, 121, 35, 145, 71, 115, 203,
		224, 20, 183, 1, 99, 223, 230, 255, 76,
	];

	const XCHACHA_ENCRYPT_BYTES_EXPECTED: [u8; 48] = [
		35, 174, 252, 59, 215, 65, 5, 237, 198, 2, 51, 72, 239, 88, 36, 177, 136, 252, 64, 157,
		141, 53, 138, 98, 185, 2, 75, 173, 253, 99, 133, 207, 145, 54, 100, 51, 44, 230, 60, 5,
		157, 70, 110, 145, 166, 41, 215, 95,
	];

	const XCHACHA_ENCRYPT_BYTES_WITH_AAD_EXPECTED: [u8; 48] = [
		35, 174, 252, 59, 215, 65, 5, 237, 198, 2, 51, 72, 239, 88, 36, 177, 136, 252, 64, 157,
		141, 53, 138, 98, 185, 2, 75, 173, 253, 99, 133, 207, 110, 4, 255, 118, 55, 88, 24, 170,
		101, 74, 104, 122, 105, 216, 225, 243,
	];

	#[tokio::test]
	async fn aes_encrypt_bytes() {
		let ciphertext = StreamEncryption::encrypt_bytes(
			Key::new(KEY),
			Nonce::Aes256Gcm(AES_NONCE),
			Algorithm::Aes256Gcm,
			&PLAINTEXT,
			&[],
		)
		.await
		.unwrap();

		assert_eq!(AES_ENCRYPT_BYTES_EXPECTED.to_vec(), ciphertext)
	}

	#[tokio::test]
	async fn aes_encrypt_bytes_with_aad() {
		let ciphertext = StreamEncryption::encrypt_bytes(
			Key::new(KEY),
			Nonce::Aes256Gcm(AES_NONCE),
			Algorithm::Aes256Gcm,
			&PLAINTEXT,
			&AAD,
		)
		.await
		.unwrap();

		assert_eq!(AES_ENCRYPT_BYTES_WITH_AAD_EXPECTED.to_vec(), ciphertext)
	}

	#[tokio::test]
	async fn aes_decrypt_bytes() {
		let plaintext = StreamDecryption::decrypt_bytes(
			Key::new(KEY),
			Nonce::Aes256Gcm(AES_NONCE),
			Algorithm::Aes256Gcm,
			&AES_ENCRYPT_BYTES_EXPECTED,
			&[],
		)
		.await
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().to_vec())
	}

	#[tokio::test]
	async fn aes_decrypt_bytes_with_aad() {
		let plaintext = StreamDecryption::decrypt_bytes(
			Key::new(KEY),
			Nonce::Aes256Gcm(AES_NONCE),
			Algorithm::Aes256Gcm,
			&AES_ENCRYPT_BYTES_WITH_AAD_EXPECTED,
			&AAD,
		)
		.await
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().to_vec())
	}

	#[tokio::test]
	#[should_panic]
	async fn aes_decrypt_bytes_missing_aad() {
		StreamDecryption::decrypt_bytes(
			Key::new(KEY),
			Nonce::Aes256Gcm(AES_NONCE),
			Algorithm::Aes256Gcm,
			&AES_ENCRYPT_BYTES_WITH_AAD_EXPECTED,
			&[],
		)
		.await
		.unwrap();
	}

	#[tokio::test]
	async fn xchacha_encrypt_bytes() {
		let ciphertext = StreamEncryption::encrypt_bytes(
			Key::new(KEY),
			Nonce::XChaCha20Poly1305(XCHACHA_NONCE),
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&[],
		)
		.await
		.unwrap();

		assert_eq!(XCHACHA_ENCRYPT_BYTES_EXPECTED.to_vec(), ciphertext)
	}

	#[tokio::test]
	async fn xchacha_encrypt_bytes_with_aad() {
		let ciphertext = StreamEncryption::encrypt_bytes(
			Key::new(KEY),
			Nonce::XChaCha20Poly1305(XCHACHA_NONCE),
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&AAD,
		)
		.await
		.unwrap();

		assert_eq!(XCHACHA_ENCRYPT_BYTES_WITH_AAD_EXPECTED.to_vec(), ciphertext)
	}

	#[tokio::test]
	async fn xchacha_decrypt_bytes() {
		let plaintext = StreamDecryption::decrypt_bytes(
			Key::new(KEY),
			Nonce::XChaCha20Poly1305(XCHACHA_NONCE),
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_ENCRYPT_BYTES_EXPECTED,
			&[],
		)
		.await
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().to_vec())
	}

	#[tokio::test]
	async fn xchacha_decrypt_bytes_with_aad() {
		let plaintext = StreamDecryption::decrypt_bytes(
			Key::new(KEY),
			Nonce::XChaCha20Poly1305(XCHACHA_NONCE),
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_ENCRYPT_BYTES_WITH_AAD_EXPECTED,
			&AAD,
		)
		.await
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().to_vec())
	}

	#[tokio::test]
	#[should_panic]
	async fn xchacha_decrypt_bytes_missing_aad() {
		StreamDecryption::decrypt_bytes(
			Key::new(KEY),
			Nonce::XChaCha20Poly1305(XCHACHA_NONCE),
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_ENCRYPT_BYTES_WITH_AAD_EXPECTED,
			&[],
		)
		.await
		.unwrap();
	}
}
