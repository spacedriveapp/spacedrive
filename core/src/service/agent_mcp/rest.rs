//! # Space endpoint REST surface
//!
//! The REST half of `docs/core/design/space-endpoint.md`: the tool registry
//! over plain HTTP, the block-read protocol a mount client caches by, and the
//! event stream. It shares the MCP transport's screening, authorization and
//! limits, so a grant's budgets count requests from both surfaces together.
//!
//! Every request is checked in the same order as MCP: Host, Origin and query
//! string, the client address's lockout, the token, the path spelling, then
//! `authorize` with the capability the route needs, then the space id, then
//! the grant's rate and call budgets. Every answer outside 2xx is the JSON
//! error shape of the spec, and no message carries a host path.
//!
//! Until spaces exist, a space is a grant and its id is the grant id;
//! [`space_scope`] is the one place that mapping lives.

use std::{
	io::{Read, Seek, SeekFrom},
	net::{IpAddr, SocketAddr},
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::{Duration, SystemTime},
};

use axum::{
	body::Bytes,
	extract::{rejection::BytesRejection, ConnectInfo, State},
	http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
	response::{IntoResponse, Response},
	routing::any,
	Router,
};
use chrono::{DateTime, SecondsFormat, Utc};
use percent_encoding::percent_decode_str;
use serde_json::{json, Value};
use tracing::{debug, warn};
use uuid::Uuid;

use super::{
	events,
	http::{self, CancelOnDrop, Endpoint},
	limits::{PAGE_DEFAULT, PAGE_MAX},
	paths,
	tools::{self, Tool},
	SKILL,
};
use crate::{
	common::contain::{self, Directory, Kind, Stat},
	config::MANAGED_DIR,
	context::CoreContext,
	ops::{
		files::bucket as files_bucket,
		grants::{self, authorize, bucket::BucketUrl, store, GrantCap, GrantError, GrantScope},
		indexing::{metadata::refuse_dataless_blocking, state::EntryKind},
	},
	volume::{VolumeBackend, VolumeError},
};

/// Where the REST surface is mounted.
pub const SPACES_PATH: &str = "/v1/spaces";
/// The block size a mount caches by.
pub const BLOCK_SIZE: u64 = 1024 * 1024;
/// The most one block read answers.
pub const MAX_READ_BYTES: u64 = 8 * BLOCK_SIZE;

pub(super) fn routes() -> Router<Arc<Endpoint>> {
	Router::new()
		.route(SPACES_PATH, any(handle))
		.route(&format!("{SPACES_PATH}/*rest"), any(handle))
}

/// What a request under [`SPACES_PATH`] names. Paths are decoded and
/// checked; the space id is not, since only `authorize` can judge it.
#[derive(Debug, PartialEq, Eq)]
enum Route {
	Spaces,
	Tools { space: String },
	Tool { space: String, name: String },
	Stat { space: String, path: String },
	List { space: String, path: String },
	Blocks { space: String, path: String },
	Events { space: String },
}

impl Route {
	fn parse(uri_path: &str) -> Result<Self, Response> {
		let not_found = || error(StatusCode::NOT_FOUND, "no such route");
		let rest = uri_path.strip_prefix(SPACES_PATH).ok_or_else(not_found)?;
		let rest = match rest {
			"" | "/" => return Ok(Self::Spaces),
			rest => rest.strip_prefix('/').ok_or_else(not_found)?,
		};
		let (space, rest) = rest.split_once('/').unwrap_or((rest, ""));
		let space = segment(space)?;
		let (verb, tail) = rest.split_once('/').unwrap_or((rest, ""));
		Ok(match verb {
			"tools" if tail.is_empty() => Self::Tools { space },
			"tools" => Self::Tool {
				space,
				name: segment(tail)?,
			},
			"stat" => Self::Stat {
				space,
				path: decode_path(tail)?,
			},
			"list" => Self::List {
				space,
				path: decode_path(tail)?,
			},
			"blocks" => Self::Blocks {
				space,
				path: decode_path(tail)?,
			},
			"events" if tail.is_empty() => Self::Events { space },
			_ => return Err(not_found()),
		})
	}

	fn method(&self) -> Method {
		match self {
			Self::Tool { .. } => Method::POST,
			_ => Method::GET,
		}
	}
}

fn segment(raw: &str) -> Result<String, Response> {
	let decoded = percent_decode_str(raw)
		.decode_utf8()
		.map_err(|_| error(StatusCode::BAD_REQUEST, "paths must be UTF-8"))?;
	if decoded.contains('/') {
		return Err(error(
			StatusCode::BAD_REQUEST,
			"a path segment cannot contain an encoded '/'",
		));
	}
	Ok(decoded.into_owned())
}

/// A space-relative path from its URL spelling: each segment percent-decoded
/// on its own, so an encoded `/` can never join two segments.
fn decode_path(raw: &str) -> Result<String, Response> {
	let raw = raw.strip_suffix('/').unwrap_or(raw);
	if raw.is_empty() {
		return Ok(String::new());
	}
	let mut segments = Vec::new();
	for raw in raw.split('/') {
		let decoded = segment(raw)?;
		if decoded == "." || decoded == ".." {
			return Err(error(
				StatusCode::BAD_REQUEST,
				"paths cannot contain '.' or '..' segments",
			));
		}
		segments.push(decoded);
	}
	let path = segments.join("/");
	paths::check(&path).map_err(|reason| error(StatusCode::BAD_REQUEST, reason))?;
	Ok(path)
}

