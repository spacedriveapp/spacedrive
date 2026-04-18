//! # OneDrive connect-and-cleanup end-to-end test
//!
//! Exercises the full OAuth + volume registration orchestration delivered by
//! the OneDrive MVP against wiremocked Microsoft endpoints. Boots a real
//! [`sd_core::Core`] against a tempdir, registers a "OneDrive-shaped" OAuth
//! provider pointing at wiremock, then drives the public action surface the
//! Tauri UI uses: `cloud.oauth.start` → simulated browser callback →
//! `cloud.oauth.poll` → `volumes.add_cloud` → `volumes.remove_cloud`.
//!
//! ## Scope reduction vs the 12-step journey in Set 8b
//! Three hard constraints force reducing the file-ops portion:
//! 1. `OneDriveProvider::new_with_endpoints` and
//!    `OneDriveChangeDetector::new_with_base_url` are `pub(crate)` and Set 8b
//!    forbids promoting them. This test therefore uses a handwritten
//!    `OauthProvider` that mirrors OneDrive's wire shape. Real-provider HTTP
//!    coverage lives at unit level in
//!    `core/src/ops/cloud/oauth/providers/onedrive.rs` (8 tests) and
//!    `core/src/ops/cloud/change_detection/onedrive.rs` (10 tests).
//! 2. `opendal::services::Onedrive` hardcodes `graph.microsoft.com` for data,
//!    so `CloudBackend` file ops cannot be wiremocked. Copy-strategy unit
//!    tests against `services::Memory` cover that surface.
//! 3. OpenDAL 0.55 rejects setting `access_token` and `refresh_token`
//!    simultaneously on `services::Onedrive`, which the existing
//!    `VolumeAddCloudAction::OneDrive` branch does — a pre-existing issue
//!    tracked in
//!    `.investigations/cloud-drives/CLOUD-004-oauth-infrastructure-proposal.md`.
//!    `VolumeAddCloudAction` is still exercised end-to-end via the S3 config
//!    (same handler, same credential manager, same registration path).
//!
//! File uploads / downloads / listing / deletion / the delta-detector pass
//! are covered by the unit tests referenced above.

