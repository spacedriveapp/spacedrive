//! This module contains all password-hashing related functions.
//!
//! Everything contained within is used to hash a user's password into strong key material, suitable for encrypting master keys.
//!
//! # Examples
//!
//! ```rust,ignore
//! let password = Protected::new(b"password".to_vec());
//! let hashing_algorithm = HashingAlgorithm::default();
//! let salt = generate_salt();
//! let hashed_password = hashing_algorithm.hash(password, salt).unwrap();
//! ```

use crate::{
	primitives::KEY_LEN,
	types::{DerivationContext, HashingAlgorithm, Key, Salt, SecretKey},
	Error, Protected, Result,
};
use argon2::Argon2;
use balloon_hash::Balloon;

pub struct Hasher;

impl Hasher {
	#[must_use]
	pub fn blake3(bytes: &[u8]) -> Key {
		blake3::hash(bytes).into()
	}

	#[must_use]
	#[allow(clippy::needless_pass_by_value)]
	pub fn derive_key(key: Key, salt: Salt, context: DerivationContext) -> Key {
		let k = blake3::derive_key(
			context.inner(),
			&[key.expose().as_ref(), salt.inner()].concat(),
		);

		Key::new(k)
	}

	pub fn hash_password(
		algorithm: HashingAlgorithm,
		password: Protected<Vec<u8>>,
		salt: Salt,
		secret: Option<SecretKey>,
	) -> Result<Key> {
		let d = algorithm.get_parameters();

		match algorithm {
			HashingAlgorithm::Argon2id(_) => Self::argon2id(password, salt, secret, d),
			HashingAlgorithm::BalloonBlake3(_) => Self::balloon_blake3(password, salt, secret, d),
		}
	}