async fn handle(
	State(endpoint): State<Arc<Endpoint>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	method: Method,
	uri: Uri,
	headers: HeaderMap,
	body: Result<Bytes, BytesRejection>,
) -> Response {
	match serve(&endpoint, peer.ip(), method, &uri, &headers, body).await {
		Ok(response) | Err(response) => response,
	}
}

async fn serve(
	endpoint: &Arc<Endpoint>,
	ip: IpAddr,
	method: Method,
	uri: &Uri,
	headers: &HeaderMap,
	body: Result<Bytes, BytesRejection>,
) -> Result<Response, Response> {
	screen(endpoint, &method, headers, uri)?;
	if let Some(wait) = endpoint.locked_out(ip) {
		return Err(too_many(wait, "too many failed authentications"));
	}
	let Some(token) = http::bearer(headers).map(str::to_string) else {
		endpoint.auth_failed(ip);
		return Err(unauthorized());
	};
	let route = Route::parse(uri.path())?;
	if method != route.method() {
		let mut response = error(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
		let allow = if route.method() == Method::POST {
			"POST"
		} else {
			"GET"
		};
		response
			.headers_mut()
			.insert(header::ALLOW, HeaderValue::from_static(allow));
		return Err(response);
	}
	let request = Request {
		endpoint,
		ip,
		token: &token,
	};
	match route {
		Route::Spaces => request.spaces().await,
		Route::Tools { space } => {
			request
				.space_scope(&space, "rest.tools", "", GrantCap::Read)
				.await?;
			Ok(json_ok(json!({ "tools": tools::definitions() })))
		}
		Route::Tool { space, name } => request.tool(&space, &name, body).await,
		Route::Stat { space, path } => {
			let scope = request
				.space_scope(&space, "rest.stat", &path, GrantCap::Read)
				.await?;
			let (grant, library) = (scope.grant_id, scope.library_id);
			let context = endpoint.context.clone();
			let value = request
				.blocking((grant, library), false, move || {
					match BucketUrl::parse(&scope.path) {
						Some(url) => block_on(bucket_stat(&context, &url, &path)),
						None => stat(&scope, &path),
					}
				})
				.await?;
			Ok(request.json(grant, value))
		}
		Route::List { space, path } => {
			let (cursor, limit) = list_query(uri)?;
			let scope = request
				.space_scope(&space, "rest.list", &path, GrantCap::Read)
				.await?;
			let (grant, library) = (scope.grant_id, scope.library_id);
			let context = endpoint.context.clone();
			let value = request
				.blocking((grant, library), false, move || {
					match BucketUrl::parse(&scope.path) {
						Some(url) => block_on(bucket_list(&context, &url, &path, cursor, limit)),
						None => list(&scope, &path, cursor, limit),
					}
				})
				.await?;
			Ok(request.json(grant, value))
		}
		Route::Blocks { space, path } => {
			let scope = request
				.space_scope(&space, "rest.blocks", &path, GrantCap::Read)
				.await?;
			let range = parse_range(headers)?;
			let if_match = headers
				.get(header::IF_MATCH)
				.map(|value| value.to_str().unwrap_or_default().to_string());
			let (grant, library) = (scope.grant_id, scope.library_id);
			let context = endpoint.context.clone();
			let block = request
				.blocking((grant, library), true, move || {
					match BucketUrl::parse(&scope.path) {
						Some(url) => {
							block_on(bucket_block(&context, &url, range, if_match.as_deref()))
						}
						None => read_block(&scope, range, if_match.as_deref()),
					}
				})
				.await?;
			let returned = block.bytes.len() as u64;
			endpoint.with_usage(grant, |usage| usage.block_bytes.add(returned));
			Ok(block.into_response())
		}
		Route::Events { space } => {
			let scope = request
				.space_scope(&space, "rest.events", "", GrantCap::Read)
				.await?;
			let last_event_id = headers
				.get("last-event-id")
				.and_then(|value| value.to_str().ok())
				.map(str::to_string);
			Ok(events::open(endpoint.clone(), scope, last_event_id).await)
		}
	}
}

/// Checks that need nothing but the request line and headers.
///
/// The endpoint sends no CORS headers, so any request carrying `Origin`, and
/// any preflight, is refused. A token in the query string is refused before
/// anything could use it.
fn screen(
	endpoint: &Endpoint,
	method: &Method,
	headers: &HeaderMap,
	uri: &Uri,
) -> Result<(), Response> {
	if let Err(refusal) = endpoint.screen_host(headers) {
		return Err(match refusal.status() {
			StatusCode::FORBIDDEN => error(
				StatusCode::FORBIDDEN,
				"the Host header does not name this endpoint",
			),
			status => error(status, "request headers are too large"),
		});
	}
	if headers.contains_key(header::ORIGIN) || method == Method::OPTIONS {
		return Err(error(
			StatusCode::FORBIDDEN,
			"cross-origin requests are not served",
		));
	}
	let query = uri.query().unwrap_or_default();
	let carries_token = url::form_urlencoded::parse(query.as_bytes()).any(|(key, value)| {
		matches!(
			key.to_ascii_lowercase().as_str(),
			"token" | "access_token" | "authorization" | "bearer"
		) || value.starts_with("sdg_")
	});
	if carries_token {
		return Err(error(
			StatusCode::BAD_REQUEST,
			"send the grant token in the Authorization header, never in the URL",
		));
	}
	Ok(())
}

struct Request<'a> {
	endpoint: &'a Arc<Endpoint>,
	ip: IpAddr,
	token: &'a str,
}