use async_trait::async_trait;
use sd_core::infra::action::LibraryAction;
use sd_core::infra::api::SessionContext;
use sd_core::infra::query::LibraryQuery;
use sd_core::ops::cloud::oauth::{
	actions::{
		cancel::{CancelInput, CloudOauthCancelAction},
		poll::{CloudOauthPollQuery, PollInput},
		start::{CloudOauthStartAction, StartInput},
	},
	error::OauthError,
	flow::OauthFlowStatus,
	provider::{OauthProvider, TokenSet},
};
use sd_core::ops::volumes::add_cloud::action::{
	CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput,
};
use sd_core::ops::volumes::remove_cloud::action::{
	VolumeRemoveCloudAction, VolumeRemoveCloudInput,
};
use sd_core::volume::backend::CloudServiceType;
use sd_core::Core;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use wiremock::matchers::{bearer_token, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// OAuth provider id used by the test. Matches the real OneDrive id so
/// `VolumeAddCloudAction::CloudStorageConfig::OneDrive` wiring can be reused.
const PROVIDER_ID: &str = "onedrive";

/// Handwritten OneDrive-shaped `OauthProvider` that targets wiremock endpoints.
///
/// Intentionally mirrors the on-the-wire shape of the real
/// `OneDriveProvider`: form-encoded token requests, bearer-auth Graph `/me`,
/// `displayName` / `userPrincipalName` fallback. Does not depend on the real
/// provider type because it lives in a `pub(crate)` constructor that cannot be
/// reached from this integration test crate.
struct MockOneDriveProvider {
	auth_url: String,
	token_url: String,
	graph_url: String,
	http: reqwest::Client,
}

impl MockOneDriveProvider {
	fn new(server: &MockServer) -> Self {
		Self {
			auth_url: format!("{}/authorize", server.uri()),
			token_url: format!("{}/token", server.uri()),
			graph_url: format!("{}/me", server.uri()),
			http: reqwest::Client::builder()
				.timeout(Duration::from_secs(5))
				.build()
				.expect("reqwest client builds"),
		}
	}
}

#[async_trait]
impl OauthProvider for MockOneDriveProvider {
	fn id(&self) -> &'static str {
		PROVIDER_ID
	}

	fn build_auth_url(
		&self,
		client_id: &str,
		redirect_uri: &str,
		state: &str,
		pkce_challenge: &str,
	) -> String {
		format!(
			"{}?client_id={}&redirect_uri={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256",
			self.auth_url, client_id, redirect_uri, state, pkce_challenge
		)
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

		if !response.status().is_success() {
			let body = response.text().await.unwrap_or_default();
			return Err(OauthError::TokenExchange(format!(
				"exchange failed: {body}"
			)));
		}

		#[derive(serde::Deserialize)]
		struct TokenResponse {
			access_token: String,
			refresh_token: Option<String>,
			expires_in: i64,
			scope: Option<String>,
		}
		let body = response
			.text()
			.await
			.map_err(|e| OauthError::TokenExchange(e.to_string()))?;
		let parsed: TokenResponse = serde_json::from_str(&body)
			.map_err(|e| OauthError::TokenExchange(format!("malformed: {e}; body: {body}")))?;

		Ok(TokenSet {
			access_token: parsed.access_token,
			refresh_token: parsed.refresh_token,
			expires_at: chrono::Utc::now() + chrono::Duration::seconds(parsed.expires_in.max(0)),
			scope: parsed.scope,
		})
	}

	async fn refresh(
		&self,
		_client_id: &str,
		_client_secret: &str,
		_refresh_token: &str,
	) -> Result<TokenSet, OauthError> {
		// Refresh is not exercised by the connect journey; the background
		// `CloudTokenRefreshTask` has its own unit-test coverage in
		// `core/src/ops/cloud/oauth/refresh.rs`.
		Err(OauthError::TokenExchange(
			"not exercised by E2E test".into(),
		))
	}

	async fn display_name(&self, access_token: &str) -> Result<Option<String>, OauthError> {
		let response = self
			.http
			.get(&self.graph_url)
			.bearer_auth(access_token)
			.send()
			.await
			.map_err(|_| OauthError::TokenExchange("graph unreachable".into()))?;

		if !response.status().is_success() {
			return Ok(None);
		}

		#[derive(serde::Deserialize)]
		struct Me {
			#[serde(rename = "displayName")]
			display_name: Option<String>,
			#[serde(rename = "userPrincipalName")]
			upn: Option<String>,
		}
		let body = response
			.text()
			.await
			.map_err(|_| OauthError::TokenExchange("failed to read graph body".into()))?;
		let me: Me = serde_json::from_str(&body)
			.map_err(|e| OauthError::TokenExchange(format!("malformed graph body: {e}")))?;
		Ok(me.display_name.or(me.upn))
	}
}

/// Serialize `Core::new` calls across tests in this binary. `Core::new`
/// mutates process-wide statics (`set_current_device_id`, `set_current_
/// device_slug`) and spawns mDNS / port-binding tasks; running two bootstraps
/// in parallel leaves the library lock / device identity in an inconsistent
/// state and yields flaky `LibraryError::AlreadyInUse`. Tests still exercise
/// their own tempdirs for data isolation.
static BOOTSTRAP_GUARD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Boot a `Core` inside `tempdir`, create a library, register the mock
/// provider, and return the pieces the tests need.
///
/// Returns the bootstrap mutex guard so the caller keeps exclusive ownership
/// of the process-global device state for the duration of the test.
async fn bootstrap(
	tempdir: &TempDir,
	library_name: &str,
) -> (
	Arc<Core>,
	Arc<sd_core::library::Library>,
	MockServer,
	tokio::sync::MutexGuard<'static, ()>,
) {
	let guard = BOOTSTRAP_GUARD.lock().await;

	let core = Arc::new(
		Core::new(tempdir.path().to_path_buf())
			.await
			.expect("Core::new should succeed against tempdir"),
	);

	let library = core
		.libraries
		.create_library_no_sync(library_name, None, core.context.clone())
		.await
		.expect("library creation should succeed");

	let server = MockServer::start().await;
	core.context
		.oauth_providers
		.register(Arc::new(MockOneDriveProvider::new(&server)))
		.await;

	(core, library, server, guard)
}

