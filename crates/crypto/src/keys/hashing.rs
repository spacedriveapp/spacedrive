//! This module contains all password-hashing related functions.
//!
//! Everything contained within is used to hash a user's password into strong key material, suitable for encrypting master keys.
//!
//! # Examples
//!
//! ```rust,ignore
//! let password = Protected::new(b"password".to_vec());
//! let hashing_algorithm = HashingAlgorithm::Argon2id(Params::Standard);
//! let salt = generate_salt();
//! let hashed_password = hashing_algorithm.hash(password, salt).unwrap();
//! ```

use crate::{
	primitives::{
		types::{Key, Salt, SecretKey},
		KEY_LEN,
	},
	Error, Protected, Result,
};
use argon2::Argon2;
use balloon_hash::Balloon;

/// These parameters define the password-hashing level.
///
/// The greater the parameter, the longer the password will take to hash.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
	feature = "serde",
	derive(serde::Serialize),
	derive(serde::Deserialize)
)]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum Params {
	Standard,
	Hardened,
	Paranoid,
}

/// This defines all available password hashing algorithms.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
	feature = "serde",
	derive(serde::Serialize),
	derive(serde::Deserialize),
	serde(tag = "name", content = "params")
)]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum HashingAlgorithm {
	Argon2id(Params),
	BalloonBlake3(Params),
}

impl HashingAlgorithm {
	/// This function should be used to hash passwords. It handles all appropriate parameters, and uses hashing with a secret key (if provided).
	#[allow(clippy::needless_pass_by_value)]
	pub fn hash(
		&self,
		password: Protected<Vec<u8>>,
		salt: Salt,
		secret: Option<SecretKey>,
	) -> Result<Key> {
		match self {
			Self::Argon2id(params) => PasswordHasher::argon2id(password, salt, secret, *params),
			Self::BalloonBlake3(params) => {
				PasswordHasher::balloon_blake3(password, salt, secret, *params)
			}
		}
	}
}

impl Params {
	/// This function is used to generate parameters for password hashing.
	///
	/// This should not be called directly. Call it via the `HashingAlgorithm` struct (e.g. `HashingAlgorithm::Argon2id(Params::Standard).hash()`)
	#[must_use]
	pub fn argon2id(&self) -> argon2::Params {
		match self {
			// We can use `.unwrap()` here as the values are hardcoded, and this shouldn't error
			Self::Standard => argon2::Params::new(131_072, 8, 4, None).unwrap(),
			Self::Hardened => argon2::Params::new(262_144, 8, 4, None).unwrap(),
			Self::Paranoid => argon2::Params::new(524_288, 8, 4, None).unwrap(),
		}
	}

	/// This function is used to generate parameters for password hashing.
	///
	/// This should not be called directly. Call it via the `HashingAlgorithm` struct (e.g. `HashingAlgorithm::BalloonBlake3(Params::Standard).hash()`)
	#[must_use]
	pub fn balloon_blake3(&self) -> balloon_hash::Params {
		match self {
			// We can use `.unwrap()` here as the values are hardcoded, and this shouldn't error
			Self::Standard => balloon_hash::Params::new(131_072, 2, 1).unwrap(),
			Self::Hardened => balloon_hash::Params::new(262_144, 2, 1).unwrap(),
			Self::Paranoid => balloon_hash::Params::new(524_288, 2, 1).unwrap(),
		}
	}
}

struct PasswordHasher;