impl Request<'_> {
	/// Run `authorize` and turn a refusal into the spec's answer.
	async fn authorize(&self, op: &str, path: &str, cap: GrantCap) -> Result<GrantScope, Response> {
		match authorize(&self.endpoint.context, self.token, op, Path::new(path), cap).await {
			Ok(scope) => Ok(scope),
			Err(refusal) if refusal.is_credential() => {
				self.endpoint.auth_failed(self.ip);
				Err(unauthorized())
			}
			Err(GrantError::Internal(reason)) => {
				warn!(%reason, "space endpoint request could not be authorized");
				Err(error(
					StatusCode::SERVICE_UNAVAILABLE,
					"the request could not be recorded; try again",
				))
			}
			Err(GrantError::RootUnavailable(_)) => Err(error(
				StatusCode::SERVICE_UNAVAILABLE,
				"the space root is unavailable",
			)),
			Err(GrantError::InvalidPath(_)) => Err(error(StatusCode::BAD_REQUEST, "invalid path")),
			Err(refusal) => Err(error(
				StatusCode::FORBIDDEN,
				http::refusal_message(&refusal),
			)),
		}
	}

	/// The scope a request on `space` may use, after its rate is taken.
	///
	/// A space is a grant until the space table exists, so a space id other
	/// than the token's grant id answers 404, the same as one that does not
	/// exist. When spaces land, this is the function that changes.
	async fn space_scope(
		&self,
		space: &str,
		op: &str,
		path: &str,
		cap: GrantCap,
	) -> Result<GrantScope, Response> {
		let scope = self.authorize(op, path, cap).await?;
		if Uuid::parse_str(space).ok() != Some(scope.grant_id) {
			return Err(error(StatusCode::NOT_FOUND, "no such space"));
		}
		self.take_rate(scope.grant_id)?;
		Ok(scope)
	}

	fn take_rate(&self, grant: Uuid) -> Result<(), Response> {
		let limits = &self.endpoint.limits;
		self.endpoint
			.with_usage(grant, |usage| {
				usage.bucket.take(limits.rate_per_second, limits.rate_burst)
			})
			.map_err(|wait| too_many(wait, "the grant is over its request rate"))
	}

	/// Count `bytes` returned against the grant's hourly budget.
	fn count(&self, grant: Uuid, bytes: usize) {
		self.endpoint
			.with_usage(grant, |usage| usage.bytes.add(bytes as u64));
	}

	/// A JSON answer, counted against the grant's byte budget.
	fn json(&self, grant: Uuid, value: Value) -> Response {
		let response = json_ok(value);
		self.count(grant, response_len(&response));
		response
	}

	/// Run filesystem work on the blocking pool inside one of the grant's
	/// call slots, so block reads share the call budget with tools.
	///
	/// `blocks` marks a block read, which spends the grant's block byte
	/// budget rather than the one tools share. A spent budget answers 429
	/// with the seconds until its window resets.
	async fn blocking<T: Send + 'static>(
		&self,
		(grant, library): (Uuid, Uuid),
		blocks: bool,
		work: impl FnOnce() -> Result<T, Response> + Send + 'static,
	) -> Result<T, Response> {
		let limits = &self.endpoint.limits;
		let spent = self.endpoint.with_usage(grant, |usage| {
			if blocks {
				usage.block_bytes.spent(limits.block_bytes_per_hour)
			} else {
				usage.bytes.spent(limits.bytes_per_hour)
			}
		});
		if let Some(wait) = spent {
			return Err(too_many(
				wait,
				"the grant has returned its hourly byte budget",
			));
		}
		let slot = self
			.endpoint
			.begin_call(grant, false, blocks)
			.map_err(|reason| too_many(Duration::from_secs(1), &reason))?;
		// The worker owns the slot, so a read stalled on a slow volume keeps
		// counting against the budgets until it really returns, even after
		// its client got an answer.
		let worker = tokio::task::spawn_blocking(move || {
			let result = work();
			drop(slot);
			result
		});
		tokio::select! {
			joined = tokio::time::timeout(self.endpoint.limits.call_timeout, worker) => match joined {
				Ok(Ok(result)) => result,
				Ok(Err(_)) => Err(error(
					StatusCode::SERVICE_UNAVAILABLE,
					"the request failed; try again",
				)),
				Err(_) => Err(error(
					StatusCode::SERVICE_UNAVAILABLE,
					&format!(
						"the request ran past {} s; try again",
						self.endpoint.limits.call_timeout.as_secs()
					),
				)),
			},
			_ = http::ended(self.endpoint, library, grant) => Err(unauthorized()),
		}
	}

	async fn spaces(&self) -> Result<Response, Response> {
		let scope = self.authorize("rest.spaces", "", GrantCap::Read).await?;
		self.take_rate(scope.grant_id)?;
		let unavailable = || {
			error(
				StatusCode::SERVICE_UNAVAILABLE,
				"the grant could not be read; try again",
			)
		};
		let library = self
			.endpoint
			.context
			.libraries()
			.await
			.get_library(scope.library_id)
			.await
			.ok_or_else(unavailable)?;
		let row = store::by_id(library.db().conn(), scope.grant_id)
			.await
			.map_err(|e| {
				warn!(error = %e, "could not read a grant for the space list");
				unavailable()
			})?
			.ok_or_else(unauthorized)?;
		let name = row
			.label
			.filter(|label| !label.trim().is_empty())
			.or_else(|| {
				scope
					.root
					.file_name()
					.map(|name| name.to_string_lossy().into_owned())
			})
			.unwrap_or_default();
		Ok(json_ok(json!([{
			"id": scope.grant_id,
			"name": name,
			"caps": row.caps,
			"expires_at": timestamp(row.expires_at),
		}])))
	}

	async fn tool(
		&self,
		space: &str,
		name: &str,
		body: Result<Bytes, BytesRejection>,
	) -> Result<Response, Response> {
		let body = body.map_err(|rejection| {
			let status = rejection.status();
			if status == StatusCode::PAYLOAD_TOO_LARGE {
				error(status, "the body is larger than the endpoint accepts")
			} else {
				error(StatusCode::BAD_REQUEST, "the body could not be read")
			}
		})?;
		let arguments: Value = if body.iter().all(u8::is_ascii_whitespace) {
			json!({})
		} else {
			serde_json::from_slice(&body)
				.map_err(|_| error(StatusCode::BAD_REQUEST, "the body is not JSON"))?
		};
		if !arguments.is_object() {
			return Err(error(
				StatusCode::BAD_REQUEST,
				"the body is the tool's arguments object",
			));
		}
		let tool = Tool::parse(name);
		let path = match tool {
			Some(tool) => tools::target_path(tool, &arguments)
				.map_err(|message| error(StatusCode::BAD_REQUEST, &message))?,
			None => String::new(),
		};
		paths::check(&path).map_err(|reason| error(StatusCode::BAD_REQUEST, reason))?;
		let cap = tool.map_or(GrantCap::Read, Tool::cap);
		let scope = self
			.space_scope(space, &format!("rest.tool.{name}"), &path, cap)
			.await?;
		let Some(tool) = tool else {
			return Err(error(StatusCode::NOT_FOUND, "no such tool"));
		};

		let endpoint = self.endpoint;
		let slot = endpoint
			.begin_call(scope.grant_id, tool.exclusive(), false)
			.map_err(|reason| too_many(Duration::from_secs(1), &reason))?;
		let cancel = Arc::new(AtomicBool::new(false));
		let _cancel_on_drop = CancelOnDrop(cancel.clone());
		let (grant, library, root) = (scope.grant_id, scope.library_id, scope.root.clone());
		let replaces = arguments.get("expect").is_some();
		let work = tools::call(
			endpoint.context.clone(),
			self.token,
			tool,
			scope,
			arguments,
			cancel.clone(),
		);
		let outcome = tokio::select! {
			outcome = tokio::time::timeout(endpoint.limits.call_timeout, work) => outcome,
			_ = http::ended(endpoint, library, grant) => {
				debug!(%grant, "cancelled a REST tool call whose grant ended");
				return Err(unauthorized());
			}
		};
		drop(slot);
		let output = match outcome {
			Ok(Ok(output)) => output,
			Ok(Err(message)) => return Err(error(status_for(&message), &message)),
			Err(_) => {
				cancel.store(true, Ordering::Relaxed);
				return Err(error(
					StatusCode::SERVICE_UNAVAILABLE,
					&format!(
						"the call ran past {} s and was cancelled; narrow the path or pattern",
						endpoint.limits.call_timeout.as_secs()
					),
				));
			}
		};
		endpoint
			.feed
			.record(tool, &root, replaces, output.structured());
		if tool != Tool::GetSkill {
			return Ok(self.json(grant, output.structured().clone()));
		}
		self.count(grant, SKILL.len());
		Ok((
			StatusCode::OK,
			[(
				header::CONTENT_TYPE,
				HeaderValue::from_static("text/markdown; charset=utf-8"),
			)],
			SKILL,
		)
			.into_response())
	}
}