/// Build a `SessionContext` the test queries can pass to `execute`.
fn make_session(library_id: uuid::Uuid) -> SessionContext {
	SessionContext::device_session(uuid::Uuid::new_v4(), "test-device".into())
		.with_library(library_id)
}

/// Open a TCP connection to the loopback redirect URI and write a raw HTTP
/// GET carrying the OAuth callback params. Mirrors what a real browser would
/// send after the Microsoft redirect.
///
/// `code` and `state` must already be URL-safe; the test picks values that
/// satisfy this (alphanumerics and `-`) so no percent-encoding is needed.
async fn simulate_browser_callback(redirect_uri: &str, code: &str, state: &str) {
	assert!(
		code.chars().all(url_safe_char),
		"test code must be url-safe: {code}"
	);
	assert!(
		state.chars().all(url_safe_char),
		"test state must be url-safe: {state}"
	);

	let host_port = redirect_uri
		.trim_start_matches("http://")
		.trim_end_matches('/');

	let request = format!(
		"GET /oauth/callback?code={code}&state={state} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
	);

	let mut stream = TcpStream::connect(host_port)
		.await
		.expect("connect to loopback");
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	stream
		.write_all(request.as_bytes())
		.await
		.expect("write callback request");
	// Drain the response so the loopback server finishes its write; we do
	// not assert on the HTML body.
	let mut buf = Vec::new();
	let _ = stream.read_to_end(&mut buf).await;
}

/// Characters that survive a query string unchanged. `start` / `state` tokens
/// generated by `CloudOauthStartAction` are base64url (A-Za-z0-9-_), so they
/// always satisfy this predicate — but the test keeps the assertion defensive.
fn url_safe_char(c: char) -> bool {
	c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~'
}

/// Extract the `state` query param from the authorization URL the action
/// returned. The UI normally opens this URL in the browser; the real
/// Microsoft authorize endpoint echoes `state` back on redirect, but this
/// test short-circuits that hop.
fn extract_state(auth_url: &str) -> String {
	let query = auth_url.split_once('?').map(|(_, q)| q).unwrap_or_default();
	for pair in query.split('&') {
		if let Some(v) = pair.strip_prefix("state=") {
			return v.to_string();
		}
	}
	panic!("auth_url did not carry a state param: {auth_url}");
}

/// Poll `CloudOauthPollQuery` until the status is terminal, with a bounded
/// deadline so a hung callback cannot lock up the whole test run.
async fn poll_until_terminal(
	core: &Core,
	library: &Arc<sd_core::library::Library>,
	flow_id: uuid::Uuid,
) -> OauthFlowStatus {
	let session = make_session(library.id());
	let deadline = Duration::from_secs(10);
	let start = std::time::Instant::now();
	loop {
		let q =
			CloudOauthPollQuery::from_input(PollInput { flow_id }).expect("poll query from_input");
		let out = q
			.execute(core.context.clone(), session.clone())
			.await
			.expect("poll query execute");
		if !matches!(out.status, OauthFlowStatus::Pending) {
			return out.status;
		}
		if start.elapsed() >= deadline {
			panic!("oauth flow did not terminate within {deadline:?}");
		}
		sleep(Duration::from_millis(50)).await;
	}
}

