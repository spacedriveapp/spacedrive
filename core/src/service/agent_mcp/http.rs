//! # Agent MCP transport
//!
//! The streamable HTTP transport, authentication, sessions and limits.
//!
//! Each request is checked in a fixed order so a refusal never depends on
//! what the request asked for: header size, Host, Origin and query string
//! first, then the client address's failed-authentication lockout, then the
//! grant through `authorize`, then the grant's rate, its session, and its call
//! budgets. Only then does a tool run.
//!
//! Every request carrying a token is audited before it is answered: the
//! decision row `authorize_with` writes is completed with the response's size,
//! result and duration, a rate-limited request is folded into its grant's
//! per-minute tally, and a locked-out address is tallied per minute. A write
//! that fails answers 503 instead of the response.

use std::{
	collections::HashMap,
	net::{IpAddr, SocketAddr},
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, Mutex,
	},
	time::{Duration, Instant},
};

use axum::{
	body::{Bytes, HttpBody},
	extract::{ConnectInfo, DefaultBodyLimit, State},
	http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
	response::{IntoResponse, Response},
	routing::{self, any, get},
	Router,
};
use rand::RngCore;
use serde_json::{json, Value};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, warn};
use uuid::Uuid;

use super::{
	limits::{Bucket, ByteWindow, Failures, Limits},
	oauth, paths, tools, MCP_PATH,
};
use crate::{
	context::CoreContext,
	ops::grants::{
		audit::{self, TallyKind},
		authorize_with, store, AuditId, AuditResult, GrantCap, GrantError, GrantScope, GrantStatus,
		RequestMeta,
	},
};

/// Protocol revisions this server speaks, newest first.
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";
/// JSON-RPC error code for refusals by the server rather than the protocol.
const REFUSED: i64 = -32001;

struct Session {
	grant: Uuid,
	last_seen: Instant,
}

struct GrantUsage {
	bucket: Bucket,
	bytes: ByteWindow,
	calls: usize,
	grep: bool,
}

/// What the endpoint holds between requests. Nothing here authorizes: a
/// session or a usage row only narrows what a valid token may do.
pub struct Endpoint {
	pub(super) context: Arc<CoreContext>,
	pub(super) oauth: oauth::OAuth,
	bound: SocketAddr,
	limits: Limits,
	sessions: Mutex<HashMap<String, Session>>,
	usage: Mutex<HashMap<Uuid, GrantUsage>>,
	failures: Mutex<HashMap<IpAddr, Failures>>,
	calls: Arc<Semaphore>,
}

impl Endpoint {
	pub fn new(context: Arc<CoreContext>, bound: SocketAddr, limits: Limits) -> Self {
		Self {
			oauth: oauth::OAuth::new(context.data_dir.clone()),
			context,
			bound,
			calls: Arc::new(Semaphore::new(limits.calls_total)),
			limits,
			sessions: Mutex::default(),
			usage: Mutex::default(),
			failures: Mutex::default(),
		}
	}

	fn with_usage<T>(&self, grant: Uuid, f: impl FnOnce(&mut GrantUsage) -> T) -> T {
		let mut usage = self.usage.lock().expect("usage lock");
		let entry = usage.entry(grant).or_insert_with(|| GrantUsage {
			bucket: Bucket::full(self.limits.rate_burst),
			bytes: ByteWindow::new(),
			calls: 0,
			grep: false,
		});
		f(entry)
	}

	pub(super) fn locked_out(&self, ip: IpAddr) -> Option<Duration> {
		self.failures
			.lock()
			.expect("failures lock")
			.get(&ip)
			.and_then(Failures::locked)
	}

	pub(super) fn auth_failed(&self, ip: IpAddr) {
		self.failures
			.lock()
			.expect("failures lock")
			.entry(ip)
			.or_insert_with(Failures::new)
			.fail(
				self.limits.auth_failures_per_minute,
				self.limits.auth_lockout,
			);
	}

