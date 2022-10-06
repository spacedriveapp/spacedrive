use std::io::{Read, Seek, Write};

use aead::{
	stream::{DecryptorLE31, EncryptorLE31},
	KeyInit, Payload,
};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use zeroize::Zeroize;

use crate::{
	error::Error,
	primitives::{Algorithm, Mode, BLOCK_SIZE},
	protected::Protected,
};

pub enum StreamEncryption {
	XChaCha20Poly1305(Box<EncryptorLE31<XChaCha20Poly1305>>),
	Aes256Gcm(Box<EncryptorLE31<Aes256Gcm>>),
}

pub enum StreamDecryption {
	Aes256Gcm(Box<DecryptorLE31<Aes256Gcm>>),
	XChaCha20Poly1305(Box<DecryptorLE31<XChaCha20Poly1305>>),
}

impl StreamEncryption {
	pub fn new(
		key: Protected<[u8; 32]>,
		nonce: &[u8],
		algorithm: Algorithm,
	) -> Result<Self, Error> {
		if nonce.len() != algorithm.nonce_len(Mode::Stream) {
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

	// This should be used for every block, except the final block
	pub fn encrypt_next<'msg, 'aad>(
		&mut self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.encrypt_next(payload),
			Self::Aes256Gcm(s) => s.encrypt_next(payload),
		}
	}

	// This should be used to encrypt the final block of data
	// This takes ownership of `self` to prevent usage after finalization
	pub fn encrypt_last<'msg, 'aad>(
		self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.encrypt_last(payload),
			Self::Aes256Gcm(s) => s.encrypt_last(payload),
		}
	}

	pub fn encrypt_streams<R, W>(
		mut self,
		mut reader: R,
		mut writer: W,
		aad: &[u8],
	) -> Result<(), Error>
	where
		R: Read + Seek,
		W: Write + Seek,
	{
		let mut read_buffer = vec![0u8; BLOCK_SIZE];
		let read_count = reader.read(&mut read_buffer).map_err(Error::Io)?;
		if read_count == BLOCK_SIZE {
			let payload = Payload {
				aad,
				msg: &read_buffer,
			};

			let encrypted_data = self.encrypt_next(payload).map_err(|_| Error::Encrypt)?;

			// zeroize before writing, so any potential errors won't result in a potential data leak
			read_buffer.zeroize();

			// Using `write` instead of `write_all` so we can check the amount of bytes written
			let write_count = writer.write(&encrypted_data).map_err(Error::Io)?;

			if read_count != write_count - 16 {
				// -16 to account for the AEAD tag
				return Err(Error::WriteMismatch);
			}
		} else {
			// we use `..read_count` in order to only use the read data, and not zeroes also
			let payload = Payload {
				aad,
				msg: &read_buffer[..read_count],
			};

			let encrypted_data = self.encrypt_last(payload).map_err(|_| Error::Encrypt)?;

			// zeroize before writing, so any potential errors won't result in a potential data leak
			read_buffer.zeroize();

			// Using `write` instead of `write_all` so we can check the amount of bytes written
			let write_count = writer.write(&encrypted_data).map_err(Error::Io)?;

			if read_count != write_count - 16 {
				// -16 to account for the AEAD tag
				return Err(Error::WriteMismatch);
			}
		}

		writer.flush().map_err(Error::Io)?;

		Ok(())
	}
}

impl StreamDecryption {
	pub fn new(
		key: Protected<[u8; 32]>,
		nonce: &[u8],
		algorithm: Algorithm,
	) -> Result<Self, Error> {
		if nonce.len() != algorithm.nonce_len(Mode::Stream) {
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

	// This should be used for every block, except the final block
	pub fn decrypt_next<'msg, 'aad>(
		&mut self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.decrypt_next(payload),
			Self::Aes256Gcm(s) => s.decrypt_next(payload),
		}
	}

	// This should be used to decrypt the final block of data
	// This takes ownership of `self` to prevent usage after finalization
	pub fn decrypt_last<'msg, 'aad>(
		self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(s) => s.decrypt_last(payload),
			Self::Aes256Gcm(s) => s.decrypt_last(payload),
		}
	}

	pub fn decrypt_streams<R, W>(
		mut self,
		mut reader: R,
		mut writer: W,
		aad: &[u8],
	) -> Result<(), Error>
	where
		R: Read + Seek,
		W: Write + Seek,
	{
		let mut read_buffer = vec![0u8; BLOCK_SIZE];
		let read_count = reader.read(&mut read_buffer).map_err(Error::Io)?;
		if read_count == (BLOCK_SIZE + 16) {
			let payload = Payload {
				aad,
				msg: &read_buffer,
			};

			let mut decrypted_data = self.decrypt_next(payload).map_err(|_| Error::Decrypt)?;

			// Using `write` instead of `write_all` so we can check the amount of bytes written
			let write_count = writer.write(&decrypted_data).map_err(Error::Io)?;

			// zeroize before writing, so any potential errors won't result in a potential data leak
			decrypted_data.zeroize();

			if read_count - 16 != write_count {
				// -16 to account for the AEAD tag
				return Err(Error::WriteMismatch);
			}
		} else {
			let payload = Payload {
				aad,
				msg: &read_buffer[..read_count],
			};

			let mut decrypted_data = self.decrypt_last(payload).map_err(|_| Error::Decrypt)?;

			// Using `write` instead of `write_all` so we can check the amount of bytes written
			let write_count = writer.write(&decrypted_data).map_err(Error::Io)?;

			// zeroize before writing, so any potential errors won't result in a potential data leak
			decrypted_data.zeroize();

			if read_count - 16 != write_count {
				// -16 to account for the AEAD tag
				return Err(Error::WriteMismatch);
			}
		}

		writer.flush().map_err(Error::Io)?;

		Ok(())
	}
}
