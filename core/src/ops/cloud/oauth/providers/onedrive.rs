//! Microsoft OneDrive (consumer / `tenant=common`) OAuth 2.0 provider.
//!
//! Targets personal Microsoft accounts via the common endpoint so a single
//! BYO app registration works for every consumer user. Business / SharePoint
//! accounts require a tenant-specific issuer and are intentionally out of
//! scope for the MVP (see
//! `.investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#pr-4`).
//!
//! Loopback ports are the fixed set Microsoft enforces on exact-match redirect
//! URIs for desktop apps. The UI tutorial instructs users to register all five
//! ports so that `loopback::bind_available` can fall back when a port is busy.

use super::super::{
	error::OauthError,
	provider::{OauthProvider, TokenSet},
};
use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;
use std::time::Duration;

/// Redirect URIs Microsoft accepts for Spacedrive's desktop flow.
///
/// Microsoft requires exact-match registered URIs, so the app registration
/// MUST list all five. We attempt them in order; the first free port wins.
const LOOPBACK_PORTS: &[u16] = &[53682, 53683, 53684, 53685, 53686];

/// Space-separated scope string. `offline_access` is required to receive a
/// `refresh_token`; `User.Read` is needed for the Graph `/me` display name
/// lookup that labels the volume in the UI.
const SCOPES: &str = "Files.ReadWrite.All offline_access User.Read";

/// Microsoft identity platform provider for OneDrive personal accounts.
///
/// Endpoints are stored on the struct so tests can swap them for a
/// [`wiremock::MockServer`]. In production the defaults from [`Self::new`] are
/// always used.
pub struct OneDriveProvider {
	auth_url: String,
	token_url: String,
	graph_url: String,
	http: reqwest::Client,
}

impl Default for OneDriveProvider {
	fn default() -> Self {
		Self::new()
	}
}

impl OneDriveProvider {
	/// Construct the provider with Microsoft's production endpoints.
	pub fn new() -> Self {
		Self::new_with_endpoints(
			"https://login.microsoftonline.com/common/oauth2/v2.0/authorize".into(),
			"https://login.microsoftonline.com/common/oauth2/v2.0/token".into(),
			"https://graph.microsoft.com/v1.0/me".into(),
		)
	}

	/// Testability hook — see tests in this module.
	pub(crate) fn new_with_endpoints(
		auth_url: String,
		token_url: String,
		graph_url: String,
	) -> Self {
		let http = reqwest::Client::builder()
			.timeout(Duration::from_secs(30))
			.build()
			.expect("reqwest client builds");
		Self {
			auth_url,
			token_url,
			graph_url,
			http,
		}
	}
}

/// Raw token response parsed from Microsoft's `/token` endpoint.
///
/// `refresh_token` is optional on refresh responses because Microsoft may or
/// may not rotate it; the caller is responsible for preserving the prior
/// refresh token in that case.
#[derive(Debug, Deserialize)]
struct TokenResponse {
	access_token: String,
	#[serde(default)]
	refresh_token: Option<String>,
	expires_in: i64,
	#[serde(default)]
	scope: Option<String>,
	#[serde(default)]
	token_type: Option<String>,
}

/// Minimal projection of the Graph `/me` payload.
///
/// `displayName` is the preferred label; `userPrincipalName` is the fallback
/// (it's always populated on Microsoft accounts whereas `displayName` may be
/// blank for newly-created identities).
#[derive(Debug, Deserialize)]
struct GraphMeResponse {
	#[serde(rename = "displayName")]
	display_name: Option<String>,
	#[serde(rename = "userPrincipalName")]
	user_principal_name: Option<String>,
}