/// The status a tool's own failure message maps to. Tools report failures
/// as text, so this reads the messages the tools and the write path send.
fn status_for(message: &str) -> StatusCode {
	let lower = message.to_ascii_lowercase();
	if lower.starts_with("stale:") {
		StatusCode::PRECONDITION_FAILED
	} else if lower.contains("no such file or directory") || lower.starts_with("not found") {
		StatusCode::NOT_FOUND
	} else if lower.contains("exists; name the revision") {
		StatusCode::CONFLICT
	} else if lower.contains("permission denied")
		|| lower.contains("outside the granted")
		|| lower.contains("read-only")
		|| lower.contains("cannot modify")
		|| lower.contains("symlink")
	{
		StatusCode::FORBIDDEN
	} else if lower.contains("unknown grant token")
		|| lower.contains("has expired")
		|| lower.contains("was revoked")
	{
		StatusCode::UNAUTHORIZED
	} else if lower.starts_with("i/o error") || lower.contains("unavailable") {
		StatusCode::SERVICE_UNAVAILABLE
	} else {
		StatusCode::BAD_REQUEST
	}
}

/// The version stamp of a file: size, modification time in nanoseconds, and
/// file identity, quoted as an HTTP entity tag.
/// Every surface describes a file through [`Stat`], from a handle, so a
/// list entry, a stat and a block read stamp one version the same way.
/// File indices are not exposed on stable Rust off unix, so there the
/// identity is 0 and the stamp rests on size and time.
fn etag_of(meta: &Stat) -> String {
	// Signed, so distinct times before 1970 stay distinct.
	let mtime = meta
		.modified
		.map_or(0, |at| match at.duration_since(std::time::UNIX_EPOCH) {
			Ok(after) => after.as_nanos() as i128,
			Err(before) => -(before.duration().as_nanos() as i128),
		});
	format!("\"f-{}-{mtime}-{}\"", meta.len, meta.identity)
}

