//! This module contains the crate's STREAM implementation, and wrappers that allow us to support multiple AEADs.
use std::io::{Cursor, Read, Write};

use aead::{
	stream::{DecryptorLE31, EncryptorLE31},
	Buffer, KeyInit, Payload,
};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use serde::{Deserialize, Serialize};
use specta::Type;
use zeroize::Zeroize;

use crate::{primitives::BLOCK_SIZE, Error, Protected, Result};

/// These are all possible algorithms that can be used for encryption and decryption
#[derive(Clone, Copy, Eq, PartialEq, Type, Serialize, Deserialize)]
#[allow(clippy::use_self)]
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
	pub fn new(key: Protected<[u8; 32]>, nonce: &[u8], algorithm: Algorithm) -> Result<Self> {
		if nonce.len() != algorithm.nonce_len() {
			return Err(Error::NonceLengthMismatch);
		}

		let encryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
					.map_err(|_| Error::StreamModeInit)?;

				let stream = EncryptorLE31::from_aead(cipher, nonce.into());
				Self::XChaCha20Poly1305(Box::new(stream))
			}
			Algorithm::Aes256Gcm => {
				let cipher =
					Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::StreamModeInit)?;

				let stream = EncryptorLE31::from_aead(cipher, nonce.into());
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
	/// Measures are in place to zeroize any buffers that may contain sensitive information.
	///
	/// It requires a reader, a writer, and any AAD to go with it.
	///
	/// The AAD will be authenticated with each block of data.
	pub fn encrypt_streams<R, W>(mut self, mut reader: R, mut writer: W, aad: &[u8]) -> Result<()>
	where
		R: Read,
		W: Write,
	{
		let mut read_buffer = vec![0u8; BLOCK_SIZE].into_boxed_slice();
		loop {
			let read_count = reader.read(&mut read_buffer).map_err(Error::Io)?;
			if read_count == BLOCK_SIZE {
				let payload = Payload {
					aad,
					msg: &read_buffer,
				};

				let encrypted_data = self.encrypt_next(payload).map_err(|_| {
					read_buffer.zeroize();
					Error::Encrypt
				})?;

				// zeroize before writing, so any potential errors won't result in a potential data leak
				// this specific zeroize technically isn't needed due to the boxed slice, but performance impact is
				// negligible and it's good practice either way

				writer.write_all(&encrypted_data)?;
			} else {
				// we use `..read_count` in order to only use the read data, and not zeroes also
				let payload = Payload {
					aad,
					msg: &read_buffer[..read_count],
				};

				let encrypted_data = self.encrypt_last(payload).map_err(|_| {
					read_buffer.zeroize();
					Error::Encrypt
				})?;

				// zeroize before writing, so any potential errors won't result in a potential data leak
				read_buffer.zeroize();

				writer.write_all(&encrypted_data)?;

				break;
			}
		}

		writer.flush().map_err(Error::Io)?;

		Ok(())
	}

	/// This should ideally only be used for small amounts of data
	///
	/// It is just a thin wrapper around `encrypt_streams()`, but reduces the amount of code needed elsewhere.
	#[allow(unused_mut)]
	pub fn encrypt_bytes(
		key: Protected<[u8; 32]>,
		nonce: &[u8],
		algorithm: Algorithm,
		bytes: &[u8],
		aad: &[u8],
	) -> Result<Vec<u8>> {
		let mut writer = Cursor::new(Vec::<u8>::new());
		let encryptor = Self::new(key, nonce, algorithm)?;

		match encryptor.encrypt_streams(bytes, &mut writer, aad) {
			Ok(_) => Ok(writer.into_inner()),
			Err(e) => Err(e),
		}
	}
}

impl StreamDecryption {
	/// This should be used to initialize a stream decryption object.
	///
	/// The master key, nonce and algorithm that were used for encryption should be provided.
	#[allow(clippy::needless_pass_by_value)]
	pub fn new(key: Protected<[u8; 32]>, nonce: &[u8], algorithm: Algorithm) -> Result<Self> {
		if nonce.len() != algorithm.nonce_len() {
			return Err(Error::NonceLengthMismatch);
		}

		let decryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
					.map_err(|_| Error::StreamModeInit)?;

				let stream = DecryptorLE31::from_aead(cipher, nonce.into());
				Self::XChaCha20Poly1305(Box::new(stream))
			}
			Algorithm::Aes256Gcm => {
				let cipher =
					Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::StreamModeInit)?;

				let stream = DecryptorLE31::from_aead(cipher, nonce.into());
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
	/// Measures are in place to zeroize any buffers that may contain sensitive information.
	///
	/// It requires a reader, a writer, and any AAD that was used.
	///
	/// The AAD will be authenticated with each block of data - if the AAD doesn't match what was used during encryption, an error will be returned.
	pub fn decrypt_streams<R, W>(mut self, mut reader: R, mut writer: W, aad: &[u8]) -> Result<()>
	where
		R: Read,
		W: Write,
	{
		let mut read_buffer = vec![0u8; BLOCK_SIZE + 16].into_boxed_slice();

		loop {
			let read_count = reader.read(&mut read_buffer).map_err(Error::Io)?;
			if read_count == (BLOCK_SIZE + 16) {
				let payload = Payload {
					aad,
					msg: &read_buffer,
				};

				let mut decrypted_data = self.decrypt_next(payload).map_err(|_| Error::Decrypt)?;

				// Using `write` instead of `write_all` so we can check the amount of bytes written
				// Zeroize buffer on write error
				writer.write_all(&decrypted_data).map_err(|e| {
					decrypted_data.zeroize();
					Error::Io(e)
				})?;

			// decrypted_data.zeroize();
			} else {
				let payload = Payload {
					aad,
					msg: &read_buffer[..read_count],
				};

				let mut decrypted_data = self.decrypt_last(payload).map_err(|_| Error::Decrypt)?;

				// Zeroize buffer on write error
				writer.write_all(&decrypted_data).map_err(|e| {
					decrypted_data.zeroize();
					Error::Io(e)
				})?;

				// decrypted_data.zeroize();

				break;
			}
		}

		writer.flush().map_err(Error::Io)?;

		Ok(())
	}

	/// This should ideally only be used for small amounts of data
	///
	/// It is just a thin wrapper around `decrypt_streams()`, but reduces the amount of code needed elsewhere.
	#[allow(unused_mut)]
	pub fn decrypt_bytes(
		key: Protected<[u8; 32]>,
		nonce: &[u8],
		algorithm: Algorithm,
		bytes: &[u8],
		aad: &[u8],
	) -> Result<Protected<Vec<u8>>> {
		let mut writer = Cursor::new(Vec::<u8>::new());

		let decryptor = Self::new(key, nonce, algorithm)?;

		match decryptor.decrypt_streams(bytes, &mut writer, aad) {
			Ok(_) => Ok(Protected::new(writer.into_inner())),
			Err(e) => Err(e),
		}
	}
}
