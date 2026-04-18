//! Error types for the OAuth 2.0 subsystem.
//!
//! Every variant maps a specific failure mode the UI needs to distinguish:
//! CSRF mismatches and user denials are surfaced so the frontend can show
//! tailored messages instead of a generic "something went wrong".

use thiserror::Error;
use uuid::Uuid;

/// Errors raised by OAuth flow setup, loopback capture, token exchange, and refresh.
#[derive(Debug, Error)]
pub enum OauthError {
	/// The requested provider id is not registered in `OauthProviderRegistry`.
	#[error("Unknown provider: {0}")]
	UnknownProvider(String),

	/// The flow id was never issued or has been evicted by the janitor.
	#[error("No flow with id {0}")]
	UnknownFlow(Uuid),

	/// All registered loopback ports were occupied; the user should retry later.
	#[error("Loopback port unavailable: all registered ports are occupied")]
	NoAvailablePort,

	/// The callback's `state` parameter did not match the one issued at start.
	///
	/// Treat as a CSRF attempt: abort the flow and log a warning.
	#[error("State parameter mismatch (possible CSRF attack)")]
	StateMismatch,

	/// The provider returned `error=access_denied` in the callback.
	#[error("User denied authorization")]
	UserDenied,

	/// Token endpoint returned an error or malformed body.
	#[error("Token exchange failed: {0}")]
	TokenExchange(String),

	/// Refresh endpoint failed — callers should mark the credential as needing re-auth.
	#[error("Refresh failed: {0}")]
	Refresh(String),

	/// Internal loopback server failure (bind / accept / parse).
	#[error("Loopback server error: {0}")]
	Loopback(String),

	/// No callback arrived within the configured window (default 5 minutes).
	#[error("Timeout waiting for callback")]
	Timeout,

	/// `client_id`/`client_secret`/`redirect_uri` validation failed before dispatch.
	#[error("Client configuration invalid: {0}")]
	InvalidClient(String),

	/// HTTP transport error when calling the provider's token or userinfo endpoint.
	#[error("HTTP error: {0}")]
	Http(String),
}

impl From<reqwest::Error> for OauthError {
	fn from(value: reqwest::Error) -> Self {
		OauthError::Http(value.to_string())
	}
}
