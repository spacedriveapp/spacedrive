use crate::primitives::SALT_LEN;
use argon2::{Argon2, Params};
use secrecy::{ExposeSecret, Secret};

// This is really basic for the time being - I just want things functional before making them perfect
// I will be adding a parameter levelling system, which will be serializable/deserializable with regards to the header
// It'll allow for 3 "tiers" of security, each tier increasing the parameters (and hashing time)
pub fn password_hash_argon2id(password: Secret<Vec<u8>>, salt: [u8; SALT_LEN]) -> Secret<[u8; 32]> {
	let mut key = [0u8; 32];

	let argon2 = Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		Params::default(),
	);

	let _result = argon2
		.hash_password_into(password.expose_secret(), &salt, &mut key)
		.unwrap();

	// Manual drop so we can ensure that it's gone
	drop(password);

	Secret::new(key)
}