fn timestamp(at: DateTime<Utc>) -> String {
	at.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// A stat answer, or a list entry when `block_size` is left out.
fn entry(path: String, meta: &Stat) -> Value {
	let kind = match meta.kind {
		Kind::Symlink => "symlink",
		Kind::Dir => "directory",
		Kind::File => "file",
		Kind::Other => "other",
	};
	let mut value = json!({
		"path": path,
		"kind": kind,
		"size": meta.len,
		"modified_at": meta.modified.map(|at| timestamp(at.into())),
	});
	if meta.kind == Kind::File {
		value["etag"] = json!(etag_of(meta));
	}
	value
}

fn fs_error(message: String) -> Response {
	error(status_for(&message), &message)
}

fn stat(scope: &GrantScope, requested: &str) -> Result<Value, Response> {
	// The spelling the client sent, so a link at the path is reported as a
	// link rather than as what `authorize` resolved it to. It is described
	// through a handle on its parent beneath the root, so a link earlier in
	// the spelling cannot point the description outside it.
	let named = requested.strip_suffix('/').unwrap_or(requested);
	let named = if named == "." { "" } else { named };
	let link = contain::stat_beneath(&scope.root, Path::new(named))
		.map_err(|e| fs_error(tools::io_message(&e)))?;
	let mut value = if link.kind == Kind::File {
		let (_, meta) = tools::open_file(scope).map_err(fs_error)?;
		entry(requested.to_string(), &Stat::from(&meta))
	} else {
		entry(requested.to_string(), &link)
	};
	value["block_size"] = json!(BLOCK_SIZE);
	Ok(value)
}

fn list_query(uri: &Uri) -> Result<(Option<String>, usize), Response> {
	let mut cursor = None;
	let mut limit = PAGE_DEFAULT as usize;
	for (key, value) in url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes()) {
		match key.as_ref() {
			"cursor" if !value.is_empty() => cursor = Some(value.into_owned()),
			"limit" => {
				limit = value
					.parse::<usize>()
					.map_err(|_| error(StatusCode::BAD_REQUEST, "limit must be a number"))?
					.clamp(1, PAGE_MAX as usize);
			}
			_ => {}
		}
	}
	Ok((cursor, limit))
}

fn list(
	scope: &GrantScope,
	requested: &str,
	cursor: Option<String>,
	limit: usize,
) -> Result<Value, Response> {
	let dir = Directory::open(&scope.root, &scope.relative).map_err(|e| match e.kind() {
		std::io::ErrorKind::NotADirectory => error(StatusCode::BAD_REQUEST, "not a directory"),
		_ => fs_error(tools::io_message(&e)),
	})?;
	let mut names: Vec<_> = dir
		.names()
		.map_err(|e| fs_error(tools::io_message(&e)))?
		.into_iter()
		.filter(|name| {
			cursor
				.as_deref()
				.map_or(true, |after| name.as_encoded_bytes() > after.as_bytes())
		})
		.collect();
	names.sort_by(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));
	let more = names.len() > limit;
	names.truncate(limit);
	let entries: Vec<Value> = names
		.iter()
		.filter_map(|name| {
			// An entry removed since the listing was read is left out.
			let meta = dir.stat(name).ok()?;
			let shown = name.to_string_lossy();
			let path = if requested.is_empty() {
				shown.into_owned()
			} else {
				format!("{requested}/{shown}")
			};
			Some(entry(path, &meta))
		})
		.collect();
	let mut value = json!({ "entries": entries });
	if more {
		if let Some(last) = names.last() {
			value["next_cursor"] = json!(last.to_string_lossy());
		}
	}
	Ok(value)
}

