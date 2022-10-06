use crate::protected::Protected;
use aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;

use crate::{error::Error, primitives::Algorithm};

// Although these two objects are identical, I think it'll be good practice to keep their usage separate.
// One for encryption, and one for decryption. This can easily be changed if needed.
pub enum MemoryEncryption {
	XChaCha20Poly1305(Box<XChaCha20Poly1305>),
	Aes256Gcm(Box<Aes256Gcm>),
}

pub enum MemoryDecryption {
	XChaCha20Poly1305(Box<XChaCha20Poly1305>),
	Aes256Gcm(Box<Aes256Gcm>),
}

impl MemoryEncryption {
	pub fn new(key: Protected<[u8; 32]>, algorithm: Algorithm) -> Result<Self, Error> {
		let encryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
					.map_err(|_| Error::MemoryModeInit)?;
					
				Self::XChaCha20Poly1305(Box::new(cipher))
			}
			Algorithm::Aes256Gcm => {
				let cipher =
					Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::MemoryModeInit)?;

				Self::Aes256Gcm(Box::new(cipher))
			}
		};

		Ok(encryption_object)
	}

	pub fn encrypt<'msg, 'aad>(
		&self,
		plaintext: impl Into<Payload<'msg, 'aad>>,
		nonce: &[u8],
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(m) => m.encrypt(nonce.into(), plaintext),
			Self::Aes256Gcm(m) => m.encrypt(nonce.into(), plaintext),
		}
	}
}

impl MemoryDecryption {
	pub fn new(key: Protected<[u8; 32]>, algorithm: Algorithm) -> Result<Self, Error> {
		let decryption_object = match algorithm {
			Algorithm::XChaCha20Poly1305 => {
				let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
					.map_err(|_| Error::MemoryModeInit)?;

				Self::XChaCha20Poly1305(Box::new(cipher))
			}
			Algorithm::Aes256Gcm => {
				let cipher =
					Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::MemoryModeInit)?;

				Self::Aes256Gcm(Box::new(cipher))
			}
		};

		Ok(decryption_object)
	}

	pub fn decrypt<'msg, 'aad>(
		&self,
		ciphertext: impl Into<Payload<'msg, 'aad>>,
		nonce: &[u8],
	) -> aead::Result<Vec<u8>> {
		match self {
			Self::XChaCha20Poly1305(m) => m.decrypt(nonce.into(), ciphertext),
			Self::Aes256Gcm(m) => m.decrypt(nonce.into(), ciphertext),
		}
	}
}
