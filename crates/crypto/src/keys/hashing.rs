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
	primitives::{Key, Salt, KEY_LEN},
	Error, Protected, ProtectedVec, Result,
};
use argon2::Argon2;
use balloon_hash::Balloon;

/// These parameters define the password-hashing level.
///
/// The harder the parameter, the longer the password will take to hash.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
	feature = "serde",
	derive(serde::Serialize),
	derive(serde::Deserialize)
)]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
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
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub enum HashingAlgorithm {
	Argon2id(Params),
	BalloonBlake3(Params),
}

impl HashingAlgorithm {
	/// This function should be used to hash passwords. It handles all appropriate parameters, and uses hashing with a secret key (if provided).
	#[allow(clippy::needless_pass_by_value)]
	pub fn hash(
		&self,
		password: ProtectedVec<u8>,
		salt: Salt,
		secret: Option<ProtectedVec<u8>>,
	) -> Result<Protected<Key>> {
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
			// The values are NOT final, as we need to find a good average.
			// It's very hardware dependant but we should aim for at least 64MB of RAM usage on standard
			// Provided they all take one (ish) second or longer, and less than 3/4 seconds (for paranoid), they will be fine
			// It's not so much the parameters themselves that matter, it's the duration (and ensuring that they use enough RAM to hinder ASIC brute-force attacks)
			Self::Standard => argon2::Params::new(131_072, 8, 4, None).unwrap(),
			Self::Paranoid => argon2::Params::new(262_144, 8, 4, None).unwrap(),
			Self::Hardened => argon2::Params::new(524_288, 8, 4, None).unwrap(),
		}
	}

	/// This function is used to generate parameters for password hashing.
	///
	/// This should not be called directly. Call it via the `HashingAlgorithm` struct (e.g. `HashingAlgorithm::Argon2id(Params::Standard).hash()`)
	#[must_use]
	pub fn balloon_blake3(&self) -> balloon_hash::Params {
		match self {
			// We can use `.unwrap()` here as the values are hardcoded, and this shouldn't error
			// The values are NOT final, as we need to find a good average.
			// It's very hardware dependant but we should aim for at least 64MB of RAM usage on standard
			// Provided they all take one (ish) second or longer, and less than 3/4 seconds (for paranoid), they will be fine
			// It's not so much the parameters themselves that matter, it's the duration (and ensuring that they use enough RAM to hinder ASIC brute-force attacks)
			Self::Standard => balloon_hash::Params::new(131_072, 1, 1).unwrap(),
			Self::Paranoid => balloon_hash::Params::new(262_144, 1, 1).unwrap(),
			Self::Hardened => balloon_hash::Params::new(524_288, 1, 1).unwrap(),
		}
	}
}

struct PasswordHasher;

impl PasswordHasher {
	#[allow(clippy::needless_pass_by_value)]
	fn argon2id(
		password: ProtectedVec<u8>,
		salt: Salt,
		secret: Option<ProtectedVec<u8>>,
		params: Params,
	) -> Result<Protected<Key>> {
		let secret = secret.map_or(Protected::new(vec![]), |k| k);

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
			.map_or(Err(Error::PasswordHash), |_| Ok(Protected::new(key)))
	}

	#[allow(clippy::needless_pass_by_value)]
	fn balloon_blake3(
		password: ProtectedVec<u8>,
		salt: Salt,
		secret: Option<ProtectedVec<u8>>,
		params: Params,
	) -> Result<Protected<Key>> {
		let secret = secret.map_or(Protected::new(vec![]), |k| k);

		let mut key = [0u8; KEY_LEN];

		let balloon = Balloon::<blake3::Hasher>::new(
			balloon_hash::Algorithm::Balloon,
			params.balloon_blake3(),
			Some(secret.expose()),
		);

		balloon
			.hash_into(password.expose(), &salt, &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Protected::new(key)))
	}
}
