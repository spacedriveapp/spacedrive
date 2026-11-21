//! # OAuth 2.1 for the agent MCP endpoint
//!
//! MCP clients that cannot take a pasted header token connect with only the
//! endpoint's URL and the authorization flow from the MCP specification. The
//! daemon is both the protected resource and its own authorization server, so
//! nothing leaves the machine and no outside identity provider is involved.
//!
//! A client finds its way from the 401 on `/mcp`: the `WWW-Authenticate`
//! header names the protected resource metadata (RFC 9728), which names this
//! daemon as the authorization server, whose metadata (RFC 8414) lists the
//! endpoints below. The client registers itself (RFC 7591), sends the owner's
//! browser to `/oauth/authorize` with a PKCE challenge, and the owner picks
//! the folder, the access and the lifetime on a page the daemon serves. Only
//! then is a grant minted, through the same path `grants.create` uses.
//!
//! The access token is the grant's own `sdg_` token, so `/mcp` checks it with
//! [`crate::ops::grants::authorize`] exactly like a pasted one, and revoking
//! the grant ends it. The refresh token is bound to the grant and the client
//! and rotates on use, see [`crate::ops::grants::oauth`].
//!
//! Registered clients are public clients with no secret, kept in a small file
//! in the data directory so a restart does not strand them. Authorization
//! requests and codes live only in memory and expire within minutes.

use std::{
	collections::HashMap,
	net::{IpAddr, SocketAddr},
	path::PathBuf,
	sync::{Arc, Mutex},
	time::{Duration, Instant},
};

use axum::{
	body::Bytes,
	extract::{ConnectInfo, State},
	http::{header, HeaderMap, HeaderValue, StatusCode, Uri},
	response::{IntoResponse, Response},
};
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use url::Url;
use uuid::Uuid;

use super::{http::Endpoint, MCP_PATH};
use crate::{
	context::CoreContext,
	infra::action::error::ActionError,
	ops::grants::{
		create::{self, Mint},
		oauth::{self as grant_oauth, RefreshError},
		store, ttl, GrantCap,
	},
};

pub const PROTECTED_RESOURCE_PATH: &str = "/.well-known/oauth-protected-resource";
pub const AUTHORIZATION_SERVER_PATH: &str = "/.well-known/oauth-authorization-server";
pub const REGISTER_PATH: &str = "/oauth/register";
pub const AUTHORIZE_PATH: &str = "/oauth/authorize";
pub const TOKEN_PATH: &str = "/oauth/token";
pub const REVOKE_PATH: &str = "/oauth/revoke";

/// `read` is every grant; `write` asks for a read-write grant.
const SCOPES: &[&str] = &["read", "write"];
const CLIENTS_FILE: &str = "agent_mcp_oauth_clients.json";
const MAX_CLIENTS: usize = 100;
const MAX_REDIRECT_URIS: usize = 10;
const MAX_PENDING: usize = 64;
const PENDING_LIFETIME: Duration = Duration::from_secs(10 * 60);
const CODE_LIFETIME: Duration = Duration::from_secs(60);
/// Lifetimes the consent page offers, in seconds, inside the grant bounds.
const LIFETIMES: &[(u32, &str)] = &[
	(15 * 60, "15 minutes"),
	(60 * 60, "1 hour"),
	(4 * 60 * 60, "4 hours"),
	(12 * 60 * 60, "12 hours"),
];
const DEFAULT_LIFETIME: u32 = 60 * 60;

/// A dynamically registered public client.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Client {
	client_id: String,
	client_name: Option<String>,
	redirect_uris: Vec<String>,
	issued_at: i64,
}

/// An authorization request waiting for the owner on the consent page.
struct Pending {
	client_id: String,
	client_name: String,
	redirect_uri: String,
	state: Option<String>,
	challenge: String,
	write: bool,
	created: Instant,
}

/// An issued authorization code. A spent code is kept until it expires so a
/// second use can be recognized and the grant it produced revoked.
struct Code {
	client_id: String,
	redirect_uri: String,
	challenge: String,
	library_id: Uuid,
	grant_id: Uuid,
	caps: GrantCap,
	expires_at: DateTime<Utc>,
	tokens: Option<(String, String)>,
	issued: Instant,
}

/// The authorization server's state.
pub struct OAuth {
	clients_file: PathBuf,
	clients: tokio::sync::Mutex<Option<Vec<Client>>>,
	pending: Mutex<HashMap<String, Pending>>,
	codes: Mutex<HashMap<String, Code>>,
}

impl OAuth {
	pub fn new(data_dir: PathBuf) -> Self {
		Self {
			clients_file: data_dir.join(CLIENTS_FILE),
			clients: tokio::sync::Mutex::new(None),
			pending: Mutex::default(),
			codes: Mutex::default(),
		}
	}

