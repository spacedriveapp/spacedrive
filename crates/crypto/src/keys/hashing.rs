use crate::{error::Error, primitives::SALT_LEN};
use argon2::Argon2;
use secrecy::{ExposeSecret, Secret};

// These names are not final
// I'm considering adding an `(i32)` to each, to allow specific versioning of each parameter version
// These will be serializable/deserializable with regards to the header/storage of this information
#[derive(Clone, Copy)]
pub enum Params {
	Standard,
	Hardened,
	Paranoid,
}

impl Params {
	#[must_use]
	pub fn get_argon2_params(&self) -> argon2::Params {
		match self {
			// We can use `.unwrap()` here as the values are hardcoded, and this shouldn't error
			// The values are NOT final, as we need to find a good average.
			// It's very hardware dependant but we should aim for at least 16MB of RAM usage on standard
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

// Shouldn't be called directly - call it on the `HashingAlgorithm` struct
/// This function should NOT be called directly!
///
/// Call it via the `HashingAlgorithm` struct (e.g. `HashingAlgorithm::Argon2id(Params::Standard).hash()`)
pub fn password_hash_argon2id(
	password: Secret<Vec<u8>>,
	salt: [u8; SALT_LEN],
	params: Params,
) -> Result<Secret<[u8; 32]>, Error> {
	let mut key = [0u8; 32];

	let argon2 = Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		params.get_argon2_params(),
	);

	let result = argon2.hash_password_into(password.expose_secret(), &salt, &mut key);

	// Manual drop so we can ensure that it's gone
	drop(password);

	if result.is_ok() {
		Ok(Secret::new(key))
	} else {
		Err(Error::PasswordHash)
	}
}