/// The one range a block read names: `bytes=a-b` or `bytes=a-`.
fn parse_range(headers: &HeaderMap) -> Result<(u64, Option<u64>), Response> {
	let invalid = |message| error(StatusCode::BAD_REQUEST, message);
	let mut values = headers.get_all(header::RANGE).iter();
	let (Some(value), None) = (values.next(), values.next()) else {
		return Err(invalid("send one Range header, such as bytes=0-1048575"));
	};
	let spec = value
		.to_str()
		.ok()
		.and_then(|value| value.trim().strip_prefix("bytes="))
		.ok_or_else(|| invalid("the range must be in bytes"))?;
	if spec.contains(',') {
		return Err(invalid("send one range per request"));
	}
	let (start, end) = spec
		.split_once('-')
		.ok_or_else(|| invalid("the range is malformed"))?;
	let start: u64 = start
		.trim()
		.parse()
		.map_err(|_| invalid("the range needs a first byte"))?;
	let end = match end.trim() {
		"" => None,
		end => Some(
			end.parse::<u64>()
				.map_err(|_| invalid("the range is malformed"))?,
		),
	};
	if end.is_some_and(|end| end < start) {
		return Err(invalid("the range ends before it starts"));
	}
	Ok((start, end))
}

struct Block {
	bytes: Vec<u8>,
	start: u64,
	size: u64,
	etag: String,
}

impl IntoResponse for Block {
	fn into_response(self) -> Response {
		let last = self.start + self.bytes.len() as u64 - 1;
		let mut response = (StatusCode::PARTIAL_CONTENT, self.bytes).into_response();
		let headers = response.headers_mut();
		headers.insert(
			header::CONTENT_TYPE,
			HeaderValue::from_static("application/octet-stream"),
		);
		headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
		if let Ok(value) =
			HeaderValue::from_str(&format!("bytes {}-{last}/{}", self.start, self.size))
		{
			headers.insert(header::CONTENT_RANGE, value);
		}
		if let Ok(value) = HeaderValue::from_str(&self.etag) {
			headers.insert(header::ETAG, value);
		}
		response
	}
}

/// Read a range through one handle, so the etag and the bytes describe the
/// same file even if the path is replaced meanwhile.
fn read_block(
	scope: &GrantScope,
	(start, end): (u64, Option<u64>),
	if_match: Option<&str>,
) -> Result<Block, Response> {
	// A stat the provider answers locally; opening a placeholder would download it.
	if refuse_dataless_blocking(&scope.path).is_err() {
		return Err(error(
			StatusCode::SERVICE_UNAVAILABLE,
			"the file is a cloud placeholder and is not on this device",
		));
	}
	let (mut file, meta) = tools::open_file(scope).map_err(fs_error)?;
	let size = meta.len();
	let etag = etag_of(&Stat::from(&meta));
	if let Some(wanted) = if_match {
		let matches = wanted
			.split(',')
			.map(str::trim)
			.any(|tag| tag == "*" || tag == etag);
		if !matches {
			return Err(stale(&etag));
		}
	}
	if start >= size {
		return Err(unsatisfiable(size));
	}
	let last = end
		.unwrap_or(u64::MAX)
		.min(size - 1)
		.min(start + MAX_READ_BYTES - 1);
	let want = last - start + 1;
	file.seek(SeekFrom::Start(start))
		.map_err(|e| fs_error(tools::io_message(&e)))?;
	let mut bytes = Vec::with_capacity(want as usize);
	(&mut file)
		.take(want)
		.read_to_end(&mut bytes)
		.map_err(|e| fs_error(tools::io_message(&e)))?;
	// A write to the open file during the read would mix versions, so the
	// bytes are sent only if the handle still reports the version checked.
	let after = file
		.metadata()
		.map_err(|e| fs_error(tools::io_message(&e)))?;
	let current = etag_of(&Stat::from(&after));
	if current != etag {
		return Err(stale(&current));
	}
	if bytes.is_empty() {
		return Err(unsatisfiable(size));
	}
	Ok(Block {
		bytes,
		start,
		size,
		etag,
	})
}

fn json_ok(value: Value) -> Response {
	(
		StatusCode::OK,
		[(
			header::CONTENT_TYPE,
			HeaderValue::from_static("application/json"),
		)],
		value.to_string(),
	)
		.into_response()
}

fn response_len(response: &Response) -> usize {
	use axum::body::HttpBody;
	response.body().size_hint().exact().unwrap_or_default() as usize
}

fn code(status: StatusCode) -> &'static str {
	match status {
		StatusCode::UNAUTHORIZED => "unauthorized",
		StatusCode::FORBIDDEN => "forbidden",
		StatusCode::NOT_FOUND => "not_found",
		StatusCode::CONFLICT => "conflict",
		StatusCode::PRECONDITION_FAILED => "stale",
		StatusCode::PAYLOAD_TOO_LARGE => "too_large",
		StatusCode::RANGE_NOT_SATISFIABLE => "range",
		StatusCode::TOO_MANY_REQUESTS => "limited",
		StatusCode::SERVICE_UNAVAILABLE => "unavailable",
		_ => "invalid",
	}
}

fn error_body(status: StatusCode, message: &str) -> Value {
	json!({ "error": { "code": code(status), "message": message } })
}

fn error(status: StatusCode, message: &str) -> Response {
	let mut response = json_ok(error_body(status, message));
	*response.status_mut() = status;
	response
}

/// The one answer for an unknown, expired or revoked token.
fn unauthorized() -> Response {
	let mut response = error(
		StatusCode::UNAUTHORIZED,
		"invalid, expired or revoked grant token",
	);
	response.headers_mut().insert(
		header::WWW_AUTHENTICATE,
		HeaderValue::from_static("Bearer realm=\"spacedrive\""),
	);
	response
}