	async fn client(&self, client_id: &str) -> Option<Client> {
		let mut clients = self.clients.lock().await;
		self.load(&mut clients)
			.await
			.iter()
			.find(|c| c.client_id == client_id)
			.cloned()
	}

	async fn register(&self, client: Client) -> Result<(), String> {
		let mut clients = self.clients.lock().await;
		let list = self.load(&mut clients).await;
		if list.len() >= MAX_CLIENTS {
			list.remove(0);
		}
		list.push(client);
		let bytes = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
		let staged = self.clients_file.with_extension("json.tmp");
		tokio::fs::write(&staged, bytes)
			.await
			.map_err(|e| e.to_string())?;
		tokio::fs::rename(&staged, &self.clients_file)
			.await
			.map_err(|e| e.to_string())
	}

	async fn load<'a>(&self, slot: &'a mut Option<Vec<Client>>) -> &'a mut Vec<Client> {
		if slot.is_none() {
			// A missing or unreadable file starts an empty registry; clients
			// whose id is gone register again.
			let loaded = match tokio::fs::read(&self.clients_file).await {
				Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
					warn!(error = %e, "ignoring an unreadable OAuth client registry");
					Vec::new()
				}),
				Err(_) => Vec::new(),
			};
			*slot = Some(loaded);
		}
		slot.as_mut().expect("loaded above")
	}
}

/// The origin clients reach this daemon at, from a Host already checked to be
/// loopback with the bound port. Metadata is built from it so the issuer and
/// resource match whichever loopback spelling the client used.
pub fn base_url(headers: &HeaderMap) -> String {
	let host = headers
		.get(header::HOST)
		.and_then(|h| h.to_str().ok())
		.unwrap_or("127.0.0.1");
	format!("http://{host}")
}

/// The `WWW-Authenticate` value a 401 from `/mcp` carries.
pub fn challenge(base: &str) -> String {
	format!(
		"Bearer realm=\"spacedrive\", resource_metadata=\"{base}{PROTECTED_RESOURCE_PATH}{MCP_PATH}\""
	)
}

/// What a request to an OAuth route may carry in `Origin`.
enum Origin {
	/// None at all: native clients send none and the routes send no CORS.
	Absent,
	/// The daemon's own origin, for the consent form the daemon served.
	Own,
}

fn guard(endpoint: &Endpoint, headers: &HeaderMap, origin: Origin) -> Result<String, Response> {
	endpoint.screen_host(headers)?;
	let base = base_url(headers);
	let sent = headers.get(header::ORIGIN).map(|o| o.to_str().unwrap_or(""));
	let allowed = match origin {
		Origin::Absent => sent.is_none(),
		Origin::Own => sent == Some(base.as_str()),
	};
	if !allowed {
		return Err((StatusCode::FORBIDDEN, "origin not allowed").into_response());
	}
	Ok(base)
}

pub async fn protected_resource(
	State(endpoint): State<Arc<Endpoint>>,
	headers: HeaderMap,
) -> Response {
	let base = match guard(&endpoint, &headers, Origin::Absent) {
		Ok(base) => base,
		Err(refusal) => return refusal,
	};
	json_response(
		StatusCode::OK,
		json!({
			"resource": format!("{base}{MCP_PATH}"),
			"authorization_servers": [base],
			"bearer_methods_supported": ["header"],
			"scopes_supported": SCOPES,
			"resource_name": "Spacedrive",
		}),
	)
}

pub async fn authorization_server(
	State(endpoint): State<Arc<Endpoint>>,
	headers: HeaderMap,
) -> Response {
	let base = match guard(&endpoint, &headers, Origin::Absent) {
		Ok(base) => base,
		Err(refusal) => return refusal,
	};
	json_response(
		StatusCode::OK,
		json!({
			"issuer": base,
			"authorization_endpoint": format!("{base}{AUTHORIZE_PATH}"),
			"token_endpoint": format!("{base}{TOKEN_PATH}"),
			"registration_endpoint": format!("{base}{REGISTER_PATH}"),
			"revocation_endpoint": format!("{base}{REVOKE_PATH}"),
			"scopes_supported": SCOPES,
			"response_types_supported": ["code"],
			"response_modes_supported": ["query"],
			"grant_types_supported": ["authorization_code", "refresh_token"],
			"token_endpoint_auth_methods_supported": ["none"],
			"revocation_endpoint_auth_methods_supported": ["none"],
			"code_challenge_methods_supported": ["S256"],
			"authorization_response_iss_parameter_supported": true,
			"client_id_metadata_document_supported": false,
		}),
	)
}

