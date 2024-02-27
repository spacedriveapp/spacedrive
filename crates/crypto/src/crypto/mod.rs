//! This module contains all encryption and decryption items. These are used throughout the crate for all encryption/decryption needs.

mod stream;

pub use self::stream::{Decryptor, Encryptor};

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use crate::{
		crypto::{Decryptor, Encryptor},
		primitives::{
			AAD_LEN, AEAD_TAG_LEN, AES_256_GCM_SIV_NONCE_LEN, BLOCK_LEN, KEY_LEN,
			XCHACHA20_POLY1305_NONCE_LEN,
		},
		rng::CryptoRng,
		types::{Aad, Algorithm, EncryptedKey, Key, Nonce},
	};

	// const KEY: Key = Key::new([0x23; KEY_LEN]);

	const XCHACHA20_POLY1305_NONCE: Nonce =
		Nonce::XChaCha20Poly1305([0xE9; XCHACHA20_POLY1305_NONCE_LEN]);

	const AES_256_GCM_SIV_NONCE: Nonce = Nonce::Aes256GcmSiv([0xE9; AES_256_GCM_SIV_NONCE_LEN]);

	const PLAINTEXT: [u8; 32] = [0x5A; 32];
	// const PLAINTEXT_KEY: Key = Key::new([1u8; KEY_LEN]);

	const AAD: Aad = Aad::Standard([0x92; AAD_LEN]);

	// for the `const` arrays below, [0] is without AAD, [1] is with AAD

	const AES_256_GCM_SIV_BYTES_EXPECTED: [[u8; 48]; 2] = [
		[
			41, 231, 183, 92, 73, 104, 69, 207, 245, 250, 21, 50, 145, 41, 104, 165, 130, 59, 70,
			185, 65, 77, 215, 15, 131, 214, 183, 47, 166, 223, 185, 181, 117, 138, 62, 204, 246,
			227, 198, 32, 132, 5, 97, 120, 15, 70, 229, 218,
		],
		[
			3, 180, 75, 64, 231, 67, 228, 189, 149, 69, 47, 83, 8, 214, 103, 12, 21, 11, 39, 108,
			7, 142, 10, 169, 85, 163, 76, 53, 53, 69, 160, 134, 2, 87, 72, 121, 75, 186, 102, 176,
			163, 170, 81, 101, 242, 237, 173, 133,
		],
	];

	const XCHACHA20_POLY1305_BYTES_EXPECTED: [[u8; 48]; 2] = [
		[
			35, 174, 252, 59, 215, 65, 5, 237, 198, 2, 51, 72, 239, 88, 36, 177, 136, 252, 64, 157,
			141, 53, 138, 98, 185, 2, 75, 173, 253, 99, 133, 207, 145, 54, 100, 51, 44, 230, 60, 5,
			157, 70, 110, 145, 166, 41, 215, 95,
		],
		[
			35, 174, 252, 59, 215, 65, 5, 237, 198, 2, 51, 72, 239, 88, 36, 177, 136, 252, 64, 157,
			141, 53, 138, 98, 185, 2, 75, 173, 253, 99, 133, 207, 125, 139, 247, 158, 207, 216, 60,
			114, 72, 44, 6, 212, 233, 141, 251, 239,
		],
	];

	const XCHACHA20_POLY1305_ENCRYPTED_KEY: EncryptedKey = EncryptedKey::new(
		[
			120, 245, 167, 96, 140, 26, 94, 182, 157, 89, 104, 19, 180, 3, 127, 234, 211, 167, 27,
			198, 214, 110, 209, 57, 226, 89, 16, 246, 166, 56, 222, 148, 40, 198, 237, 205, 45, 49,
			205, 18, 69, 102, 16, 78, 199, 141, 246, 165,
		],
		XCHACHA20_POLY1305_NONCE,
	);

	const AES_256_GCM_ENCRYPTED_KEY: EncryptedKey = EncryptedKey::new(
		[
			227, 231, 27, 182, 122, 118, 64, 35, 125, 176, 152, 244, 156, 26, 234, 96, 178, 121,
			73, 213, 228, 189, 45, 152, 189, 68, 214, 187, 123, 182, 91, 83, 216, 50, 174, 13, 157,
			121, 165, 129, 227, 220, 139, 166, 9, 71, 215, 145,
		],
		AES_256_GCM_SIV_NONCE,
	);

	#[test]
	fn aes_256_gcm_siv_encrypt_bytes() {
		let output = Encryptor::encrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, AES_256_GCM_SIV_BYTES_EXPECTED[0]);
	}

	#[test]
	fn aes_256_gcm_siv_encrypt_bytes_with_aad() {
		let output = Encryptor::encrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&PLAINTEXT,
			AAD,
		)
		.unwrap();

		assert_eq!(output, AES_256_GCM_SIV_BYTES_EXPECTED[1]);
	}

	#[test]
	fn aes_256_gcm_siv_decrypt_bytes() {
		let output = Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&AES_256_GCM_SIV_BYTES_EXPECTED[0],
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output.expose(), &PLAINTEXT);
	}

	#[test]
	fn aes_256_gcm_siv_decrypt_bytes_with_aad() {
		let output = Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&AES_256_GCM_SIV_BYTES_EXPECTED[1],
			AAD,
		)
		.unwrap();

		assert_eq!(output.expose(), &PLAINTEXT);
	}

	#[test]
	fn aes_256_gcm_siv_encrypt_key() {
		let output = Encryptor::encrypt_key(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&Key::new([1u8; KEY_LEN]),
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, AES_256_GCM_ENCRYPTED_KEY);
	}

	#[test]
	fn aes_256_gcm_siv_decrypt_key() {
		let output = Decryptor::decrypt_key(
			&Key::new([0x23; KEY_LEN]),
			Algorithm::Aes256GcmSiv,
			&AES_256_GCM_ENCRYPTED_KEY,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, Key::new([1u8; KEY_LEN]));
	}

	#[test]
	fn aes_256_gcm_siv_encrypt_tiny() {
		let output = Encryptor::encrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, AES_256_GCM_SIV_BYTES_EXPECTED[0]);
	}

	#[test]
	fn aes_256_gcm_siv_decrypt_tiny() {
		let output = Decryptor::decrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&AES_256_GCM_SIV_BYTES_EXPECTED[0],
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output.expose(), &PLAINTEXT);
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn aes_256_gcm_siv_encrypt_tiny_too_large() {
		Encryptor::encrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&vec![0u8; BLOCK_LEN],
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn aes_256_gcm_siv_decrypt_tiny_too_large() {
		Decryptor::decrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&vec![0u8; BLOCK_LEN + AEAD_TAG_LEN],
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "Decrypt")]
	fn aes_256_gcm_siv_decrypt_bytes_missing_aad() {
		Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
			&AES_256_GCM_SIV_BYTES_EXPECTED[1],
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn aes_256_gcm_siv_encrypt_and_decrypt_5_blocks() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	#[ignore]
	fn aes_256_gcm_siv_encrypt_and_decrypt_128mib() {
		let buf = vec![1u8; BLOCK_LEN * 128].into_boxed_slice();

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let output = writer.into_inner().into_boxed_slice();

		assert_eq!(buf, output);
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn aes_256_gcm_siv_encrypt_and_decrypt_5_blocks_with_aad() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, AAD)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, AAD)
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn aes_256_gcm_siv_encrypt_and_decrypt_5_blocks_async() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		encryptor
			.encrypt_streams_async(&mut reader, &mut writer, Aad::Null)
			.await
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		decryptor
			.decrypt_streams_async(&mut reader, &mut writer, Aad::Null)
			.await
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn aes_256_gcm_siv_encrypt_and_decrypt_5_blocks_with_aad_async() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		encryptor
			.encrypt_streams_async(&mut reader, &mut writer, AAD)
			.await
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::Aes256GcmSiv,
		)
		.unwrap();

		decryptor
			.decrypt_streams_async(&mut reader, &mut writer, AAD)
			.await
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	fn xchacha20_poly1305_encrypt_bytes() {
		let output = Encryptor::encrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, XCHACHA20_POLY1305_BYTES_EXPECTED[0]);
	}

	#[test]
	fn xchacha20_poly1305_encrypt_key() {
		let output = Encryptor::encrypt_key(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&Key::new([1u8; KEY_LEN]),
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, XCHACHA20_POLY1305_ENCRYPTED_KEY);
	}

	#[test]
	fn xchacha20_poly1305_decrypt_key() {
		let output = Decryptor::decrypt_key(
			&Key::new([0x23; KEY_LEN]),
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_ENCRYPTED_KEY,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, Key::new([1u8; KEY_LEN]));
	}

	#[test]
	fn xchacha20_poly1305_encrypt_tiny() {
		let output = Encryptor::encrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, XCHACHA20_POLY1305_BYTES_EXPECTED[0]);
	}

	#[test]
	fn xchacha20_poly1305_decrypt_tiny() {
		let output = Decryptor::decrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_BYTES_EXPECTED[0],
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output.expose(), &PLAINTEXT);
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn xchacha20_poly1305_encrypt_tiny_too_large() {
		Encryptor::encrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&vec![0u8; BLOCK_LEN],
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn xchacha20_poly1305_decrypt_tiny_too_large() {
		Decryptor::decrypt_tiny(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&vec![0u8; BLOCK_LEN + AEAD_TAG_LEN],
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	fn xchacha20_poly1305_encrypt_bytes_with_aad() {
		let output = Encryptor::encrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			AAD,
		)
		.unwrap();

		assert_eq!(output, XCHACHA20_POLY1305_BYTES_EXPECTED[1]);
	}

	#[test]
	fn xchacha20_poly1305_decrypt_bytes() {
		let output = Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_BYTES_EXPECTED[0],
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output.expose(), &PLAINTEXT);
	}

	#[test]
	fn xchacha20_poly1305_decrypt_bytes_with_aad() {
		let output = Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_BYTES_EXPECTED[1],
			AAD,
		)
		.unwrap();

		assert_eq!(output.expose(), &PLAINTEXT);
	}

	#[test]
	#[should_panic(expected = "Decrypt")]
	fn xchacha20_poly1305_decrypt_bytes_missing_aad() {
		Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_BYTES_EXPECTED[1],
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn xchacha20_poly1305_encrypt_and_decrypt_5_blocks() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	#[ignore]
	fn xchacha20_poly1305_encrypt_and_decrypt_128mib() {
		let buf = vec![1u8; BLOCK_LEN * 128].into_boxed_slice();

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, Aad::Null)
			.unwrap();

		let output = writer.into_inner().into_boxed_slice();

		assert_eq!(buf, output);
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn xchacha20_poly1305_encrypt_and_decrypt_5_blocks_with_aad() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		encryptor
			.encrypt_streams(&mut reader, &mut writer, AAD)
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, AAD)
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn xchacha20_poly1305_encrypt_and_decrypt_5_blocks_async() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		encryptor
			.encrypt_streams_async(&mut reader, &mut writer, Aad::Null)
			.await
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		decryptor
			.decrypt_streams_async(&mut reader, &mut writer, Aad::Null)
			.await
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn xchacha20_poly1305_encrypt_and_decrypt_5_blocks_with_aad_async() {
		let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

		let mut reader = Cursor::new(&buf);
		let mut writer = Cursor::new(Vec::new());

		let encryptor = Encryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		encryptor
			.encrypt_streams_async(&mut reader, &mut writer, AAD)
			.await
			.unwrap();

		let mut reader = Cursor::new(writer.into_inner());
		let mut writer = Cursor::new(Vec::new());

		let decryptor = Decryptor::new(
			&Key::new([0x23; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
		)
		.unwrap();

		decryptor
			.decrypt_streams_async(&mut reader, &mut writer, AAD)
			.await
			.unwrap();

		let output = writer.into_inner();

		assert_eq!(buf, output);
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn encrypt_with_invalid_nonce() {
		Encryptor::encrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn encrypt_with_null_nonce() {
		Encryptor::encrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&Nonce::XChaCha20Poly1305([0u8; 20]),
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn encrypt_with_null_key() {
		Encryptor::encrypt_bytes(
			&Key::new([0u8; KEY_LEN]),
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT,
			Aad::Null,
		)
		.unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn decrypt_with_invalid_nonce() {
		Decryptor::decrypt_bytes(
			&Key::new([0x23; KEY_LEN]),
			&AES_256_GCM_SIV_NONCE,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_BYTES_EXPECTED[0],
			Aad::Null,
		)
		.unwrap();
	}
}
