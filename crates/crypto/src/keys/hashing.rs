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
use crate::Protected;
use crate::{error::Error, primitives::SALT_LEN};
use argon2::Argon2;

/// These parameters define the password-hashing level.
///
/// The harder the parameter, the longer the password will take to hash.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(clippy::use_self)]
pub enum Params {
	Standard,
	Hardened,
	Paranoid,
}

/// This defines all available password hashing algorithms.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HashingAlgorithm {
	Argon2id(Params),
}

/// This is so we can iterate over all hashing algorithms and parameters.
///
/// The main usage is for pre-hashing a key during mounting.
pub const HASHING_ALGORITHM_LIST: [HashingAlgorithm; 3] = [
	HashingAlgorithm::Argon2id(Params::Standard),
	HashingAlgorithm::Argon2id(Params::Hardened),
	HashingAlgorithm::Argon2id(Params::Paranoid),
];

impl HashingAlgorithm {
	/// This function should be used to hash passwords
	///
	/// It also handles all the password hashing parameters.
	pub fn hash(
		&self,
		password: Protected<Vec<u8>>,
		salt: [u8; SALT_LEN],
	) -> Result<Protected<[u8; 32]>, Error> {
		match self {
			Self::Argon2id(params) => password_hash_argon2id(password, salt, *params),
		}
	}
}

impl Params {
	/// This function is used to generate parameters for password hashing.
	///
	/// This should not be called directly. Call it via the `HashingAlgorithm` struct (e.g. `HashingAlgorithm::Argon2id(Params::Standard).hash()`)
	#[must_use]
	pub fn get_argon2_params(&self) -> argon2::Params {
		match self {
			// We can use `.unwrap()` here as the values are hardcoded, and this shouldn't error
			// The values are NOT final, as we need to find a good average.
			// It's very hardware dependant but we should aim for at least 64MB of RAM usage on standard
			// Provided they all take one (ish) second or longer, and less than 3/4 seconds (for paranoid), they will be fine
			// It's not so much the parameters themselves that matter, it's the duration (and ensuring that they use enough RAM to hinder ASIC brute-force attacks)
			Self::Standard => {
				argon2::Params::new(131_072, 8, 4, Some(argon2::Params::DEFAULT_OUTPUT_LEN))
					.unwrap()
			}
			Self::Paranoid => {
				argon2::Params::new(262_144, 8, 4, Some(argon2::Params::DEFAULT_OUTPUT_LEN))
					.unwrap()
			}
			Self::Hardened => {
				argon2::Params::new(524_288, 8, 4, Some(argon2::Params::DEFAULT_OUTPUT_LEN))
					.unwrap()
			}
		}
	}
}

/// This function should NOT be called directly!
///
/// Call it via the `HashingAlgorithm` struct (e.g. `HashingAlgorithm::Argon2id(Params::Standard).hash()`)
#[allow(clippy::needless_pass_by_value)]
pub fn password_hash_argon2id(
	password: Protected<Vec<u8>>,
	salt: [u8; SALT_LEN],
	params: Params,
) -> Result<Protected<[u8; 32]>, Error> {
	let mut key = [0u8; 32];

	let argon2 = Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		params.get_argon2_params(),
	);

	let result = argon2.hash_password_into(password.expose(), &salt, &mut key);

	if result.is_ok() {
		Ok(Protected::new(key))
	} else {
		Err(Error::PasswordHash)
	}
}