#[derive(Deserialize)]
struct Registration {
	#[serde(default)]
	redirect_uris: Vec<String>,
	client_name: Option<String>,
	grant_types: Option<Vec<String>>,
	response_types: Option<Vec<String>>,
}

pub async fn register(
	State(endpoint): State<Arc<Endpoint>>,
	headers: HeaderMap,
	body: Bytes,
) -> Response {
	if let Err(refusal) = guard(&endpoint, &headers, Origin::Absent) {
		return refusal;
	}
	let Ok(registration) = serde_json::from_slice::<Registration>(&body) else {
		return oauth_error(
			StatusCode::BAD_REQUEST,
			"invalid_client_metadata",
			"send the registration as a JSON object",
		);
	};
	if registration.redirect_uris.is_empty()
		|| registration.redirect_uris.len() > MAX_REDIRECT_URIS
		|| !registration.redirect_uris.iter().all(|uri| redirect_allowed(uri))
	{
		return oauth_error(
			StatusCode::BAD_REQUEST,
			"invalid_redirect_uri",
			"redirect URIs must be https, http on a loopback host, or a private app scheme, without a fragment",
		);
	}
	let grant_types = registration
		.grant_types
		.unwrap_or_else(|| vec!["authorization_code".to_string(), "refresh_token".to_string()]);
	let response_types = registration
		.response_types
		.unwrap_or_else(|| vec!["code".to_string()]);
	if !grant_types
		.iter()
		.all(|g| g == "authorization_code" || g == "refresh_token")
		|| !response_types.iter().all(|r| r == "code")
	{
		return oauth_error(
			StatusCode::BAD_REQUEST,
			"invalid_client_metadata",
			"only the authorization_code and refresh_token grants and the code response type are supported",
		);
	}
	let client_name = registration
		.client_name
		.map(|name| name.chars().filter(|c| !c.is_control()).take(100).collect::<String>())
		.filter(|name| !name.trim().is_empty());
	let client = Client {
		client_id: format!("sdc_{}", random_hex(16)),
		client_name,
		redirect_uris: registration.redirect_uris,
		issued_at: Utc::now().timestamp(),
	};
	if let Err(e) = endpoint.oauth.register(client.clone()).await {
		warn!(error = %e, "could not save an OAuth client registration");
		return oauth_error(
			StatusCode::INTERNAL_SERVER_ERROR,
			"server_error",
			"the registration could not be saved",
		);
	}
	info!(client = %client.client_id, name = ?client.client_name, "OAuth client registered");
	json_response(
		StatusCode::CREATED,
		json!({
			"client_id": client.client_id,
			"client_id_issued_at": client.issued_at,
			"client_name": client.client_name,
			"redirect_uris": client.redirect_uris,
			"grant_types": grant_types,
			"response_types": response_types,
			"token_endpoint_auth_method": "none",
		}),
	)
}

/// Whether a client may register `uri` as a redirect.
///
/// Plain http is allowed only to a loopback host, where native clients run a
/// listener. Schemes a browser would execute or read locally are refused.
fn redirect_allowed(uri: &str) -> bool {
	let Ok(url) = Url::parse(uri) else {
		return false;
	};
	if url.fragment().is_some() {
		return false;
	}
	match url.scheme() {
		"https" => url.host_str().is_some_and(|h| !h.is_empty()),
		"http" => url.host_str().is_some_and(loopback_name),
		"javascript" | "data" | "file" | "vbscript" | "about" | "blob" | "ftp" | "ws"
		| "wss" => false,
		_ => true,
	}
}

