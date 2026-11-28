//! Acceptance for the agent MCP write tools: \`write_file\`, \`make_dir\`,
//! \`move\` and \`delete\` over streamable HTTP, on top of the agent write
//! operations (\`docs/core/design/agent-writes.md\`). A writable grant writes
//! against the content hash \`stat\` returned, a stale hash is refused with
//! the current one, a create collision keeps both files, undo reverses a
//! write, and read grants and protected files are refused.

#![cfg(unix)]

mod helpers;

use std::{path::Path, time::Duration};

use helpers::*;
use reqwest::{header, StatusCode};
use sd_core::{
	infra::action::LibraryAction,
	ops::{
		files::undo::UndoJob,
		grants::{
			create::{CreateGrantAction, CreateGrantInput},
			GrantCap,
		},
	},
	service::agent_mcp::{self, Limits},
};
use serde_json::{json, Value};
use uuid::Uuid;

async fn grant(harness: &IndexingHarness, path: &Path, caps: GrantCap) -> anyhow::Result<String> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(caps),
		ttl_seconds: None,
		label: Some("mcp writes".to_string()),
		kind: None,
		token_format: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.token)
}

struct Client {
	http: reqwest::Client,
	url: String,
	token: String,
	session: Option<String>,
	next_id: u64,
}

impl Client {
	async fn connect(url: &str, token: &str) -> Self {
		let mut client = Self {
			http: reqwest::Client::new(),
			url: url.to_string(),
			token: token.to_string(),
			session: None,
			next_id: 1,
		};
		let (status, headers, body) = client
			.request(
				"initialize",
				json!({
					"protocolVersion": "2025-06-18",
					"capabilities": {},
					"clientInfo": { "name": "writes", "version": "0" },
				}),
			)
			.await;
		assert_eq!(status, StatusCode::OK, "{body}");
		client.session = headers
			.get("mcp-session-id")
			.map(|session| session.to_str().unwrap().to_string());
		client
	}

	async fn request(
		&mut self,
		method: &str,
		params: Value,
	) -> (StatusCode, header::HeaderMap, Value) {
		let id = self.next_id;
		self.next_id += 1;
		let mut request = self
			.http
			.post(&self.url)
			.bearer_auth(&self.token)
			.header(header::ACCEPT, "application/json, text/event-stream")
			.header(header::CONTENT_TYPE, "application/json")
			.body(
				json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
					.to_string(),
			);
		if let Some(session) = &self.session {
			request = request.header("mcp-session-id", session);
		}
		let response = request.send().await.expect("request");
		let status = response.status();
		let headers = response.headers().clone();
		let body =
			serde_json::from_slice(&response.bytes().await.expect("body")).unwrap_or(Value::Null);
		(status, headers, body)
	}

	/// A tool call's HTTP status and its result.
	async fn call(&mut self, tool: &str, arguments: Value) -> (StatusCode, Value) {
		let (status, _, body) = self
			.request(
				"tools/call",
				json!({ "name": tool, "arguments": arguments }),
			)
			.await;
		(status, body["result"].clone())
	}

	async fn ok(&mut self, tool: &str, arguments: Value) -> Value {
		let (status, result) = self.call(tool, arguments.clone()).await;
		assert_eq!(status, StatusCode::OK, "{tool} {arguments}");
		assert_eq!(
			result["isError"],
			json!(false),
			"{tool} {arguments}: {result}"
		);
		result["structuredContent"].clone()
	}

	/// A tool call that ran and failed: its error text.
	async fn fails(&mut self, tool: &str, arguments: Value) -> String {
		let (status, result) = self.call(tool, arguments.clone()).await;
		assert_eq!(status, StatusCode::OK, "{tool} {arguments}");
		assert_eq!(
			result["isError"],
			json!(true),
			"{tool} {arguments}: {result}"
		);
		result["content"][0]["text"].as_str().unwrap().to_string()
	}

	async fn hash(&mut self, path: &str) -> String {
		self.ok("stat", json!({ "path": path })).await["content_hash"]
			.as_str()
			.expect("a content hash")
			.to_string()
	}
}

fn read(path: &Path) -> String {
	std::fs::read_to_string(path).expect("readable")
}

