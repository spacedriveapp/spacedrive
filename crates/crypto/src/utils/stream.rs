use aead::{
	stream::{DecryptorLE31, EncryptorLE31},
	KeyInit, Payload,
};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use secrecy::{ExposeSecret, Secret};

use crate::{primitives::{Algorithm, Mode}, error::Error};

pub enum StreamEncryption {
	XChaCha20Poly1305(Box<EncryptorLE31<XChaCha20Poly1305>>),
	Aes256Gcm(Box<EncryptorLE31<Aes256Gcm>>),
}

pub enum StreamDecryption {
	Aes256Gcm(Box<DecryptorLE31<Aes256Gcm>>),
	XChaCha20Poly1305(Box<DecryptorLE31<XChaCha20Poly1305>>),
}

impl StreamEncryption {
	pub fn init(key: Secret<[u8; 32]>, nonce: &[u8], algorithm: Algorithm) -> Result<Self, Error> {
		if nonce.len() != algorithm.nonce_len(Mode::Stream) {
			return Err(Error::NonceLengthMismatch)
		}

		let encryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose_secret()).unwrap();
				drop(key);

				let stream = EncryptorLE31::from_aead(cipher, nonce.into());
				StreamEncryption::XChaCha20Poly1305(Box::new(stream))
			}
			Algorithm::Aes256Gcm => {
				let cipher = Aes256Gcm::new_from_slice(key.expose_secret()).unwrap();
				drop(key);

				let stream = EncryptorLE31::from_aead(cipher, nonce.into());
				StreamEncryption::Aes256Gcm(Box::new(stream))
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
			StreamEncryption::XChaCha20Poly1305(s) => s.encrypt_next(payload),
			StreamEncryption::Aes256Gcm(s) => s.encrypt_next(payload),
		}
	}

	// This should be used to encrypt the final block of data
	// This takes ownership of `self` to prevent usage after finalization
	pub fn encrypt_last<'msg, 'aad>(
		self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			StreamEncryption::XChaCha20Poly1305(s) => s.encrypt_last(payload),
			StreamEncryption::Aes256Gcm(s) => s.encrypt_last(payload),
		}
	}
}

impl StreamDecryption {
	pub fn init(key: Secret<[u8; 32]>, nonce: &[u8], algorithm: Algorithm) -> Result<Self, Error> {
		if nonce.len() != algorithm.nonce_len(Mode::Stream) {
			return Err(Error::NonceLengthMismatch)
		}

		let decryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose_secret()).unwrap();
				drop(key);

				let stream = DecryptorLE31::from_aead(cipher, nonce.into());
				StreamDecryption::XChaCha20Poly1305(Box::new(stream))
			}
			Algorithm::Aes256Gcm => {
				let cipher = Aes256Gcm::new_from_slice(key.expose_secret()).unwrap();
				drop(key);

				let stream = DecryptorLE31::from_aead(cipher, nonce.into());
				StreamDecryption::Aes256Gcm(Box::new(stream))
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
			StreamDecryption::XChaCha20Poly1305(s) => s.decrypt_next(payload),
			StreamDecryption::Aes256Gcm(s) => s.decrypt_next(payload),
		}
	}

	// This should be used to decrypt the final block of data
	// This takes ownership of `self` to prevent usage after finalization
	pub fn decrypt_last<'msg, 'aad>(
		self,
		payload: impl Into<Payload<'msg, 'aad>>,
	) -> aead::Result<Vec<u8>> {
		match self {
			StreamDecryption::XChaCha20Poly1305(s) => s.decrypt_last(payload),
			StreamDecryption::Aes256Gcm(s) => s.decrypt_last(payload),
		}
	}
}