	fn open_session(&self, grant: Uuid) -> Option<String> {
		let mut sessions = self.sessions.lock().expect("sessions lock");
		let idle = self.limits.session_idle;
		sessions.retain(|_, session| session.last_seen.elapsed() < idle);
		let open = sessions.values().filter(|s| s.grant == grant).count();
		if open >= self.limits.sessions_per_grant {
			return None;
		}
		let mut secret = [0u8; 32];
		rand::rngs::OsRng.fill_bytes(&mut secret);
		let id = hex::encode(secret);
		sessions.insert(
			id.clone(),
			Session {
				grant,
				last_seen: Instant::now(),
			},
		);
		Some(id)
	}

	/// Whether `id` is a live session of `grant`. A session id presented with
	/// another grant's token is treated as unknown.
	fn touch_session(&self, id: &str, grant: Uuid) -> bool {
		let mut sessions = self.sessions.lock().expect("sessions lock");
		match sessions.get_mut(id) {
			Some(session)
				if session.grant == grant
					&& session.last_seen.elapsed() < self.limits.session_idle =>
			{
				session.last_seen = Instant::now();
				true
			}
			_ => false,
		}
	}

	fn close_session(&self, id: &str, grant: Uuid) -> bool {
		let mut sessions = self.sessions.lock().expect("sessions lock");
		if sessions.get(id).is_some_and(|s| s.grant == grant) {
			sessions.remove(id);
			return true;
		}
		false
	}

	/// Claim a call slot for `grant`, or say which budget is spent.
	fn begin_call(self: &Arc<Self>, grant: Uuid, grep: bool) -> Result<CallSlot, String> {
		let total = self
			.calls
			.clone()
			.try_acquire_owned()
			.map_err(|_| "the endpoint is at its limit of tool calls in flight".to_string())?;
		self.with_usage(grant, |usage| {
			if let Some(wait) = usage.bytes.spent(self.limits.bytes_per_hour) {
				return Err(format!(
					"the grant has returned its hourly byte budget; retry in {} s",
					wait.as_secs().max(1)
				));
			}
			if usage.calls >= self.limits.calls_per_grant {
				return Err("the grant is at its limit of tool calls in flight".to_string());
			}
			if grep && usage.grep {
				return Err("the grant already has a grep in flight".to_string());
			}
			usage.calls += 1;
			usage.grep |= grep;
			Ok(())
		})?;
		Ok(CallSlot {
			endpoint: self.clone(),
			grant,
			grep,
			_total: total,
		})
	}
}

/// A claimed tool call. Dropping it gives the slot back, whether the call
/// finished, timed out or its client went away.
struct CallSlot {
	endpoint: Arc<Endpoint>,
	grant: Uuid,
	grep: bool,
	_total: OwnedSemaphorePermit,
}

impl Drop for CallSlot {
	fn drop(&mut self) {
		let grep = self.grep;
		self.endpoint.with_usage(self.grant, |usage| {
			usage.calls = usage.calls.saturating_sub(1);
			if grep {
				usage.grep = false;
			}
		});
	}
}

struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
	fn drop(&mut self) {
		self.0.store(true, Ordering::Relaxed);
	}
}

pub fn router(endpoint: Arc<Endpoint>) -> Router {
	let body_limit = endpoint.limits.max_body_bytes;
	let resource_metadata = format!("{}{MCP_PATH}", oauth::PROTECTED_RESOURCE_PATH);
	Router::new()
		.route(MCP_PATH, any(handle))
		.route(
			oauth::PROTECTED_RESOURCE_PATH,
			get(oauth::protected_resource),
		)
		.route(&resource_metadata, get(oauth::protected_resource))
		.route(
			oauth::AUTHORIZATION_SERVER_PATH,
			get(oauth::authorization_server),
		)
		.route(oauth::REGISTER_PATH, routing::post(oauth::register))
		.route(
			oauth::AUTHORIZE_PATH,
			get(oauth::authorize).post(oauth::consent),
		)
		.route(oauth::TOKEN_PATH, routing::post(oauth::token))
		.route(oauth::REVOKE_PATH, routing::post(oauth::revoke))
		.layer(DefaultBodyLimit::max(body_limit))
		.with_state(endpoint)
}