#[async_trait]
impl OauthProvider for OneDriveProvider {
	fn id(&self) -> &'static str {
		"onedrive"
	}

	fn loopback_ports(&self) -> &'static [u16] {
		LOOPBACK_PORTS
	}

	fn build_auth_url(
		&self,
		client_id: &str,
		redirect_uri: &str,
		state: &str,
		pkce_challenge: &str,
	) -> String {
		// `Url::parse_with_params` handles percent-encoding for every value —
		// critical for `redirect_uri`, `scope`, and `state`.
		let mut url = match Url::parse(&self.auth_url) {
			Ok(u) => u,
			Err(e) => {
				// `auth_url` is a hard-coded constant in production; a parse
				// error indicates a test misconfiguration. Return the raw
				// string so the caller surfaces a meaningful error instead of
				// panicking on a URL that simply will not resolve.
				tracing::error!(error = %e, auth_url = %self.auth_url, "onedrive auth_url invalid");
				return self.auth_url.clone();
			}
		};
		url.query_pairs_mut()
			.append_pair("client_id", client_id)
			.append_pair("response_type", "code")
			.append_pair("redirect_uri", redirect_uri)
			.append_pair("response_mode", "query")
			.append_pair("scope", SCOPES)
			.append_pair("state", state)
			.append_pair("code_challenge", pkce_challenge)
			.append_pair("code_challenge_method", "S256")
			// `select_account` forces the account chooser for users with
			// multiple Microsoft identities; without it, Microsoft silently
			// reuses a cached session which is a surprising UX for users
			// adding a second OneDrive to Spacedrive.
			.append_pair("prompt", "select_account");
		url.to_string()
	}

	async fn exchange_code(
		&self,
		client_id: &str,
		client_secret: &str,
		redirect_uri: &str,
		code: &str,
		pkce_verifier: &str,
	) -> Result<TokenSet, OauthError> {
		let form = [
			("client_id", client_id),
			("client_secret", client_secret),
			("code", code),
			("redirect_uri", redirect_uri),
			("grant_type", "authorization_code"),
			("code_verifier", pkce_verifier),
		];
		let response = self
			.http
			.post(&self.token_url)
			.form(&form)
			.send()
			.await
			.map_err(|e| OauthError::TokenExchange(format!("request failed: {e}")))?;

		parse_token_response(response, /* is_refresh = */ false, None).await
	}

	async fn refresh(
		&self,
		client_id: &str,
		client_secret: &str,
		refresh_token: &str,
	) -> Result<TokenSet, OauthError> {
		let form = [
			("client_id", client_id),
			("client_secret", client_secret),
			("refresh_token", refresh_token),
			("grant_type", "refresh_token"),
		];
		let response = self
			.http
			.post(&self.token_url)
			.form(&form)
			.send()
			.await
			.map_err(|e| OauthError::TokenExchange(format!("request failed: {e}")))?;

		parse_token_response(response, /* is_refresh = */ true, Some(refresh_token)).await
	}

	async fn display_name(&self, access_token: &str) -> Result<Option<String>, OauthError> {
		let response = match self
			.http
			.get(&self.graph_url)
			.bearer_auth(access_token)
			.send()
			.await
		{
			Ok(r) => r,
			Err(e) => {
				// Non-fatal: caller tolerates `None`. Log so operators can
				// investigate a pattern of transient failures.
				tracing::warn!(error = %e, "graph /me request failed");
				return Ok(None);
			}
		};

		if !response.status().is_success() {
			tracing::warn!(status = %response.status(), "graph /me returned non-2xx");
			return Ok(None);
		}

		let body_text = match response.text().await {
			Ok(t) => t,
			Err(e) => {
				tracing::warn!(error = %e, "graph /me body read failed");
				return Ok(None);
			}
		};

		let body: GraphMeResponse = match serde_json::from_str(&body_text) {
			Ok(b) => b,
			Err(e) => {
				tracing::warn!(error = %e, "graph /me returned malformed json");
				return Ok(None);
			}
		};

		// Prefer `displayName`; fall back to UPN; otherwise `None` lets the
		// caller use the provider id as a default label.
		Ok(body.display_name.or(body.user_principal_name))
	}
}

