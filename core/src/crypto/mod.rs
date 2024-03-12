#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	clippy::expect_used,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::as_conversions,
	clippy::dbg_macro
)]
#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

// use sd_crypto::types::{DerivationContext, MagicBytes};

// pub mod error;
// pub use error::{KeyManagerError, Result};

// pub mod keymanager;
// pub use keymanager::{DisplayKey, KeyManager, KeyType, KeyVersion, RootKey, UserKey};

/*
/// Used for OS keyrings to identify our items.
pub const KEYRING_APP_IDENTIFIER: &str = "Spacedrive";

/// Used for OS keyrings to identify our items.
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

/// Defines the context string for BLAKE3-KDF in regards to root key derivation
pub const ROOT_KEY_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 12:53:54 root key derivation");

/// Defines the context string for BLAKE3-KDF in regards to master password hash derivation
pub const MASTER_PASSWORD_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 15:35:41 master password hash derivation");

/// Defines the context string for BLAKE3-KDF in regards to file key derivation (for file encryption)
pub const FILE_KEYSLOT_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 12:54:12 file key derivation");
*/

// /// Defines the context string for BLAKE3-KDF in regards to key derivation (for the key manager)
// pub const KEY_MOUNTING_CONTEXT: DerivationContext =
// 	DerivationContext::new("spacedrive 2023-05-24 11:43:07 key mounting derivation");

// /// Defines the context string for BLAKE3-KDF in regards to key derivation (for encrypted words)
// pub const ENCRYPTED_WORD_CONTEXT: DerivationContext =
// 	DerivationContext::new("spacedrive 2023-05-22 18:01:02 encrypted word derivation");

// /// Defines the context string for BLAKE3-KDF in regards to key derivation (for test vectors)
// pub const TEST_VECTOR_CONTEXT: DerivationContext =
// 	DerivationContext::new("spacedrive 2023-05-22 14:37:16 test vector derivation");

// /// Encrypted file magic bytes - "ballapp" and then a null byte.
// pub const FILE_MAGIC_BYTES: MagicBytes<8> =
// 	MagicBytes::new([0x62, 0x61, 0x6C, 0x6C, 0x61, 0x70, 0x70, 0x00]);
