//! Acceptance for the agent MCP endpoint (`core::service::agent_mcp`).
//!
//! Starts the endpoint on an ephemeral loopback port over one `Core` and
//! talks to it the way an MCP client does: initialize, the initialized
//! notification, tools/list, then every tool. It checks the refusals the
//! endpoint owes: 401 for unknown, revoked and expired tokens, 403 for
//! another subtree, escapes, bad Host and bad Origin, the read cap, and the
//! per-grant limits.

#![cfg(unix)]

mod helpers;

use std::{path::Path, time::Duration};

use chrono::Utc;
use helpers::*;
use reqwest::{header, Method, StatusCode};
use sd_core::{
	infra::{action::LibraryAction, db::entities::agent_grant},
	library::AddOverrides,
	ops::{
		grants::{
			create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
			revoke::{RevokeGrantAction, RevokeGrantInput},
			GrantCap,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
	service::agent_mcp::{self, Limits},
};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};

async fn grant(harness: &IndexingHarness, path: &Path) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(GrantCap::Read),
		ttl_seconds: None,
		label: Some("mcp acceptance".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

/// A minimal MCP client over streamable HTTP.
struct Client {
	http: reqwest::Client,
	url: String,
	token: String,
	session: Option<String>,
	next_id: u64,
}

struct Reply {
	status: StatusCode,
	headers: header::HeaderMap,
	body: Value,
}

impl Client {
	fn new(url: &str, token: &str) -> Self {
		Self {
			http: reqwest::Client::new(),
			url: url.to_string(),
			token: token.to_string(),
			session: None,
			next_id: 1,
		}
	}

	async fn send(&mut self, message: Value) -> Reply {
		let mut request = self
			.http
			.post(&self.url)
			.bearer_auth(&self.token)
			.header(header::ACCEPT, "application/json, text/event-stream")
			.header(header::CONTENT_TYPE, "application/json")
			.body(message.to_string());
		if let Some(session) = &self.session {
			request = request.header("mcp-session-id", session);
		}
		let response = request.send().await.expect("request");
		let status = response.status();
		let headers = response.headers().clone();
		let bytes = response.bytes().await.expect("body");
		let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
		Reply {
			status,
			headers,
			body,
		}
	}

	async fn request(&mut self, method: &str, params: Value) -> Reply {
		let id = self.next_id;
		self.next_id += 1;
		self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
			.await
	}

	async fn initialize(&mut self) -> Reply {
		let reply = self
			.request(
				"initialize",
				json!({
					"protocolVersion": "2025-06-18",
					"capabilities": {},
					"clientInfo": { "name": "acceptance", "version": "0" },
				}),
			)
			.await;
		if let Some(session) = reply.headers.get("mcp-session-id") {
			self.session = Some(session.to_str().unwrap().to_string());
		}
		reply
	}

	async fn call(&mut self, tool: &str, arguments: Value) -> Reply {
		self.request(
			"tools/call",
			json!({ "name": tool, "arguments": arguments }),
		)
		.await
	}

	/// A successful tool call's structured content.
	async fn ok(&mut self, tool: &str, arguments: Value) -> Value {
		let reply = self.call(tool, arguments.clone()).await;
		assert_eq!(
			reply.status,
			StatusCode::OK,
			"{tool} {arguments}: {}",
			reply.body
		);
		let result = &reply.body["result"];
		assert_eq!(
			result["isError"],
			json!(false),
			"{tool} {arguments}: {result}"
		);
		result["structuredContent"].clone()
	}
}

fn paths(items: &Value, field: &str) -> Vec<String> {
	items[field]
		.as_array()
		.unwrap()
		.iter()
		.map(|item| item["path"].as_str().unwrap().to_string())
		.collect()
}

#[tokio::test]
async fn agent_mcp_serves_a_grant_and_refuses_everything_else() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_acceptance")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/docs/readme.md", "# Readme\nline two\nline three\n")
		.await?;
	tracked
		.write_file("project/src/main.rs", "fn main() {\n\tlet needle = 1;\n}\n")
		.await?;
	tracked
		.write_file("project/.hidden/notes.txt", "a hidden needle\n")
		.await?;
	tracked
		.write_file("project/big.txt", &"0123456789abcdef".repeat(128 * 1024))
		.await?;
	tracked.write_file("private/secret.txt", "secret").await?;
	tracked.track().await?;
	let outside = harness.create_test_dir("outside").await?;
	outside.write_file("secret.txt", "outside secret").await?;

	let base = std::fs::canonicalize(harness.temp_path())?;
	let project = base.join("tracked/project");
	std::os::unix::fs::symlink(base.join("outside/secret.txt"), project.join("escape.txt"))?;
	std::os::unix::fs::symlink(base.join("outside"), project.join("escape_dir"))?;

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;
	let url = format!("http://{bound}/mcp");

	let project_grant = grant(&harness, &project).await?;
	let private_grant = grant(&harness, &base.join("tracked/private")).await?;

	// An unknown token is refused before anything else.
	let mut stranger = Client::new(&url, &format!("sdg_{}", "0".repeat(64)));
	let reply = stranger.initialize().await;
	assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
	assert!(reply.headers.contains_key(header::WWW_AUTHENTICATE));

	// The handshake.
	let mut client = Client::new(&url, &project_grant.token);
	let reply = client.initialize().await;
	assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
	assert_eq!(reply.body["result"]["protocolVersion"], "2025-06-18");
	assert!(client.session.is_some(), "initialize mints a session");
	let reply = client
		.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
		.await;
	assert_eq!(reply.status, StatusCode::ACCEPTED);
	let reply = client.request("tools/list", json!({})).await;
	let names: Vec<&str> = reply.body["result"]["tools"]
		.as_array()
		.unwrap()
		.iter()
		.map(|tool| tool["name"].as_str().unwrap())
		.collect();
	for tool in [
		"list_dir",
		"stat",
		"read",
		"glob",
		"grep",
		"search",
		"search_content",
		"get_skill",
		"write_file",
		"make_dir",
		"move",
		"delete",
		"list_changes",
	] {
		assert!(names.contains(&tool), "{tool} missing from {names:?}");
	}
	assert_eq!(names.len(), 13, "nothing beyond the curated set");

	// get_skill returns the compiled-in guide.
	let reply = client.call("get_skill", json!({})).await;
	let skill = reply.body["result"]["content"][0]["text"].as_str().unwrap();
	assert_eq!(skill, agent_mcp::SKILL);
	assert!(skill.contains("Results can be incomplete"));

	// list_dir: hidden entries included, links listed and not followed.
	let listing = client.ok("list_dir", json!({ "path": "" })).await;
	let listed = paths(&listing, "entries");
	for name in [
		".hidden",
		"big.txt",
		"docs",
		"escape.txt",
		"escape_dir",
		"src",
	] {
		assert!(
			listed.contains(&name.to_string()),
			"{name} not in {listed:?}"
		);
	}
	let escape = listing["entries"]
		.as_array()
		.unwrap()
		.iter()
		.find(|e| e["name"] == "escape.txt")
		.unwrap();
	assert_eq!(escape["type"], "symlink");
	let page = client
		.ok("list_dir", json!({ "path": ".", "limit": 2 }))
		.await;
	assert_eq!(page["entries"].as_array().unwrap().len(), 2);
	let rest = client
		.ok(
			"list_dir",
			json!({ "path": ".", "cursor": page["next_cursor"] }),
		)
		.await;
	assert_eq!(rest["entries"].as_array().unwrap().len(), listed.len() - 2);
	let docs = client.ok("list_dir", json!({ "path": "docs/" })).await;
	assert_eq!(paths(&docs, "entries"), vec!["docs/readme.md"]);

	// stat
	let stat = client.ok("stat", json!({ "path": "docs/readme.md" })).await;
	assert_eq!(stat["type"], "file");
	assert_eq!(stat["size"], 29);
	assert_eq!(stat["path"], "docs/readme.md");

	// read: whole file, a line range, and the per-call cap.
	let reply = client
		.call("read", json!({ "path": "docs/readme.md" }))
		.await;
	let result = &reply.body["result"];
	assert_eq!(
		result["content"][0]["text"],
		"# Readme\nline two\nline three\n"
	);
	assert_eq!(result["structuredContent"]["eof"], true);
	let reply = client
		.call(
			"read",
			json!({ "path": "docs/readme.md", "start_line": 2, "line_count": 1 }),
		)
		.await;
	let result = &reply.body["result"];
	assert_eq!(result["content"][0]["text"], "line two\n");
	assert_eq!(result["structuredContent"]["next_line"], 3);
	let capped = client.ok("read", json!({ "path": "big.txt" })).await;
	assert_eq!(capped["bytes_read"], 64 * 1024, "64 KiB by default");
	assert_eq!(capped["truncated"], true);
	assert_eq!(capped["next_offset"], 64 * 1024);
	let capped = client
		.ok(
			"read",
			json!({ "path": "big.txt", "max_bytes": 1 << 30, "length": 1 << 30 }),
		)
		.await;
	assert_eq!(capped["bytes_read"], 1024 * 1024, "never more than 1 MiB");
	let tail = client
		.ok(
			"read",
			json!({ "path": "big.txt", "offset": 2 * 1024 * 1024 - 4 }),
		)
		.await;
	assert_eq!(
		(tail["bytes_read"].clone(), tail["eof"].clone()),
		(json!(4), json!(true))
	);

	// glob and grep reach hidden files and answer in grant-relative paths.
	let globbed = client.ok("glob", json!({ "pattern": "**/*.md" })).await;
	assert_eq!(paths(&globbed, "matches"), vec!["docs/readme.md"]);
	let globbed = client
		.ok("glob", json!({ "pattern": "*.txt", "path": ".hidden" }))
		.await;
	assert_eq!(paths(&globbed, "matches"), vec![".hidden/notes.txt"]);
	let grepped = client.ok("grep", json!({ "pattern": "needle" })).await;
	let mut found = paths(&grepped, "matches");
	found.sort();
	assert_eq!(found, vec![".hidden/notes.txt", "src/main.rs"]);
	assert!(
		!found.iter().any(|p| p.contains("escape")),
		"links are not followed"
	);
	let grepped = client
		.ok("grep", json!({ "pattern": "needle", "path": "src" }))
		.await;
	assert_eq!(grepped["matches"][0]["line_number"], 2);
	let grepped = client
		.ok(
			"grep",
			json!({ "pattern": "needle", "path": "src/main.rs" }),
		)
		.await;
	assert_eq!(
		paths(&grepped, "matches"),
		vec!["src/main.rs"],
		"a file scope"
	);
	let reply = client
		.call("grep", json!({ "pattern": "x".repeat(2000) }))
		.await;
	assert_eq!(reply.body["result"]["isError"], true, "pattern length cap");

	// search answers from the index, inside the grant only.
	let searched = client.ok("search", json!({ "query": "readme" })).await;
	assert_eq!(paths(&searched, "results"), vec!["docs/readme.md"]);
	let searched = client.ok("search", json!({ "query": "secret" })).await;
	assert!(paths(&searched, "results").is_empty(), "{searched}");

	// search_content answers from the text index, inside the grant only, with
	// paths relative to the grant root.
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	let found = loop {
		let found = client
			.ok("search_content", json!({ "query": "needle" }))
			.await;
		if found["pending_files"] == json!(0)
			&& paths(&found, "results").contains(&"src/main.rs".to_string())
		{
			break found;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"the text index never settled: {found}"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	};
	let main = found["results"]
		.as_array()
		.unwrap()
		.iter()
		.find(|hit| hit["path"] == "src/main.rs")
		.unwrap();
	assert_eq!(main["kind"], "code", "{found}");
	assert_eq!(main["matched"], json!(["needle"]), "{found}");
	let found = client
		.ok(
			"search_content",
			json!({ "query": "needle", "path": "src/main.rs" }),
		)
		.await;
	assert_eq!(paths(&found, "results"), vec!["src/main.rs"]);
	let found = client
		.ok(
			"search_content",
			json!({ "query": "\"line two\"", "path": "docs" }),
		)
		.await;
	assert_eq!(paths(&found, "results"), vec!["docs/readme.md"]);
	let found = client
		.ok(
			"search_content",
			json!({ "query": "needle", "kinds": ["markdown"] }),
		)
		.await;
	assert!(paths(&found, "results").is_empty(), "{found}");
	let found = client
		.ok("search_content", json!({ "query": "secret" }))
		.await;
	assert!(paths(&found, "results").is_empty(), "{found}");
	let reply = client
		.call(
			"search_content",
			json!({ "query": "needle", "path": "../private" }),
		)
		.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);

	// A source that reads no file contents has no text index, and the refusal
	// names no host path.
	let plain = harness.create_test_dir("plain").await?;
	plain.write_file("notes.txt", "a plain needle\n").await?;
	TrackSourceAction::from_input(TrackSourceInput {
		path: base.join("plain"),
		name: None,
		overrides: AddOverrides {
			identify_content: Some(false),
			..AddOverrides::default()
		},
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let plain_grant = grant(&harness, &base.join("plain")).await?;
	let mut plain_client = Client::new(&url, &plain_grant.token);
	plain_client.initialize().await;
	let reply = plain_client
		.call("search_content", json!({ "query": "needle" }))
		.await;
	let result = &reply.body["result"];
	assert_eq!(result["isError"], json!(true), "{}", reply.body);
	let message = result["content"][0]["text"].as_str().unwrap();
	assert!(message.contains("use grep"), "{message}");
	assert!(!message.contains(&*base.to_string_lossy()), "{message}");

	// Escapes are refused with 403.
	let secret = base.join("tracked/private/secret.txt");
	for path in [
		"../private/secret.txt",
		"docs/../../private/secret.txt",
		"escape.txt",
		"escape_dir/secret.txt",
		secret.to_str().unwrap(),
		project.join("docs/readme.md").to_str().unwrap(),
		"/etc/passwd",
	] {
		let reply = client.call("read", json!({ "path": path })).await;
		assert_eq!(
			reply.status,
			StatusCode::FORBIDDEN,
			"{path}: {}",
			reply.body
		);
		assert!(
			!reply.body.to_string().contains(base.to_str().unwrap()),
			"no host path in {}",
			reply.body
		);
	}
	let reply = client
		.call("list_dir", json!({ "path": "escape_dir" }))
		.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN);

	// A token for another subtree reaches neither this grant's files nor
	// its session.
	let mut other = Client::new(&url, &private_grant.token);
	other.session = client.session.clone();
	let reply = other.call("read", json!({ "path": "secret.txt" })).await;
	assert_eq!(
		reply.status,
		StatusCode::NOT_FOUND,
		"a session is bound to its grant"
	);
	assert_eq!(other.initialize().await.status, StatusCode::OK);
	assert_eq!(
		other.ok("read", json!({ "path": "secret.txt" })).await["bytes_read"],
		6
	);
	for path in [
		"../project/docs/readme.md",
		project.join("docs/readme.md").to_str().unwrap(),
	] {
		let reply = other.call("read", json!({ "path": path })).await;
		assert_eq!(reply.status, StatusCode::FORBIDDEN, "{path}");
	}

	// Transport guards.
	let http = reqwest::Client::new();
	let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} });
	let reply = http
		.post(&url)
		.bearer_auth(&project_grant.token)
		.header(header::ORIGIN, "https://evil.example")
		.header(header::CONTENT_TYPE, "application/json")
		.body(initialize.to_string())
		.send()
		.await?;
	assert_eq!(reply.status(), StatusCode::FORBIDDEN, "foreign Origin");
	assert!(reply
		.headers()
		.get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
		.is_none());
	let reply = http
		.post(&url)
		.bearer_auth(&project_grant.token)
		.header(header::HOST, format!("rebind.example:{}", bound.port()))
		.header(header::CONTENT_TYPE, "application/json")
		.body(initialize.to_string())
		.send()
		.await?;
	assert_eq!(reply.status(), StatusCode::FORBIDDEN, "rebinding Host");
	let reply = http.request(Method::OPTIONS, &url).send().await?;
	assert_eq!(reply.status(), StatusCode::FORBIDDEN, "OPTIONS");
	let reply = http
		.post(format!("{url}?access_token={}", project_grant.token))
		.header(header::CONTENT_TYPE, "application/json")
		.body(initialize.to_string())
		.send()
		.await?;
	assert_eq!(reply.status(), StatusCode::BAD_REQUEST, "token in the URL");
	let reply = http.get(&url).send().await?;
	assert_eq!(
		reply.status(),
		StatusCode::METHOD_NOT_ALLOWED,
		"no SSE stream"
	);
	let reply = http
		.post(&url)
		.bearer_auth(&project_grant.token)
		.body(vec![b' '; 65 * 1024])
		.send()
		.await?;
	assert_eq!(reply.status(), StatusCode::PAYLOAD_TOO_LARGE);
	let reply = client
		.send(json!({ "jsonrpc": "2.0", "id": 99, "method": "tools/list" }))
		.await;
	assert_eq!(reply.status, StatusCode::OK);
	let mut sessionless = Client::new(&url, &project_grant.token);
	let reply = sessionless.request("tools/list", json!({})).await;
	assert_eq!(
		reply.status,
		StatusCode::BAD_REQUEST,
		"a session is required"
	);

	// Revocation and expiry refuse the very next call with 401.
	RevokeGrantAction::from_input(RevokeGrantInput {
		id: private_grant.grant.id,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let reply = other.call("read", json!({ "path": "secret.txt" })).await;
	assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "revoked");
	agent_grant::Entity::update_many()
		.col_expr(
			agent_grant::Column::ExpiresAt,
			Expr::value(Utc::now() - chrono::Duration::seconds(1)),
		)
		.filter(agent_grant::Column::Uuid.eq(project_grant.grant.id))
		.exec(harness.library.db().conn())
		.await?;
	let reply = client
		.call("stat", json!({ "path": "docs/readme.md" }))
		.await;
	assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "expired");
	assert_eq!(
		reply.body["error"],
		stranger.initialize().await.body["error"],
		"one body for every 401"
	);

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn agent_mcp_enforces_rate_session_and_lockout_limits() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_limits")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("a.txt", "a").await?;
	tracked.track().await?;
	let root = std::fs::canonicalize(harness.temp_path())?.join("tracked");
	let token = grant(&harness, &root).await?.token;

	let limits = Limits {
		rate_per_second: 0.01,
		rate_burst: 2.0,
		sessions_per_grant: 1,
		auth_failures_per_minute: 2,
		..Limits::default()
	};
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, limits)?;
	let url = format!("http://{bound}/mcp");

	let mut client = Client::new(&url, &token);
	assert_eq!(client.initialize().await.status, StatusCode::OK);
	let mut second = Client::new(&url, &token);
	assert_eq!(
		second.initialize().await.status,
		StatusCode::TOO_MANY_REQUESTS,
		"one session per grant here"
	);
	let reply = client.request("ping", json!({})).await;
	assert_eq!(
		reply.status,
		StatusCode::TOO_MANY_REQUESTS,
		"burst of 2 spent"
	);
	assert!(reply.headers.contains_key(header::RETRY_AFTER));

	let mut stranger = Client::new(&url, &format!("sdg_{}", "1".repeat(64)));
	for _ in 0..2 {
		assert_eq!(stranger.initialize().await.status, StatusCode::UNAUTHORIZED);
	}
	assert_eq!(stranger.initialize().await.status, StatusCode::UNAUTHORIZED);
	let reply = stranger.initialize().await;
	assert_eq!(reply.status, StatusCode::TOO_MANY_REQUESTS, "locked out");
	assert!(reply.headers.contains_key(header::RETRY_AFTER));

	tokio::time::sleep(Duration::from_millis(10)).await;
	harness.shutdown().await?;
	Ok(())
}
