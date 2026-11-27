//! The agent MCP endpoint as a mount transport.
//!
//! Every Spacedrive node that lends folders to agents already answers
//! `list_dir`, `stat` and ranged `read` over MCP's streamable HTTP
//! transport, authenticated by the grant token. This transport speaks just
//! that subset, so a thin client can mount a granted folder from any node
//! whose MCP endpoint it can reach, through the same grant checks, audit and
//! limits an agent gets. The endpoint's per-grant rate and concurrency limits
//! apply: a refused request is retried after the `Retry-After` it names.

use super::{RemoteNode, Transport};
use crate::service::mounts::vfs::{MountError, NodeKind};
use async_trait::async_trait;
use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use reqwest::{header, StatusCode};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use tracing::{debug, warn};

const PROTOCOL_VERSION: &str = "2025-06-18";
const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";
/// The most one `read` call returns.
const READ_MAX: u64 = 1024 * 1024;
/// Entries per `list_dir` page.
const PAGE: u32 = 1000;
/// Retries of a request the endpoint refused for rate or concurrency.
const BUSY_RETRIES: u32 = 30;
/// Longest wait between retries, whatever `Retry-After` says.
const BUSY_WAIT_MAX: Duration = Duration::from_secs(5);

/// A session with one node's agent MCP endpoint.
pub struct McpTransport {
	http: reqwest::Client,
	endpoint: String,
	token: String,
	session: Mutex<Option<String>>,
	next_id: AtomicU64,
}

/// `http://host:port` names the endpoint's default path; a URL with a path
/// is used as given.
fn endpoint_url(url: &str) -> Result<String, MountError> {
	let parsed = url::Url::parse(url)
		.map_err(|err| MountError::Unavailable(format!("{url} is not a URL: {err}")))?;
	if !matches!(parsed.scheme(), "http" | "https") {
		return Err(MountError::Unavailable(format!(
			"{url} is not an http or https URL"
		)));
	}
	if parsed.query().is_some() {
		return Err(MountError::Denied(
			"the URL cannot carry a query; pass the token with --token".to_string(),
		));
	}
	if parsed.path() == "/" {
		return Ok(parsed
			.join("mcp")
			.map_err(|err| MountError::Unavailable(err.to_string()))?
			.to_string());
	}
	Ok(parsed.to_string())
}

impl McpTransport {
	/// Open a session, refusing at once when the token is not accepted.
	pub async fn connect(url: &str, token: &str) -> Result<Self, MountError> {
		let transport = Self {
			http: reqwest::Client::builder()
				.timeout(Duration::from_secs(60))
				.build()
				.map_err(|err| MountError::Unavailable(err.to_string()))?,
			endpoint: endpoint_url(url)?,
			token: token.to_string(),
			session: Mutex::new(None),
			next_id: AtomicU64::new(1),
		};
		transport.session().await?;
		Ok(transport)
	}

	/// The current session, opening one when there is none.
	async fn session(&self) -> Result<String, MountError> {
		let mut session = self.session.lock().await;
		if let Some(id) = session.as_ref() {
			return Ok(id.clone());
		}
		let (headers, _) = self
			.post(
				None,
				json!({
					"jsonrpc": "2.0",
					"id": self.next_id.fetch_add(1, Ordering::Relaxed),
					"method": "initialize",
					"params": {
						"protocolVersion": PROTOCOL_VERSION,
						"capabilities": {},
						"clientInfo": { "name": "sd-mount", "version": env!("CARGO_PKG_VERSION") },
					},
				}),
			)
			.await?;
		let id = headers
			.get(SESSION_HEADER)
			.and_then(|value| value.to_str().ok())
			.ok_or_else(|| MountError::Unavailable("the endpoint opened no session".to_string()))?
			.to_string();
		self.post(
			Some(&id),
			json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
		)
		.await?;
		*session = Some(id.clone());
		Ok(id)
	}

