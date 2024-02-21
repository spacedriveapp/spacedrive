//! This module contains all encryption and decryption items. These are used throughout the crate for all encryption/decryption needs.

mod stream;

pub use self::stream::{Decryptor, Encryptor};

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use crate::{
		crypto::{Decryptor, Encryptor},
		primitives::{AAD_LEN, AEAD_TAG_LEN, BLOCK_LEN, KEY_LEN, XCHACHA20_POLY1305_NONCE_LEN},
		rng::CryptoRng,
		types::{Aad, Algorithm, EncryptedKey, Key, Nonce},
	};

	const KEY: Key = Key::new([0x23; KEY_LEN]);

	const XCHACHA20_POLY1305_NONCE: Nonce =
		Nonce::XChaCha20Poly1305([0xE9; XCHACHA20_POLY1305_NONCE_LEN]);

	const PLAINTEXT: [u8; 32] = [0x5A; 32];
	const PLAINTEXT_KEY: Key = Key::new([1u8; KEY_LEN]);

	const AAD: Aad = Aad::Standard([0x92; AAD_LEN]);

	// for the `const` arrays below, [0] is without AAD, [1] is with AAD

	// const AES_256_GCM_BYTES_EXPECTED: [[u8; 48]; 2] = [
	// 	[
	// 		38, 96, 235, 51, 131, 187, 162, 152, 183, 13, 174, 87, 108, 113, 198, 88, 106, 121,
	// 		208, 37, 20, 10, 2, 107, 69, 147, 171, 141, 46, 255, 181, 123, 24, 150, 104, 25, 70,
	// 		198, 169, 232, 124, 99, 151, 226, 84, 113, 184, 134,
	// 	],
	// 	[
	// 		38, 96, 235, 51, 131, 187, 162, 152, 183, 13, 174, 87, 108, 113, 198, 88, 106, 121,
	// 		208, 37, 20, 10, 2, 107, 69, 147, 171, 141, 46, 255, 181, 123, 43, 30, 101, 208, 66,
	// 		161, 70, 155, 17, 183, 159, 99, 236, 116, 184, 135,
	// 	],
	// ];

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

	// const AES_256_GCM_ENCRYPTED_KEY: EncryptedKey = EncryptedKey::new(
	// 	[
	// 		125, 59, 176, 104, 216, 224, 249, 195, 236, 86, 245, 12, 55, 42, 157, 3, 49, 34, 139,
	// 		126, 79, 81, 89, 48, 30, 200, 240, 214, 117, 164, 238, 32, 6, 159, 3, 111, 114, 28,
	// 		176, 224, 187, 185, 123, 20, 164, 197, 171, 31,
	// 	],
	// 	AES_256_GCM_NONCE,
	// );

	// #[test]
	// fn aes_256_gcm_encrypt_bytes() {
	// 	let output = Encryptor::encrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&PLAINTEXT,
	// 		Aad::Null,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output, AES_256_GCM_BYTES_EXPECTED[0]);
	// }

	// #[test]
	// fn aes_256_gcm_encrypt_bytes_with_aad() {
	// 	let output = Encryptor::encrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&PLAINTEXT,
	// 		AAD,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output, AES_256_GCM_BYTES_EXPECTED[1]);
	// }

	// #[test]
	// fn aes_256_gcm_decrypt_bytes() {
	// 	let output = Decryptor::decrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&AES_256_GCM_BYTES_EXPECTED[0],
	// 		Aad::Null,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output.expose(), &PLAINTEXT);
	// }

	// #[test]
	// fn aes_256_gcm_decrypt_bytes_with_aad() {
	// 	let output = Decryptor::decrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&AES_256_GCM_BYTES_EXPECTED[1],
	// 		AAD,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output.expose(), &PLAINTEXT);
	// }

	// #[test]
	// fn aes_256_gcm_encrypt_key() {
	// 	let output = Encryptor::encrypt_key(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&PLAINTEXT_KEY,
	// 		Aad::Null,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output, AES_256_GCM_ENCRYPTED_KEY);
	// }

	// #[test]
	// fn aes_256_gcm_decrypt_key() {
	// 	let output = Decryptor::decrypt_key(
	// 		&KEY,
	// 		Algorithm::Aes256Gcm,
	// 		&AES_256_GCM_ENCRYPTED_KEY,
	// 		Aad::Null,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output, PLAINTEXT_KEY);
	// }

	// #[test]
	// fn aes_256_gcm_encrypt_tiny() {
	// 	let output = Encryptor::encrypt_tiny(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&PLAINTEXT,
	// 		Aad::Null,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output, AES_256_GCM_BYTES_EXPECTED[0]);
	// }

	// #[test]
	// fn aes_256_gcm_decrypt_tiny() {
	// 	let output = Decryptor::decrypt_tiny(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&AES_256_GCM_BYTES_EXPECTED[0],
	// 		Aad::Null,
	// 	)
	// 	.unwrap();

	// 	assert_eq!(output.expose(), &PLAINTEXT);
	// }

	// #[test]
	// #[should_panic(expected = "LengthMismatch")]
	// fn aes_256_gcm_encrypt_tiny_too_large() {
	// 	Encryptor::encrypt_tiny(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&vec![0u8; BLOCK_LEN],
	// 		Aad::Null,
	// 	)
	// 	.unwrap();
	// }

	// #[test]
	// #[should_panic(expected = "LengthMismatch")]
	// fn aes_256_gcm_decrypt_tiny_too_large() {
	// 	Decryptor::decrypt_tiny(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&vec![0u8; BLOCK_LEN + AEAD_TAG_LEN],
	// 		Aad::Null,
	// 	)
	// 	.unwrap();
	// }

	// #[test]
	// #[should_panic(expected = "Decrypt")]
	// fn aes_256_gcm_decrypt_bytes_missing_aad() {
	// 	Decryptor::decrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::Aes256Gcm,
	// 		&AES_256_GCM_BYTES_EXPECTED[1],
	// 		Aad::Null,
	// 	)
	// 	.unwrap();
	// }

	// #[test]
	// #[cfg_attr(miri, ignore)]
	// fn aes_256_gcm_encrypt_and_decrypt_5_blocks() {
	// 	let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

	// 	let mut reader = Cursor::new(&buf);
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let encryptor = Encryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	encryptor
	// 		.encrypt_streams(&mut reader, &mut writer, Aad::Null)
	// 		.unwrap();

	// 	let mut reader = Cursor::new(writer.into_inner());
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let decryptor = Decryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	decryptor
	// 		.decrypt_streams(&mut reader, &mut writer, Aad::Null)
	// 		.unwrap();

	// 	let output = writer.into_inner();

	// 	assert_eq!(buf, output);
	// }

	// #[test]
	// #[ignore]
	// fn aes_256_gcm_encrypt_and_decrypt_128mib() {
	// 	let buf = vec![1u8; BLOCK_LEN * 128].into_boxed_slice();

	// 	let mut reader = Cursor::new(&buf);
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let encryptor = Encryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	encryptor
	// 		.encrypt_streams(&mut reader, &mut writer, Aad::Null)
	// 		.unwrap();

	// 	let mut reader = Cursor::new(writer.into_inner());
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let decryptor = Decryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	decryptor
	// 		.decrypt_streams(&mut reader, &mut writer, Aad::Null)
	// 		.unwrap();

	// 	let output = writer.into_inner().into_boxed_slice();

	// 	assert_eq!(buf, output);
	// }

	// #[test]
	// #[cfg_attr(miri, ignore)]
	// fn aes_256_gcm_encrypt_and_decrypt_5_blocks_with_aad() {
	// 	let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

	// 	let mut reader = Cursor::new(&buf);
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let encryptor = Encryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	encryptor
	// 		.encrypt_streams(&mut reader, &mut writer, AAD)
	// 		.unwrap();

	// 	let mut reader = Cursor::new(writer.into_inner());
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let decryptor = Decryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	decryptor
	// 		.decrypt_streams(&mut reader, &mut writer, AAD)
	// 		.unwrap();

	// 	let output = writer.into_inner();

	// 	assert_eq!(buf, output);
	// }

	// #[tokio::test]
	// #[cfg(feature = "tokio")]
	// #[cfg_attr(miri, ignore)]
	// async fn aes_256_gcm_encrypt_and_decrypt_5_blocks_async() {
	// 	let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

	// 	let mut reader = Cursor::new(&buf);
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let encryptor = Encryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	encryptor
	// 		.encrypt_streams_async(&mut reader, &mut writer, Aad::Null)
	// 		.await
	// 		.unwrap();

	// 	let mut reader = Cursor::new(writer.into_inner());
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let decryptor = Decryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	decryptor
	// 		.decrypt_streams_async(&mut reader, &mut writer, Aad::Null)
	// 		.await
	// 		.unwrap();

	// 	let output = writer.into_inner();

	// 	assert_eq!(buf, output);
	// }

	// #[tokio::test]
	// #[cfg(feature = "tokio")]
	// #[cfg_attr(miri, ignore)]
	// async fn aes_256_gcm_encrypt_and_decrypt_5_blocks_with_aad_async() {
	// 	let buf = CryptoRng::generate_vec(BLOCK_LEN * 5);

	// 	let mut reader = Cursor::new(&buf);
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let encryptor = Encryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	encryptor
	// 		.encrypt_streams_async(&mut reader, &mut writer, AAD)
	// 		.await
	// 		.unwrap();

	// 	let mut reader = Cursor::new(writer.into_inner());
	// 	let mut writer = Cursor::new(Vec::new());

	// 	let decryptor = Decryptor::new(&KEY, &AES_256_GCM_NONCE, Algorithm::Aes256Gcm).unwrap();

	// 	decryptor
	// 		.decrypt_streams_async(&mut reader, &mut writer, AAD)
	// 		.await
	// 		.unwrap();

	// 	let output = writer.into_inner();

	// 	assert_eq!(buf, output);
	// }

	#[test]
	fn xchacha20_poly1305_encrypt_bytes() {
		let output = Encryptor::encrypt_bytes(
			&KEY,
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
			&KEY,
			&XCHACHA20_POLY1305_NONCE,
			Algorithm::XChaCha20Poly1305,
			&PLAINTEXT_KEY,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, XCHACHA20_POLY1305_ENCRYPTED_KEY);
	}

	#[test]
	fn xchacha20_poly1305_decrypt_key() {
		let output = Decryptor::decrypt_key(
			&KEY,
			Algorithm::XChaCha20Poly1305,
			&XCHACHA20_POLY1305_ENCRYPTED_KEY,
			Aad::Null,
		)
		.unwrap();

		assert_eq!(output, PLAINTEXT_KEY);
	}

	#[test]
	fn xchacha20_poly1305_encrypt_tiny() {
		let output = Encryptor::encrypt_tiny(
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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
			&KEY,
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

	// #[test]
	// #[should_panic(expected = "Validity")]
	// fn encrypt_with_invalid_nonce() {
	// 	Encryptor::encrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::XChaCha20Poly1305,
	// 		&PLAINTEXT,
	// 		Aad::Null,
	// 	)
	// 	.unwrap();
	// }

	#[test]
	#[should_panic(expected = "Validity")]
	fn encrypt_with_null_nonce() {
		Encryptor::encrypt_bytes(
			&KEY,
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

	// #[test]
	// #[should_panic(expected = "Validity")]
	// fn decrypt_with_invalid_nonce() {
	// 	Decryptor::decrypt_bytes(
	// 		&KEY,
	// 		&AES_256_GCM_NONCE,
	// 		Algorithm::XChaCha20Poly1305,
	// 		&XCHACHA20_POLY1305_BYTES_EXPECTED[0],
	// 		Aad::Null,
	// 	)
	// 	.unwrap();
	// }
}
