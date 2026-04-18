//! OAuth 2.0 provider abstraction.
//!
//! Concrete providers (OneDrive, Google Drive, Dropbox) implement [`OauthProvider`]
//! with their own issuer URL, scope set, and loopback port constraints. The trait
//! is deliberately stateless per BYO: `client_id`/`client_secret` travel as method
//! parameters, never stored on `self`.

use super::error::OauthError;
use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Access / refresh token pair returned by the provider after code exchange or refresh.
///
/// The `expires_at` field is computed from the provider's `expires_in` at response
/// time so callers do not have to re-translate relative expiry into an absolute one.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TokenSet {
	/// Short-lived bearer token used to call the provider's API.
	pub access_token: String,

	/// Long-lived refresh token. Optional because some providers rotate on every
	/// refresh and may return `None` if the user revokes offline access mid-flow.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub refresh_token: Option<String>,

	/// Absolute expiry of `access_token` in UTC.
	pub expires_at: chrono::DateTime<chrono::Utc>,

	/// Space-separated list of scopes actually granted (may differ from requested).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub scope: Option<String>,
}

/// Generic OAuth 2.0 provider abstraction.
///
/// Each concrete provider implements this trait with its own issuer URL, token
/// endpoint, and scope set. All flows are BYO: the caller supplies `client_id`
/// and `client_secret` at every method call, so nothing is stored on `self`
/// beyond provider identity.
#[async_trait]
pub trait OauthProvider: Send + Sync + 'static {
	/// Stable string identifying this provider (e.g. `"onedrive"`, `"gdrive"`).
	fn id(&self) -> &'static str;

	/// Build the authorization URL the user's browser opens.
	///
	/// `redirect_uri` is the loopback URI Spacedrive will listen on. `state` is
	/// the CSRF token Spacedrive generates per flow. `pkce_challenge` is the
	/// S256 PKCE challenge — the verifier is retained by the flow.
	fn build_auth_url(
		&self,
		client_id: &str,
		redirect_uri: &str,
		state: &str,
		pkce_challenge: &str,
	) -> String;

	/// Exchange the authorization code for tokens.
	async fn exchange_code(
		&self,
		client_id: &str,
		client_secret: &str,
		redirect_uri: &str,
		code: &str,
		pkce_verifier: &str,
	) -> Result<TokenSet, OauthError>;

	/// Refresh an access token using a refresh token.
	///
	/// May return an updated `refresh_token` — callers MUST persist it if so
	/// (some providers rotate refresh tokens on every refresh).
	async fn refresh(
		&self,
		client_id: &str,
		client_secret: &str,
		refresh_token: &str,
	) -> Result<TokenSet, OauthError>;

	/// Candidate loopback ports for this provider's registered redirect URIs.
	///
	/// Microsoft requires exact-match redirect URIs; most other providers accept
	/// any ephemeral port. Providers that require exact-match return their
	/// registered ports; providers that accept any return `&[]` to mean
	/// "choose any free port".
	fn loopback_ports(&self) -> &[u16] {
		&[]
	}

	/// After token exchange, fetch a friendly display name from the provider's
	/// user API (e.g. Microsoft Graph `/me`). Used to label the volume in the UI.
	///
	/// Default implementation returns `None` so providers without a user-info
	/// endpoint do not have to implement this.
	async fn display_name(&self, access_token: &str) -> Result<Option<String>, OauthError> {
		let _ = access_token;
		Ok(None)
	}
}

/// Registry mapping provider id → `Arc<dyn OauthProvider>`.
///
/// Starts empty; concrete providers are registered at core startup (Set 4 and
/// later). The registry is cheap to clone because it wraps an `Arc<RwLock<..>>`.
#[derive(Clone, Default)]
pub struct OauthProviderRegistry {
	inner: Arc<RwLock<HashMap<&'static str, Arc<dyn OauthProvider>>>>,
}

