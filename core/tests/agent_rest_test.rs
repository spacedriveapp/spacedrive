//! Acceptance for the space endpoint's REST surface, block reads and event
//! stream (`docs/core/design/space-endpoint.md`).
//!
//! Starts the endpoint on an ephemeral loopback port over one `Core` and
//! talks to it as a harness and a mount client do: plain HTTP, a bearer
//! token, the tool registry under `/v1/spaces/<space>/tools`, ranged block
//! reads with etags, and an SSE stream of changes.

#![cfg(unix)]

mod helpers;

use std::{path::Path, time::Duration};

use futures::StreamExt;
use helpers::*;
use reqwest::{header, StatusCode};
use sd_core::{
	infra::action::LibraryAction,
	ops::grants::{
		create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
		revoke::{RevokeGrantAction, RevokeGrantInput},
		GrantCap,
	},
	service::agent_mcp::{self, Limits},
};
use serde_json::{json, Value};

const MIB: usize = 1024 * 1024;

async fn grant(
	harness: &IndexingHarness,
	path: &Path,
	caps: GrantCap,
) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(caps),
		ttl_seconds: None,
		label: Some("rest acceptance".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

fn limits() -> Limits {
	Limits {
		rate_per_second: 1000.0,
		rate_burst: 1000.0,
		..Limits::default()
	}
}

struct Rest {
	http: reqwest::Client,
	base: String,
	token: String,
	space: String,
}

struct Reply {
	status: StatusCode,
	headers: header::HeaderMap,
	bytes: Vec<u8>,
}

impl Reply {
	fn json(&self) -> Value {
		serde_json::from_slice(&self.bytes).unwrap_or(Value::Null)
	}

	fn header(&self, name: &str) -> String {
		self.headers
			.get(name)
			.map(|value| value.to_str().unwrap().to_string())
			.unwrap_or_default()
	}

	fn code(&self) -> String {
		self.json()["error"]["code"]
			.as_str()
			.unwrap_or_default()
			.to_string()
	}
}

impl Rest {
	fn new(base: &str, token: &str, space: &str) -> Self {
		Self {
			http: reqwest::Client::new(),
			base: base.to_string(),
			token: token.to_string(),
			space: space.to_string(),
		}
	}

	fn url(&self, rest: &str) -> String {
		format!("{}/v1/spaces/{}/{rest}", self.base, self.space)
	}

	async fn send(&self, request: reqwest::RequestBuilder) -> Reply {
		let response = request
			.bearer_auth(&self.token)
			.send()
			.await
			.expect("request");
		Reply {
			status: response.status(),
			headers: response.headers().clone(),
			bytes: response.bytes().await.expect("body").to_vec(),
		}
	}

	async fn get(&self, rest: &str) -> Reply {
		self.send(self.http.get(self.url(rest))).await
	}

	async fn blocks(&self, path: &str, range: Option<&str>, if_match: Option<&str>) -> Reply {
		let mut request = self.http.get(self.url(&format!("blocks/{path}")));
		if let Some(range) = range {
			request = request.header(header::RANGE, range);
		}
		if let Some(tag) = if_match {
			request = request.header(header::IF_MATCH, tag);
		}
		self.send(request).await
	}

	async fn tool(&self, name: &str, arguments: Value) -> Reply {
		self.send(
			self.http
				.post(self.url(&format!("tools/{name}")))
				.header(header::CONTENT_TYPE, "application/json")
				.body(arguments.to_string()),
		)
		.await
	}

	async fn ok(&self, name: &str, arguments: Value) -> Value {
		let reply = self.tool(name, arguments.clone()).await;
		assert_eq!(
			reply.status,
			StatusCode::OK,
			"{name} {arguments}: {}",
			reply.json()
		);
		reply.json()
	}
}

/// The MCP `tools/list` result for `token`, for comparing the surfaces.
async fn mcp_tools(base: &str, token: &str) -> Value {
	let http = reqwest::Client::new();
	let post = |body: Value, session: Option<String>| {
		let mut request = http
			.post(format!("{base}/mcp"))
			.bearer_auth(token)
			.header(header::ACCEPT, "application/json, text/event-stream")
			.header(header::CONTENT_TYPE, "application/json")
			.body(body.to_string());
		if let Some(session) = session {
			request = request.header("mcp-session-id", session);
		}
		request.send()
	};
	let init = post(
		json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
			"protocolVersion": "2025-06-18", "capabilities": {},
			"clientInfo": { "name": "rest", "version": "0" },
		}}),
		None,
	)
	.await
	.unwrap();
	let session = init.headers()["mcp-session-id"]
		.to_str()
		.unwrap()
		.to_string();
	let bytes = post(
		json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
		Some(session),
	)
	.await
	.unwrap()
	.bytes()
	.await
	.unwrap();
	let reply: Value = serde_json::from_slice(&bytes).unwrap();
	reply["result"].clone()
}

