//! # Space endpoint mount transport
//!
//! The transport a thin mount uses against a node's space endpoint
//! (`docs/core/design/space-endpoint.md`): `stat` and `list` for metadata,
//! ranged block reads pinned to the file's etag, and the server-sent event
//! stream for invalidation. Every request carries the grant token and goes
//! through the node's `authorize`, audit and limits.
//!
//! The event stream reconnects with `Last-Event-ID` whenever it drops, so a
//! short outage replays what changed. A `reset` drops every cached answer,
//! and `end`, or a refused token, ends the mount.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo() -> Result<(), sd_core::service::mounts::vfs::MountError> {
//! use sd_core::service::mounts::client::{endpoint::EndpointTransport, Transport};
//!
//! let Ok(node) = EndpointTransport::connect("http://127.0.0.1:7765", "sdg_...", None).await
//! else {
//! 	return Ok(());
//! };
//! let readme = node.read("docs/readme.md", 0, 4096, None).await?;
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use reqwest::{header, Response, StatusCode};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use super::{Change, RemoteNode, Transport, TOKEN_REFUSED};
use crate::service::mounts::vfs::{MountError, NodeKind};

/// Entries per listing page, the most the endpoint answers.
const PAGE: u32 = 1000;
/// Bytes per read request. The endpoint answers up to 8 MiB; four blocks
/// keep a cold read responsive while the cache reads ahead.
const READ_MAX: u64 = 4 * 1024 * 1024;
/// Retries of a request the endpoint refused for rate.
const BUSY_RETRIES: u32 = 30;
/// Longest wait between retries, whatever `Retry-After` says.
const BUSY_WAIT_MAX: Duration = Duration::from_secs(5);
/// Longest wait before reconnecting a dropped event stream.
const STREAM_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Why [`EndpointTransport::connect`] did not connect.
#[derive(Debug)]
pub enum ConnectError {
	/// The node answers but has no space endpoint, so it predates it.
	NoEndpoint,
	Refused(MountError),
}

/// One granted space on a node's space endpoint.
pub struct EndpointTransport {
	http: reqwest::Client,
	/// `<url>/v1/spaces/<space>`, the prefix of every route.
	base: url::Url,
	token: String,
	changes: broadcast::Sender<Change>,
	stream: JoinHandle<()>,
}

impl Drop for EndpointTransport {
	fn drop(&mut self) {
		self.stream.abort();
	}
}

fn bad_url(url: &str, why: impl std::fmt::Display) -> ConnectError {
	ConnectError::Refused(MountError::Unavailable(format!("{url}: {why}")))
}

impl EndpointTransport {
	/// Find the space the token grants and start following its changes.
	/// With `space` unset the token must reach exactly one space.
	pub async fn connect(
		url: &str,
		token: &str,
		space: Option<&str>,
	) -> Result<Self, ConnectError> {
		let mut root = url::Url::parse(url).map_err(|err| bad_url(url, err))?;
		if !matches!(root.scheme(), "http" | "https") {
			return Err(bad_url(url, "not an http or https URL"));
		}
		if root.query().is_some() {
			return Err(ConnectError::Refused(MountError::Denied(
				"the URL cannot carry a query; pass the token with --token".to_string(),
			)));
		}
		let http = reqwest::Client::builder()
			.timeout(Duration::from_secs(60))
			.build()
			.map_err(|err| bad_url(url, err))?;
		root.path_segments_mut()
			.map_err(|_| bad_url(url, "not a base URL"))?
			.pop_if_empty()
			.extend(["v1", "spaces"]);

		let response = send(http.get(root.clone()).bearer_auth(token))
			.await
			.map_err(ConnectError::Refused)?;
		if response.status() == StatusCode::NOT_FOUND {
			return Err(ConnectError::NoEndpoint);
		}
		let spaces: Value = ok_json(response).await.map_err(ConnectError::Refused)?;
		let ids: Vec<&str> = spaces
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(|space| space["id"].as_str())
			.collect();
		let id = match (space, ids.as_slice()) {
			(Some(wanted), _) if ids.contains(&wanted) => wanted.to_string(),
			(Some(wanted), _) => {
				return Err(ConnectError::Refused(MountError::Denied(format!(
					"the token does not reach space {wanted}"
				))))
			}
			(None, [only]) => only.to_string(),
			(None, []) => {
				return Err(ConnectError::Refused(MountError::Denied(
					"the token reaches no space".to_string(),
				)))
			}
			(None, _) => {
				return Err(ConnectError::Refused(MountError::Denied(format!(
					"the token reaches several spaces; pick one with --space: {}",
					ids.join(", ")
				))))
			}
		};
		let mut base = root;
		base.path_segments_mut()
			.map_err(|_| bad_url(url, "not a base URL"))?
			.push(&id);

		info!(space = %id, "mounting over the space endpoint");
		let (changes, _) = broadcast::channel(1024);
		let stream = tokio::spawn(follow(
			http.clone(),
			route(&base, "events", ""),
			token.to_string(),
			changes.clone(),
		));
		Ok(Self {
			http,
			base,
			token: token.to_string(),
			changes,
			stream,
		})
	}

