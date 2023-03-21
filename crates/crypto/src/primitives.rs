//! This module contains constant values, functions and types that are used around the crate.
//!
//! This includes things such as cryptographically-secure random salt/master key/nonce generation,
//! lengths for master keys and even the STREAM block size.
use crate::types::DerivationContext;

#[cfg(feature = "keymanager")]
use crate::keys::keymanager::StoredKeyVersion;

/// This is the salt size
pub const SALT_LEN: usize = 16;

/// The nonce size for XChaCha20-Poly1305, minus the last 4 bytes (due to STREAM with a 31+1 bit counter)
pub const XCHACHA20_POLY1305_NONCE_LEN: usize = 20;

/// The nonce size for AES-256-GCM, minus the last 4 bytes (due to STREAM with a 31+1 bit counter)
pub const AES_256_GCM_NONCE_LEN: usize = 8;

/// The length of the secret key, in bytes.
pub const SECRET_KEY_LEN: usize = 18;

/// The block size used for STREAM encryption/decryption. This size seems to offer the best performance compared to alternatives.
///
/// The file size gain is 16 bytes per 1048576 bytes (due to the AEAD tag), plus the size of the header.
pub const BLOCK_LEN: usize = 1_048_576;

/// This is the default AEAD tag size for all encryption algorithms used within the crate.
pub const AEAD_TAG_LEN: usize = 16;

pub const AAD_LEN: usize = 32;

/// The length of encrypted master keys
pub const ENCRYPTED_KEY_LEN: usize = KEY_LEN + AEAD_TAG_LEN;

/// The length of plain master/hashed keys
pub const KEY_LEN: usize = 32;

/// Used for OS keyrings to identify our items.
pub const APP_IDENTIFIER: &str = "Spacedrive";

/// Used for OS keyrings to identify our items.
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

#[cfg(feature = "encoding")]
pub use crate::header::file::LATEST_FILE_HEADER;

/// Defines the latest `StoredKeyVersion`
#[cfg(feature = "keymanager")]
pub const LATEST_STORED_KEY: StoredKeyVersion = StoredKeyVersion::V1;

/// Defines the context string for BLAKE3-KDF in regards to root key derivation
pub const ROOT_KEY_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 12:53:54 root key derivation");

/// Defines the context string for BLAKE3-KDF in regards to master password hash derivation
pub const MASTER_PASSWORD_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 15:35:41 master password hash derivation");

/// Defines the context string for BLAKE3-KDF in regards to file key derivation (for file encryption)
pub const FILE_KEYSLOT_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2022-12-14 12:54:12 file key derivation");

pub(super) const ARGON2ID_STANDARD: (u32, u32, u32) = (131_072, 8, 4);
pub(super) const ARGON2ID_HARDENED: (u32, u32, u32) = (262_144, 8, 4);
pub(super) const ARGON2ID_PARANOID: (u32, u32, u32) = (524_288, 8, 4);
pub(super) const B3BALLOON_STANDARD: (u32, u32, u32) = (131_072, 2, 1);
pub(super) const B3BALLOON_HARDENED: (u32, u32, u32) = (262_144, 2, 1);
pub(super) const B3BALLOON_PARANOID: (u32, u32, u32) = (524_288, 2, 1);
