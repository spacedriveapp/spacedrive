//! This module contains constant values, functions and types that are used around the crate.
//!
//! This includes things such as cryptographically-secure random salt/master key/nonce generation,
//! lengths for master keys and even the STREAM block size.
use zeroize::Zeroize;

use crate::{
	header::{
		file::FileHeaderVersion, keyslot::KeyslotVersion, metadata::MetadataVersion,
		preview_media::PreviewMediaVersion,
	},
	keys::keymanager::StoredKeyVersion,
	Error, Result,
};

pub mod types;

/// This is the salt size.
pub const SALT_LEN: usize = 16;

/// The length of the secret key, in bytes.
pub const SECRET_KEY_LEN: usize = 18;

/// The block size used for STREAM encryption/decryption. This size seems to offer the best performance compared to alternatives.
///
/// The file size gain is 16 bytes per 1048576 bytes (due to the AEAD tag), plus the size of the header.
pub const BLOCK_LEN: usize = 1_048_576;

/// This is the default AEAD tag size for all encryption algorithms used within the crate.
pub const AEAD_TAG_LEN: usize = 16;

/// The length of encrypted master keys (`KEY_LEN` + `AEAD_TAG_LEN`)
pub const ENCRYPTED_KEY_LEN: usize = 48;

/// The length of plain master/hashed keys
pub const KEY_LEN: usize = 32;

/// Used for OS keyrings to identify our items.
pub const APP_IDENTIFIER: &str = "Spacedrive";

/// Used for OS keyrings to identify our items.
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

/// Defines the latest `FileHeaderVersion`
pub const LATEST_FILE_HEADER: FileHeaderVersion = FileHeaderVersion::V1;

/// Defines the latest `KeyslotVersion`
pub const LATEST_KEYSLOT: KeyslotVersion = KeyslotVersion::V1;

/// Defines the latest `MetadataVersion`
pub const LATEST_METADATA: MetadataVersion = MetadataVersion::V1;

/// Defines the latest `PreviewMediaVersion`
pub const LATEST_PREVIEW_MEDIA: PreviewMediaVersion = PreviewMediaVersion::V1;

/// Defines the latest `StoredKeyVersion`
pub const LATEST_STORED_KEY: StoredKeyVersion = StoredKeyVersion::V1;

/// Defines the context string for BLAKE3-KDF in regards to root key derivation
pub const ROOT_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:53:54 root key derivation";

/// Defines the context string for BLAKE3-KDF in regards to master password hash derivation
pub const MASTER_PASSWORD_CONTEXT: &str =
	"spacedrive 2022-12-14 15:35:41 master password hash derivation";

/// Defines the context string for BLAKE3-KDF in regards to file key derivation (for file encryption)
pub const FILE_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:54:12 file key derivation";

/// This is used for converting a `&[u8]` to an array of bytes.
///
/// It does `Clone`, with `to_vec()`.
///
/// This function calls `zeroize` on any data it can
pub fn to_array<const I: usize>(bytes: &[u8]) -> Result<[u8; I]> {
	bytes.to_vec().try_into().map_err(|mut b: Vec<u8>| {
		b.zeroize();
		Error::VecArrSizeMismatch
	})
}