fn loopback_name(host: &str) -> bool {
	matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// Whether `presented` names `registered`. A loopback http redirect matches
/// on any port, since a native client picks a free port per run (RFC 8252).
fn redirect_matches(registered: &str, presented: &str) -> bool {
	if registered == presented {
		return true;
	}
	let (Ok(a), Ok(b)) = (Url::parse(registered), Url::parse(presented)) else {
		return false;
	};
	a.scheme() == "http"
		&& b.scheme() == "http"
		&& a.host_str().is_some_and(loopback_name)
		&& a.host_str() == b.host_str()
		&& a.path() == b.path()
		&& a.query() == b.query()
		&& b.fragment().is_none()
}

/// Whether an RFC 8707 `resource` names this endpoint.
fn resource_ok(resource: &str, base: &str) -> bool {
	let Ok(url) = Url::parse(resource) else {
		return false;
	};
	let Ok(own) = Url::parse(base) else {
		return false;
	};
	url.scheme() == "http"
		&& url.host_str().is_some_and(loopback_name)
		&& url.port() == own.port()
		&& matches!(url.path(), "/" | MCP_PATH)
		&& url.query().is_none()
}

/// Parse a query string or form body, refusing a repeated parameter as the
/// OAuth specifications require.
fn params(raw: &[u8]) -> Result<HashMap<String, String>, String> {
	let mut out = HashMap::new();
	for (key, value) in url::form_urlencoded::parse(raw) {
		if out.insert(key.to_string(), value.to_string()).is_some() {
			return Err(format!("parameter {key} is repeated"));
		}
	}
	Ok(out)
}

pub async fn authorize(
	State(endpoint): State<Arc<Endpoint>>,
	headers: HeaderMap,
	uri: Uri,
) -> Response {
	let base = match guard(&endpoint, &headers, Origin::Absent) {
		Ok(base) => base,
		Err(refusal) => return refusal,
	};
	let query = match params(uri.query().unwrap_or("").as_bytes()) {
		Ok(query) => query,
		Err(reason) => return error_page(StatusCode::BAD_REQUEST, &reason),
	};
	let Some(client) = endpoint
		.oauth
		.client(query.get("client_id").map(String::as_str).unwrap_or(""))
		.await
	else {
		return error_page(
			StatusCode::BAD_REQUEST,
			"This client is not registered with Spacedrive. Remove the server from your MCP client and add it again.",
		);
	};
	let redirect_uri = match query.get("redirect_uri") {
		Some(uri) if client.redirect_uris.iter().any(|r| redirect_matches(r, uri)) => uri.clone(),
		None if client.redirect_uris.len() == 1 => client.redirect_uris[0].clone(),
		_ => {
			return error_page(
				StatusCode::BAD_REQUEST,
				"The redirect address does not match the client's registration.",
			)
		}
	};
	// From here on the redirect is trusted, so errors go back to the client.
	let state = query.get("state").cloned();
	let back = |error: &str, description: &str| {
		redirect(
			&redirect_uri,
			&[
				("error", error),
				("error_description", description),
				("state", state.as_deref().unwrap_or("")),
				("iss", &base),
			],
			state.is_some(),
		)
	};
	if query.get("response_type").map(String::as_str) != Some("code") {
		return back("unsupported_response_type", "only response_type=code is supported");
	}
	let challenge = match (query.get("code_challenge"), query.get("code_challenge_method")) {
		(Some(challenge), Some(method)) if method == "S256" && challenge_ok(challenge) => {
			challenge.clone()
		}
		_ => return back("invalid_request", "a PKCE code_challenge with method S256 is required"),
	};
	if let Some(resource) = query.get("resource") {
		if !resource_ok(resource, &base) {
			return back("invalid_target", "the resource is not this MCP endpoint");
		}
	}
	let scopes: Vec<&str> = query
		.get("scope")
		.map(|s| s.split_whitespace().collect())
		.unwrap_or_default();
	if scopes.iter().any(|s| !SCOPES.contains(s)) {
		return back("invalid_scope", "the scopes are read and write");
	}

	let id = random_hex(32);
	let pending = Pending {
		client_name: client
			.client_name
			.clone()
			.unwrap_or_else(|| "An MCP client".to_string()),
		client_id: client.client_id,
		redirect_uri,
		state,
		challenge,
		write: scopes.contains(&"write"),
		created: Instant::now(),
	};
	let page = consent_page(
		&endpoint.context,
		&id,
		&pending,
		&Choice {
			path: String::new(),
			write: pending.write,
			ttl: DEFAULT_LIFETIME,
		},
		None,
	)
	.await;
	{
		let mut map = endpoint.oauth.pending.lock().expect("pending lock");
		map.retain(|_, p| p.created.elapsed() < PENDING_LIFETIME);
		if map.len() >= MAX_PENDING {
			return error_page(
				StatusCode::TOO_MANY_REQUESTS,
				"Too many authorization requests are waiting. Try again in a few minutes.",
			);
		}
		map.insert(id, pending);
	}
	page
}

/// What the owner chose on the consent page.
struct Choice {
	path: String,
	write: bool,
	ttl: u32,
}

pub async fn consent(
	State(endpoint): State<Arc<Endpoint>>,
	headers: HeaderMap,
	body: Bytes,
) -> Response {
	let base = match guard(&endpoint, &headers, Origin::Own) {
		Ok(base) => base,
		Err(refusal) => return refusal,
	};
	let form = match params(&body) {
		Ok(form) => form,
		Err(reason) => return error_page(StatusCode::BAD_REQUEST, &reason),
	};
	let id = form.get("request").cloned().unwrap_or_default();
	let Some(pending) = take_pending(&endpoint, &id) else {
		return error_page(
			StatusCode::BAD_REQUEST,
			"This request has expired or was already answered. Start the connection again from your MCP client.",
		);
	};
	let state = pending.state.clone();
	let reply = |pairs: &[(&str, &str)]| {
		let mut all = pairs.to_vec();
		all.push(("state", state.as_deref().unwrap_or("")));
		all.push(("iss", &base));
		redirect(&pending.redirect_uri, &all, state.is_some())
	};
	if form.get("decision").map(String::as_str) != Some("approve") {
		info!(client = %pending.client_id, "owner denied an OAuth request");
		return reply(&[
			("error", "access_denied"),
			("error_description", "the owner denied access"),
		]);
	}

	let choice = Choice {
		path: form.get("path").map(|p| p.trim().to_string()).unwrap_or_default(),
		write: form.get("access").map(String::as_str) == Some("read_write"),
		ttl: form
			.get("ttl")
			.and_then(|t| t.parse().ok())
			.unwrap_or(DEFAULT_LIFETIME),
	};
	match mint(&endpoint, &pending, &choice).await {
		Ok(code) => reply(&[("code", &code)]),
		Err(problem) => {
			let page = consent_page(&endpoint.context, &id, &pending, &choice, Some(&problem)).await;
			let mut map = endpoint.oauth.pending.lock().expect("pending lock");
			map.insert(id, pending);
			page
		}
	}
}

fn take_pending(endpoint: &Endpoint, id: &str) -> Option<Pending> {
	let mut map = endpoint.oauth.pending.lock().expect("pending lock");
	map.remove(id)
		.filter(|p| p.created.elapsed() < PENDING_LIFETIME)
}

/// Mint the grant the owner approved and an authorization code for it.
async fn mint(endpoint: &Endpoint, pending: &Pending, choice: &Choice) -> Result<String, String> {
	if choice.path.is_empty() {
		return Err("Choose a folder.".to_string());
	}
	let lifetime = ttl(Some(choice.ttl))?;
	let path = match choice.path.strip_prefix("~/") {
		Some(rest) => dirs::home_dir()
			.map(|home| home.join(rest))
			.unwrap_or_else(|| PathBuf::from(&choice.path)),
		None => PathBuf::from(&choice.path),
	};
	if !path.is_absolute() {
		return Err("Enter the folder's full path.".to_string());
	}
	let context = &endpoint.context;
	let mut problem = "No library is open.".to_string();
	for library in context.libraries().await.list().await {
		let target = match create::target(context, library.id(), &path).await {
			Ok(target) => target,
			Err(e) => {
				problem = action_message(e);
				continue;
			}
		};
		let caps = if choice.write {
			GrantCap::ReadWrite
		} else {
			GrantCap::Read
		};
		let minted = create::insert(
			context,
			&library,
			Mint {
				target,
				caps,
				lifetime,
				label: Some(format!("{} (OAuth)", pending.client_name)),
				oauth_client: Some(pending.client_id.clone()),
			},
		)
		.await
		.map_err(action_message)?;
		let refresh = minted
			.refresh
			.ok_or_else(|| "the grant has no refresh token".to_string())?;
		let code = random_hex(32);
		let mut codes = endpoint.oauth.codes.lock().expect("codes lock");
		codes.retain(|_, c| c.issued.elapsed() < CODE_LIFETIME);
		codes.insert(
			code.clone(),
			Code {
				client_id: pending.client_id.clone(),
				redirect_uri: pending.redirect_uri.clone(),
				challenge: pending.challenge.clone(),
				library_id: library.id(),
				grant_id: minted.grant.id,
				caps,
				expires_at: minted.grant.expires_at,
				tokens: Some((minted.token, refresh)),
				issued: Instant::now(),
			},
		);
		info!(client = %pending.client_id, grant = %minted.grant.id, "owner approved an OAuth request");
		return Ok(code);
	}
	Err(problem)
}

fn action_message(error: ActionError) -> String {
	match error {
		ActionError::InvalidInput(message) => message,
		other => {
			warn!(error = %other, "could not mint a grant for an OAuth request");
			"The grant could not be created. Check the daemon log.".to_string()
		}
	}
}

pub async fn token(
	State(endpoint): State<Arc<Endpoint>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	body: Bytes,
) -> Response {
	let base = match guard(&endpoint, &headers, Origin::Absent) {
		Ok(base) => base,
		Err(refusal) => return refusal,
	};
	let ip = peer.ip();
	if let Some(wait) = endpoint.locked_out(ip) {
		let mut response = oauth_error(
			StatusCode::TOO_MANY_REQUESTS,
			"invalid_request",
			"too many failed attempts",
		);
		if let Ok(value) = HeaderValue::from_str(&wait.as_secs().max(1).to_string()) {
			response.headers_mut().insert(header::RETRY_AFTER, value);
		}
		return response;
	}
	let form = match params(&body) {
		Ok(form) => form,
		Err(reason) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request", &reason),
	};
	let field = |name: &str| form.get(name).map(String::as_str).unwrap_or("");
	match field("grant_type") {
		"authorization_code" => exchange(&endpoint, ip, &base, &form).await,
		"refresh_token" => {
			if endpoint.oauth.client(field("client_id")).await.is_none() {
				endpoint.auth_failed(ip);
				return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client", "unknown client");
			}
			match grant_oauth::refresh(&endpoint.context, field("client_id"), field("refresh_token"))
				.await
			{
				Ok(rotated) => tokens(
					&rotated.access_token,
					&rotated.refresh_token,
					rotated.caps,
					rotated.expires_at,
				),
				Err(RefreshError::Invalid) => {
					endpoint.auth_failed(ip);
					oauth_error(
						StatusCode::BAD_REQUEST,
						"invalid_grant",
						"the refresh token is invalid, or its grant was revoked or has expired",
					)
				}
				Err(RefreshError::Internal(reason)) => {
					warn!(%reason, "token refresh failed");
					oauth_error(
						StatusCode::SERVICE_UNAVAILABLE,
						"temporarily_unavailable",
						"try again",
					)
				}
			}
		}
		_ => oauth_error(
			StatusCode::BAD_REQUEST,
			"unsupported_grant_type",
			"use authorization_code or refresh_token",
		),
	}
}

async fn exchange(
	endpoint: &Endpoint,
	ip: IpAddr,
	base: &str,
	form: &HashMap<String, String>,
) -> Response {
	let field = |name: &str| form.get(name).map(String::as_str).unwrap_or("");
	if let Some(resource) = form.get("resource") {
		if !resource_ok(resource, base) {
			return oauth_error(
				StatusCode::BAD_REQUEST,
				"invalid_target",
				"the resource is not this MCP endpoint",
			);
		}
	}
	let invalid = || {
		endpoint.auth_failed(ip);
		oauth_error(
			StatusCode::BAD_REQUEST,
			"invalid_grant",
			"the authorization code is invalid, expired or already used",
		)
	};
	let (outcome, replayed) = {
		let mut codes = endpoint.oauth.codes.lock().expect("codes lock");
		codes.retain(|_, c| c.issued.elapsed() < CODE_LIFETIME);
		let Some(code) = codes.get_mut(field("code")) else {
			return invalid();
		};
		let verified = code.client_id == field("client_id")
			&& code.redirect_uri == field("redirect_uri")
			&& pkce_ok(field("code_verifier"), &code.challenge);
		// A code is good for one attempt. A second presentation, or a first
		// that fails verification, means it may have been intercepted, so
		// the grant it produced is revoked.
		match (code.tokens.take(), verified) {
			(Some(tokens), true) => (Some((tokens, code.caps, code.expires_at)), None),
			_ => (None, Some((code.library_id, code.grant_id))),
		}
	};
	if let Some((library_id, grant_id)) = replayed {
		if let Some(library) = endpoint
			.context
			.libraries()
			.await
			.get_library(library_id)
			.await
		{
			match store::revoke(library.db().conn(), grant_id).await {
				Ok(_) => warn!(grant = %grant_id, "authorization code misused; grant revoked"),
				Err(e) => warn!(grant = %grant_id, error = %e, "could not revoke after code misuse"),
			}
		}
		return invalid();
	}
	match outcome {
		Some(((access, refresh), caps, expires_at)) => tokens(&access, &refresh, caps, expires_at),
		None => invalid(),
	}
}

pub async fn revoke(
	State(endpoint): State<Arc<Endpoint>>,
	headers: HeaderMap,
	body: Bytes,
) -> Response {
	if let Err(refusal) = guard(&endpoint, &headers, Origin::Absent) {
		return refusal;
	}
	let form = match params(&body) {
		Ok(form) => form,
		Err(reason) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request", &reason),
	};
	let presented = form.get("token").map(String::as_str).unwrap_or("");
	if let Err(e) = grant_oauth::revoke_token(&endpoint.context, presented).await {
		warn!(error = %e, "token revocation failed");
		return oauth_error(
			StatusCode::SERVICE_UNAVAILABLE,
			"temporarily_unavailable",
			"try again",
		);
	}
	StatusCode::OK.into_response()
}

fn tokens(access: &str, refresh: &str, caps: GrantCap, expires_at: DateTime<Utc>) -> Response {
	let scope = match caps {
		GrantCap::Read => "read",
		GrantCap::ReadWrite => "read write",
	};
	let mut response = json_response(
		StatusCode::OK,
		json!({
			"access_token": access,
			"token_type": "Bearer",
			"expires_in": (expires_at - Utc::now()).num_seconds().max(0),
			"refresh_token": refresh,
			"scope": scope,
		}),
	);
	response
		.headers_mut()
		.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
	response
}

/// Whether a PKCE challenge has the shape S256 produces.
fn challenge_ok(challenge: &str) -> bool {
	challenge.len() == 43
		&& challenge
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether `verifier` is a valid PKCE verifier whose S256 is `challenge`.
fn pkce_ok(verifier: &str, challenge: &str) -> bool {
	let shaped = (43..=128).contains(&verifier.len())
		&& verifier
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'));
	shaped
		&& base64::engine::general_purpose::URL_SAFE_NO_PAD
			.encode(Sha256::digest(verifier.as_bytes()))
			== challenge
}

fn random_hex(bytes: usize) -> String {
	let mut secret = vec![0u8; bytes];
	rand::rngs::OsRng.fill_bytes(&mut secret);
	hex::encode(secret)
}

/// A 303 to the client's redirect with `pairs` appended to its query.
fn redirect(to: &str, pairs: &[(&str, &str)], with_state: bool) -> Response {
	let Ok(mut url) = Url::parse(to) else {
		return error_page(StatusCode::BAD_REQUEST, "The redirect address is invalid.");
	};
	{
		let mut query = url.query_pairs_mut();
		for (key, value) in pairs {
			if *key == "state" && !with_state {
				continue;
			}
			query.append_pair(key, value);
		}
	}
	let mut response = StatusCode::SEE_OTHER.into_response();
	if let Ok(value) = HeaderValue::from_str(url.as_str()) {
		response.headers_mut().insert(header::LOCATION, value);
	}
	response
		.headers_mut()
		.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	response
}

fn json_response(status: StatusCode, body: Value) -> Response {
	(
		status,
		[
			(header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
			(header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
		],
		body.to_string(),
	)
		.into_response()
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
	json_response(
		status,
		json!({ "error": error, "error_description": description }),
	)
}

fn escape(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	for c in text.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\'' => out.push_str("&#39;"),
			_ => out.push(c),
		}
	}
	out
}

fn html(status: StatusCode, title: &str, body: &str) -> Response {
	let page = format!(
		"<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} - Spacedrive</title><style>{STYLE}</style></head><body><main>{body}</main></body></html>",
		escape(title)
	);
	(
		status,
		[
			(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8")),
			(header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
			(
				header::CONTENT_SECURITY_POLICY,
				HeaderValue::from_static(
					"default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'",
				),
			),
			(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")),
			(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")),
			(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
		],
		page,
	)
		.into_response()
}

fn error_page(status: StatusCode, message: &str) -> Response {
	html(
		status,
		"Cannot connect",
		&format!("<h1>Cannot connect</h1><p>{}</p>", escape(message)),
	)
}

async fn consent_page(
	context: &CoreContext,
	id: &str,
	pending: &Pending,
	choice: &Choice,
	problem: Option<&str>,
) -> Response {
	let mut roots = Vec::new();
	for library in context.libraries().await.list().await {
		for source in context.volume_index().sources_of(library.id()) {
			roots.push(source.root.display().to_string());
		}
	}
	roots.sort();
	roots.dedup();
	let suggestions: String = roots
		.iter()
		.map(|root| format!("<option value=\"{}\">", escape(root)))
		.collect();
	let lifetimes: String = LIFETIMES
		.iter()
		.map(|(seconds, label)| {
			let selected = if *seconds == choice.ttl { " selected" } else { "" };
			format!("<option value=\"{seconds}\"{selected}>{label}</option>")
		})
		.collect();
	let destination = Url::parse(&pending.redirect_uri)
		.ok()
		.map(|url| match url.host_str() {
			Some(host) => format!("{}://{host}", url.scheme()),
			None => format!("{}:", url.scheme()),
		})
		.unwrap_or_default();
	let problem = problem
		.map(|p| format!("<p class=\"problem\">{}</p>", escape(p)))
		.unwrap_or_default();
	let (read, write) = if choice.write {
		("", " checked")
	} else {
		(" checked", "")
	};
	let client = escape(&pending.client_name);
	let body = format!(
		r#"<h1>Allow {client} to use a folder?</h1>
<p class="muted">An MCP client on this computer asked this Spacedrive device for access. The name is what the client calls itself; Spacedrive cannot verify it. When you answer, your browser returns to <code>{destination}</code>.</p>
{problem}
<form method="post" action="{AUTHORIZE_PATH}">
<input type="hidden" name="request" value="{id}">
<label for="path">Folder</label>
<input id="path" name="path" list="sources" required autocomplete="off" value="{path}" placeholder="/Users/you/Projects/notes">
<datalist id="sources">{suggestions}</datalist>
<p class="hint">The client can reach this folder and everything inside it, and nothing else. It must be inside a tracked source.</p>
<fieldset><legend>Access</legend>
<label class="choice"><input type="radio" name="access" value="read"{read}> Read only</label>
<label class="choice"><input type="radio" name="access" value="read_write"{write}> Read and write</label>
</fieldset>
<label for="ttl">Access ends after</label>
<select id="ttl" name="ttl">{lifetimes}</select>
<div class="actions"><button type="submit" name="decision" value="deny">Deny</button><button type="submit" name="decision" value="approve" class="primary">Allow</button></div>
</form>
<p class="hint">The client keeps access until it ends or you revoke it with <code>sd grant revoke</code>.</p>"#,
		id = escape(id),
		path = escape(&choice.path),
		destination = escape(&destination),
	);
	html(StatusCode::OK, "Allow access", &body)
}

const STYLE: &str = "body{font:15px/1.5 system-ui,sans-serif;margin:0;background:#f4f4f5;color:#18181b}main{max-width:520px;margin:48px auto;background:#fff;border-radius:12px;padding:28px 32px;box-shadow:0 1px 3px rgba(0,0,0,.12)}h1{font-size:20px;margin:0 0 8px}.muted,.hint{color:#52525b}.hint{font-size:13px;margin:6px 0 16px}label{display:block;font-weight:600;margin-top:12px}input[type=text],input:not([type]),select{width:100%;box-sizing:border-box;padding:8px 10px;border:1px solid #d4d4d8;border-radius:8px;font:inherit}fieldset{border:0;padding:0;margin:12px 0 0}legend{font-weight:600}.choice{font-weight:400;margin-top:4px}.actions{display:flex;gap:12px;justify-content:flex-end;margin-top:24px}button{font:inherit;padding:8px 18px;border-radius:8px;border:1px solid #d4d4d8;background:#fff;cursor:pointer}.primary{background:#2563eb;border-color:#2563eb;color:#fff}.problem{background:#fef2f2;color:#991b1b;padding:8px 12px;border-radius:8px}code{font-size:13px}@media(prefers-color-scheme:dark){body{background:#18181b;color:#f4f4f5}main{background:#27272a}.muted,.hint{color:#a1a1aa}input:not([type]),select,button{background:#18181b;color:#f4f4f5;border-color:#3f3f46}}";

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn redirects_must_be_safe() {
		assert!(redirect_allowed("https://client.example/callback"));
		assert!(redirect_allowed("http://127.0.0.1:43123/callback"));
		assert!(redirect_allowed("http://localhost/callback"));
		assert!(redirect_allowed("com.example.app:/oauth"));
		assert!(!redirect_allowed("http://client.example/callback"));
		assert!(!redirect_allowed("https://client.example/cb#frag"));
		assert!(!redirect_allowed("javascript:alert(1)"));
		assert!(!redirect_allowed("file:///etc/passwd"));
		assert!(!redirect_allowed("not a url"));
	}

	#[test]
	fn loopback_redirects_match_on_any_port() {
		assert!(redirect_matches(
			"http://127.0.0.1:1000/cb",
			"http://127.0.0.1:2000/cb"
		));
		assert!(!redirect_matches(
			"http://127.0.0.1:1000/cb",
			"http://127.0.0.1:2000/other"
		));
		assert!(!redirect_matches(
			"https://client.example/cb",
			"https://client.example/cb2"
		));
	}

	#[test]
	fn resources_must_name_this_endpoint() {
		let base = "http://127.0.0.1:7765";
		assert!(resource_ok("http://127.0.0.1:7765/mcp", base));
		assert!(resource_ok("http://localhost:7765/mcp", base));
		assert!(!resource_ok("http://127.0.0.1:7766/mcp", base));
		assert!(!resource_ok("https://evil.example/mcp", base));
	}

	#[test]
	fn pkce_s256_is_checked() {
		// RFC 7636 appendix B.
		let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
		let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
		assert!(challenge_ok(challenge));
		assert!(pkce_ok(verifier, challenge));
		assert!(!pkce_ok("short", challenge));
		assert!(!pkce_ok(&verifier.replace('d', "e"), challenge));
	}

	#[test]
	fn repeated_parameters_are_refused() {
		assert!(params(b"a=1&b=2").is_ok());
		assert!(params(b"a=1&a=2").is_err());
	}

	#[test]
	fn markup_is_escaped() {
		assert_eq!(escape("<a href='x'>&\"</a>"), "&lt;a href=&#39;x&#39;&gt;&amp;&quot;&lt;/a&gt;");
	}
}
