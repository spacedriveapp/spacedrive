//! This module contains all encryption and decryption items. These are used throughout the crate for all encryption/decryption needs.

use crate::Result;

mod stream;

pub use self::stream::{Decryptor, Encryptor};

/// This is used to exhaustively read from an asynchronous reader into a buffer.
///
/// This function returns on three possible conditions, and they are:
///
/// - when the reader has been exhausted (reaches EOF)
/// - when the buffer has been entirely populated
/// - when an error has been generated
///
/// It returns the amount of total bytes read, which will be <= the buffer's size.
fn exhaustive_read<R>(reader: &mut R, buffer: &mut [u8]) -> Result<usize>
where
	R: std::io::Read,
{
	let mut read_count = 0;

	loop {
		let i = reader.read(&mut buffer[read_count..])?;
		read_count += i;
		if i == 0 || read_count == buffer.len() {
			// if we're EOF or the buffer is filled
			break Ok(read_count);
		}
	}
}

/// This is used to exhaustively read from an asynchronous reader into a buffer.
///
/// This function returns on three possible conditions, and they are:
///
/// - when the reader has been exhausted (reaches EOF)
/// - when the buffer has been entirely populated
/// - when an error has been generated
///
/// It returns the amount of total bytes read, which will be <= the buffer's size.
#[cfg(feature = "async")]
async fn exhaustive_read_async<R>(reader: &mut R, buffer: &mut [u8]) -> Result<usize>
where
	R: tokio::io::AsyncReadExt + Unpin + Send,
{
	let mut read_count = 0;

	loop {
		let i = reader.read(&mut buffer[read_count..]).await?;
		read_count += i;
		if i == 0 || read_count == buffer.len() {
			// if we're EOF or the buffer is filled
			break Ok(read_count);
		}
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use std::io::Cursor;

	use rand::{RngCore, SeedableRng};
	use rand_chacha::ChaCha20Rng;

	use crate::{
		primitives::{BLOCK_LEN, ENCRYPTED_KEY_LEN, KEY_LEN},
		types::{Algorithm, EncryptedKey, Key, Nonce},
	};

	use super::*;

	const KEY: Key = Key::new([
		0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23,
		0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23,
		0x23, 0x23,
	]);

	const AES_NONCE: Nonce = Nonce::Aes256Gcm([0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9]);
	const XCHACHA_NONCE: Nonce = Nonce::XChaCha20Poly1305([
		0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9, 0xE9,
		0xE9, 0xE9, 0xE9, 0xE9, 0xE9,
	]);

	const PLAINTEXT: [u8; 32] = [
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A,
	];

	const AAD: [u8; 16] = [
		0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92, 0x92,
		0x92,
	];

	// for the `const` arrays below, [0] is without AAD, [1] is with AAD

	const AES_BYTES_EXPECTED: [[u8; 48]; 2] = [
		[
			38, 96, 235, 51, 131, 187, 162, 152, 183, 13, 174, 87, 108, 113, 198, 88, 106, 121,
			208, 37, 20, 10, 2, 107, 69, 147, 171, 141, 46, 255, 181, 123, 24, 150, 104, 25, 70,
			198, 169, 232, 124, 99, 151, 226, 84, 113, 184, 134,
		],
		[
			38, 96, 235, 51, 131, 187, 162, 152, 183, 13, 174, 87, 108, 113, 198, 88, 106, 121,
			208, 37, 20, 10, 2, 107, 69, 147, 171, 141, 46, 255, 181, 123, 172, 121, 35, 145, 71,
			115, 203, 224, 20, 183, 1, 99, 223, 230, 255, 76,
		],
	];

	const XCHACHA_BYTES_EXPECTED: [[u8; 48]; 2] = [
		[
			35, 174, 252, 59, 215, 65, 5, 237, 198, 2, 51, 72, 239, 88, 36, 177, 136, 252, 64, 157,
			141, 53, 138, 98, 185, 2, 75, 173, 253, 99, 133, 207, 145, 54, 100, 51, 44, 230, 60, 5,
			157, 70, 110, 145, 166, 41, 215, 95,
		],
		[
			35, 174, 252, 59, 215, 65, 5, 237, 198, 2, 51, 72, 239, 88, 36, 177, 136, 252, 64, 157,
			141, 53, 138, 98, 185, 2, 75, 173, 253, 99, 133, 207, 110, 4, 255, 118, 55, 88, 24,
			170, 101, 74, 104, 122, 105, 216, 225, 243,
		],
	];

	const PLAINTEXT_KEY: [u8; KEY_LEN] = [1u8; KEY_LEN];

	const XCHACHA_ENCRYPTED_KEY: EncryptedKey = EncryptedKey::new([
		120, 245, 167, 96, 140, 26, 94, 182, 157, 89, 104, 19, 180, 3, 127, 234, 211, 167, 27, 198,
		214, 110, 209, 57, 226, 89, 16, 246, 166, 56, 222, 148, 40, 198, 237, 205, 45, 49, 205, 18,
		69, 102, 16, 78, 199, 141, 246, 165,
	]);

	const AES_ENCRYPTED_KEY: EncryptedKey = EncryptedKey::new([
		125, 59, 176, 104, 216, 224, 249, 195, 236, 86, 245, 12, 55, 42, 157, 3, 49, 34, 139, 126,
		79, 81, 89, 48, 30, 200, 240, 214, 117, 164, 238, 32, 6, 159, 3, 111, 114, 28, 176, 224,
		187, 185, 123, 20, 164, 197, 171, 31,
	]);

	#[test]
	fn aes_encrypt_bytes() {
		let ciphertext =
			Encryptor::encrypt_bytes(KEY, AES_NONCE, Algorithm::Aes256Gcm, &PLAINTEXT, &[])
				.unwrap();

		assert_eq!(AES_BYTES_EXPECTED[0].to_vec(), ciphertext);
	}

	#[test]
	fn aes_encrypt_bytes_with_aad() {
		let ciphertext =
			Encryptor::encrypt_bytes(KEY, AES_NONCE, Algorithm::Aes256Gcm, &PLAINTEXT, &AAD)
				.unwrap();

		assert_eq!(AES_BYTES_EXPECTED[1].to_vec(), ciphertext);
	}

	#[test]
	fn aes_decrypt_bytes() {
		let plaintext = Decryptor::decrypt_bytes(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			&AES_BYTES_EXPECTED[0],
			&[],
		)
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().clone());
	}

	#[test]
	fn aes_decrypt_bytes_with_aad() {
		let plaintext = Decryptor::decrypt_bytes(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			&AES_BYTES_EXPECTED[1],
			&AAD,
		)
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().clone());
	}

	#[test]
	fn aes_encrypt_key() {
		Encryptor::encrypt_key(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			Key::new(PLAINTEXT_KEY),
			&[],
		)
		.unwrap();
	}

	#[test]
	fn aes_decrypt_key() {
		Decryptor::decrypt_key(KEY, AES_NONCE, Algorithm::Aes256Gcm, AES_ENCRYPTED_KEY, &[])
			.unwrap();
	}

	#[test]
	fn aes_encrypt_fixed() {
		Encryptor::encrypt_fixed::<KEY_LEN, ENCRYPTED_KEY_LEN>(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			&PLAINTEXT_KEY,
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn aes_encrypt_fixed_bad_length() {
		Encryptor::encrypt_fixed::<KEY_LEN, KEY_LEN>(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			&PLAINTEXT_KEY,
			&[],
		)
		.unwrap();
	}

	#[test]
	fn aes_decrypt_fixed() {
		Decryptor::decrypt_fixed::<ENCRYPTED_KEY_LEN, KEY_LEN>(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			AES_ENCRYPTED_KEY.inner(),
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn aes_decrypt_fixed_bad_length() {
		Decryptor::decrypt_fixed::<ENCRYPTED_KEY_LEN, ENCRYPTED_KEY_LEN>(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			AES_ENCRYPTED_KEY.inner(),
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "Decrypt")]
	fn aes_decrypt_bytes_missing_aad() {
		Decryptor::decrypt_bytes(
			KEY,
			AES_NONCE,
			Algorithm::Aes256Gcm,
			&AES_BYTES_EXPECTED[1],
			&[],
		)
		.unwrap();
	}

	#[test]
	fn aes_encrypt_and_decrypt_5_blocks() {
		let mut buf = vec![0u8; BLOCK_LEN * 5];
		ChaCha20Rng::from_entropy().fill_bytes(&mut buf);
		let mut reader = Cursor::new(buf.clone());
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(KEY, AES_NONCE, Algorithm::Aes256Gcm).unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, &[])
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(KEY, AES_NONCE, Algorithm::Aes256Gcm).unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, &[])
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	fn aes_encrypt_and_decrypt_5_blocks_with_aad() {
		let mut buf = vec![0u8; BLOCK_LEN * 5];
		ChaCha20Rng::from_entropy().fill_bytes(&mut buf);
		let mut reader = Cursor::new(buf.clone());
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(KEY, AES_NONCE, Algorithm::Aes256Gcm).unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, &AAD)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(KEY, AES_NONCE, Algorithm::Aes256Gcm).unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, &AAD)
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	fn xchacha_encrypt_bytes() {
		let ciphertext = Encryptor::encrypt_bytes(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&[],
		)
		.unwrap();

		assert_eq!(XCHACHA_BYTES_EXPECTED[0].to_vec(), ciphertext);
	}

	#[test]
	fn xchacha_encrypt_key() {
		Encryptor::encrypt_key(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			Key::new(PLAINTEXT_KEY),
			&[],
		)
		.unwrap();
	}

	#[test]
	fn xchacha_decrypt_key() {
		Decryptor::decrypt_key(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			XCHACHA_ENCRYPTED_KEY,
			&[],
		)
		.unwrap();
	}

	#[test]
	fn xchacha_encrypt_fixed() {
		Encryptor::encrypt_fixed::<KEY_LEN, ENCRYPTED_KEY_LEN>(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT_KEY,
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn xchacha_encrypt_fixed_bad_length() {
		Encryptor::encrypt_fixed::<KEY_LEN, KEY_LEN>(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT_KEY,
			&[],
		)
		.unwrap();
	}

	#[test]
	fn xchacha_decrypt_keyh() {
		Decryptor::decrypt_fixed::<ENCRYPTED_KEY_LEN, KEY_LEN>(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			XCHACHA_ENCRYPTED_KEY.inner(),
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn xchacha_decrypt_key_bad_length() {
		Decryptor::decrypt_fixed::<ENCRYPTED_KEY_LEN, ENCRYPTED_KEY_LEN>(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			XCHACHA_ENCRYPTED_KEY.inner(),
			&[],
		)
		.unwrap();
	}

	#[test]
	fn xchacha_encrypt_bytes_with_aad() {
		let ciphertext = Encryptor::encrypt_bytes(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&AAD,
		)
		.unwrap();

		assert_eq!(XCHACHA_BYTES_EXPECTED[1].to_vec(), ciphertext);
	}

	#[test]
	fn xchacha_decrypt_bytes() {
		let plaintext = Decryptor::decrypt_bytes(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_BYTES_EXPECTED[0],
			&[],
		)
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().clone());
	}

	#[test]
	fn xchacha_decrypt_bytes_with_aad() {
		let plaintext = Decryptor::decrypt_bytes(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_BYTES_EXPECTED[1],
			&AAD,
		)
		.unwrap();

		assert_eq!(PLAINTEXT.to_vec(), plaintext.expose().clone());
	}

	#[test]
	#[should_panic(expected = "Decrypt")]
	fn xchacha_decrypt_bytes_missing_aad() {
		Decryptor::decrypt_bytes(
			KEY,
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_BYTES_EXPECTED[1],
			&[],
		)
		.unwrap();
	}

	#[test]
	fn xchacha_encrypt_and_decrypt_5_blocks() {
		let mut buf = vec![0u8; BLOCK_LEN * 5];
		ChaCha20Rng::from_entropy().fill_bytes(&mut buf);
		let mut reader = Cursor::new(buf.clone());
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(KEY, XCHACHA_NONCE, Algorithm::XChaCha20Poly1305).unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, &[])
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(KEY, XCHACHA_NONCE, Algorithm::XChaCha20Poly1305).unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, &[])
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	fn xchacha_encrypt_and_decrypt_5_blocks_with_aad() {
		let mut buf = vec![0u8; BLOCK_LEN * 5];
		ChaCha20Rng::from_entropy().fill_bytes(&mut buf);
		let mut reader = Cursor::new(buf.clone());
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(KEY, XCHACHA_NONCE, Algorithm::XChaCha20Poly1305).unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, &AAD)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(KEY, XCHACHA_NONCE, Algorithm::XChaCha20Poly1305).unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, &AAD)
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn encrypt_with_invalid_nonce() {
		Encryptor::encrypt_bytes(
			KEY,
			AES_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "NullType")]
	fn encrypt_with_null_nonce() {
		Encryptor::encrypt_bytes(
			KEY,
			Nonce::XChaCha20Poly1305([0u8; 20]),
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "NullType")]
	fn encrypt_with_null_key() {
		Encryptor::encrypt_bytes(
			Key::new([0u8; KEY_LEN]),
			XCHACHA_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			&[],
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn decrypt_with_invalid_nonce() {
		Decryptor::decrypt_bytes(
			KEY,
			AES_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA_BYTES_EXPECTED[0],
			&[],
		)
		.unwrap();
	}
}