	async fn get(
		&self,
		kind: &str,
		path: &str,
		query: &[(&str, String)],
	) -> Result<Value, MountError> {
		let mut url = route(&self.base, kind, path);
		url.query_pairs_mut().extend_pairs(query);
		ok_json(send(self.http.get(url).bearer_auth(&self.token)).await?).await
	}
}

/// `<base>/<kind>/<path>`, each path segment percent-encoded on its own. The
/// root is the empty path, which leaves a trailing slash.
fn route(base: &url::Url, kind: &str, path: &str) -> url::Url {
	let mut url = base.clone();
	if let Ok(mut segments) = url.path_segments_mut() {
		segments.push(kind);
		if path.is_empty() {
			if kind != "events" {
				segments.push("");
			}
		} else {
			segments.extend(path.split('/'));
		}
	}
	url
}

/// Send, retrying while the endpoint says the grant is over its rate.
async fn send(request: reqwest::RequestBuilder) -> Result<Response, MountError> {
	let mut attempt = 0;
	loop {
		let request = request
			.try_clone()
			.ok_or_else(|| MountError::Unavailable("request cannot be retried".to_string()))?;
		let response = request
			.send()
			.await
			.map_err(|err| MountError::Unavailable(format!("cannot reach the node: {err}")))?;
		if response.status() != StatusCode::TOO_MANY_REQUESTS || attempt >= BUSY_RETRIES {
			return Ok(response);
		}
		attempt += 1;
		let wait = retry_after(&response).min(BUSY_WAIT_MAX);
		debug!(?wait, attempt, "endpoint busy; retrying");
		tokio::time::sleep(wait).await;
	}
}

fn retry_after(response: &Response) -> Duration {
	response
		.headers()
		.get(header::RETRY_AFTER)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.parse::<u64>().ok())
		.map_or(Duration::from_millis(200), Duration::from_secs)
}

/// The body of a 2xx answer, or the error its status and message mean.
async fn ok_json(response: Response) -> Result<Value, MountError> {
	let status = response.status();
	let body = json_body(response).await;
	if status.is_success() {
		return Ok(body);
	}
	Err(refusal(status, &body))
}