async fn handle(
	State(endpoint): State<Arc<Endpoint>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	method: Method,
	uri: Uri,
	headers: HeaderMap,
	body: Bytes,
) -> Response {
	if let Err(refusal) = screen(&endpoint, &headers, &uri) {
		return refusal;
	}
	let base = oauth::base_url(&headers);
	match method {
		Method::POST => post(&endpoint, peer.ip(), &base, &headers, &body).await,
		Method::DELETE => delete(&endpoint, peer.ip(), &base, &headers).await,
		Method::GET => {
			let mut response = plain(
				StatusCode::METHOD_NOT_ALLOWED,
				"this server does not open an SSE stream",
			);
			response
				.headers_mut()
				.insert(header::ALLOW, HeaderValue::from_static("POST, DELETE"));
			response
		}
		_ => plain(StatusCode::FORBIDDEN, "method not allowed"),
	}
}

/// Checks that need nothing but the request line and headers.
///
/// Host must name loopback with the bound port, which refuses a page that
/// rebinds its own domain to 127.0.0.1. Any Origin is refused, since MCP
/// clients outside a browser send none, and the endpoint sends no CORS
/// headers. A token in the query string is refused before it can be used.
fn screen(endpoint: &Endpoint, headers: &HeaderMap, uri: &Uri) -> Result<(), Response> {
	endpoint.screen_host(headers)?;
	if headers.contains_key(header::ORIGIN) {
		return Err(plain(StatusCode::FORBIDDEN, "origin not allowed"));
	}
	if let Some(query) = uri.query() {
		let lower = query.to_ascii_lowercase();
		if lower.contains("sdg_") || lower.contains("token") {
			return Err(plain(
				StatusCode::BAD_REQUEST,
				"send the grant token in the Authorization header, never in the URL",
			));
		}
	}
	Ok(())
}

impl Endpoint {
	/// The header size and Host checks every route shares.
	pub(super) fn screen_host(&self, headers: &HeaderMap) -> Result<(), Response> {
		let header_bytes: usize = headers
			.iter()
			.map(|(name, value)| name.as_str().len() + value.len())
			.sum();
		if header_bytes > self.limits.max_header_bytes {
			return Err(plain(
				StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
				"request headers are too large",
			));
		}
		let host_ok = headers
			.get(header::HOST)
			.and_then(|host| host.to_str().ok())
			.is_some_and(|host| loopback_host(host, self.bound.port()));
		if !host_ok {
			return Err(plain(StatusCode::FORBIDDEN, "not a loopback host"));
		}
		Ok(())
	}
}