fn too_many(wait: Duration, message: &str) -> Response {
	let mut response = error(StatusCode::TOO_MANY_REQUESTS, message);
	let seconds = wait.as_secs_f64().ceil().max(1.0) as u64;
	if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
		response.headers_mut().insert(header::RETRY_AFTER, value);
	}
	response
}

fn stale(etag: &str) -> Response {
	let mut body = error_body(
		StatusCode::PRECONDITION_FAILED,
		"the file changed since you read it",
	);
	body["error"]["etag"] = json!(etag);
	let mut response = json_ok(body);
	*response.status_mut() = StatusCode::PRECONDITION_FAILED;
	if let Ok(value) = HeaderValue::from_str(etag) {
		response.headers_mut().insert(header::ETAG, value);
	}
	response
}

fn unsatisfiable(size: u64) -> Response {
	let mut response = error(
		StatusCode::RANGE_NOT_SATISFIABLE,
		"the range starts at or past the end of the file",
	);
	if let Ok(value) = HeaderValue::from_str(&format!("bytes */{size}")) {
		response.headers_mut().insert(header::CONTENT_RANGE, value);
	}
	response
}

/// Run a bucket request on the blocking worker `blocking` gave it, so it
/// spends the same budgets and slots as a read of a local file.
fn block_on<T>(work: impl std::future::Future<Output = T>) -> T {
	tokio::runtime::Handle::current().block_on(work)
}

async fn bucket_backend(
	context: &CoreContext,
	url: &BucketUrl,
) -> Result<Arc<dyn VolumeBackend>, Response> {
	grants::bucket::backend(&context.volume_manager, url)
		.await
		.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e.to_string()))
}

fn bucket_failure(e: VolumeError) -> Response {
	match e {
		VolumeError::PreconditionFailed(_) => error(
			StatusCode::PRECONDITION_FAILED,
			"the file changed since you read it",
		),
		e => error(
			StatusCode::SERVICE_UNAVAILABLE,
			&format!("unavailable: {e}"),
		),
	}
}

/// A bucket's entity tag as an HTTP one. S3 sends them quoted; a service
/// that does not gets quotes added, so the header stays valid.
fn quoted(etag: &str) -> String {
	if etag.starts_with('"') || etag.starts_with("W/") {
		etag.to_string()
	} else {
		format!("\"{etag}\"")
	}
}

fn bucket_file(path: String, size: u64, modified: Option<SystemTime>, etag: &str) -> Value {
	json!({
		"path": path,
		"kind": "file",
		"size": size,
		"modified_at": modified.map(|at| timestamp(at.into())),
		"etag": quoted(etag),
	})
}

fn bucket_directory(path: String) -> Value {
	json!({ "path": path, "kind": "directory", "size": 0, "modified_at": null })
}

/// Stat an object, or a prefix with objects under it. The grant's own
/// prefix is always a directory, even before anything is written to it.
async fn bucket_stat(
	context: &CoreContext,
	url: &BucketUrl,
	requested: &str,
) -> Result<Value, Response> {
	let backend = bucket_backend(context, url).await?;
	let object = match requested.is_empty() {
		true => None,
		false => files_bucket::object(backend.as_ref(), &url.key)
			.await
			.map_err(bucket_failure)?,
	};
	let mut value = match object {
		Some(object) => bucket_file(
			requested.to_string(),
			object.size,
			object.modified,
			&object.etag,
		),
		None if requested.is_empty()
			|| files_bucket::is_directory(backend.as_ref(), url)
				.await
				.map_err(bucket_failure)? =>
		{
			bucket_directory(requested.to_string())
		}
		None => return Err(error(StatusCode::NOT_FOUND, "not found")),
	};
	value["block_size"] = json!(BLOCK_SIZE);
	Ok(value)
}

async fn bucket_list(
	context: &CoreContext,
	url: &BucketUrl,
	requested: &str,
	cursor: Option<String>,
	limit: usize,
) -> Result<Value, Response> {
	let backend = bucket_backend(context, url).await?;
	let mut children = grants::bucket::list(backend.as_ref(), url)
		.await
		.map_err(bucket_failure)?;
	if children.is_empty() && !requested.is_empty() {
		let object = backend
			.entity_tag(Path::new(&url.key))
			.await
			.map_err(bucket_failure)?;
		if object.is_some() {
			return Err(error(StatusCode::BAD_REQUEST, "not a directory"));
		}
		if !files_bucket::is_directory(backend.as_ref(), url)
			.await
			.map_err(bucket_failure)?
		{
			return Err(error(StatusCode::NOT_FOUND, "not found"));
		}
	}
	children.retain(|child| {
		child.name != MANAGED_DIR
			&& cursor
				.as_deref()
				.map_or(true, |after| child.name.as_str() > after)
	});
	let more = children.len() > limit;
	children.truncate(limit);
	let entries: Vec<Value> = children
		.iter()
		.map(|child| {
			let path = if requested.is_empty() {
				child.name.clone()
			} else {
				format!("{requested}/{}", child.name)
			};
			match (&child.kind, &child.etag) {
				(EntryKind::Directory, _) | (_, None) => bucket_directory(path),
				(_, Some(etag)) => bucket_file(path, child.size, child.modified, etag),
			}
		})
		.collect();
	let mut value = json!({ "entries": entries });
	if more {
		if let Some(last) = children.last() {
			value["next_cursor"] = json!(last.name);
		}
	}
	Ok(value)
}