	/// One JSON-RPC message, retried while the endpoint says it is busy.
	async fn post(
		&self,
		session: Option<&str>,
		message: Value,
	) -> Result<(header::HeaderMap, Value), MountError> {
		let mut attempt = 0;
		loop {
			let mut request = self
				.http
				.post(&self.endpoint)
				.bearer_auth(&self.token)
				.header(header::ACCEPT, "application/json, text/event-stream")
				.header(VERSION_HEADER, PROTOCOL_VERSION)
				.header(header::CONTENT_TYPE, "application/json")
				.body(message.to_string());
			if let Some(session) = session {
				request = request.header(SESSION_HEADER, session);
			}
			let response = request
				.send()
				.await
				.map_err(|err| MountError::Unavailable(format!("cannot reach the node: {err}")))?;
			let status = response.status();
			let headers = response.headers().clone();
			if status == StatusCode::TOO_MANY_REQUESTS && attempt < BUSY_RETRIES {
				attempt += 1;
				let wait = headers
					.get(header::RETRY_AFTER)
					.and_then(|value| value.to_str().ok())
					.and_then(|value| value.parse::<u64>().ok())
					.map_or(Duration::from_millis(200), Duration::from_secs)
					.min(BUSY_WAIT_MAX);
				debug!(?wait, attempt, "endpoint busy; retrying");
				tokio::time::sleep(wait).await;
				continue;
			}
			let body = response.bytes().await.unwrap_or_default();
			let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
			let message = value["error"]["message"]
				.as_str()
				.map(str::to_string)
				.unwrap_or_else(|| String::from_utf8_lossy(&body).into_owned());
			return match status {
				StatusCode::OK | StatusCode::ACCEPTED => Ok((headers, value)),
				StatusCode::UNAUTHORIZED => Err(MountError::Denied(
					"invalid, expired or revoked grant token".to_string(),
				)),
				StatusCode::NOT_FOUND => Err(MountError::Unavailable(SESSION_LOST.to_string())),
				StatusCode::FORBIDDEN => Err(refusal(&message)),
				_ => Err(MountError::Unavailable(format!("{status}: {message}"))),
			};
		}
	}

	/// Call one tool and return its result: `structuredContent` and the
	/// `content` blocks.
	async fn call(&self, tool: &str, arguments: Value) -> Result<Value, MountError> {
		let message = |id| {
			json!({
				"jsonrpc": "2.0",
				"id": id,
				"method": "tools/call",
				"params": { "name": tool, "arguments": arguments },
			})
		};
		let session = self.session().await?;
		let id = self.next_id.fetch_add(1, Ordering::Relaxed);
		let value = match self.post(Some(&session), message(id)).await {
			Err(MountError::Unavailable(reason)) if reason == SESSION_LOST => {
				// The node restarted or dropped an idle session; open another.
				warn!("the endpoint forgot this mount's session; opening a new one");
				self.session.lock().await.take();
				let session = self.session().await?;
				let id = self.next_id.fetch_add(1, Ordering::Relaxed);
				self.post(Some(&session), message(id)).await?.1
			}
			other => other?.1,
		};
		if let Some(message) = value["error"]["message"].as_str() {
			return Err(MountError::Unavailable(message.to_string()));
		}
		let result = &value["result"];
		if result["isError"].as_bool() == Some(true) {
			let message = result["content"][0]["text"].as_str().unwrap_or("refused");
			return Err(refusal(message));
		}
		Ok(result.clone())
	}
}

const SESSION_LOST: &str = "unknown session";

/// The kernel-level meaning of a message the endpoint refused with.
fn refusal(message: &str) -> MountError {
	match message {
		"no such file or directory" | "invalid path" => MountError::NotFound,
		"not a directory" | "not a regular file" => MountError::NotADirectory,
		"is a directory" => MountError::IsADirectory,
		"the granted folder is unavailable" => MountError::Unavailable(message.to_string()),
		_ if message.starts_with("I/O error") || message.starts_with("the call ran past") => {
			MountError::Unavailable(message.to_string())
		}
		_ => MountError::Denied(message.to_string()),
	}
}