/// Shared parsing path for both code-exchange and refresh responses.
///
/// `prior_refresh_token` is used only when `is_refresh` is true and Microsoft
/// omits the field from the response — Microsoft sometimes rotates, sometimes
/// doesn't, so we always preserve a usable refresh token for the caller.
async fn parse_token_response(
	response: reqwest::Response,
	is_refresh: bool,
	prior_refresh_token: Option<&str>,
) -> Result<TokenSet, OauthError> {
	let status = response.status();
	let body = response
		.text()
		.await
		.map_err(|e| OauthError::TokenExchange(format!("failed to read body: {e}")))?;

	if !status.is_success() {
		// Preserve the provider error payload verbatim so callers (and logs)
		// can see `error=invalid_grant` / `error_description=...`.
		let prefix = if is_refresh { "refresh" } else { "exchange" };
		return Err(OauthError::TokenExchange(format!(
			"{prefix} failed (status {status}): {body}"
		)));
	}

	let parsed: TokenResponse = serde_json::from_str(&body).map_err(|e| {
		OauthError::TokenExchange(format!("malformed token response: {e}; body: {body}"))
	})?;

	if let Some(kind) = parsed.token_type.as_deref() {
		if !kind.eq_ignore_ascii_case("Bearer") {
			tracing::warn!(token_type = %kind, "unexpected token_type from microsoft");
		}
	}

	// Microsoft sometimes omits `refresh_token` on refresh when the existing
	// one is still valid; in that case reuse the old one so the caller has a
	// usable credential regardless.
	let refresh_token = parsed
		.refresh_token
		.or_else(|| prior_refresh_token.map(str::to_string));

	let expires_at = chrono::Utc::now() + chrono::Duration::seconds(parsed.expires_in.max(0));

	Ok(TokenSet {
		access_token: parsed.access_token,
		refresh_token,
		expires_at,
		scope: parsed.scope,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use wiremock::matchers::{bearer_token, method, path};
	use wiremock::{Mock, MockServer, ResponseTemplate};

	fn make_provider(server: &MockServer) -> OneDriveProvider {
		OneDriveProvider::new_with_endpoints(
			format!("{}/authorize", server.uri()),
			format!("{}/token", server.uri()),
			format!("{}/me", server.uri()),
		)
	}

	#[tokio::test]
	async fn test_id_is_onedrive() {
		let provider = OneDriveProvider::new();
		assert_eq!(provider.id(), "onedrive");
	}

	#[tokio::test]
	async fn test_loopback_ports_matches_microsoft_registered_uris() {
		let provider = OneDriveProvider::new();
		let ports = provider.loopback_ports();
		assert_eq!(ports.len(), 5, "Microsoft exact-match registration set");
		assert_eq!(ports, &[53682u16, 53683, 53684, 53685, 53686]);
	}

	#[tokio::test]
	async fn test_build_auth_url_contains_required_params() {
		let provider = OneDriveProvider::new();
		let url_str = provider.build_auth_url(
			"client-abc",
			"http://127.0.0.1:53682/oauth/callback",
			"state-xyz",
			"challenge-123",
		);
		let url = Url::parse(&url_str).expect("auth url must be parseable");
		let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

		assert_eq!(
			params.get("client_id").map(String::as_str),
			Some("client-abc")
		);
		assert_eq!(
			params.get("response_type").map(String::as_str),
			Some("code")
		);
		assert_eq!(
			params.get("redirect_uri").map(String::as_str),
			Some("http://127.0.0.1:53682/oauth/callback")
		);
		assert_eq!(
			params.get("response_mode").map(String::as_str),
			Some("query")
		);
		assert_eq!(params.get("scope").map(String::as_str), Some(SCOPES));
		assert_eq!(params.get("state").map(String::as_str), Some("state-xyz"));
		assert_eq!(
			params.get("code_challenge").map(String::as_str),
			Some("challenge-123")
		);
		assert_eq!(
			params.get("code_challenge_method").map(String::as_str),
			Some("S256")
		);
		assert_eq!(
			params.get("prompt").map(String::as_str),
			Some("select_account")
		);
	}

	#[tokio::test]
	async fn test_exchange_code_happy_path() {
		let server = MockServer::start().await;
		Mock::given(method("POST"))
			.and(path("/token"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"access_token": "at-1",
				"refresh_token": "rt-1",
				"expires_in": 3600,
				"scope": "Files.ReadWrite.All offline_access User.Read",
				"token_type": "Bearer"
			})))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		let before = chrono::Utc::now();
		let tokens = provider
			.exchange_code(
				"cid",
				"csec",
				"http://127.0.0.1:53682/oauth/callback",
				"code",
				"verif",
			)
			.await
			.expect("exchange should succeed");

		assert_eq!(tokens.access_token, "at-1");
		assert_eq!(tokens.refresh_token.as_deref(), Some("rt-1"));
		assert!(
			tokens.expires_at > before + chrono::Duration::seconds(3500),
			"expires_at should be ~1h in the future"
		);
		assert_eq!(
			tokens.scope.as_deref(),
			Some("Files.ReadWrite.All offline_access User.Read")
		);
	}

	#[tokio::test]
	async fn test_exchange_code_invalid_grant() {
		let server = MockServer::start().await;
		Mock::given(method("POST"))
			.and(path("/token"))
			.respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
				"error": "invalid_grant",
				"error_description": "AADSTS70000: authorization code expired"
			})))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		let err = provider
			.exchange_code(
				"cid",
				"csec",
				"http://127.0.0.1:53682/oauth/callback",
				"code",
				"verif",
			)
			.await
			.expect_err("invalid_grant must surface as an error");

		match err {
			OauthError::TokenExchange(msg) => {
				assert!(
					msg.contains("invalid_grant"),
					"error message should include invalid_grant, got: {msg}"
				);
			}
			other => panic!("expected TokenExchange, got {other:?}"),
		}
	}

	#[tokio::test]
	async fn test_refresh_happy_path_with_rotation() {
		let server = MockServer::start().await;
		Mock::given(method("POST"))
			.and(path("/token"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"access_token": "at-2",
				"refresh_token": "rt-rotated",
				"expires_in": 3600,
				"token_type": "Bearer"
			})))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		let tokens = provider
			.refresh("cid", "csec", "rt-old")
			.await
			.expect("refresh should succeed");

		assert_eq!(tokens.access_token, "at-2");
		assert_eq!(
			tokens.refresh_token.as_deref(),
			Some("rt-rotated"),
			"rotated refresh token must be preserved"
		);
	}

	#[tokio::test]
	async fn test_refresh_preserves_old_refresh_when_omitted() {
		let server = MockServer::start().await;
		Mock::given(method("POST"))
			.and(path("/token"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"access_token": "at-3",
				"expires_in": 3600,
				"token_type": "Bearer"
			})))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		let tokens = provider
			.refresh("cid", "csec", "rt-old")
			.await
			.expect("refresh should succeed");

		assert_eq!(tokens.access_token, "at-3");
		assert_eq!(
			tokens.refresh_token.as_deref(),
			Some("rt-old"),
			"old refresh token must be preserved when microsoft omits it"
		);
	}

	#[tokio::test]
	async fn test_display_name_prefers_display_name() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me"))
			.and(bearer_token("token-abc"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"displayName": "Alice",
				"userPrincipalName": "alice@contoso.com"
			})))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		let name = provider
			.display_name("token-abc")
			.await
			.expect("display_name should succeed");
		assert_eq!(name.as_deref(), Some("Alice"));
	}

	#[tokio::test]
	async fn test_display_name_falls_back_to_upn() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"userPrincipalName": "alice@contoso.com"
			})))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		let name = provider
			.display_name("token-abc")
			.await
			.expect("display_name should succeed");
		assert_eq!(name.as_deref(), Some("alice@contoso.com"));
	}

	#[tokio::test]
	async fn test_display_name_returns_none_on_http_error() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me"))
			.respond_with(ResponseTemplate::new(500))
			.mount(&server)
			.await;

		let provider = make_provider(&server);
		// Per trait doc, a failed display_name call is non-fatal and yields
		// Ok(None) so the caller can fall back to the provider id.
		let name = provider
			.display_name("token-abc")
			.await
			.expect("http errors must be mapped to Ok(None), not Err");
		assert!(name.is_none(), "500 response should yield None");
	}
}