async fn json_body(response: Response) -> Value {
	let bytes = response.bytes().await.unwrap_or_default();
	serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

/// The kernel-level meaning of an error answer.
fn refusal(status: StatusCode, body: &Value) -> MountError {
	let message = body["error"]["message"].as_str().unwrap_or("refused");
	match status {
		StatusCode::UNAUTHORIZED => MountError::Denied(TOKEN_REFUSED.to_string()),
		StatusCode::NOT_FOUND => MountError::NotFound,
		StatusCode::FORBIDDEN => MountError::Denied(message.to_string()),
		StatusCode::PRECONDITION_FAILED => {
			MountError::Unavailable("the file changed while it was read".to_string())
		}
		StatusCode::BAD_REQUEST => match message {
			"not a directory" => MountError::NotADirectory,
			"is a directory" | "not a regular file" => MountError::IsADirectory,
			_ => MountError::NotFound,
		},
		_ => MountError::Unavailable(format!("{status}: {message}")),
	}
}

fn node(value: &Value) -> RemoteNode {
	RemoteNode {
		kind: match value["kind"].as_str() {
			Some("file") => Some(NodeKind::File),
			Some("directory") => Some(NodeKind::Directory),
			_ => None,
		},
		size: value["size"].as_u64().unwrap_or(0),
		modified: value["modified_at"]
			.as_str()
			.and_then(|text| DateTime::parse_from_rfc3339(text).ok())
			.map(|at| SystemTime::from(at.with_timezone(&Utc))),
		created: None,
		accessed: None,
		etag: value["etag"].as_str().map(str::to_string),
	}
}

#[async_trait]
impl Transport for EndpointTransport {
	async fn stat(&self, path: &str) -> Result<RemoteNode, MountError> {
		Ok(node(&self.get("stat", path, &[]).await?))
	}

	async fn list(&self, path: &str) -> Result<Vec<(String, RemoteNode)>, MountError> {
		let mut entries = Vec::new();
		let mut cursor: Option<String> = None;
		loop {
			let mut query = vec![("limit", PAGE.to_string())];
			if let Some(cursor) = &cursor {
				query.push(("cursor", cursor.clone()));
			}
			let page = self.get("list", path, &query).await?;
			for entry in page["entries"].as_array().into_iter().flatten() {
				let Some(name) = entry["path"].as_str().and_then(|p| p.rsplit('/').next()) else {
					continue;
				};
				entries.push((name.to_string(), node(entry)));
			}
			cursor = page["next_cursor"].as_str().map(str::to_string);
			if cursor.is_none() {
				return Ok(entries);
			}
		}
	}

	async fn read(
		&self,
		path: &str,
		offset: u64,
		len: u64,
		etag: Option<&str>,
	) -> Result<Bytes, MountError> {
		let len = len.clamp(1, READ_MAX);
		let mut request = self
			.http
			.get(route(&self.base, "blocks", path))
			.bearer_auth(&self.token)
			.header(
				header::RANGE,
				format!("bytes={offset}-{}", offset + len - 1),
			);
		if let Some(etag) = etag {
			request = request.header(header::IF_MATCH, etag);
		}
		let response = send(request).await?;
		match response.status() {
			StatusCode::PARTIAL_CONTENT | StatusCode::OK => response
				.bytes()
				.await
				.map_err(|err| MountError::Unavailable(format!("read cut short: {err}"))),
			StatusCode::RANGE_NOT_SATISFIABLE => Ok(Bytes::new()),
			status => Err(refusal(status, &json_body(response).await)),
		}
	}

	fn max_read(&self) -> u64 {
		READ_MAX
	}

	fn changes(&self) -> Option<broadcast::Receiver<Change>> {
		Some(self.changes.subscribe())
	}
}

/// Follow the space's event stream for as long as the grant holds,
/// reconnecting from the last event seen.
async fn follow(
	http: reqwest::Client,
	url: url::Url,
	token: String,
	out: broadcast::Sender<Change>,
) {
	// The stream idles out after a minute by design, so the client timeout
	// that suits other requests does not apply here.
	let http = reqwest::Client::builder().build().unwrap_or(http);
	let mut last_id: Option<String> = None;
	let mut backoff = Duration::from_millis(250);
	loop {
		let mut request = http
			.get(url.clone())
			.bearer_auth(&token)
			.header(header::ACCEPT, "text/event-stream");
		if let Some(id) = &last_id {
			request = request.header("Last-Event-ID", id.as_str());
		}
		match request.send().await {
			Ok(response) if response.status().is_success() => {
				backoff = Duration::from_millis(250);
				if read_stream(response, &mut last_id, &out).await {
					return;
				}
			}
			Ok(response)
				if matches!(
					response.status(),
					StatusCode::UNAUTHORIZED | StatusCode::NOT_FOUND
				) =>
			{
				let _ = out.send(Change::End);
				return;
			}
			Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
				backoff = retry_after(&response).max(backoff);
			}
			Ok(response) => warn!(status = %response.status(), "event stream refused; retrying"),
			Err(err) => debug!(error = %err, "event stream unreachable; retrying"),
		}
		// Changes may have been missed while disconnected and the replay may
		// not cover them, so the next connection's reset or replay decides.
		tokio::time::sleep(backoff).await;
		backoff = (backoff * 2).min(STREAM_BACKOFF_MAX);
	}
}

