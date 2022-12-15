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

use crate::primitives::KEY_LEN;
use crate::Protected;
use crate::{primitives::SALT_LEN, Error, Result};
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
	derive(serde::Deserialize)
)]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub enum HashingAlgorithm {
	Argon2id(Params),
	BalloonBlake3(Params),
}

impl HashingAlgorithm {
	/// This function should be used to hash passwords
	///
	/// It also handles all the password hashing parameters.
	pub fn hash(
		&self,
		password: Protected<Vec<u8>>,
		salt: [u8; SALT_LEN],
	) -> Result<Protected<[u8; KEY_LEN]>> {
		match self {
			Self::Argon2id(params) => PasswordHasher::argon2id(password, salt, *params),
			Self::BalloonBlake3(params) => PasswordHasher::balloon_blake3(password, salt, *params),
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
		password: Protected<Vec<u8>>,
		salt: [u8; SALT_LEN],
		params: Params,
	) -> Result<Protected<[u8; KEY_LEN]>> {
		let mut key = [0u8; KEY_LEN];

		let argon2 = Argon2::new(
			argon2::Algorithm::Argon2id,
			argon2::Version::V0x13,
			params.argon2id(),
		);

		argon2
			.hash_password_into(password.expose(), &salt, &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Protected::new(key)))
	}

	fn balloon_blake3(
		password: Protected<Vec<u8>>,
		salt: [u8; SALT_LEN],
		params: Params,
	) -> Result<Protected<[u8; KEY_LEN]>> {
		let mut key = [0u8; KEY_LEN];

		let balloon = Balloon::<blake3::Hasher>::new(
			balloon_hash::Algorithm::Balloon,
			params.balloon_blake3(),
			None,
		);

		balloon
			.hash_into(password.expose(), &salt, &mut key)
			.map_or(Err(Error::PasswordHash), |_| Ok(Protected::new(key)))
	}
}