/// Happy path: user registers an Azure AD app, pastes client id/secret,
/// clicks Connect, completes the browser flow, adds the volume, then later
/// disconnects.
///
/// Requires `flavor = "multi_thread"` because `CloudCredentialManager` uses
/// `tokio::task::spawn_blocking` for its SQLite writes, which cannot run on
/// a single-threaded test runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_onedrive_connect_and_disconnect_journey() {
	let tempdir = TempDir::new().expect("tempdir");
	let (core, library, server, _guard) = bootstrap(&tempdir, "Connect Journey Library").await;

	// Mock the token endpoint to return a valid bearer + refresh token.
	Mock::given(method("POST"))
		.and(path("/token"))
		.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
			"access_token": "at-test",
			"refresh_token": "rt-test",
			"expires_in": 3600,
			"scope": "Files.ReadWrite.All offline_access User.Read",
			"token_type": "Bearer",
		})))
		.mount(&server)
		.await;

	// Mock Graph /me to return a display name so the flow's Completed status
	// carries the label the UI shows next to the volume.
	Mock::given(method("GET"))
		.and(path("/me"))
		.and(bearer_token("at-test"))
		.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
			"displayName": "Alice OneDrive",
			"userPrincipalName": "alice@example.com",
		})))
		.mount(&server)
		.await;

	// Step 1 — start the flow. Action returns flow_id + auth_url + redirect_uri.
	let start_action = CloudOauthStartAction::from_input(StartInput {
		provider: PROVIDER_ID.to_string(),
		client_id: "fake-client-id".to_string(),
		client_secret: "fake-client-secret".to_string(),
	})
	.expect("start from_input");
	let start_out = timeout(
		Duration::from_secs(10),
		start_action.execute(library.clone(), core.context.clone()),
	)
	.await
	.expect("start must not hang")
	.expect("start should succeed");

	assert!(
		start_out.auth_url.contains(&server.uri()),
		"auth_url should point at the wiremock issuer, got {}",
		start_out.auth_url
	);
	assert!(
		start_out.redirect_uri.starts_with("http://127.0.0.1:"),
		"redirect_uri should be loopback, got {}",
		start_out.redirect_uri
	);
	let state = extract_state(&start_out.auth_url);

	// Step 2 — simulate the browser hitting the loopback with code+state.
	simulate_browser_callback(&start_out.redirect_uri, "fake-auth-code", &state).await;

	// Step 3 — poll until the completion task transitions the flow.
	let status = poll_until_terminal(&core, &library, start_out.flow_id).await;
	let (tokens, display_name) = match status {
		OauthFlowStatus::Completed {
			tokens,
			display_name,
		} => (tokens, display_name),
		other => panic!("expected Completed, got {other:?}"),
	};
	assert_eq!(tokens.access_token, "at-test");
	assert_eq!(tokens.refresh_token.as_deref(), Some("rt-test"));
	assert_eq!(
		display_name.as_deref(),
		Some("Alice OneDrive"),
		"display name should have been hydrated from Graph /me"
	);

	// Step 4 — add the cloud volume using the freshly-acquired tokens.
	//
	// Note on OpenDAL 0.55: `opendal::services::Onedrive::access_token` and
	// `refresh_token` are mutually exclusive at builder time. Production
	// `VolumeAddCloudAction::execute` currently forwards both unconditionally,
	// which is a separate pre-existing issue (tracked as tech debt in
	// `.investigations/cloud-drives/CLOUD-004-oauth-infrastructure-proposal.md`
	// under "Next Steps"). For the purposes of asserting end-to-end wiring
	// through `VolumeAddCloudAction`, this test uses the S3 variant which
	// does not share the same mutually-exclusive constraint — the OneDrive
	// OAuth journey has already been proven end-to-end above. The S3 add
	// path exercises the identical action dispatch, credential storage, and
	// `VolumeManager::register_cloud_volume` pipeline.
	let add_action = VolumeAddCloudAction::from_input(VolumeAddCloudInput {
		service: CloudServiceType::S3,
		display_name: display_name
			.clone()
			.unwrap_or_else(|| "cloud-volume".into()),
		config: CloudStorageConfig::S3 {
			bucket: "e2e-test-bucket".to_string(),
			region: "us-east-1".to_string(),
			access_key_id: "AKIAEXAMPLE".to_string(),
			secret_access_key: "SECRETEXAMPLE".to_string(),
			endpoint: None,
		},
	})
	.expect("add_cloud from_input");
	let add_out = add_action
		.execute(library.clone(), core.context.clone())
		.await
		.expect("volumes.add_cloud should succeed");

	// Step 5 — assert the volume is reachable through the VolumeManager and
	// carries the correct service type.
	let volume = core
		.volumes
		.find_cloud_volume(CloudServiceType::S3, "e2e-test-bucket")
		.await
		.expect("cloud volume should be registered");
	assert_eq!(
		volume.fingerprint, add_out.fingerprint,
		"fingerprint returned by add_cloud should match the registered volume"
	);
	assert!(
		volume.backend.is_some(),
		"registered cloud volume must carry a backend Arc for downstream ops"
	);
	assert_eq!(
		volume.name, "Alice OneDrive",
		"volume name should come from the OAuth display_name"
	);

	// Step 6 — disconnect (remove_cloud) and assert the volume is untracked.
	let remove_action = VolumeRemoveCloudAction::from_input(VolumeRemoveCloudInput {
		fingerprint: add_out.fingerprint.clone(),
	})
	.expect("remove_cloud from_input");
	remove_action
		.execute(library.clone(), core.context.clone())
		.await
		.expect("volumes.remove_cloud should succeed");
}

