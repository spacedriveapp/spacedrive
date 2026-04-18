//! `cloud.oauth.start` — begin an OAuth 2.0 browser sign-in flow.
//!
//! Registered as a library action because cloud volumes always live within a
//! library scope. The action is synchronous-ish: it binds a loopback listener,
//! generates PKCE+state, and spawns a detached task to drive the flow to
//! completion. The UI polls `cloud.oauth.poll` to observe progress.

use super::super::{
	error::OauthError,
	flow::{OauthFlow, OauthFlowStatus},
	provider::pkce_s256_challenge,
};
use super::complete::{complete_flow, CompleteParams};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
};
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;
use uuid::Uuid;

/// Input for `cloud.oauth.start`.
///
/// `client_id` and `client_secret` are BYO — Spacedrive never ships hardcoded
/// public clients. The UI guides the user through creating their own app in
/// the provider's developer console.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StartInput {
	/// Provider id matching [`crate::ops::cloud::oauth::OauthProvider::id`]
	/// (e.g. `"onedrive"`, `"gdrive"`).
	pub provider: String,
	/// User's OAuth client id.
	pub client_id: String,
	/// User's OAuth client secret.
	pub client_secret: String,
}

/// Output for `cloud.oauth.start`.
///
/// `auth_url` is opened in the system browser by the core (best-effort) and
/// also returned so the UI can expose a "copy link" fallback when the
/// automatic launch fails.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StartOutput {
	/// Identifier the UI passes to `cloud.oauth.poll` / `cloud.oauth.cancel`.
	pub flow_id: Uuid,
	/// URL the user must visit to consent.
	pub auth_url: String,
	/// Loopback redirect URI used by this flow; surfaced for debugging and
	/// for the "copy redirect URI" button in the tutorial UI.
	pub redirect_uri: String,
}

/// Library action entry point (see module-level doc).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudOauthStartAction {
	input: StartInput,
}

impl CloudOauthStartAction {
	/// Construct the action from its wire input.
	pub fn new(input: StartInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for CloudOauthStartAction {
	type Input = StartInput;
	type Output = StartOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self::new(input))
	}

	async fn execute(
		self,
		_library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		validate_inputs(&self.input).map_err(|e| ActionError::InvalidInput(e.to_string()))?;

		let provider = context
			.oauth_providers
			.get(&self.input.provider)
			.await
			.ok_or_else(|| {
				ActionError::InvalidInput(
					OauthError::UnknownProvider(self.input.provider.clone()).to_string(),
				)
			})?;

		let (listener, redirect_uri) = bind_listener(provider.loopback_ports())
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;

		let flow_id = Uuid::new_v4();
		let state = random_urlsafe(32);
		let pkce_verifier = random_urlsafe(32);
		let pkce_challenge = pkce_s256_challenge(&pkce_verifier);

		let auth_url = provider.build_auth_url(
			&self.input.client_id,
			&redirect_uri,
			&state,
			&pkce_challenge,
		);

		let (cancel_tx, cancel_rx) = watch::channel(false);
		let flow = OauthFlow {
			id: flow_id,
			provider_id: self.input.provider.clone(),
			client_id: self.input.client_id.clone(),
			client_secret: self.input.client_secret.clone(),
			redirect_uri: redirect_uri.clone(),
			state: state.clone(),
			pkce_verifier,
			created_at: chrono::Utc::now(),
			terminal_at: None,
			status: OauthFlowStatus::Pending,
		};
		context.oauth_flows.insert(flow, cancel_tx);

		// Best-effort browser launch. The UI always shows a copyable link in
		// case the OS has no default browser or the launch silently fails.
		if let Err(e) = webbrowser::open(&auth_url) {
			tracing::warn!(%flow_id, error = %e, "failed to open system browser; UI should surface the copyable URL");
		}

		let store = context.oauth_flows.clone();
		tokio::spawn(async move {
			complete_flow(CompleteParams {
				flow_id,
				listener,
				expected_state: state,
				cancel: cancel_rx,
				store,
				provider,
			})
			.await;
		});

		Ok(StartOutput {
			flow_id,
			auth_url,
			redirect_uri,
		})
	}