async fn undo(harness: &IndexingHarness, id: &Value) {
	let id: Uuid = serde_json::from_value(id.clone()).expect("an undo id");
	let handle = harness
		.library
		.jobs()
		.dispatch(UndoJob::new(id, None))
		.await
		.expect("dispatched");
	tokio::time::timeout(Duration::from_secs(30), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
}

#[tokio::test]
async fn agent_mcp_write_tools_are_fresh_keep_both_and_undoable() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_writes")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("project/notes.md", "first").await?;
	tracked.write_file("project/AGENTS.md", "rules").await?;
	tracked
		.write_file("project/vendor/CLAUDE.md", "vendored")
		.await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;
	let url = format!("http://{bound}/mcp");
	let mut agent =
		Client::connect(&url, &grant(&harness, &project, GrantCap::ReadWrite).await?).await;

	// The write tools are listed and say they write.
	let (_, _, listed) = agent.request("tools/list", json!({})).await;
	for name in ["write_file", "make_dir", "move", "delete"] {
		let tool = listed["result"]["tools"]
			.as_array()
			.unwrap()
			.iter()
			.find(|tool| tool["name"] == name)
			.unwrap_or_else(|| panic!("{name} is listed"));
		assert_eq!(tool["annotations"]["readOnlyHint"], json!(false), "{name}");
	}

	// stat names the hash; a write against it lands and returns the next one.
	let first = agent.hash("notes.md").await;
	assert_eq!(first, blake3::hash(b"first").to_hex().as_str());
	let written = agent
		.ok(
			"write_file",
			json!({ "path": "notes.md", "content": "second", "expect": { "content_hash": first } }),
		)
		.await;
	assert_eq!(read(&project.join("notes.md")), "second");
	let second = written["state"]["content_hash"]
		.as_str()
		.unwrap()
		.to_string();
	assert_eq!(second, blake3::hash(b"second").to_hex().as_str());
	assert_eq!(written["conflict"], json!(false));
	let overwrite = written["undo"].clone();

	// The old hash is stale now, and the refusal names the current one.
	let refused = agent
		.fails(
			"write_file",
			json!({ "path": "notes.md", "content": "lost", "expect": { "content_hash": first } }),
		)
		.await;
	assert!(refused.starts_with("stale:"), "{refused}");
	assert!(refused.contains(&second), "{refused}");
	assert!(
		!refused.contains(project.to_str().unwrap()),
		"no host path: {refused}"
	);
	assert_eq!(read(&project.join("notes.md")), "second");

	// A byte range, and base64.
	agent
		.ok(
			"write_file",
			json!({ "path": "notes.md", "content": "S", "offset": 0, "expect": { "content_hash": second } }),
		)
		.await;
	assert_eq!(read(&project.join("notes.md")), "Second");
	agent
		.ok(
			"write_file",
			json!({ "path": "bin/data", "content": "AAH/", "encoding": "base64" }),
		)
		.await;
	assert_eq!(std::fs::read(project.join("bin/data"))?, vec![0, 1, 255]);

	// A create at a taken name keeps both.
	let kept = agent
		.ok(
			"write_file",
			json!({ "path": "notes.md", "content": "mine" }),
		)
		.await;
	assert_eq!(kept["conflict"], json!(true));
	assert_eq!(kept["state"]["path"], json!("notes (agent conflict 1).md"));
	assert_eq!(read(&project.join("notes.md")), "Second");
	assert_eq!(read(&project.join("notes (agent conflict 1).md")), "mine");

	// make_dir, move and delete.
	agent.ok("make_dir", json!({ "path": "drafts/old" })).await;
	assert!(project.join("drafts/old").is_dir());
	let hash = agent.hash("notes (agent conflict 1).md").await;
	let moved = agent
		.ok(
			"move",
			json!({ "path": "notes (agent conflict 1).md", "to": "drafts/mine.md", "expect": { "content_hash": hash } }),
		)
		.await;
	assert_eq!(moved["state"]["path"], json!("drafts/mine.md"));
	assert_eq!(read(&project.join("drafts/mine.md")), "mine");
	let stale_delete = agent
		.fails(
			"delete",
			json!({ "path": "drafts/mine.md", "expect": { "content_hash": first } }),
		)
		.await;
	assert!(stale_delete.starts_with("stale:"), "{stale_delete}");
	let hash = agent.hash("drafts/mine.md").await;
	let deleted = agent
		.ok(
			"delete",
			json!({ "path": "drafts/mine.md", "expect": { "content_hash": hash } }),
		)
		.await;
	assert!(!project.join("drafts/mine.md").exists());

	// Undo reverses each write.
	undo(&harness, &deleted["undo"]).await;
	assert_eq!(read(&project.join("drafts/mine.md")), "mine");
	undo(&harness, &moved["undo"]).await;
	assert_eq!(read(&project.join("notes (agent conflict 1).md")), "mine");
	let hash = agent.hash("notes.md").await;
	let v = agent
		.ok(
			"write_file",
			json!({ "path": "notes.md", "content": "third", "expect": { "content_hash": hash } }),
		)
		.await;
	undo(&harness, &v["undo"]).await;
	assert_eq!(read(&project.join("notes.md")), "Second");
	assert!(overwrite.is_string());

	// Protected files: the endpoint's grant check refuses the path, and the
	// write operations refuse a move onto one or a directory holding one.
	let agents_hash = agent.hash("AGENTS.md").await;
	let (status, _) = agent
		.call(
			"write_file",
			json!({ "path": "AGENTS.md", "content": "new", "expect": { "content_hash": agents_hash } }),
		)
		.await;
	assert_eq!(status, StatusCode::FORBIDDEN);
	let (status, _) = agent
		.call("make_dir", json!({ "path": ".git/hooks" }))
		.await;
	assert_eq!(status, StatusCode::FORBIDDEN);
	let hash = agent.hash("notes.md").await;
	let onto = agent
		.fails(
			"move",
			json!({ "path": "notes.md", "to": "AGENTS.md", "expect": { "content_hash": hash } }),
		)
		.await;
	assert!(onto.contains("cannot modify"), "{onto}");
	let vendor = agent.hash("vendor").await;
	let holding = agent
		.fails(
			"delete",
			json!({ "path": "vendor", "expect": { "content_hash": vendor } }),
		)
		.await;
	assert!(holding.contains("cannot modify"), "{holding}");
	assert_eq!(read(&project.join("AGENTS.md")), "rules");
	assert_eq!(read(&project.join("vendor/CLAUDE.md")), "vendored");
	assert!(!project.join(".git").exists());
	let escape = agent
		.fails(
			"move",
			json!({ "path": "notes.md", "to": "../outside.md", "expect": { "content_hash": hash } }),
		)
		.await;
	assert!(escape.contains(".."), "{escape}");
	assert!(project.join("notes.md").exists());

	// A read grant writes nothing.
	let mut reader = Client::connect(&url, &grant(&harness, &project, GrantCap::Read).await?).await;
	let (status, _) = reader
		.call("write_file", json!({ "path": "new.md", "content": "x" }))
		.await;
	assert_eq!(status, StatusCode::FORBIDDEN);
	assert!(!project.join("new.md").exists());

	harness.shutdown().await?;
	Ok(())
}