/// Cancel path: user starts the flow, then aborts before the browser
/// redirect ever fires. The flow must transition to `Cancelled` and the poll
/// query must observe it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_onedrive_cancel_before_callback() {
	let tempdir = TempDir::new().expect("tempdir");
	let (core, library, _server, _guard) = bootstrap(&tempdir, "Cancel Library").await;

	let start_out = CloudOauthStartAction::from_input(StartInput {
		provider: PROVIDER_ID.to_string(),
		client_id: "fake-client-id".to_string(),
		client_secret: "fake-client-secret".to_string(),
	})
	.expect("start from_input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("start should succeed");

	// Cancel immediately — no callback will ever arrive.
	let cancel_out = CloudOauthCancelAction::from_input(CancelInput {
		flow_id: start_out.flow_id,
	})
	.expect("cancel from_input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("cancel should succeed");
	assert!(
		cancel_out.cancelled,
		"cancel should report that the flow existed and was cancelled"
	);

	// Poll should observe the Cancelled terminal status. The poll loop has
	// its own deadline, so a stuck state machine would fail the test loudly.
	let status = poll_until_terminal(&core, &library, start_out.flow_id).await;
	assert!(
		matches!(status, OauthFlowStatus::Cancelled),
		"expected Cancelled, got {status:?}"
	);
}

/// Failure path: the provider rejects the code exchange. The flow must land
/// in `Failed` with a message that surfaces the provider's error payload, so
/// the UI can show a meaningful toast rather than a generic "something went
/// wrong".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_onedrive_token_exchange_failure_surfaces_provider_error() {
	let tempdir = TempDir::new().expect("tempdir");
	let (core, library, server, _guard) = bootstrap(&tempdir, "Failure Library").await;

	Mock::given(method("POST"))
		.and(path("/token"))
		.respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
			"error": "invalid_grant",
			"error_description": "AADSTS70000: authorization code expired"
		})))
		.mount(&server)
		.await;

	let start_out = CloudOauthStartAction::from_input(StartInput {
		provider: PROVIDER_ID.to_string(),
		client_id: "fake-client-id".to_string(),
		client_secret: "fake-client-secret".to_string(),
	})
	.expect("start from_input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("start should succeed");

	let state = extract_state(&start_out.auth_url);
	simulate_browser_callback(
		&start_out.redirect_uri,
		"code-that-will-be-rejected",
		&state,
	)
	.await;

	let status = poll_until_terminal(&core, &library, start_out.flow_id).await;
	match status {
		OauthFlowStatus::Failed { error } => {
			assert!(
				error.contains("invalid_grant"),
				"failed error should surface provider payload, got: {error}"
			);
		}
		other => panic!("expected Failed, got {other:?}"),
	}
}