/// Whether `authority` is a loopback name with `port`.
fn loopback_host(authority: &str, port: u16) -> bool {
	let Some((host, given)) = authority.rsplit_once(':') else {
		return false;
	};
	given.parse::<u16>() == Ok(port) && matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
	let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
	let (scheme, token) = value.split_once(' ')?;
	scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// What a JSON-RPC message asks for, enough to authorize it.
struct Request {
	id: Option<Value>,
	method: Option<String>,
	params: Value,
}

impl Request {
	fn parse(body: &[u8]) -> Result<Self, Response> {
		let message: Value = serde_json::from_slice(body)
			.map_err(|_| rpc_error(StatusCode::BAD_REQUEST, Value::Null, -32700, "parse error"))?;
		let Value::Object(mut object) = message else {
			return Err(rpc_error(
				StatusCode::BAD_REQUEST,
				Value::Null,
				-32600,
				"send one JSON-RPC message per request",
			));
		};
		Ok(Self {
			id: object.remove("id").filter(|id| !id.is_null()),
			method: object
				.remove("method")
				.and_then(|m| m.as_str().map(str::to_string)),
			params: object.remove("params").unwrap_or_else(|| json!({})),
		})
	}

	fn id(&self) -> Value {
		self.id.clone().unwrap_or(Value::Null)
	}

	/// The grep or glob pattern a tool call carries, for the audit.
	fn pattern(&self) -> Option<String> {
		if self.method.as_deref() != Some("tools/call") {
			return None;
		}
		self.params["arguments"]["pattern"]
			.as_str()
			.map(str::to_string)
	}

	/// The audit name, the grant-relative path and the capability this
	/// message needs.
	fn target(&self) -> (String, String, GrantCap) {
		match self.method.as_deref() {
			Some("tools/call") => {
				let name = self.params["name"].as_str().unwrap_or("unknown");
				let path = self.params["arguments"]["path"].as_str().unwrap_or("");
				let cap = tools::Tool::parse(name).map_or(GrantCap::Read, tools::Tool::cap);
				(format!("mcp.{name}"), path.to_string(), cap)
			}
			Some(method) => (format!("mcp.{method}"), String::new(), GrantCap::Read),
			None => ("mcp.response".to_string(), String::new(), GrantCap::Read),
		}
	}
}

/// Run `authorize` and turn a refusal into the response the spec and the
/// threat model call for.
async fn check(
	endpoint: &Endpoint,
	ip: IpAddr,
	base: &str,
	token: &str,
	op: &str,
	path: &str,
	cap: GrantCap,
	meta: &RequestMeta,
	id: &Value,
) -> Result<(GrantScope, AuditId), Response> {
	match authorize_with(&endpoint.context, token, op, Path::new(path), cap, meta).await {
		Ok(authorized) => Ok(authorized),
		Err(refusal) if refusal.is_credential() => {
			endpoint.auth_failed(ip);
			Err(unauthorized(id.clone(), base))
		}
		Err(GrantError::Internal(reason)) => {
			warn!(%reason, "agent MCP request could not be authorized");
			Err(unavailable(id.clone()))
		}
		Err(refusal) => Err(forbidden(id.clone(), refusal_message(&refusal))),
	}
}

async fn post(
	endpoint: &Arc<Endpoint>,
	ip: IpAddr,
	base: &str,
	headers: &HeaderMap,
	body: &[u8],
) -> Response {
	let started = Instant::now();
	if let Some(wait) = endpoint.locked_out(ip) {
		return endpoint.refuse_locked_out(ip, wait).await;
	}
	let Some(token) = bearer(headers) else {
		endpoint.auth_failed(ip);
		return unauthorized(Value::Null, base);
	};
	let request = match Request::parse(body) {
		Ok(request) => request,
		Err(response) => return response,
	};
	let id = request.id();
	if let Some(version) = headers.get(VERSION_HEADER) {
		if !version
			.to_str()
			.is_ok_and(|v| PROTOCOL_VERSIONS.contains(&v))
		{
			return rpc_error(
				StatusCode::BAD_REQUEST,
				id,
				-32600,
				"unsupported protocol version",
			);
		}
	}

	let (op, path, cap) = request.target();
	let meta = RequestMeta {
		client: Some(ip),
		session: session_id(headers).map(str::to_string),
		pattern: request.pattern(),
	};
	let (scope, row) = match check(endpoint, ip, base, token, &op, &path, cap, &meta, &id).await {
		Ok(authorized) => authorized,
		Err(response) => return response,
	};
	if let Err(reason) = paths::check(&path) {
		let refused = (forbidden(id.clone(), reason), AuditResult::Denied);
		return endpoint.finish(&scope, row, started, refused, &id).await;
	}
	if let Err(wait) = endpoint.with_usage(scope.grant_id, |usage| {
		usage
			.bucket
			.take(endpoint.limits.rate_per_second, endpoint.limits.rate_burst)
	}) {
		let response = too_many(id.clone(), wait, "the grant is over its request rate");
		return endpoint
			.fold_rate_limited(&scope, row, ip, response, &id)
			.await;
	}
	let served = serve(endpoint, base, token, headers, scope.clone(), request).await;
	endpoint.finish(&scope, row, started, served, &id).await
}

/// Answer an authorized request, with the result its audit row records.
async fn serve(
	endpoint: &Arc<Endpoint>,
	base: &str,
	token: &str,
	headers: &HeaderMap,
	scope: GrantScope,
	request: Request,
) -> (Response, AuditResult) {
	let id = request.id();
	let grant = scope.grant_id;
	if request.method.as_deref() == Some("initialize") {
		let Some(session) = endpoint.open_session(grant) else {
			let response = too_many(
				id,
				Duration::from_secs(60),
				"the grant has too many open sessions",
			);
			return (response, AuditResult::Limited);
		};
		let mut response = rpc_result(id, initialize(&request.params));
		if let Ok(value) = HeaderValue::from_str(&session) {
			response.headers_mut().insert(SESSION_HEADER, value);
		}
		return (response, AuditResult::Ok);
	}

	let Some(session) = session_id(headers) else {
		let response = rpc_error(
			StatusCode::BAD_REQUEST,
			id,
			-32600,
			"missing Mcp-Session-Id; send initialize first",
		);
		return (response, AuditResult::Error);
	};
	if !endpoint.touch_session(session, grant) {
		let response = rpc_error(StatusCode::NOT_FOUND, id, REFUSED, "unknown session");
		return (response, AuditResult::Denied);
	}

	// Notifications and responses carry no id and get no body.
	if request.id.is_none() || request.method.is_none() {
		return (StatusCode::ACCEPTED.into_response(), AuditResult::Ok);
	}

	match request.method.as_deref() {
		Some("ping") => (rpc_result(id, json!({})), AuditResult::Ok),
		Some("tools/list") => (
			rpc_result(id, json!({ "tools": tools::definitions() })),
			AuditResult::Ok,
		),
		Some("tools/call") => call(endpoint, base, token, scope, request).await,
		_ => (
			rpc_error(StatusCode::OK, id, -32601, "method not found"),
			AuditResult::Error,
		),
	}
}

async fn call(
	endpoint: &Arc<Endpoint>,
	base: &str,
	token: &str,
	scope: GrantScope,
	request: Request,
) -> (Response, AuditResult) {
	let id = request.id();
	let name = request.params["name"].as_str().unwrap_or_default();
	let Some(tool) = tools::Tool::parse(name) else {
		let response = rpc_error(StatusCode::OK, id, -32602, "unknown tool");
		return (response, AuditResult::Error);
	};
	let arguments = request
		.params
		.get("arguments")
		.cloned()
		.unwrap_or_else(|| json!({}));
	let slot = match endpoint.begin_call(scope.grant_id, tool.exclusive()) {
		Ok(slot) => slot,
		Err(reason) => {
			let response = too_many(id, Duration::from_secs(1), &reason);
			return (response, AuditResult::Limited);
		}
	};

	let cancel = Arc::new(AtomicBool::new(false));
	let _cancel_on_drop = CancelOnDrop(cancel.clone());
	let grant = scope.grant_id;
	let library = scope.library_id;
	let work = tools::call(
		endpoint.context.clone(),
		token,
		tool,
		scope,
		arguments,
		cancel.clone(),
	);
	let outcome = tokio::select! {
		outcome = tokio::time::timeout(endpoint.limits.call_timeout, work) => outcome,
		_ = ended(endpoint, library, grant) => {
			debug!(%grant, "cancelled a tool call whose grant ended");
			return (unauthorized(id, base), AuditResult::Denied);
		}
	};
	let (result, audited) = match outcome {
		Ok(Ok(output)) => (output.into_result(), AuditResult::Ok),
		Ok(Err(message)) => (tools::error_result(&message), AuditResult::Error),
		Err(_) => {
			cancel.store(true, Ordering::Relaxed);
			let message = format!(
				"the call ran past {} s and was cancelled; narrow the path or pattern",
				endpoint.limits.call_timeout.as_secs()
			);
			(tools::error_result(&message), AuditResult::Error)
		}
	};
	drop(slot);
	let response = rpc_body(id, json!({ "result": result }));
	endpoint.with_usage(grant, |usage| usage.bytes.add(response.len() as u64));
	(json_response(StatusCode::OK, response), audited)
}

/// Resolves once the grant is revoked, expires or disappears, polling the
/// row so a call in flight stops within a poll interval of that.
async fn ended(endpoint: &Endpoint, library_id: Uuid, grant: Uuid) {
	let mut tick = tokio::time::interval(endpoint.limits.revocation_poll);
	tick.tick().await;
	loop {
		tick.tick().await;
		let Some(library) = endpoint
			.context
			.libraries()
			.await
			.get_library(library_id)
			.await
		else {
			return;
		};
		match store::by_id(library.db().conn(), grant).await {
			Ok(Some(row)) if store::status(&row, chrono::Utc::now()) == GrantStatus::Active => {}
			Ok(_) => return,
			Err(e) => warn!(%grant, error = %e, "could not re-check a grant during a call"),
		}
	}
}

async fn delete(endpoint: &Arc<Endpoint>, ip: IpAddr, base: &str, headers: &HeaderMap) -> Response {
	let started = Instant::now();
	if let Some(wait) = endpoint.locked_out(ip) {
		return endpoint.refuse_locked_out(ip, wait).await;
	}
	let Some(token) = bearer(headers) else {
		endpoint.auth_failed(ip);
		return unauthorized(Value::Null, base);
	};
	let meta = RequestMeta {
		client: Some(ip),
		session: session_id(headers).map(str::to_string),
		pattern: None,
	};
	let (scope, row) = match check(
		endpoint,
		ip,
		base,
		token,
		"mcp.session.end",
		"",
		GrantCap::Read,
		&meta,
		&Value::Null,
	)
	.await
	{
		Ok(authorized) => authorized,
		Err(response) => return response,
	};
	let served = match session_id(headers) {
		None => (
			plain(StatusCode::BAD_REQUEST, "missing Mcp-Session-Id"),
			AuditResult::Error,
		),
		Some(session) if endpoint.close_session(session, scope.grant_id) => {
			(StatusCode::NO_CONTENT.into_response(), AuditResult::Ok)
		}
		Some(_) => (
			plain(StatusCode::NOT_FOUND, "unknown session"),
			AuditResult::Denied,
		),
	};
	endpoint
		.finish(&scope, row, started, served, &Value::Null)
		.await
}

impl Endpoint {
	async fn library_db(&self, library_id: Uuid) -> Option<sea_orm::DatabaseConnection> {
		let library = self
			.context
			.libraries()
			.await
			.get_library(library_id)
			.await?;
		Some(library.db().conn().clone())
	}

	/// Complete an authorized request's audit row, then hand back its
	/// response, or 503 when the row cannot be completed.
	async fn finish(
		&self,
		scope: &GrantScope,
		row: AuditId,
		started: Instant,
		(response, result): (Response, AuditResult),
		id: &Value,
	) -> Response {
		let bytes = response.body().size_hint().exact();
		let written = match self.library_db(scope.library_id).await {
			Some(db) => audit::finish(&db, row, bytes, result, started.elapsed())
				.await
				.map_err(|e| e.to_string()),
			None => Err("the library is closed".to_string()),
		};
		match written {
			Ok(()) => response,
			Err(reason) => {
				warn!(grant = %scope.grant_id, %reason, "refused a request whose audit could not be completed");
				unavailable(id.clone())
			}
		}
	}

	/// Count a request over its grant's rate in the grant's tally instead of
	/// keeping its own row, then hand back the 429.
	async fn fold_rate_limited(
		&self,
		scope: &GrantScope,
		row: AuditId,
		ip: IpAddr,
		response: Response,
		id: &Value,
	) -> Response {
		let written = match self.library_db(scope.library_id).await {
			Some(db) => audit::fold_rate_limited(&db, row, scope.grant_id, Some(ip))
				.await
				.map_err(|e| e.to_string()),
			None => Err("the library is closed".to_string()),
		};
		match written {
			Ok(()) => response,
			Err(reason) => {
				warn!(grant = %scope.grant_id, %reason, "could not tally a rate-limited request");
				unavailable(id.clone())
			}
		}
	}

	/// Count a request from a locked-out address in every open library, since
	/// its token is never looked up, then answer 429.
	async fn refuse_locked_out(&self, ip: IpAddr, wait: Duration) -> Response {
		for library in self.context.libraries().await.list().await {
			let counted = audit::tally(
				library.db().conn(),
				TallyKind::LockedOut,
				&ip.to_string(),
				None,
				Some(ip),
			)
			.await;
			if let Err(e) = counted {
				warn!(client = %ip, error = %e, "could not tally a locked-out request");
				return unavailable(Value::Null);
			}
		}
		too_many(Value::Null, wait, "too many failed authentications")
	}
}

fn session_id(headers: &HeaderMap) -> Option<&str> {
	headers.get(SESSION_HEADER).and_then(|v| v.to_str().ok())
}

fn initialize(params: &Value) -> Value {
	let asked = params["protocolVersion"].as_str().unwrap_or_default();
	let version = PROTOCOL_VERSIONS
		.iter()
		.find(|v| **v == asked)
		.unwrap_or(&PROTOCOL_VERSIONS[0]);
	json!({
		"protocolVersion": version,
		"capabilities": { "tools": { "listChanged": false } },
		"serverInfo": {
			"name": "spacedrive",
			"title": "Spacedrive",
			"version": env!("CARGO_PKG_VERSION"),
		},
		"instructions": "Read-only access to one folder lent by a Spacedrive grant. Paths are relative to that folder. Call get_skill once before other tools: it explains the path rules, the read caps, and why grep and search results can be incomplete.",
	})
}

/// A refusal's message without any host path in it.
fn refusal_message(refusal: &GrantError) -> &'static str {
	match refusal {
		GrantError::Escape(_) => "the path is outside the granted folder",
		GrantError::InvalidPath(_) => "invalid path",
		GrantError::ReadOnly => "the grant is read-only",
		GrantError::Protected(_) => "the grant cannot modify that file",
		GrantError::RootUnavailable(_) => "the granted folder is unavailable",
		_ => "refused",
	}
}