#[tokio::test]
async fn rest_serves_tools_and_block_reads_from_one_registry() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_rest")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/docs/readme.md", "# Readme\nline two\n")
		.await?;
	tracked.write_file("project/small.txt", "hello").await?;
	tracked.write_file("project/empty.txt", "").await?;
	tracked.write_file("private/secret.txt", "secret").await?;
	tracked.track().await?;
	let outside = harness.create_test_dir("outside").await?;
	outside.write_file("secret.txt", "outside secret").await?;

	let base = std::fs::canonicalize(harness.temp_path())?;
	let project = base.join("tracked/project");
	// 2.5 MiB of a repeating pattern, so every offset's byte is known.
	let pattern: Vec<u8> = (0..(5 * MIB / 2)).map(|i| (i % 251) as u8).collect();
	std::fs::write(project.join("blocks.bin"), &pattern)?;
	let big: Vec<u8> = (0..(10 * MIB)).map(|i| (i % 253) as u8).collect();
	std::fs::write(project.join("big.bin"), &big)?;
	std::fs::create_dir_all(project.join("page"))?;
	for i in 0..5 {
		std::fs::write(project.join(format!("page/f{i}.txt")), "x")?;
	}
	std::os::unix::fs::symlink(base.join("outside/secret.txt"), project.join("escape.txt"))?;

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, limits())?;
	let url = format!("http://{bound}");
	let issued = grant(&harness, &project, GrantCap::Read).await?;
	let space = issued.grant.id.to_string();
	let rest = Rest::new(&url, &issued.token, &space);

	// The space list names the one grant the token reaches.
	let reply = rest.send(rest.http.get(format!("{url}/v1/spaces"))).await;
	assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
	let spaces = reply.json();
	assert_eq!(spaces.as_array().unwrap().len(), 1);
	assert_eq!(spaces[0]["id"], json!(space));
	assert_eq!(spaces[0]["name"], json!("rest acceptance"));
	assert_eq!(spaces[0]["caps"], json!("read"));
	assert!(spaces[0]["expires_at"].is_string());

	// One registry: the REST tool table is MCP's tools/list, byte for byte.
	let reply = rest.get("tools").await;
	assert_eq!(reply.status, StatusCode::OK);
	assert_eq!(reply.json(), mcp_tools(&url, &issued.token).await);

	// Tool calls answer with the structured content.
	let read = rest.ok("read", json!({ "path": "small.txt" })).await;
	assert_eq!(read["bytes_read"], json!(5));
	assert_eq!(read["path"], json!("small.txt"));
	let listing = rest.ok("list_dir", json!({ "path": "docs" })).await;
	assert_eq!(listing["entries"][0]["path"], json!("docs/readme.md"));
	let stat = rest.ok("stat", json!({ "path": "small.txt" })).await;
	assert_eq!(stat["size"], json!(5));
	assert!(stat["content_hash"].is_string(), "{stat}");
	let reply = rest.tool("get_skill", json!({})).await;
	assert_eq!(reply.status, StatusCode::OK);
	assert!(reply.header("content-type").starts_with("text/markdown"));
	assert_eq!(String::from_utf8(reply.bytes).unwrap(), agent_mcp::SKILL);
	let reply = rest.tool("read", json!({ "path": "missing.txt" })).await;
	assert_eq!(reply.status, StatusCode::NOT_FOUND, "{}", reply.json());
	assert_eq!(reply.code(), "not_found");
	let reply = rest
		.tool("read", json!({ "path": "small.txt", "offset": "x" }))
		.await;
	assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.json());
	let reply = rest.tool("nope", json!({})).await;
	assert_eq!(reply.status, StatusCode::NOT_FOUND);
	let reply = rest
		.tool("write_file", json!({ "path": "new.txt", "content": "x" }))
		.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN, "read-only grant");
	assert_eq!(reply.code(), "forbidden");

	// Stat: files carry an etag and the block size; the root stats too.
	let reply = rest.get("stat/blocks.bin").await;
	assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
	let stat = reply.json();
	assert_eq!(stat["kind"], json!("file"));
	assert_eq!(stat["size"], json!(pattern.len()));
	assert_eq!(stat["block_size"], json!(MIB));
	let etag = stat["etag"].as_str().unwrap().to_string();
	assert!(etag.starts_with("\"f-") && etag.ends_with('"'), "{etag}");
	let root = rest.get("stat/").await.json();
	assert_eq!(root["kind"], json!("directory"));
	assert_eq!(root["path"], json!(""));
	assert!(root.get("etag").is_none());
	assert_eq!(rest.get("stat/missing").await.status, StatusCode::NOT_FOUND);

	// List: sorted by name, links reported and not followed, paged by name.
	let listing = rest.get("list/").await.json();
	let names: Vec<&str> = listing["entries"]
		.as_array()
		.unwrap()
		.iter()
		.map(|e| e["path"].as_str().unwrap())
		.collect();
	assert_eq!(
		names,
		[
			"big.bin",
			"blocks.bin",
			"docs",
			"empty.txt",
			"escape.txt",
			"page",
			"small.txt"
		]
	);
	let link = &listing["entries"][4];
	assert_eq!(link["kind"], json!("symlink"));
	assert!(link.get("etag").is_none());
	assert!(listing.get("next_cursor").is_none());
	let mut seen = Vec::new();
	let mut cursor: Option<String> = None;
	loop {
		let query = match &cursor {
			Some(cursor) => format!("list/page?limit=2&cursor={cursor}"),
			None => "list/page?limit=2".to_string(),
		};
		let page = rest.get(&query).await.json();
		for entry in page["entries"].as_array().unwrap() {
			seen.push(entry["path"].as_str().unwrap().to_string());
		}
		match page["next_cursor"].as_str() {
			Some(next) => cursor = Some(next.to_string()),
			None => break,
		}
	}
	assert_eq!(
		seen,
		(0..5).map(|i| format!("page/f{i}.txt")).collect::<Vec<_>>()
	);
	assert_eq!(
		rest.get("list/small.txt").await.status,
		StatusCode::BAD_REQUEST
	);

	// Block reads: aligned, unaligned, the short last block, open ranges.
	let reply = rest
		.blocks("blocks.bin", Some("bytes=1048576-2097151"), None)
		.await;
	assert_eq!(reply.status, StatusCode::PARTIAL_CONTENT);
	assert_eq!(reply.bytes, pattern[MIB..2 * MIB]);
	assert_eq!(
		reply.header("content-range"),
		format!("bytes 1048576-2097151/{}", pattern.len())
	);
	assert_eq!(reply.header("content-length"), MIB.to_string());
	assert_eq!(reply.header("content-type"), "application/octet-stream");
	assert_eq!(reply.header("etag"), etag);
	let reply = rest
		.blocks("blocks.bin", Some("bytes=1000-1999"), None)
		.await;
	assert_eq!(reply.bytes, pattern[1000..2000]);
	let reply = rest
		.blocks("blocks.bin", Some("bytes=2097152-3145727"), None)
		.await;
	assert_eq!(reply.status, StatusCode::PARTIAL_CONTENT);
	assert_eq!(reply.bytes, pattern[2 * MIB..]);
	assert_eq!(
		reply.header("content-range"),
		format!("bytes 2097152-{}/{}", pattern.len() - 1, pattern.len())
	);
	let reply = rest
		.blocks("blocks.bin", Some("bytes=2500000-"), None)
		.await;
	assert_eq!(reply.bytes, pattern[2_500_000..]);

	// One answer carries at most 8 MiB.
	let reply = rest.blocks("big.bin", Some("bytes=0-"), None).await;
	assert_eq!(reply.status, StatusCode::PARTIAL_CONTENT);
	assert_eq!(reply.bytes.len(), 8 * MIB);
	assert_eq!(reply.bytes, big[..8 * MIB]);
	assert_eq!(
		reply.header("content-range"),
		format!("bytes 0-{}/{}", 8 * MIB - 1, big.len())
	);

	// Past the end, and on an empty file, the answer is 416.
	let reply = rest
		.blocks("blocks.bin", Some("bytes=3145728-"), None)
		.await;
	assert_eq!(reply.status, StatusCode::RANGE_NOT_SATISFIABLE);
	assert_eq!(
		reply.header("content-range"),
		format!("bytes */{}", pattern.len())
	);
	assert_eq!(reply.code(), "range");
	let reply = rest.blocks("empty.txt", Some("bytes=0-"), None).await;
	assert_eq!(reply.status, StatusCode::RANGE_NOT_SATISFIABLE);

	// A missing or multiple range is refused.
	let reply = rest.blocks("blocks.bin", None, None).await;
	assert_eq!(reply.status, StatusCode::BAD_REQUEST);
	assert_eq!(reply.code(), "invalid");
	let reply = rest.blocks("blocks.bin", Some("bytes=0-1,5-6"), None).await;
	assert_eq!(reply.status, StatusCode::BAD_REQUEST);

	// If-Match passes while the file is unchanged and fails once it changes.
	let reply = rest
		.blocks("blocks.bin", Some("bytes=0-9"), Some(&etag))
		.await;
	assert_eq!(reply.status, StatusCode::PARTIAL_CONTENT);
	std::fs::write(project.join("blocks.bin"), b"rewritten")?;
	let reply = rest
		.blocks("blocks.bin", Some("bytes=0-9"), Some(&etag))
		.await;
	assert_eq!(reply.status, StatusCode::PRECONDITION_FAILED);
	assert_eq!(reply.code(), "stale");
	let current = reply.json()["error"]["etag"].as_str().unwrap().to_string();
	assert_ne!(current, etag);
	assert_eq!(
		rest.get("stat/blocks.bin").await.json()["etag"],
		json!(current)
	);
	assert!(reply.bytes.len() < 512, "no file bytes in a 412");

	// A link out of the root, and a dot segment, are refused.
	let reply = rest.blocks("escape.txt", Some("bytes=0-"), None).await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.json());
	assert_eq!(reply.code(), "forbidden");
	assert!(!String::from_utf8_lossy(&reply.bytes).contains(base.to_str().unwrap()));
	for path in ["docs%2F..%2Fsmall.txt", "..%2Fprivate%2Fsecret.txt", "a//b"] {
		let reply = rest.blocks(path, Some("bytes=0-"), None).await;
		assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{path}");
	}

	// A space the token does not grant is the same 404 as no space at all.
	let private = grant(&harness, &base.join("tracked/private"), GrantCap::Read).await?;
	for other in [private.grant.id.to_string(), "not-a-space".to_string()] {
		let wrong = Rest::new(&url, &issued.token, &other);
		let reply = wrong.get("stat/small.txt").await;
		assert_eq!(reply.status, StatusCode::NOT_FOUND, "{other}");
		assert_eq!(reply.code(), "not_found");
	}

	// The token travels in the header only, from no browser, and every bad
	// token gets the same 401.
	let reply = rest
		.get(&format!("stat/small.txt?token={}", issued.token))
		.await;
	assert_eq!(reply.status, StatusCode::BAD_REQUEST);
	let reply = rest
		.send(
			rest.http
				.get(rest.url("stat/small.txt"))
				.header(header::ORIGIN, "https://example.com"),
		)
		.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN);
	let reply = rest
		.send(
			rest.http
				.request(reqwest::Method::OPTIONS, rest.url("tools")),
		)
		.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN);
	let stranger = Rest::new(&url, &format!("sdg_{}", "0".repeat(64)), &space);
	let unknown = stranger.get("stat/small.txt").await;
	assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);
	assert_eq!(
		unknown.header("www-authenticate"),
		"Bearer realm=\"spacedrive\""
	);
	RevokeGrantAction::from_input(RevokeGrantInput {
		id: private.grant.id,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let revoked = Rest::new(&url, &private.token, &private.grant.id.to_string());
	let reply = revoked.get("stat/secret.txt").await;
	assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
	assert_eq!(reply.bytes, unknown.bytes, "one body for every 401");

	harness.shutdown().await?;
	Ok(())
}

/// One SSE event: its id, name and data.
#[derive(Debug)]
struct Event {
	id: Option<String>,
	name: String,
	data: Value,
}

struct Stream {
	body: futures::stream::BoxStream<'static, reqwest::Result<bytes::Bytes>>,
	buffer: String,
}

impl Stream {
	async fn open(rest: &Rest, last_event_id: Option<&str>) -> (StatusCode, Option<Self>) {
		let mut request = rest
			.http
			.get(rest.url("events"))
			.bearer_auth(&rest.token)
			.header(header::ACCEPT, "text/event-stream");
		if let Some(id) = last_event_id {
			request = request.header("last-event-id", id);
		}
		let response = request.send().await.expect("events");
		let status = response.status();
		if status != StatusCode::OK {
			return (status, None);
		}
		(
			status,
			Some(Self {
				body: response.bytes_stream().boxed(),
				buffer: String::new(),
			}),
		)
	}

	/// The next event, skipping comments, or `None` when the stream closed.
	async fn next(&mut self) -> Option<Event> {
		loop {
			while let Some(end) = self.buffer.find("\n\n") {
				let block: String = self.buffer.drain(..end + 2).collect();
				let mut event = Event {
					id: None,
					name: "message".to_string(),
					data: Value::Null,
				};
				let mut any = false;
				for line in block.lines() {
					if let Some(id) = line.strip_prefix("id: ") {
						event.id = Some(id.to_string());
						any = true;
					} else if let Some(name) = line.strip_prefix("event: ") {
						event.name = name.to_string();
						any = true;
					} else if let Some(data) = line.strip_prefix("data: ") {
						event.data = serde_json::from_str(data).unwrap();
					}
				}
				if any {
					return Some(event);
				}
			}
			let chunk = tokio::time::timeout(Duration::from_secs(20), self.body.next())
				.await
				.expect("an event within 20 s")?
				.ok()?;
			self.buffer.push_str(&String::from_utf8_lossy(&chunk));
		}
	}
}

#[tokio::test]
async fn rest_event_stream_carries_writes_resets_and_ends() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_rest_events")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("project/a.txt", "a").await?;
	tracked.track().await?;
	let base = std::fs::canonicalize(harness.temp_path())?;
	let project = base.join("tracked/project");

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, limits())?;
	let url = format!("http://{bound}");
	let issued = grant(&harness, &project, GrantCap::ReadWrite).await?;
	let rest = Rest::new(&url, &issued.token, &issued.grant.id.to_string());

	// A write through the tool reaches an open stream as a create.
	let (status, stream) = Stream::open(&rest, None).await;
	assert_eq!(status, StatusCode::OK);
	let mut stream = stream.unwrap();
	rest.ok(
		"write_file",
		json!({ "path": "notes/new.txt", "content": "hi" }),
	)
	.await;
	let event = stream.next().await.expect("a change");
	assert_eq!(event.name, "change");
	assert_eq!(event.data["op"], json!("create"));
	assert_eq!(event.data["path"], json!("notes/new.txt"));
	let first = event.id.clone().unwrap();
	let (log, seq) = first.rsplit_once('-').unwrap();
	assert_eq!(seq, event.data["seq"].to_string());

	// A second write, then a reconnect from the first id replays only it.
	let stat = rest.ok("stat", json!({ "path": "a.txt" })).await;
	rest.ok(
		"write_file",
		json!({ "path": "a.txt", "content": "b", "expect": { "content_hash": stat["content_hash"] } }),
	)
	.await;
	let event = stream.next().await.expect("a second change");
	assert_eq!(event.data["op"], json!("modify"));
	assert_eq!(event.data["path"], json!("a.txt"));
	drop(stream);
	let (_, replay) = Stream::open(&rest, Some(&first)).await;
	let mut replay = replay.unwrap();
	let event = replay.next().await.unwrap();
	assert_eq!(event.name, "change");
	assert_eq!(event.data["path"], json!("a.txt"));
	drop(replay);

	// An id from another log, or one the server never issued, resets.
	for id in [
		"0-1".to_string(),
		format!("{log}-999999"),
		"junk".to_string(),
	] {
		let (_, stream) = Stream::open(&rest, Some(&id)).await;
		let mut stream = stream.unwrap();
		let event = stream.next().await.unwrap();
		assert_eq!(event.name, "reset", "{id}");
	}

	// A grant holds at most four streams at once.
	let mut held = Vec::new();
	for _ in 0..4 {
		let (status, stream) = Stream::open(&rest, None).await;
		assert_eq!(status, StatusCode::OK);
		held.push(stream.unwrap());
	}
	let (status, _) = Stream::open(&rest, None).await;
	assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
	held.truncate(1);

	// Revoking the grant ends the stream.
	let mut stream = held.pop().unwrap();
	RevokeGrantAction::from_input(RevokeGrantInput {
		id: issued.grant.id,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let event = stream.next().await.expect("end");
	assert_eq!(event.name, "end");
	assert!(stream.next().await.is_none(), "the stream closes after end");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn rest_event_stream_carries_watcher_changes() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_rest_watcher")
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("project/a.txt", "a").await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, limits())?;
	let issued = grant(&harness, &project, GrantCap::Read).await?;
	let rest = Rest::new(
		&format!("http://{bound}"),
		&issued.token,
		&issued.grant.id.to_string(),
	);
	let (status, stream) = Stream::open(&rest, None).await;
	assert_eq!(status, StatusCode::OK);
	let mut stream = stream.unwrap();

	// A change made outside Spacedrive reaches the stream through the watcher.
	std::fs::write(project.join("fresh.txt"), "fresh")?;
	loop {
		let event = stream.next().await.expect("a watcher change");
		if event.name == "change" && event.data["path"] == json!("fresh.txt") {
			break;
		}
	}

	harness.shutdown().await?;
	Ok(())
}
