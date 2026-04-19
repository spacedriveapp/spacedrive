//! Background task that refreshes cloud OAuth tokens before they expire.
//!
//! Iterates over every `cloud_credentials` row whose credential variant is
//! `CredentialData::OAuth` and whose expiry is within [`REFRESH_BEFORE`] of
//! now. For each matching row, calls the registered provider's `refresh`, then
//! writes the rotated token set back through `CloudCredentialManager`.
//!
//! Tolerates providers that are not yet registered (the registry starts empty
//! until Set 4 registers OneDrive): unknown providers are simply skipped so
//! adding credentials pre-registration does not produce refresh errors.
//!
//! ## Out of scope
//! `TODO(cloud-mvp): hot-swap CloudBackend on token refresh — see
//! .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#set-3`.
//! Currently, an in-memory `CloudBackend` continues to hold the stale token
//! until the volume is reloaded. Next steps: expose
//! `VolumeManager::reload_credentials(volume_id)` that rebuilds the backend
//! and atomically swaps it.

use super::{
	error::OauthError,
	provider::{OauthProvider, OauthProviderRegistry, TokenSet},
};
use crate::{
	crypto::cloud_credentials::{CloudCredential, CloudCredentialManager, CredentialData},
	crypto::key_manager::KeyManager,
	library::LibraryManager,
	volume::CloudServiceType,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use uuid::Uuid;

/// How often the refresh loop wakes up. A tight loop would pointlessly hit the
/// database; the longest any access token should linger past expiry is a
/// single tick, so 60s is a reasonable trade-off.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// Refresh tokens that expire in this window or sooner. Five minutes matches
/// typical provider clock-skew tolerance and gives us retry headroom before
/// the token becomes unusable.
pub const REFRESH_BEFORE: chrono::Duration = chrono::Duration::minutes(5);

/// Run the refresh loop forever. Intended to be spawned once at startup.
///
/// The task never returns under normal conditions — it is a supervisor loop.
/// Errors are logged; the loop keeps ticking so a transient network blip does
/// not permanently stop refresh.
pub async fn run_refresh_task(
	library_manager: Arc<RwLock<Option<Arc<LibraryManager>>>>,
	key_manager: Arc<KeyManager>,
	registry: OauthProviderRegistry,
) -> ! {
	let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
	ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
	loop {
		ticker.tick().await;
		match tick_once(&library_manager, &key_manager, &registry).await {
			Ok(refreshed) if refreshed > 0 => {
				tracing::info!(refreshed, "cloud oauth refresh tick rotated tokens");
			}
			Ok(_) => {}
			Err(e) => tracing::warn!(error = %e, "cloud oauth refresh tick failed"),
		}
	}
}

/// One pass of the refresh loop. Exposed for targeted testing once we have a
/// test harness that can spin up a fake library — today it stays private to
/// this module.
async fn tick_once(
	library_manager: &Arc<RwLock<Option<Arc<LibraryManager>>>>,
	key_manager: &Arc<KeyManager>,
	registry: &OauthProviderRegistry,
) -> anyhow::Result<usize> {
	let Some(libraries) = library_manager.read().await.clone() else {
		return Ok(0);
	};

	let mut refreshed = 0usize;
	for library in libraries.list().await {
		let cred_manager =
			CloudCredentialManager::new(key_manager.clone(), library.db().clone(), library.id());

		// `list_credentials` returns volume fingerprints; we iterate over them
		// and fetch each credential individually. Small N (credentials are
		// rare), so the per-row decrypt cost is negligible.
		let fingerprints = match cred_manager.list_credentials().await {
			Ok(v) => v,
			Err(e) => {
				tracing::warn!(library = %library.id(), error = %e, "failed to list cloud credentials");
				continue;
			}
		};

		for fingerprint in fingerprints {
			match refresh_one(&cred_manager, &fingerprint, library.id(), registry).await {
				Ok(true) => refreshed += 1,
				Ok(false) => {}
				Err(e) => {
					// Refresh failures do not stop the loop: surface a warning
					// so operators can investigate per-volume.
					tracing::warn!(
						library = %library.id(),
						volume_fingerprint = %fingerprint,
						error = %e,
						"cloud oauth refresh failed for credential"
					);
				}
			}
		}
	}
	Ok(refreshed)
}

/// Attempt to refresh one credential. Returns `Ok(true)` if a refresh was
/// actually performed, `Ok(false)` if the credential did not need refreshing
/// (or the provider was unknown and the row was skipped).
async fn refresh_one(
	cred_manager: &CloudCredentialManager,
	fingerprint: &str,
	library_id: Uuid,
	registry: &OauthProviderRegistry,
) -> Result<bool, OauthError> {
	let credential = cred_manager
		.get_credential(library_id, fingerprint)
		.await
		.map_err(|e| OauthError::Refresh(e.to_string()))?;

	let Some(refresh_token) = extract_refresh_token(&credential) else {
		return Ok(false);
	};

	if !should_refresh(&credential) {
		return Ok(false);
	}

	let Some(provider) = registry.get(provider_id_for(credential.service)).await else {
		// Provider not yet registered in this build; skip silently. This is
		// the expected state for OneDrive credentials created in Set 4 UIs
		// running against a core that only registers Google Drive.
		return Ok(false);
	};

	let CredentialData::OAuth {
		client_id,
		client_secret,
		..
	} = &credential.data
	else {
		return Ok(false);
	};

	let tokens = provider
		.refresh(client_id, client_secret, &refresh_token)
		.await?;

	store_refreshed(
		cred_manager,
		fingerprint,
		library_id,
		&credential,
		tokens,
		&provider,
	)
	.await?;
	Ok(true)
}

fn should_refresh(credential: &CloudCredential) -> bool {
	match credential.expires_at {
		// No known expiry (e.g. Dropbox flows that rely on OpenDAL-managed
		// access tokens): skip, the backend refreshes internally.
		None => false,
		Some(expires_at) => chrono::Utc::now() + REFRESH_BEFORE >= expires_at,
	}
}

fn extract_refresh_token(credential: &CloudCredential) -> Option<String> {
	match &credential.data {
		CredentialData::OAuth { refresh_token, .. } if !refresh_token.is_empty() => {
			Some(refresh_token.clone())
		}
		_ => None,
	}
}

/// Map a `CloudServiceType` to the provider id used in `OauthProviderRegistry`.
///
/// Keeps the registry key surface small (`&'static str`) while still letting
/// the credential system speak in enum terms.
fn provider_id_for(service: CloudServiceType) -> &'static str {
	match service {
		CloudServiceType::OneDrive => "onedrive",
		CloudServiceType::GoogleDrive => "gdrive",
		CloudServiceType::Dropbox => "dropbox",
		// S3 / Azure / GCS do not use OAuth refresh tokens.
		_ => "",
	}
}