fn rpc_body(id: Value, mut fields: Value) -> Vec<u8> {
	fields["jsonrpc"] = json!("2.0");
	fields["id"] = id;
	serde_json::to_vec(&fields).unwrap_or_default()
}

fn json_response(status: StatusCode, body: Vec<u8>) -> Response {
	(
		status,
		[(
			header::CONTENT_TYPE,
			HeaderValue::from_static("application/json"),
		)],
		body,
	)
		.into_response()
}

fn rpc_result(id: Value, result: Value) -> Response {
	json_response(StatusCode::OK, rpc_body(id, json!({ "result": result })))
}

fn rpc_error(status: StatusCode, id: Value, code: i64, message: &str) -> Response {
	json_response(
		status,
		rpc_body(id, json!({ "error": { "code": code, "message": message } })),
	)
}

/// The one answer for an unknown, expired or revoked token, so the response
/// never tells them apart. The challenge points an OAuth client at the
/// metadata it needs to get a new token.
fn unauthorized(id: Value, base: &str) -> Response {
	let mut response = rpc_error(
		StatusCode::UNAUTHORIZED,
		id,
		REFUSED,
		"invalid, expired or revoked grant token",
	);
	if let Ok(value) = HeaderValue::from_str(&oauth::challenge(base)) {
		response
			.headers_mut()
			.insert(header::WWW_AUTHENTICATE, value);
	}
	response
}

