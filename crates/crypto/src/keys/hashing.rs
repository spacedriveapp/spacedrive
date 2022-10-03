use crate::primitives::SALT_LEN;
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
	pub fn get_argon2_params(&self) -> argon2::Params {
		match self {
			// We can use `.unwrap()` here as the values are hardcoded, and this shouldn't error
			// The values are the same for now as we'd need to test what's viable for most users.
			// It's very hardware dependant but we should aim for at least 16MB of RAM usage on standard
			// Provided they all take one second or longer, and less than 3/4 seconds (for paranoid), they will be fine
			// It's not so much the parameters themselves that matter, it's the duration (and ensuring that they use enough RAM to hinder ASIC brute-force attacks)
			Self::Standard => argon2::Params::new(262_144, 8, 4, Some(argon2::Params::DEFAULT_OUTPUT_LEN)).unwrap(),
			Self::Paranoid => argon2::Params::new(262_144, 8, 4, Some(argon2::Params::DEFAULT_OUTPUT_LEN)).unwrap(),
			Self::Hardened => argon2::Params::new(262_144, 8, 4, Some(argon2::Params::DEFAULT_OUTPUT_LEN)).unwrap(),
		}
	}
}

// This is really basic for the time being - I just want things functional before making them perfect
// Maybe a `Password` struct could work better, so we can call things such as `password.hash()`
// It'll allow for 3 "tiers" of security, each tier increasing the parameters (and hashing time)
pub fn password_hash_argon2id(password: Secret<Vec<u8>>, salt: [u8; SALT_LEN], params: Params) -> Secret<[u8; 32]> {
	let mut key = [0u8; 32];

	let argon2 = Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		params.get_argon2_params(),
	);

	let _result = argon2
		.hash_password_into(password.expose_secret(), &salt, &mut key)
		.unwrap();

	// Manual drop so we can ensure that it's gone
	drop(password);

	Secret::new(key)
}