impl OauthProviderRegistry {
	/// Construct an empty registry.
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a provider. Later registrations for the same id overwrite earlier
	/// ones — useful for tests that swap in a mock provider.
	pub async fn register(&self, provider: Arc<dyn OauthProvider>) {
		let id = provider.id();
		self.inner.write().await.insert(id, provider);
	}

	/// Look up a provider by id; returns `None` if not registered.
	pub async fn get(&self, id: &str) -> Option<Arc<dyn OauthProvider>> {
		self.inner.read().await.get(id).cloned()
	}

	/// List all registered provider ids — used by the refresh task to avoid
	/// refreshing credentials for providers the registry does not know about.
	pub async fn registered_ids(&self) -> Vec<&'static str> {
		self.inner.read().await.keys().copied().collect()
	}
}

/// Produce the base64url-no-pad encoding of the SHA-256 digest of `verifier`.
///
/// Exposed so tests and providers can reuse the same challenge construction
/// without repeating the base64+sha2 boilerplate.
pub fn pkce_s256_challenge(verifier: &str) -> String {
	let digest = Sha256::digest(verifier.as_bytes());
	base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;

	/// Mock provider used by action-level tests. Not registered globally —
	/// consumers construct their own instance and inject it into a fresh
	/// `OauthProviderRegistry`.
	pub struct MockOauthProvider {
		pub id: &'static str,
		pub loopback_ports: Vec<u16>,
	}

	impl MockOauthProvider {
		pub fn new(id: &'static str) -> Self {
			Self {
				id,
				loopback_ports: Vec::new(),
			}
		}
	}

	#[async_trait]
	impl OauthProvider for MockOauthProvider {
		fn id(&self) -> &'static str {
			self.id
		}

		fn build_auth_url(
			&self,
			client_id: &str,
			redirect_uri: &str,
			state: &str,
			_c: &str,
		) -> String {
			format!("https://mock.test/authorize?client_id={client_id}&redirect_uri={redirect_uri}&state={state}")
		}

		async fn exchange_code(
			&self,
			_cid: &str,
			_csec: &str,
			_ruri: &str,
			code: &str,
			_verif: &str,
		) -> Result<TokenSet, OauthError> {
			Ok(TokenSet {
				access_token: format!("access-{code}"),
				refresh_token: Some(format!("refresh-{code}")),
				expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
				scope: None,
			})
		}

		async fn refresh(
			&self,
			_cid: &str,
			_csec: &str,
			refresh_token: &str,
		) -> Result<TokenSet, OauthError> {
			Ok(TokenSet {
				access_token: format!("access-refreshed-{refresh_token}"),
				refresh_token: Some(format!("{refresh_token}-rotated")),
				expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
				scope: None,
			})
		}

		fn loopback_ports(&self) -> &[u16] {
			&self.loopback_ports
		}

		async fn display_name(&self, _access_token: &str) -> Result<Option<String>, OauthError> {
			Ok(Some("Mock User".to_string()))
		}
	}

	/// RFC 7636 Appendix B test vector: verifier => challenge must match exactly.
	#[test]
	fn test_pkce_s256_challenge_format() {
		let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
		let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
		assert_eq!(pkce_s256_challenge(verifier), expected);
	}

	#[tokio::test]
	async fn test_registry_register_and_get() {
		let reg = OauthProviderRegistry::new();
		assert!(reg.get("mock").await.is_none());
		reg.register(Arc::new(MockOauthProvider::new("mock"))).await;
		let fetched = reg.get("mock").await.expect("mock should be registered");
		assert_eq!(fetched.id(), "mock");
	}

	#[tokio::test]
	async fn test_registry_overwrites_on_duplicate_id() {
		let reg = OauthProviderRegistry::new();
		reg.register(Arc::new(MockOauthProvider::new("mock"))).await;
		reg.register(Arc::new(MockOauthProvider::new("mock"))).await;
		assert_eq!(reg.registered_ids().await, vec!["mock"]);
	}
}
