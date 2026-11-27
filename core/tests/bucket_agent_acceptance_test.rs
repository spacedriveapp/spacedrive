//! Acceptance for agents on a bucket source, against a real S3-compatible
//! service: connect a bucket, track it as a source, grant a prefix of it,
//! then list, read, grep and stat through the agent MCP endpoint, write
//! against the content hash `stat` returned, have a stale write refused with
//! the current hash, and keep both objects when a create finds the name
//! taken. Protected files, escapes, moves, deletes and read-only grants are
//! refused as they are on a local folder.
//!
//! The service is named by `SD_TEST_S3_ENDPOINT` (and optionally
//! `SD_TEST_S3_BUCKET`, `SD_TEST_S3_ACCESS_KEY`, `SD_TEST_S3_SECRET_KEY`).
//! CI runs SeaweedFS for it. Without an endpoint the test skips with a reason,
//! unless `SD_REQUIRE_S3` is set, in which case it fails.

#![cfg(unix)]

mod helpers;

use std::path::{Path, PathBuf};

use bytes::Bytes;
use helpers::*;
use reqwest::{header, StatusCode};
use sd_core::{
	infra::action::LibraryAction,
	library::AddOverrides,
	ops::{
		grants::{
			create::{CreateGrantAction, CreateGrantInput},
			GrantCap,
		},
		sources::track::{TrackSourceAction, TrackSourceInput},
		volumes::add_cloud::{CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput},
	},
	service::agent_mcp::{self, Limits},
	volume::{backend::CloudServiceType, CloudBackend, VolumeBackend},
};
use serde_json::{json, Value};
use uuid::Uuid;

struct S3 {
	endpoint: String,
	bucket: String,
	access: String,
	secret: String,
}

fn s3() -> Option<S3> {
	let Ok(endpoint) = std::env::var("SD_TEST_S3_ENDPOINT") else {
		let reason = "SD_TEST_S3_ENDPOINT is not set";
		if std::env::var_os("SD_REQUIRE_S3").is_some() {
			panic!("SD_REQUIRE_S3 is set but {reason}");
		}
		eprintln!("skipping: {reason}");
		return None;
	};
	Some(S3 {
		endpoint,
		bucket: std::env::var("SD_TEST_S3_BUCKET").unwrap_or_else(|_| "spacedrive-test".into()),
		access: std::env::var("SD_TEST_S3_ACCESS_KEY").unwrap_or_else(|_| "test".into()),
		secret: std::env::var("SD_TEST_S3_SECRET_KEY").unwrap_or_else(|_| "test".into()),
	})
}

