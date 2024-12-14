//! Encryption and decryption functionality for cookie strings using the `ChaCha20Poly1305` AEAD cipher.
//!
//! This module provides a secure way to encrypt and decrypt cookie data using the
//! `ChaCha20Poly1305` authenticated encryption algorithm. It includes functionality for:
//! - Key generation from UUIDs
//! - Encryption with random nonces
//! - Decryption with authentication
//! - Base64 encoding/decoding utilities

use base64::Engine;
use blake3;
use chacha20poly1305::{
	aead::{Aead, AeadCore, KeyInit},
	ChaCha20Poly1305, Key,
};
use std::convert::TryFrom;
use tracing::{debug, error};

/// Main struct for handling encryption and decryption operations.
/// Contains an initialized `ChaCha20Poly1305` cipher instance.
#[derive(Clone)]
pub struct CookieCipher {
	cipher: ChaCha20Poly1305,
}

/// Possible errors that can occur during cryptographic operations.
#[derive(Debug, thiserror::Error)]
pub enum CryptoCookieError {
	/// Errors that occur during encryption operations
	#[error("Encryption failed: {0}")]
	Encryption(String),
	/// Errors that occur during decryption operations
	#[error("Decryption failed: {0}")]
	Decryption(String),
	/// Errors that occur during key creation/initialization
	#[error("Key creation failed: {0}")]
	KeyCreation(String),
}

impl CookieCipher {
	/// Creates a new `CookieCipher` instance with the provided 32-byte key.
	///
	/// # Arguments
	/// * `key` - A 32-byte array used as the encryption/decryption key
	///
	/// # Returns
	/// * `Result<Self, CryptoCookieError>` - A new `CookieCipher` instance or an error
	pub fn new(key: &[u8; 32]) -> Result<Self, CryptoCookieError> {
		debug!("Initializing CookieCipher with provided key");
		let key = Key::try_from(key.as_slice()).map_err(|e| {
			error!("Failed to create key: {}", e);
			CryptoCookieError::KeyCreation(e.to_string())
		})?;

		let cipher = ChaCha20Poly1305::new(&key);
		debug!("CookieCipher initialized successfully");
		Ok(Self { cipher })
	}

	/// Encrypts the provided data using `ChaCha20Poly1305`.
	///
	/// # Arguments
	/// * `data` - The data to encrypt
	///
	/// # Returns
	/// * `Result<Vec<u8>, CryptoCookieError>` - The encrypted data with prepended nonce, or an error
	pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoCookieError> {
		debug!("Starting encryption of {} bytes", data.len());

		let nonce = ChaCha20Poly1305::generate_nonce_with_rng(&mut aead::OsRng).map_err(|e| {
			error!("Nonce generation failed: {}", e);
			CryptoCookieError::Encryption(e.to_string())
		})?;
		debug!("Generated new nonce for encryption");

		let ciphertext = self.cipher.encrypt(&nonce, data).map_err(|e| {
			error!("Encryption failed: {}", e);
			CryptoCookieError::Encryption(e.to_string())
		})?;

		let mut combined = nonce.to_vec();
		combined.extend(ciphertext);

		debug!("Successfully encrypted data to {} bytes", combined.len());
		Ok(combined)
	}

	/// Validates that the encrypted data meets the minimum length requirement.
	///
	/// # Arguments
	/// * `data` - The encrypted data to validate
	///
	/// # Returns
	/// * `Result<(), CryptoCookieError>` - Ok if valid, Error if too short
	fn validate_data_length(data: &[u8]) -> Result<(), CryptoCookieError> {
		if data.len() < 12 {
			error!("Encrypted data too short: {} bytes", data.len());
			return Err(CryptoCookieError::Decryption("Data too short".into()));
		}
		Ok(())
	}

	/// Extracts and validates the nonce from the encrypted data.
	///
	/// # Arguments
	/// * `nonce_bytes` - The bytes containing the nonce
	///
	/// # Returns
	/// * `Result<chacha20poly1305::Nonce, CryptoCookieError>` - The extracted nonce or an error
	fn extract_nonce(nonce_bytes: &[u8]) -> Result<chacha20poly1305::Nonce, CryptoCookieError> {
		chacha20poly1305::Nonce::try_from(nonce_bytes).map_err(|e| {
			error!("Failed to create nonce: {}", e);
			CryptoCookieError::Decryption(e.to_string())
		})
	}

	/// Performs the actual decryption operation.
	///
	/// # Arguments
	/// * `nonce` - The nonce to use for decryption
	/// * `ciphertext` - The encrypted data to decrypt
	///
	/// # Returns
	/// * `Result<Vec<u8>, CryptoCookieError>` - The decrypted data or an error
	fn perform_decryption(
		&self,
		nonce: &chacha20poly1305::Nonce,
		ciphertext: &[u8],
	) -> Result<Vec<u8>, CryptoCookieError> {
		self.cipher.decrypt(nonce, ciphertext).map_err(|e| {
			error!("Decryption failed: {}", e);
			CryptoCookieError::Decryption(e.to_string())
		})
	}