/// Read a range of an object only while it carries the tag its stat gave,
/// so the etag and the bytes describe the same version.
async fn bucket_block(
	context: &CoreContext,
	url: &BucketUrl,
	(start, end): (u64, Option<u64>),
	if_match: Option<&str>,
) -> Result<Block, Response> {
	let backend = bucket_backend(context, url).await?;
	let object = files_bucket::object(backend.as_ref(), &url.key)
		.await
		.map_err(bucket_failure)?
		.ok_or_else(|| error(StatusCode::NOT_FOUND, "not found"))?;
	let etag = quoted(&object.etag);
	if let Some(wanted) = if_match {
		if !wanted
			.split(',')
			.map(str::trim)
			.any(|tag| tag == "*" || tag == etag)
		{
			return Err(stale(&etag));
		}
	}
	let size = object.size;
	if start >= size {
		return Err(unsatisfiable(size));
	}
	let last = end
		.unwrap_or(u64::MAX)
		.min(size - 1)
		.min(start + MAX_READ_BYTES - 1);
	let bytes = match backend
		.read_range_if_match(Path::new(&url.key), start..last + 1, &object.etag)
		.await
	{
		Ok(bytes) => bytes,
		Err(VolumeError::PreconditionFailed(_)) => {
			let current = backend
				.entity_tag(Path::new(&url.key))
				.await
				.ok()
				.flatten()
				.map(|tag| quoted(&tag))
				.unwrap_or_default();
			return Err(stale(&current));
		}
		Err(e) => return Err(bucket_failure(e)),
	};
	Ok(Block {
		bytes: bytes.to_vec(),
		start,
		size,
		etag,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn path(raw: &str) -> Result<String, StatusCode> {
		decode_path(raw).map_err(|response| response.status())
	}

	#[test]
	fn etags_keep_times_before_1970_apart() {
		use std::time::{Duration, UNIX_EPOCH};

		let at = |modified| Stat {
			kind: Kind::File,
			len: 4,
			modified,
			accessed: None,
			created: None,
			readonly: false,
			identity: 7,
		};
		let before = |secs| Some(UNIX_EPOCH - Duration::from_millis(secs));
		assert_ne!(etag_of(&at(before(1500))), etag_of(&at(before(2500))));
		assert_eq!(etag_of(&at(before(1500))), "\"f-4--1500000000-7\"");
		assert_eq!(
			etag_of(&at(Some(UNIX_EPOCH + Duration::from_secs(2)))),
			"\"f-4-2000000000-7\""
		);
	}

	#[test]
	fn routes_decode_each_segment_on_its_own() {
		assert_eq!(Route::parse("/v1/spaces").unwrap(), Route::Spaces);
		assert_eq!(
			Route::parse("/v1/spaces/abc/stat/").unwrap(),
			Route::Stat {
				space: "abc".into(),
				path: String::new()
			}
		);
		assert_eq!(
			Route::parse("/v1/spaces/abc/blocks/a%20b/c.txt").unwrap(),
			Route::Blocks {
				space: "abc".into(),
				path: "a b/c.txt".into()
			}
		);
		assert_eq!(
			Route::parse("/v1/spaces/abc/tools/read").unwrap(),
			Route::Tool {
				space: "abc".into(),
				name: "read".into()
			}
		);
		assert!(Route::parse("/v1/spaces/abc/nope").is_err());
		assert_eq!(path("docs/"), Ok("docs".into()));
		for bad in [
			"a%2Fb", "..", "a/../b", "%2E%2E", ".", "a//b", "/etc", "a%00b",
		] {
			assert_eq!(path(bad), Err(StatusCode::BAD_REQUEST), "{bad}");
		}
	}

	#[test]
	fn ranges_are_one_closed_or_open_byte_range() {
		let range = |value: &str| {
			let mut headers = HeaderMap::new();
			headers.insert(header::RANGE, HeaderValue::from_str(value).unwrap());
			parse_range(&headers).map_err(|response| response.status())
		};
		assert_eq!(range("bytes=0-99"), Ok((0, Some(99))));
		assert_eq!(range("bytes=5-"), Ok((5, None)));
		for bad in [
			"bytes=-5",
			"bytes=0-1,4-5",
			"items=0-1",
			"bytes=9-1",
			"bytes=a-b",
		] {
			assert_eq!(range(bad), Err(StatusCode::BAD_REQUEST), "{bad}");
		}
		assert_eq!(
			parse_range(&HeaderMap::new()).map_err(|r| r.status()),
			Err(StatusCode::BAD_REQUEST)
		);
	}

	#[test]
	fn tool_failures_map_to_the_error_table() {
		assert_eq!(
			status_for("no such file or directory"),
			StatusCode::NOT_FOUND
		);
		assert_eq!(
			status_for("stale: a.txt changed since it was read; it is now x"),
			StatusCode::PRECONDITION_FAILED
		);
		assert_eq!(
			status_for("invalid arguments: missing field"),
			StatusCode::BAD_REQUEST
		);
		assert_eq!(
			status_for("a.txt exists; name the revision or content hash you read to change it"),
			StatusCode::CONFLICT
		);
	}
}
