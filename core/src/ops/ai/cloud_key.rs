//! The bring-your-own-key caption key, stored encrypted with the library key
//! in the library database like cloud volume credentials. It is kept apart
//! from the client so builds without local inference can still configure it.

use super::AiError;
use crate::crypto::cloud_credentials::{CloudCredential, CloudCredentialManager, CredentialData};
use crate::library::Library;
use crate::volume::CloudServiceType;

/// The credential slot the caption key is stored under.
const KEY_SLOT: &str = "ai:image_caption";

fn credentials(library: &Library) -> CloudCredentialManager {
	CloudCredentialManager::new(
		library.core_context().key_manager.clone(),
		library.db().clone(),
		library.id(),
	)
}

pub async fn store_key(library: &Library, key: String) -> Result<(), AiError> {
	let credential = CloudCredential::new_api_key(CloudServiceType::Other, key);
	credentials(library)
		.store_credential(library.id(), KEY_SLOT, &credential)
		.await
		.map_err(|e| AiError::Cloud(e.to_string()))
}

pub async fn clear_key(library: &Library) -> Result<(), AiError> {
	let manager = credentials(library);
	tokio::task::block_in_place(|| manager.delete_credential(library.id(), KEY_SLOT))
		.map_err(|e| AiError::Cloud(e.to_string()))
}

pub async fn has_key(library: &Library) -> bool {
	load(library).await.is_some()
}

pub(super) async fn load(library: &Library) -> Option<String> {
	let credential = credentials(library)
		.get_credential(library.id(), KEY_SLOT)
		.await
		.ok()?;
	match credential.data {
		CredentialData::ApiKey(key) => Some(key),
		_ => None,
	}
}
