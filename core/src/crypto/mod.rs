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

pub(crate) mod keymanager;