async fn grant(harness: &IndexingHarness, path: &str, caps: GrantCap) -> anyhow::Result<String> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: PathBuf::from(path),
		caps: Some(caps),
		ttl_seconds: None,
		label: Some("bucket agent".to_string()),
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
					"clientInfo": { "name": "bucket", "version": "0" },
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

	/// A tool call that ran and failed, or was refused: its error text.
	async fn fails(&mut self, tool: &str, arguments: Value) -> String {
		let (status, result) = self.call(tool, arguments.clone()).await;
		if status != StatusCode::OK {
			return format!("{status}");
		}
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

#[tokio::test]
async fn agents_read_grep_and_write_a_bucket_prefix_with_preconditions() -> anyhow::Result<()> {
	let Some(s3) = s3() else { return Ok(()) };
	let harness = IndexingHarnessBuilder::new("bucket_agent")
		.disable_watcher()
		.build()
		.await?;

	// Each run works under its own prefix of the shared test bucket.
	let run = format!("agents/{}", Uuid::now_v7().simple());
	let seed = CloudBackend::new_s3(
		&s3.bucket,
		"us-east-1",
		&s3.access,
		&s3.secret,
		Some(s3.endpoint.clone()),
	)
	.await?;
	for (key, body) in [
		("project/notes.md", "first\n"),
		("project/AGENTS.md", "rules\n"),
		(
			"project/src/main.rs",
			"fn main() {\n\tprintln!(\"needle\");\n}\n",
		),
		("project/src/lib.rs", "pub fn other() {}\n"),
		("outside/secret.txt", "not granted\n"),
	] {
		seed.write(Path::new(&format!("{run}/{key}")), Bytes::from(body))
			.await?;
	}

	// Connect the bucket, then track a prefix of it as a source.
	VolumeAddCloudAction::from_input(VolumeAddCloudInput {
		service: CloudServiceType::S3,
		display_name: "Test bucket".to_string(),
		config: CloudStorageConfig::S3 {
			bucket: s3.bucket.clone(),
			region: "us-east-1".to_string(),
			access_key_id: s3.access.clone(),
			secret_access_key: s3.secret.clone(),
			endpoint: Some(s3.endpoint.clone()),
		},
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let source_root = format!("s3://{}/{run}", s3.bucket);
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: PathBuf::from(&source_root),
		name: Some("Agents bucket".to_string()),
		overrides: AddOverrides::default(),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	assert_eq!(tracked.root, PathBuf::from(&source_root));

	// A grant names a prefix inside the tracked source; one outside every
	// bucket source is refused.
	let project = format!("{source_root}/project");
	let outside = grant(
		&harness,
		&format!("s3://{}/elsewhere", s3.bucket),
		GrantCap::Read,
	)
	.await;
	assert!(
		outside.is_err(),
		"a prefix outside the source is not grantable"
	);

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;
	let url = format!("http://{bound}/mcp");
	let mut agent =
		Client::connect(&url, &grant(&harness, &project, GrantCap::ReadWrite).await?).await;

	// Read: list, read, glob.
	let listed = agent.ok("list_dir", json!({ "path": "" })).await;
	let names: Vec<&str> = listed["entries"]
		.as_array()
		.unwrap()
		.iter()
		.map(|entry| entry["name"].as_str().unwrap())
		.collect();
	assert_eq!(names, ["AGENTS.md", "notes.md", "src"], "{listed}");
	let read = agent.ok("read", json!({ "path": "notes.md" })).await;
	assert_eq!(read["size"], json!(6));
	let lines = agent
		.ok(
			"read",
			json!({ "path": "src/main.rs", "start_line": 2, "line_count": 1 }),
		)
		.await;
	assert_eq!(lines["lines_returned"], json!(1));
	let globbed = agent
		.ok("glob", json!({ "path": "", "pattern": "**/*.rs" }))
		.await;
	assert_eq!(globbed["matches"].as_array().unwrap().len(), 2, "{globbed}");

	// Grep runs over the objects themselves.
	let found = agent
		.ok(
			"grep",
			json!({ "path": "", "pattern": "needle", "literal": true }),
		)
		.await;
	let matches = found["matches"].as_array().unwrap();
	assert_eq!(matches.len(), 1, "{found}");
	assert_eq!(matches[0]["path"], json!("src/main.rs"));
	assert_eq!(matches[0]["line_number"], json!(2));

	// The grant ends at its prefix.
	agent
		.fails("read", json!({ "path": "../outside/secret.txt" }))
		.await;

	// A write against the hash stat returned lands.
	let first = agent.hash("notes.md").await;
	assert_eq!(first, blake3::hash(b"first\n").to_hex().as_str());
	let written = agent
		.ok(
			"write_file",
			json!({ "path": "notes.md", "content": "second\n", "expect": { "content_hash": first } }),
		)
		.await;
	assert_eq!(written["conflict"], json!(false));
	assert_eq!(
		written["state"]["content_hash"],
		json!(blake3::hash(b"second\n").to_hex().to_string())
	);
	assert_eq!(
		seed.read(Path::new(&format!("{run}/project/notes.md")))
			.await?,
		"second\n"
	);

	// The same hash again is stale now, and the refusal names the current one.
	let stale = agent
		.fails(
			"write_file",
			json!({ "path": "notes.md", "content": "third\n", "expect": { "content_hash": first } }),
		)
		.await;
	assert!(stale.contains("stale"), "{stale}");
	assert!(
		stale.contains(blake3::hash(b"second\n").to_hex().as_str()),
		"{stale}"
	);

	// A change from outside Spacedrive makes the agent's hash stale too.
	let second = agent.hash("notes.md").await;
	seed.write(
		Path::new(&format!("{run}/project/notes.md")),
		Bytes::from("edited elsewhere\n"),
	)
	.await?;
	let refused = agent
		.fails(
			"write_file",
			json!({ "path": "notes.md", "content": "agent\n", "expect": { "content_hash": second } }),
		)
		.await;
	assert!(refused.contains("stale"), "{refused}");
	assert_eq!(
		seed.read(Path::new(&format!("{run}/project/notes.md")))
			.await?,
		"edited elsewhere\n"
	);

	// A create never replaces: the new object lands beside the old one.
	let created = agent
		.ok(
			"write_file",
			json!({ "path": "notes.md", "content": "mine\n" }),
		)
		.await;
	assert_eq!(created["conflict"], json!(true));
	assert_eq!(
		created["state"]["path"],
		json!("notes (agent conflict 1).md")
	);
	assert_eq!(
		seed.read(Path::new(&format!(
			"{run}/project/notes (agent conflict 1).md"
		)))
		.await?,
		"mine\n"
	);
	assert_eq!(
		seed.read(Path::new(&format!("{run}/project/notes.md")))
			.await?,
		"edited elsewhere\n"
	);
	let fresh = agent
		.ok(
			"write_file",
			json!({ "path": "drafts/new.md", "content": "new\n" }),
		)
		.await;
	assert_eq!(fresh["conflict"], json!(false));

	// Protected files, moves and deletes are refused.
	let agents_hash = agent.hash("AGENTS.md").await;
	let protected = agent
		.fails(
			"write_file",
			json!({ "path": "AGENTS.md", "content": "x", "expect": { "content_hash": agents_hash } }),
		)
		.await;
	assert!(
		protected.contains("AGENTS.md") || protected.contains("403"),
		"{protected}"
	);
	let deleted = agent
		.fails("delete", json!({ "path": "drafts/new.md", "expect": { "content_hash": blake3::hash(b"new\n").to_hex().to_string() } }))
		.await;
	assert!(deleted.contains("bucket"), "{deleted}");

	// A read grant on the same prefix reads and does not write.
	let mut reader = Client::connect(&url, &grant(&harness, &project, GrantCap::Read).await?).await;
	reader.ok("read", json!({ "path": "notes.md" })).await;
	let read_only = reader
		.fails("write_file", json!({ "path": "notes.md", "content": "x" }))
		.await;
	assert!(
		read_only.contains("read-only") || read_only.contains("403"),
		"{read_only}"
	);

	for key in [
		"project/notes.md",
		"project/notes (agent conflict 1).md",
		"project/AGENTS.md",
		"project/src/main.rs",
		"project/src/lib.rs",
		"project/drafts/new.md",
		"outside/secret.txt",
	] {
		let _ = seed.delete(Path::new(&format!("{run}/{key}"))).await;
	}
	harness.shutdown().await?;
	Ok(())
}