async fn store_refreshed(
	cred_manager: &CloudCredentialManager,
	fingerprint: &str,
	library_id: Uuid,
	prior: &CloudCredential,
	tokens: TokenSet,
	_provider: &Arc<dyn OauthProvider>,
) -> Result<(), OauthError> {
	let CredentialData::OAuth {
		client_id,
		client_secret,
		refresh_token: prior_refresh,
		..
	} = &prior.data
	else {
		return Ok(());
	};

	// Some providers rotate refresh tokens on every refresh; others keep the
	// same one. Persist whichever the provider returned, falling back to the
	// prior token when the provider omitted the field.
	let new_refresh = tokens
		.refresh_token
		.clone()
		.unwrap_or_else(|| prior_refresh.clone());

	let updated = CloudCredential::new_oauth(
		prior.service,
		tokens.access_token,
		new_refresh,
		client_id.clone(),
		client_secret.clone(),
		Some(tokens.expires_at),
	);

	cred_manager
		.store_credential(library_id, fingerprint, &updated)
		.await
		.map_err(|e| OauthError::Refresh(e.to_string()))?;

	tracing::debug!(library = %library_id, volume_fingerprint = %fingerprint, "rotated cloud oauth credential");
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample_oauth(expires_in_secs: i64) -> CloudCredential {
		CloudCredential::new_oauth(
			CloudServiceType::OneDrive,
			"access".to_string(),
			"refresh".to_string(),
			"cid".to_string(),
			"csec".to_string(),
			Some(chrono::Utc::now() + chrono::Duration::seconds(expires_in_secs)),
		)
	}

	#[test]
	fn test_should_refresh_when_near_expiry() {
		assert!(should_refresh(&sample_oauth(60)));
	}

	#[test]
	fn test_should_not_refresh_when_fresh() {
		assert!(!should_refresh(&sample_oauth(3600)));
	}

	#[test]
	fn test_should_not_refresh_without_expiry() {
		let cred = CloudCredential::new_oauth(
			CloudServiceType::Dropbox,
			"".to_string(),
			"refresh".to_string(),
			"cid".to_string(),
			"csec".to_string(),
			None,
		);
		assert!(!should_refresh(&cred));
	}

	#[test]
	fn test_extract_refresh_token_ok() {
		let cred = sample_oauth(60);
		assert_eq!(extract_refresh_token(&cred), Some("refresh".to_string()));
	}

	#[test]
	fn test_extract_refresh_token_empty() {
		let cred = CloudCredential::new_oauth(
			CloudServiceType::Dropbox,
			"".to_string(),
			"".to_string(),
			"cid".to_string(),
			"csec".to_string(),
			None,
		);
		assert_eq!(extract_refresh_token(&cred), None);
	}

	#[test]
	fn test_extract_refresh_token_non_oauth() {
		let cred = CloudCredential::new_api_key(CloudServiceType::GoogleCloudStorage, "k".into());
		assert_eq!(extract_refresh_token(&cred), None);
	}

	#[test]
	fn test_provider_id_for() {
		assert_eq!(provider_id_for(CloudServiceType::OneDrive), "onedrive");
		assert_eq!(provider_id_for(CloudServiceType::GoogleDrive), "gdrive");
		assert_eq!(provider_id_for(CloudServiceType::Dropbox), "dropbox");
		assert_eq!(provider_id_for(CloudServiceType::S3), "");
	}
}