fn time(value: &Value) -> Option<SystemTime> {
	value
		.as_str()
		.and_then(|text| DateTime::parse_from_rfc3339(text).ok())
		.map(|at| SystemTime::from(at.with_timezone(&Utc)))
}

fn node(value: &Value) -> RemoteNode {
	RemoteNode {
		kind: match value["type"].as_str() {
			Some("file") => Some(NodeKind::File),
			Some("dir") => Some(NodeKind::Directory),
			_ => None,
		},
		size: value["size"].as_u64().unwrap_or(0),
		modified: time(&value["modified"]),
		created: time(&value["created"]),
		accessed: time(&value["accessed"]),
	}
}

#[async_trait]
impl Transport for McpTransport {
	async fn stat(&self, path: &str) -> Result<RemoteNode, MountError> {
		Ok(node(
			&self.call("stat", json!({ "path": path })).await?["structuredContent"],
		))
	}

	async fn list(&self, path: &str) -> Result<Vec<(String, RemoteNode)>, MountError> {
		let mut entries = Vec::new();
		let mut cursor: Option<String> = None;
		loop {
			let page = self
				.call(
					"list_dir",
					json!({ "path": path, "limit": PAGE, "cursor": cursor }),
				)
				.await?;
			let page = &page["structuredContent"];
			for entry in page["entries"].as_array().into_iter().flatten() {
				if let Some(name) = entry["name"].as_str() {
					entries.push((name.to_string(), node(entry)));
				}
			}
			cursor = page["next_cursor"].as_str().map(str::to_string);
			if cursor.is_none() {
				return Ok(entries);
			}
		}
	}

	async fn read(&self, path: &str, offset: u64, len: u64) -> Result<Bytes, MountError> {
		let len = len.clamp(1, READ_MAX);
		let called = self
			.call(
				"read",
				json!({ "path": path, "offset": offset, "length": len, "max_bytes": len }),
			)
			.await?;
		let result = &called["structuredContent"];
		let covered = result["bytes_read"].as_u64().unwrap_or(0) as usize;
		if covered == 0 {
			return Ok(Bytes::new());
		}
		// The bytes travel as the first content block: text when they are
		// UTF-8, base64 otherwise.
		let text = called["content"][0]["text"]
			.as_str()
			.ok_or_else(|| MountError::Unavailable("the read returned no content".to_string()))?;
		let bytes = match result["encoding"].as_str() {
			Some("base64") => base64::engine::general_purpose::STANDARD
				.decode(text)
				.map_err(|err| MountError::Unavailable(format!("bad base64: {err}")))?,
			_ => text.as_bytes().to_vec(),
		};
		if bytes.len() != covered {
			return Err(MountError::Unavailable(format!(
				"the read returned {} bytes but claimed {covered}",
				bytes.len()
			)));
		}
		Ok(Bytes::from(bytes))
	}

	fn max_read(&self) -> u64 {
		READ_MAX
	}

	async fn close(&self) {
		let Some(session) = self.session.lock().await.take() else {
			return;
		};
		let _ = self
			.http
			.delete(&self.endpoint)
			.bearer_auth(&self.token)
			.header(SESSION_HEADER, session)
			.send()
			.await;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_bare_origin_names_the_default_path() {
		assert_eq!(
			endpoint_url("http://127.0.0.1:7765").unwrap(),
			"http://127.0.0.1:7765/mcp"
		);
		assert_eq!(
			endpoint_url("https://node.example:7765/agents/mcp").unwrap(),
			"https://node.example:7765/agents/mcp"
		);
		assert!(endpoint_url("http://127.0.0.1:7765/mcp?token=x").is_err());
		assert!(endpoint_url("ftp://127.0.0.1").is_err());
	}

	#[test]
	fn refusals_map_to_kernel_errors() {
		assert!(matches!(
			refusal("no such file or directory"),
			MountError::NotFound
		));
		assert!(matches!(
			refusal("not a directory"),
			MountError::NotADirectory
		));
		assert!(matches!(
			refusal("the path is outside the granted folder"),
			MountError::Denied(_)
		));
	}
}
