//! This module contains constant values and functions that are used around the crate.
//!
//! This includes things such as cryptographically-secure random salt/master key/nonce generation,
//! lengths for master keys and even the streaming block size.
use zeroize::Zeroize;

use crate::{
	header::{
		file::FileHeaderVersion, keyslot::KeyslotVersion, metadata::MetadataVersion,
		preview_media::PreviewMediaVersion,
	},
	keys::keymanager::StoredKeyVersion,
	Error, Result,
};

// pub mod rng;
pub mod types;

/// This is the default salt size, and the recommended size for argon2id.
pub const SALT_LEN: usize = 16;

pub const SECRET_KEY_LEN: usize = 18;

/// The size used for streaming encryption/decryption. This size seems to offer the best performance compared to alternatives.
///
/// The file size gain is 16 bytes per 1048576 bytes (due to the AEAD tag). Plus the size of the header.
pub const BLOCK_SIZE: usize = 1_048_576;

pub const AEAD_TAG_SIZE: usize = 16;

/// The length of the encrypted master key
pub const ENCRYPTED_KEY_LEN: usize = 48;

/// The length of the (unencrypted) master key
pub const KEY_LEN: usize = 32;

pub const PASSPHRASE_LEN: usize = 7;

pub const APP_IDENTIFIER: &str = "Spacedrive";
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

pub const LATEST_FILE_HEADER: FileHeaderVersion = FileHeaderVersion::V1;
pub const LATEST_KEYSLOT: KeyslotVersion = KeyslotVersion::V1;
pub const LATEST_METADATA: MetadataVersion = MetadataVersion::V1;
pub const LATEST_PREVIEW_MEDIA: PreviewMediaVersion = PreviewMediaVersion::V1;
pub const LATEST_STORED_KEY: StoredKeyVersion = StoredKeyVersion::V1;

pub const ROOT_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:53:54 root key derivation"; // used for deriving keys from the root key
pub const MASTER_PASSWORD_CONTEXT: &str =
	"spacedrive 2022-12-14 15:35:41 master password hash derivation"; // used for deriving keys from the master password hash
pub const FILE_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:54:12 file key derivation"; // used for deriving keys from user key/content salt hashes (for file encryption)

/// This is used for converting a `Vec<u8>` to an array of bytes
///
/// It's main usage is for converting an encrypted master key from a `Vec<u8>` to `EncryptedKey`
///
/// As the master key is encrypted at this point, it does not need to be `Protected<>`
///
/// This function still `zeroize`s any data it can
pub fn to_array<const I: usize>(bytes: &[u8]) -> Result<[u8; I]> {
	bytes.to_vec().try_into().map_err(|mut b: Vec<u8>| {
		b.zeroize();
		Error::VecArrSizeMismatch
	})
}
