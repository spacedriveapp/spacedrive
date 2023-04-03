#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::correctness)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![warn(unused_qualifications)]
#![forbid(unsafe_code)]

use sd_crypto::types::{Algorithm, DerivationContext, HashingAlgorithm};

pub mod error;
pub mod key_manager;
pub use error::Result;

/// Used for OS keyrings to identify our items.
pub const KEYRING_APP_IDENTIFIER: &str = "Spacedrive";

/// Used for OS keyrings to identify our items.
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

// /// Defines the latest `StoredKeyVersion`
// pub const LATEST_STORED_KEY: crate::keys::keymanager::StoredKeyVersion =
// 	crate::keys::keymanager::StoredKeyVersion::V1;

/// Defines the context string for BLAKE3-KDF in regards to root key derivation
pub const ROOT_KEY_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 12:53:54 root key derivation");

/// Defines the context string for BLAKE3-KDF in regards to master password hash derivation
pub const MASTER_PASSWORD_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 15:35:41 master password hash derivation");

/// Defines the context string for BLAKE3-KDF in regards to file key derivation (for file encryption)
pub const FILE_KEYSLOT_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 12:54:12 file key derivation");

#[derive(Clone, serde::Deserialize)]
pub struct OnboardingConfig {
	pub password: String,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
}

// pub(crate) mod keymanager;