	#[allow(clippy::needless_pass_by_value)]
	fn argon2id(
		password: Protected<Vec<u8>>,
		salt: Salt,
		secret: Option<SecretKey>,
		params: (u32, u32, u32),
	) -> Result<Key> {
		let secret: Protected<Vec<u8>> = secret.map_or(vec![], SecretKey::to_vec).into();
		let p = argon2::Params::new(params.0, params.1, params.2, None)
			.map_err(|_| Error::PasswordHash)?;

		let mut key = [0u8; KEY_LEN];
		let argon2 = Argon2::new_with_secret(
			secret.expose(),
			argon2::Algorithm::Argon2id,
			argon2::Version::V0x13,
			p,
		)
		.map_err(|_| Error::PasswordHash)?;

		argon2
			.hash_password_into(password.expose(), salt.inner(), &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Key::new(key)))
	}

	#[allow(clippy::needless_pass_by_value)]
	fn balloon_blake3(
		password: Protected<Vec<u8>>,
		salt: Salt,
		secret: Option<SecretKey>,
		params: (u32, u32, u32),
	) -> Result<Key> {
		let secret: Protected<Vec<u8>> = secret.map_or(vec![], SecretKey::to_vec).into();
		let p = balloon_hash::Params::new(params.0, params.1, params.2)
			.map_err(|_| Error::PasswordHash)?;

		let mut key = [0u8; KEY_LEN];

		let balloon = Balloon::<blake3::Hasher>::new(
			balloon_hash::Algorithm::Balloon,
			p,
			Some(secret.expose()),
		);

		balloon
			.hash_into(password.expose(), salt.inner(), &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Key::new(key)))
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::{
		keys::Hasher,
		types::{DerivationContext, HashingAlgorithm, Key, Params, Salt, SecretKey},
	};

	const TEST_CONTEXT: DerivationContext =
		DerivationContext::new("spacedrive 2023-02-09 17:44:14 test key derivation");

	const ARGON2ID_STANDARD: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);
	const ARGON2ID_HARDENED: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Hardened);
	const ARGON2ID_PARANOID: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Paranoid);
	const B3BALLOON_STANDARD: HashingAlgorithm = HashingAlgorithm::BalloonBlake3(Params::Standard);
	const B3BALLOON_HARDENED: HashingAlgorithm = HashingAlgorithm::BalloonBlake3(Params::Hardened);
	const B3BALLOON_PARANOID: HashingAlgorithm = HashingAlgorithm::BalloonBlake3(Params::Paranoid);

	const PASSWORD: [u8; 8] = [0x70, 0x61, 0x73, 0x73, 0x77, 0x6f, 0x72, 0x64];

	const KEY: Key = Key::new([
		0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23,
		0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23, 0x23,
		0x23, 0x23,
	]);

	const SALT: Salt = Salt::new([
		0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
		0xFF,
	]);

	const SECRET_KEY: SecretKey = SecretKey::new([
		0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
		0x55, 0x55, 0x55,
	]);

	// for the `const` arrays below, [0] is standard params, [1] is hardened and [2] is paranoid

	const HASH_ARGON2ID_EXPECTED: [Key; 3] = [
		Key::new([
			194, 153, 245, 125, 12, 102, 65, 30, 254, 191, 9, 125, 4, 113, 99, 209, 162, 43, 140,
			93, 217, 220, 222, 46, 105, 48, 123, 220, 180, 103, 20, 11,
		]),
		Key::new([
			173, 45, 167, 171, 125, 13, 245, 47, 231, 62, 175, 215, 21, 253, 84, 188, 249, 68, 229,
			98, 16, 55, 110, 202, 105, 109, 102, 71, 216, 125, 170, 66,
		]),
		Key::new([
			27, 158, 230, 75, 99, 236, 40, 137, 60, 237, 145, 119, 159, 207, 56, 50, 210, 5, 157,
			227, 162, 162, 148, 142, 230, 237, 138, 133, 112, 182, 156, 198,
		]),
	];

	const HASH_ARGON2ID_WITH_SECRET_EXPECTED: [Key; 3] = [
		Key::new([
			132, 102, 123, 67, 87, 219, 88, 76, 81, 191, 128, 41, 246, 201, 103, 155, 200, 114, 54,
			116, 240, 66, 155, 78, 73, 44, 87, 174, 231, 196, 206, 236,
		]),
		Key::new([
			246, 200, 29, 33, 86, 21, 66, 177, 154, 2, 134, 181, 254, 148, 104, 205, 235, 108, 121,
			127, 184, 230, 109, 240, 128, 101, 137, 179, 212, 89, 37, 41,
		]),
		Key::new([
			3, 60, 179, 196, 172, 30, 0, 201, 15, 9, 213, 59, 37, 219, 173, 134, 132, 166, 32, 60,
			33, 216, 3, 249, 185, 120, 110, 14, 155, 242, 134, 215,
		]),
	];

	const HASH_B3BALLOON_EXPECTED: [Key; 3] = [
		Key::new([
			105, 36, 165, 219, 22, 136, 156, 19, 32, 143, 237, 150, 236, 194, 70, 113, 73, 137,
			243, 106, 80, 31, 43, 73, 207, 210, 29, 251, 88, 6, 132, 77,
		]),
		Key::new([
			179, 71, 60, 122, 54, 72, 132, 209, 146, 96, 15, 115, 41, 95, 5, 75, 214, 135, 6, 122,
			82, 42, 158, 9, 117, 19, 19, 40, 48, 233, 207, 237,
		]),
		Key::new([
			233, 60, 62, 184, 29, 152, 111, 46, 239, 126, 98, 90, 211, 255, 151, 0, 10, 189, 61,
			84, 229, 11, 245, 228, 47, 114, 87, 74, 227, 67, 24, 141,
		]),
	];

	const HASH_B3BALLOON_WITH_SECRET_EXPECTED: [Key; 3] = [
		Key::new([
			188, 0, 43, 39, 137, 199, 91, 142, 97, 31, 98, 6, 130, 75, 251, 71, 150, 109, 29, 62,
			237, 171, 210, 22, 139, 108, 94, 190, 91, 74, 134, 47,
		]),
		Key::new([
			19, 247, 102, 192, 129, 184, 29, 147, 68, 215, 234, 146, 153, 221, 65, 134, 68, 120,
			207, 209, 184, 246, 127, 131, 9, 245, 91, 250, 220, 61, 76, 248,
		]),
		Key::new([
			165, 240, 162, 25, 172, 3, 232, 2, 43, 230, 226, 128, 174, 28, 211, 61, 139, 136, 221,
			197, 16, 83, 221, 18, 212, 190, 138, 79, 239, 148, 89, 215,
		]),
	];

	const DERIVE_B3_EXPECTED: Key = Key::new([
		27, 34, 251, 101, 201, 89, 78, 90, 20, 175, 62, 206, 200, 153, 166, 103, 118, 179, 194, 44,
		216, 26, 48, 120, 137, 157, 60, 234, 234, 53, 46, 60,
	]);

	#[test]
	fn hash_argon2id_standard() {
		let output =
			Hasher::hash_password(ARGON2ID_STANDARD, PASSWORD.to_vec().into(), SALT, None).unwrap();

		assert!(output == HASH_ARGON2ID_EXPECTED[0]);
	}

	#[test]
	fn hash_argon2id_standard_with_secret() {
		let output = Hasher::hash_password(
			ARGON2ID_STANDARD,
			PASSWORD.to_vec().into(),
			SALT,
			Some(SECRET_KEY),
		)
		.unwrap();

		assert!(output == HASH_ARGON2ID_WITH_SECRET_EXPECTED[0]);
	}

	#[test]
	fn hash_argon2id_hardened() {
		let output =
			Hasher::hash_password(ARGON2ID_HARDENED, PASSWORD.to_vec().into(), SALT, None).unwrap();

		assert!(output == HASH_ARGON2ID_EXPECTED[1]);
	}

	#[test]
	fn hash_argon2id_hardened_with_secret() {
		let output = Hasher::hash_password(
			ARGON2ID_HARDENED,
			PASSWORD.to_vec().into(),
			SALT,
			Some(SECRET_KEY),
		)
		.unwrap();

		assert!(output == HASH_ARGON2ID_WITH_SECRET_EXPECTED[1]);
	}

	#[test]
	fn hash_argon2id_paranoid() {
		let output =
			Hasher::hash_password(ARGON2ID_PARANOID, PASSWORD.to_vec().into(), SALT, None).unwrap();

		assert!(output == HASH_ARGON2ID_EXPECTED[2]);
	}

	#[test]
	fn hash_argon2id_paranoid_with_secret() {
		let output = Hasher::hash_password(
			ARGON2ID_PARANOID,
			PASSWORD.to_vec().into(),
			SALT,
			Some(SECRET_KEY),
		)
		.unwrap();

		assert!(output == HASH_ARGON2ID_WITH_SECRET_EXPECTED[2]);
	}

	#[test]
	fn hash_b3balloon_standard() {
		let output =
			Hasher::hash_password(B3BALLOON_STANDARD, PASSWORD.to_vec().into(), SALT, None)
				.unwrap();

		assert!(output == HASH_B3BALLOON_EXPECTED[0]);
	}

	#[test]
	fn hash_b3balloon_standard_with_secret() {
		let output = Hasher::hash_password(
			B3BALLOON_STANDARD,
			PASSWORD.to_vec().into(),
			SALT,
			Some(SECRET_KEY),
		)
		.unwrap();

		assert!(output == HASH_B3BALLOON_WITH_SECRET_EXPECTED[0]);
	}

	#[test]
	fn hash_b3balloon_hardened() {
		let output =
			Hasher::hash_password(B3BALLOON_HARDENED, PASSWORD.to_vec().into(), SALT, None)
				.unwrap();

		assert!(output == HASH_B3BALLOON_EXPECTED[1]);
	}

	#[test]
	fn hash_b3balloon_hardened_with_secret() {
		let output = Hasher::hash_password(
			B3BALLOON_HARDENED,
			PASSWORD.to_vec().into(),
			SALT,
			Some(SECRET_KEY),
		)
		.unwrap();

		assert!(output == HASH_B3BALLOON_WITH_SECRET_EXPECTED[1]);
	}

	#[test]
	fn hash_b3balloon_paranoid() {
		let output =
			Hasher::hash_password(B3BALLOON_PARANOID, PASSWORD.to_vec().into(), SALT, None)
				.unwrap();

		assert!(output == HASH_B3BALLOON_EXPECTED[2]);
	}

	#[test]
	fn hash_b3balloon_paranoid_with_secret() {
		let output = Hasher::hash_password(
			B3BALLOON_PARANOID,
			PASSWORD.to_vec().into(),
			SALT,
			Some(SECRET_KEY),
		)
		.unwrap();

		assert!(output == HASH_B3BALLOON_WITH_SECRET_EXPECTED[2]);
	}

	#[test]
	fn derive_b3() {
		let output = Hasher::derive_key(KEY, SALT, TEST_CONTEXT);

		assert!(output == DERIVE_B3_EXPECTED);
	}
}