/// The answer when a request's audit cannot be written: nothing is served
/// unrecorded.
fn unavailable(id: Value) -> Response {
	rpc_error(
		StatusCode::SERVICE_UNAVAILABLE,
		id,
		REFUSED,
		"the request could not be recorded; try again",
	)
}

fn forbidden(id: Value, message: &str) -> Response {
	rpc_error(StatusCode::FORBIDDEN, id, REFUSED, message)
}

fn too_many(id: Value, wait: Duration, message: &str) -> Response {
	let mut response = rpc_error(StatusCode::TOO_MANY_REQUESTS, id, REFUSED, message);
	let seconds = wait.as_secs_f64().ceil().max(1.0) as u64;
	if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
		response.headers_mut().insert(header::RETRY_AFTER, value);
	}
	response
}

fn plain(status: StatusCode, message: &'static str) -> Response {
	(status, message).into_response()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hosts_must_be_loopback_with_the_bound_port() {
		assert!(loopback_host("127.0.0.1:7765", 7765));
		assert!(loopback_host("localhost:7765", 7765));
		assert!(loopback_host("[::1]:7765", 7765));
		assert!(!loopback_host("127.0.0.1:80", 7765));
		assert!(!loopback_host("evil.example:7765", 7765));
		assert!(!loopback_host("127.0.0.1", 7765));
		assert!(!loopback_host("127.0.0.1.evil.example:7765", 7765));
	}

	#[test]
	fn bearer_tokens_are_read_from_the_header() {
		let mut headers = HeaderMap::new();
		assert_eq!(bearer(&headers), None);
		headers.insert(
			header::AUTHORIZATION,
			HeaderValue::from_static("Bearer sdg_abc"),
		);
		assert_eq!(bearer(&headers), Some("sdg_abc"));
		headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic abc"));
		assert_eq!(bearer(&headers), None);
	}
}