	/// Decrypts the provided encrypted data.
	///
	/// # Arguments
	/// * `encrypted_data` - The data to decrypt (including nonce)
	///
	/// # Returns
	/// * `Result<Vec<u8>, CryptoCookieError>` - The decrypted data or an error
	pub fn decrypt(&self, encrypted_data: &[u8]) -> Result<Vec<u8>, CryptoCookieError> {
		debug!("Starting decryption of {} bytes", encrypted_data.len());

		Self::validate_data_length(encrypted_data)?;

		let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
		let nonce = Self::extract_nonce(nonce_bytes)?;

		debug!("Extracted nonce and ciphertext for decryption");

		let plaintext = self.perform_decryption(&nonce, ciphertext)?;

		debug!("Successfully decrypted data to {} bytes", plaintext.len());
		Ok(plaintext)
	}

	/// Generates a 32-byte key from a string input using BLAKE3 hashing.
	///
	/// # Arguments
	/// * `string` - The input string (typically a UUID) to generate the key from
	///
	/// # Returns
	/// * `Result<[u8; 32], CryptoCookieError>` - A 32-byte key or an error
	pub fn generate_key_from_string(string: &str) -> Result<[u8; 32], CryptoCookieError> {
		debug!("Generating key from string: {}", string);

		if string.is_empty() {
			error!("Input string is empty");
			return Err(CryptoCookieError::KeyCreation(
				"Input string is empty".into(),
			));
		}

		// Hash the input string to get a fixed-size output
		let hash = blake3::hash(string.as_bytes());

		// Convert the hash bytes directly to an array
		let key_array: [u8; 32] = *hash.as_bytes();

		debug!("Key generated successfully");
		Ok(key_array)
	}

	/// Encodes binary data to base64 string.
	///
	/// # Arguments
	/// * `data` - The binary data to encode
	///
	/// # Returns
	/// * `String` - The base64 encoded string
	#[must_use]
	pub fn base64_encode(data: &[u8]) -> String {
		base64::engine::general_purpose::STANDARD.encode(data)
	}

	/// Decodes base64 string to binary data.
	///
	/// # Arguments
	/// * `data` - The base64 string to decode
	///
	/// # Returns
	/// * `Result<Vec<u8>, base64::DecodeError>` - The decoded binary data or an error
	pub fn base64_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
		base64::engine::general_purpose::STANDARD.decode(data)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_key_generation() {
		let key = CookieCipher::generate_key_from_string("0193b34e-0ad9-70e0-a3dd-8ec30b73a90a")
			.expect("Failed to generate key");

		assert_eq!(key.len(), 32);
	}

	#[test]
	fn test_encryption_decryption() {
		let key = CookieCipher::generate_key_from_string("0193b34e-0ad9-70e0-a3dd-8ec30b73a90a")
			.expect("Failed to generate key");
		let cipher = CookieCipher::new(&key).expect("Failed to create cipher");

		let data = b"Hello, world!";
		let encrypted = cipher.encrypt(data).expect("Failed to encrypt data");
		let decrypted = cipher.decrypt(&encrypted).expect("Failed to decrypt data");

		assert_eq!(data, decrypted.as_slice());
	}

	#[test]
	fn test_base64_encoding() {
		let data = b"Hello, world!";
		let encoded = CookieCipher::base64_encode(data);
		let decoded = CookieCipher::base64_decode(&encoded).expect("Failed to decode base64");

		assert_eq!(data, decoded.as_slice());
	}

	#[test]
	fn test_invalid_data_length() {
		let key = CookieCipher::generate_key_from_string("0193b34e-0ad9-70e0-a3dd-8ec30b73a90a")
			.expect("Failed to generate key");
		let cipher = CookieCipher::new(&key).expect("Failed to create cipher");

		let encrypted = vec![0; 10];
		let result = cipher.decrypt(&encrypted);

		assert!(result.is_err());
	}

	#[test]
	fn test_invalid_nonce() {
		let key = CookieCipher::generate_key_from_string("0193b34e-0ad9-70e0-a3dd-8ec30b73a90a")
			.expect("Failed to generate key");
		let cipher = CookieCipher::new(&key).expect("Failed to create cipher");

		let encrypted = vec![0; 12];
		let result = cipher.decrypt(&encrypted);

		assert!(result.is_err());
	}

	#[test]
	fn test_invalid_decryption() {
		let key = CookieCipher::generate_key_from_string("0193b34e-0ad9-70e0-a3dd-8ec30b73a90a")
			.expect("Failed to generate key");
		let cipher = CookieCipher::new(&key).expect("Failed to create cipher");

		let encrypted = vec![0; 24];
		let result = cipher.decrypt(&encrypted);

		assert!(result.is_err());
	}

	#[test]
	fn test_invalid_base64() {
		let result = CookieCipher::base64_decode("invalid_base64");

		assert!(result.is_err());
	}

	#[test]
	fn test_invalid_key_generation() {
		let result = CookieCipher::generate_key_from_string("");

		assert!(result.is_err());
	}

	#[test]
	fn test_invalid_decryption_operation() {
		let key = CookieCipher::generate_key_from_string("0193b34e-0ad9-70e0-a3dd-8ec30b73a90a")
			.expect("Failed to generate key");
		let cipher = CookieCipher::new(&key).expect("Failed to create cipher");

		let nonce = chacha20poly1305::Nonce::default();
		let ciphertext = vec![0; 1024];
		let result = cipher.perform_decryption(&nonce, &ciphertext);

		assert!(result.is_err());
	}
}