	fn action_kind(&self) -> &'static str {
		"cloud.oauth.start"
	}
}

crate::register_library_action!(CloudOauthStartAction, "cloud.oauth.start");

/// Validate BYO inputs upfront so callers get a precise error rather than a
/// downstream token-exchange failure. Mirrors the pattern in
/// `volumes.add_cloud` at `core/src/ops/volumes/add_cloud/action.rs:168-175`.
fn validate_inputs(input: &StartInput) -> Result<(), OauthError> {
	if input.provider.trim().is_empty() {
		return Err(OauthError::InvalidClient(
			"provider id is required".to_string(),
		));
	}
	if input.client_id.trim().is_empty() {
		return Err(OauthError::InvalidClient(
			"client_id is required".to_string(),
		));
	}
	if input.client_secret.trim().is_empty() {
		return Err(OauthError::InvalidClient(
			"client_secret is required".to_string(),
		));
	}
	Ok(())
}

/// Bind a loopback listener on one of `preferred_ports` (exact-match providers
/// like Microsoft) or an OS-assigned ephemeral port (most others).
///
/// Returns the bound `TcpListener` and the canonical `http://127.0.0.1:{port}`
/// redirect URI.
async fn bind_listener(preferred_ports: &[u16]) -> Result<(TcpListener, String), OauthError> {
	if preferred_ports.is_empty() {
		let listener = TcpListener::bind("127.0.0.1:0")
			.await
			.map_err(|e| OauthError::Loopback(e.to_string()))?;
		let port = listener
			.local_addr()
			.map_err(|e| OauthError::Loopback(e.to_string()))?
			.port();
		return Ok((listener, format!("http://127.0.0.1:{port}")));
	}

	for &port in preferred_ports {
		if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
			return Ok((listener, format!("http://127.0.0.1:{port}")));
		}
	}
	Err(OauthError::NoAvailablePort)
}

/// 32 bytes of OS randomness encoded as base64url-no-pad.
///
/// Used for both CSRF state and PKCE verifier so both have ≥ 192 bits of
/// entropy, well above the RFC 7636 minimum of 43 chars / 256 bits.
fn random_urlsafe(bytes: usize) -> String {
	let mut buf = vec![0u8; bytes];
	rand::thread_rng().fill_bytes(&mut buf);
	base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_start_rejects_empty_client_id() {
		let input = StartInput {
			provider: "onedrive".to_string(),
			client_id: "  ".to_string(),
			client_secret: "secret".to_string(),
		};
		let err = validate_inputs(&input).unwrap_err();
		assert!(matches!(err, OauthError::InvalidClient(_)));
	}

	#[test]
	fn test_start_rejects_empty_client_secret() {
		let input = StartInput {
			provider: "onedrive".to_string(),
			client_id: "cid".to_string(),
			client_secret: "".to_string(),
		};
		let err = validate_inputs(&input).unwrap_err();
		assert!(matches!(err, OauthError::InvalidClient(_)));
	}

	#[test]
	fn test_random_urlsafe_is_urlsafe() {
		let s = random_urlsafe(32);
		assert!(!s.is_empty());
		// base64url-no-pad contains only A-Za-z0-9-_
		assert!(s
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
	}

	#[tokio::test]
	async fn test_bind_listener_ephemeral() {
		let (listener, redirect) = bind_listener(&[]).await.unwrap();
		assert!(redirect.starts_with("http://127.0.0.1:"));
		drop(listener);
	}

	#[tokio::test]
	async fn test_bind_listener_preferred_port() {
		// Bind an ephemeral first to learn a port, drop it, then request it as
		// the "preferred" port. This avoids hard-coding a possibly-occupied port.
		let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let port = probe.local_addr().unwrap().port();
		drop(probe);

		let (listener, redirect) = bind_listener(&[port]).await.unwrap();
		assert_eq!(redirect, format!("http://127.0.0.1:{port}"));
		drop(listener);
	}
}