impl PasswordHasher {
	#[allow(clippy::needless_pass_by_value)]
	fn argon2id(
		password: Protected<Vec<u8>>,
		salt: Salt,
		secret: Option<SecretKey>,
		params: Params,
	) -> Result<Key> {
		let secret = secret.map_or(Protected::new(vec![]), |k| {
			Protected::new(k.expose().to_vec())
		});

		let mut key = [0u8; KEY_LEN];
		let argon2 = Argon2::new_with_secret(
			secret.expose(),
			argon2::Algorithm::Argon2id,
			argon2::Version::V0x13,
			params.argon2id(),
		)
		.map_err(|_| Error::PasswordHash)?;

		argon2
			.hash_password_into(password.expose(), &salt, &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Key::new(key)))
	}

	#[allow(clippy::needless_pass_by_value)]
	fn balloon_blake3(
		password: Protected<Vec<u8>>,
		salt: Salt,
		secret: Option<SecretKey>,
		params: Params,
	) -> Result<Key> {
		let secret = secret.map_or(Protected::new(vec![]), |k| {
			Protected::new(k.expose().to_vec())
		});

		let mut key = [0u8; KEY_LEN];

		let balloon = Balloon::<blake3::Hasher>::new(
			balloon_hash::Algorithm::Balloon,
			params.balloon_blake3(),
			Some(secret.expose()),
		);

		balloon
			.hash_into(password.expose(), &salt, &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Key::new(key)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const TEST_CONTEXT: &str = "spacedrive 2023-02-09 17:44:14 test key derivation";

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

	const SALT: Salt = Salt([
		0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
		0xFF,
	]);

	const SECRET_KEY: SecretKey = SecretKey::new([
		0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
		0x55, 0x55, 0x55,
	]);

	// for the `const` arrays below, [0] is standard params, [1] is hardened and [2] is paranoid

	const HASH_ARGON2ID_EXPECTED: [[u8; 32]; 3] = [
		[
			194, 153, 245, 125, 12, 102, 65, 30, 254, 191, 9, 125, 4, 113, 99, 209, 162, 43, 140,
			93, 217, 220, 222, 46, 105, 48, 123, 220, 180, 103, 20, 11,
		],
		[
			173, 45, 167, 171, 125, 13, 245, 47, 231, 62, 175, 215, 21, 253, 84, 188, 249, 68, 229,
			98, 16, 55, 110, 202, 105, 109, 102, 71, 216, 125, 170, 66,
		],
		[
			27, 158, 230, 75, 99, 236, 40, 137, 60, 237, 145, 119, 159, 207, 56, 50, 210, 5, 157,
			227, 162, 162, 148, 142, 230, 237, 138, 133, 112, 182, 156, 198,
		],
	];

	const HASH_ARGON2ID_WITH_SECRET_EXPECTED: [[u8; 32]; 3] = [
		[
			132, 102, 123, 67, 87, 219, 88, 76, 81, 191, 128, 41, 246, 201, 103, 155, 200, 114, 54,
			116, 240, 66, 155, 78, 73, 44, 87, 174, 231, 196, 206, 236,
		],
		[
			246, 200, 29, 33, 86, 21, 66, 177, 154, 2, 134, 181, 254, 148, 104, 205, 235, 108, 121,
			127, 184, 230, 109, 240, 128, 101, 137, 179, 212, 89, 37, 41,
		],
		[
			3, 60, 179, 196, 172, 30, 0, 201, 15, 9, 213, 59, 37, 219, 173, 134, 132, 166, 32, 60,
			33, 216, 3, 249, 185, 120, 110, 14, 155, 242, 134, 215,
		],
	];

	const HASH_B3BALLOON_EXPECTED: [[u8; 32]; 3] = [
		[
			105, 36, 165, 219, 22, 136, 156, 19, 32, 143, 237, 150, 236, 194, 70, 113, 73, 137,
			243, 106, 80, 31, 43, 73, 207, 210, 29, 251, 88, 6, 132, 77,
		],
		[
			179, 71, 60, 122, 54, 72, 132, 209, 146, 96, 15, 115, 41, 95, 5, 75, 214, 135, 6, 122,
			82, 42, 158, 9, 117, 19, 19, 40, 48, 233, 207, 237,
		],
		[
			233, 60, 62, 184, 29, 152, 111, 46, 239, 126, 98, 90, 211, 255, 151, 0, 10, 189, 61,
			84, 229, 11, 245, 228, 47, 114, 87, 74, 227, 67, 24, 141,
		],
	];

	const HASH_B3BALLOON_WITH_SECRET_EXPECTED: [[u8; 32]; 3] = [
		[
			188, 0, 43, 39, 137, 199, 91, 142, 97, 31, 98, 6, 130, 75, 251, 71, 150, 109, 29, 62,
			237, 171, 210, 22, 139, 108, 94, 190, 91, 74, 134, 47,
		],
		[
			19, 247, 102, 192, 129, 184, 29, 147, 68, 215, 234, 146, 153, 221, 65, 134, 68, 120,
			207, 209, 184, 246, 127, 131, 9, 245, 91, 250, 220, 61, 76, 248,
		],
		[
			165, 240, 162, 25, 172, 3, 232, 2, 43, 230, 226, 128, 174, 28, 211, 61, 139, 136, 221,
			197, 16, 83, 221, 18, 212, 190, 138, 79, 239, 148, 89, 215,
		],
	];

	const DERIVE_B3_EXPECTED: [u8; 32] = [
		27, 34, 251, 101, 201, 89, 78, 90, 20, 175, 62, 206, 200, 153, 166, 103, 118, 179, 194, 44,
		216, 26, 48, 120, 137, 157, 60, 234, 234, 53, 46, 60,
	];

	#[test]
	fn hash_argon2id_standard() {
		let output = ARGON2ID_STANDARD
			.hash(Protected::new(PASSWORD.to_vec()), SALT, None)
			.unwrap();

		assert_eq!(&HASH_ARGON2ID_EXPECTED[0], output.expose())
	}

	#[test]
	fn hash_argon2id_standard_with_secret() {
		let output = ARGON2ID_STANDARD
			.hash(Protected::new(PASSWORD.to_vec()), SALT, Some(SECRET_KEY))
			.unwrap();

		assert_eq!(&HASH_ARGON2ID_WITH_SECRET_EXPECTED[0], output.expose())
	}

	#[test]
	fn hash_argon2id_hardened() {
		let output = ARGON2ID_HARDENED
			.hash(Protected::new(PASSWORD.to_vec()), SALT, None)
			.unwrap();

		assert_eq!(&HASH_ARGON2ID_EXPECTED[1], output.expose())
	}

	#[test]
	fn hash_argon2id_hardened_with_secret() {
		let output = ARGON2ID_HARDENED
			.hash(Protected::new(PASSWORD.to_vec()), SALT, Some(SECRET_KEY))
			.unwrap();

		assert_eq!(&HASH_ARGON2ID_WITH_SECRET_EXPECTED[1], output.expose())
	}

	#[test]
	fn hash_argon2id_paranoid() {
		let output = ARGON2ID_PARANOID
			.hash(Protected::new(PASSWORD.to_vec()), SALT, None)
			.unwrap();

		assert_eq!(&HASH_ARGON2ID_EXPECTED[2], output.expose())
	}

	#[test]
	fn hash_argon2id_paranoid_with_secret() {
		let output = ARGON2ID_PARANOID
			.hash(Protected::new(PASSWORD.to_vec()), SALT, Some(SECRET_KEY))
			.unwrap();

		assert_eq!(&HASH_ARGON2ID_WITH_SECRET_EXPECTED[2], output.expose())
	}

	#[test]
	fn hash_b3balloon_standard() {
		let output = B3BALLOON_STANDARD
			.hash(Protected::new(PASSWORD.to_vec()), SALT, None)
			.unwrap();

		assert_eq!(&HASH_B3BALLOON_EXPECTED[0], output.expose())
	}

	#[test]
	fn hash_b3balloon_standard_with_secret() {
		let output = B3BALLOON_STANDARD
			.hash(Protected::new(PASSWORD.to_vec()), SALT, Some(SECRET_KEY))
			.unwrap();

		assert_eq!(&HASH_B3BALLOON_WITH_SECRET_EXPECTED[0], output.expose())
	}

	#[test]
	fn hash_b3balloon_hardened() {
		let output = B3BALLOON_HARDENED
			.hash(Protected::new(PASSWORD.to_vec()), SALT, None)
			.unwrap();

		assert_eq!(&HASH_B3BALLOON_EXPECTED[1], output.expose())
	}

	#[test]
	fn hash_b3balloon_hardened_with_secret() {
		let output = B3BALLOON_HARDENED
			.hash(Protected::new(PASSWORD.to_vec()), SALT, Some(SECRET_KEY))
			.unwrap();

		assert_eq!(&HASH_B3BALLOON_WITH_SECRET_EXPECTED[1], output.expose())
	}

	#[test]
	fn hash_b3balloon_paranoid() {
		let output = B3BALLOON_PARANOID
			.hash(Protected::new(PASSWORD.to_vec()), SALT, None)
			.unwrap();

		assert_eq!(&HASH_B3BALLOON_EXPECTED[2], output.expose())
	}

	#[test]
	fn hash_b3balloon_paranoid_with_secret() {
		let output = B3BALLOON_PARANOID
			.hash(Protected::new(PASSWORD.to_vec()), SALT, Some(SECRET_KEY))
			.unwrap();

		assert_eq!(&HASH_B3BALLOON_WITH_SECRET_EXPECTED[2], output.expose())
	}

	#[test]
	fn derive_b3() {
		let output = Key::derive(KEY, SALT, TEST_CONTEXT);

		assert_eq!(&DERIVE_B3_EXPECTED, output.expose())
	}
}