/// Read one connection's events. Returns true when the grant ended.
async fn read_stream(
	response: Response,
	last_id: &mut Option<String>,
	out: &broadcast::Sender<Change>,
) -> bool {
	let mut body = response.bytes_stream();
	let mut buffer = Vec::<u8>::new();
	let (mut event, mut data, mut id) = (String::new(), String::new(), None::<String>);
	while let Some(chunk) = body.next().await {
		let Ok(chunk) = chunk else {
			return false;
		};
		buffer.extend_from_slice(&chunk);
		// Decode whole lines only: a chunk can end inside a multibyte character.
		while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
			let line: Vec<u8> = buffer.drain(..=end).collect();
			let line = String::from_utf8_lossy(&line[..end]);
			let line = line.trim_end_matches('\r');
			if !line.is_empty() {
				match line.split_once(':') {
					Some(("event", value)) => event = value.trim_start().to_string(),
					Some(("data", value)) => data.push_str(value.trim_start()),
					Some(("id", value)) => id = Some(value.trim_start().to_string()),
					_ => {}
				}
				continue;
			}
			if let Some(id) = id.take() {
				*last_id = Some(id);
			}
			let changes = match event.as_str() {
				"end" => {
					let _ = out.send(Change::End);
					return true;
				}
				"reset" => vec![Change::Reset],
				"change" => serde_json::from_str::<Value>(&data)
					.map(|data| changes_of(&data))
					.unwrap_or_default(),
				_ => Vec::new(),
			};
			for change in changes {
				let _ = out.send(change);
			}
			event.clear();
			data.clear();
		}
	}
	false
}

/// The cache entries one change event makes stale, per the spec's table.
fn changes_of(data: &Value) -> Vec<Change> {
	let Some(path) = data["path"].as_str().map(PathBuf::from) else {
		return Vec::new();
	};
	match data["op"].as_str() {
		Some("move") => {
			let mut changes = vec![Change::Subtree(path)];
			if let Some(to) = data["to_path"].as_str() {
				changes.push(Change::Subtree(PathBuf::from(to)));
			}
			changes
		}
		Some("delete") => vec![Change::Subtree(path)],
		_ => vec![Change::Path(path)],
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn routes_encode_each_segment() {
		let base = url::Url::parse("http://127.0.0.1:7765/v1/spaces/abc").unwrap();
		assert_eq!(
			route(&base, "stat", "").as_str(),
			"http://127.0.0.1:7765/v1/spaces/abc/stat/"
		);
		assert_eq!(
			route(&base, "blocks", "a b/c#d.txt").as_str(),
			"http://127.0.0.1:7765/v1/spaces/abc/blocks/a%20b/c%23d.txt"
		);
		assert_eq!(
			route(&base, "events", "").as_str(),
			"http://127.0.0.1:7765/v1/spaces/abc/events"
		);
	}

	#[test]
	fn moves_and_deletes_drop_whole_subtrees() {
		let changes = changes_of(&json!({ "op": "move", "path": "a", "to_path": "b/a" }));
		assert!(
			matches!(&changes[..], [Change::Subtree(a), Change::Subtree(b)]
			if a == &PathBuf::from("a") && b == &PathBuf::from("b/a"))
		);
		assert!(matches!(
			&changes_of(&json!({ "op": "modify", "path": "x.txt" }))[..],
			[Change::Path(_)]
		));
	}

	#[test]
	fn error_answers_map_to_kernel_errors() {
		let body = |message: &str| json!({ "error": { "message": message } });
		assert!(matches!(
			refusal(StatusCode::UNAUTHORIZED, &Value::Null),
			MountError::Denied(reason) if reason == TOKEN_REFUSED
		));
		assert!(matches!(
			refusal(StatusCode::NOT_FOUND, &Value::Null),
			MountError::NotFound
		));
		assert!(matches!(
			refusal(StatusCode::BAD_REQUEST, &body("not a directory")),
			MountError::NotADirectory
		));
		assert!(matches!(
			refusal(StatusCode::PRECONDITION_FAILED, &Value::Null),
			MountError::Unavailable(_)
		));
	}
}
