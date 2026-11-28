//! Handle-based path containment over the agent MCP endpoint.
//!
//! Starts the endpoint on an ephemeral loopback port, the way
//! `agent_mcp_acceptance_test` does, and checks item 9 and 10 of the M1
//! checklist in `docs/core/design/agent-access-threat-model.md`: '..' and
//! absolute paths are refused, links out of the root are refused or listed
//! without being followed, links that stay inside work, and a directory
//! swapped for a link out of the root while requests run never answers with
//! anything from outside it.

#![cfg(unix)]

mod helpers;

use std::{
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use helpers::*;
use reqwest::{header, StatusCode};
use sd_core::{
	common::contain,
	infra::action::LibraryAction,
	ops::grants::{
		self,
		create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
		GrantCap,
	},
	service::agent_mcp::{self, Limits},
};
use serde_json::{json, Value};

/// Only the outside file holds this, so it appearing in any answer is a leak.
const OUTSIDE_MARKER: &str = "OUTSIDE-MARKER-7f3a";
/// A name only the outside directory has.
const OUTSIDE_NAME: &str = "only-outside-name.txt";

async fn grant(
	harness: &IndexingHarness,
	path: &Path,
	caps: GrantCap,
) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(caps),
		ttl_seconds: None,
		label: Some("containment".to_string()),
		kind: None,
		token_format: None,
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
	body: Value,
}

impl Reply {
	/// Whether the call was served, as opposed to refused by the grant check
	/// or by the tool.
	fn served(&self) -> bool {
		self.status == StatusCode::OK && self.body["result"]["isError"] == json!(false)
	}

	fn assert_clean(&self, what: &str) {
		let text = self.body.to_string();
		assert!(
			!text.contains(OUTSIDE_MARKER) && !text.contains(OUTSIDE_NAME),
			"{what} answered with something from outside the root: {text}"
		);
	}
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

	async fn send(&mut self, message: Value) -> (Reply, header::HeaderMap) {
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
		(Reply { status, body }, headers)
	}

	async fn request(&mut self, method: &str, params: Value) -> Reply {
		let id = self.next_id;
		self.next_id += 1;
		self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
			.await
			.0
	}

	async fn initialize(&mut self) {
		let (reply, headers) = self
			.send(json!({
				"jsonrpc": "2.0",
				"id": 0,
				"method": "initialize",
				"params": {
					"protocolVersion": "2025-06-18",
					"capabilities": {},
					"clientInfo": { "name": "containment", "version": "0" },
				},
			}))
			.await;
		assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
		self.session = headers
			.get("mcp-session-id")
			.map(|session| session.to_str().unwrap().to_string());
		let (reply, _) = self
			.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
			.await;
		assert_eq!(reply.status, StatusCode::ACCEPTED);
	}

	/// Calls a tool and checks the answer holds nothing from outside.
	async fn call(&mut self, tool: &str, arguments: Value) -> Reply {
		let reply = self
			.request(
				"tools/call",
				json!({ "name": tool, "arguments": arguments.clone() }),
			)
			.await;
		reply.assert_clean(&format!("{tool} {arguments}"));
		reply
	}

	async fn ok(&mut self, tool: &str, arguments: Value) -> Value {
		let reply = self.call(tool, arguments.clone()).await;
		assert!(reply.served(), "{tool} {arguments}: {}", reply.body);
		reply.body["result"]["structuredContent"].clone()
	}

	async fn refused(&mut self, tool: &str, arguments: Value) {
		let reply = self.call(tool, arguments.clone()).await;
		assert!(
			!reply.served(),
			"{tool} {arguments} was served: {}",
			reply.body
		);
	}
}

/// Swaps `swap` between its real directory and a symlink to the outside
/// directory, renaming a prepared link over the name and back, until `stop`
/// is set. A directory cannot be renamed over, so each half is two renames
/// and the name is briefly missing too.
fn swapper(project: PathBuf, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<u64> {
	std::thread::spawn(move || {
		let name = project.join("swap");
		let aside = project.join(".swap.real");
		let link = project.join(".swap.link");
		let mut swaps = 0;
		while !stop.load(Ordering::Relaxed) || swaps < 2000 {
			std::fs::rename(&name, &aside).unwrap();
			std::fs::rename(&link, &name).unwrap();
			std::thread::yield_now();
			std::fs::rename(&name, &link).unwrap();
			std::fs::rename(&aside, &name).unwrap();
			swaps += 1;
		}
		swaps
	})
}

#[tokio::test]
async fn agent_mcp_reads_only_beneath_the_grant_root() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_containment")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file(
			"project/docs/a.txt",
			"inside text
",
		)
		.await?;
	tracked
		.write_file(
			"project/swap/secret.txt",
			"inside swap MARKER
",
		)
		.await?;
	tracked.track().await?;
	let outside = harness.create_test_dir("outside").await?;
	outside
		.write_file(
			"secret.txt",
			&format!(
				"{OUTSIDE_MARKER} MARKER
"
			),
		)
		.await?;
	outside
		.write_file(
			OUTSIDE_NAME,
			&format!(
				"{OUTSIDE_MARKER}
"
			),
		)
		.await?;

	let base = std::fs::canonicalize(harness.temp_path())?;
	let project = base.join("tracked/project");
	let outside = base.join("outside");
	std::os::unix::fs::symlink(outside.join("secret.txt"), project.join("escape.txt"))?;
	std::os::unix::fs::symlink(&outside, project.join("escape_dir"))?;
	std::os::unix::fs::symlink("docs/a.txt", project.join("inner_link.txt"))?;
	std::os::unix::fs::symlink("docs", project.join("inner_dir"))?;
	std::os::unix::fs::symlink(&outside, project.join(".swap.link"))?;

	let limits = Limits {
		rate_per_second: 1_000_000.0,
		rate_burst: 1_000_000.0,
		..Limits::default()
	};
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, limits)?;
	let issued = grant(&harness, &project, GrantCap::Read).await?;
	let mut client = Client::new(&format!("http://{bound}/mcp"), &issued.token);
	client.initialize().await;

	// '..' and absolute paths are refused.
	let absolute = outside.join("secret.txt").display().to_string();
	for path in [
		"../outside/secret.txt",
		"docs/../../outside/secret.txt",
		absolute.as_str(),
	] {
		for tool in ["read", "stat", "list_dir"] {
			client.refused(tool, json!({ "path": path })).await;
		}
		client
			.refused("grep", json!({ "pattern": "MARKER", "path": path }))
			.await;
	}

	// Links out of the root are refused on read and on a following stat.
	for path in ["escape.txt", "escape_dir/secret.txt", "escape_dir"] {
		client.refused("read", json!({ "path": path })).await;
		client.refused("stat", json!({ "path": path })).await;
	}
	client
		.refused("list_dir", json!({ "path": "escape_dir" }))
		.await;
	client
		.refused("glob", json!({ "pattern": "*", "path": "escape_dir" }))
		.await;
	client
		.refused("grep", json!({ "pattern": "MARKER", "path": "escape_dir" }))
		.await;

	// The walkers list links as links and never follow them.
	let listing = client.ok("list_dir", json!({ "path": "" })).await;
	let entries = listing["entries"].as_array().unwrap();
	for name in ["escape.txt", "escape_dir", "inner_link.txt", "inner_dir"] {
		let entry = entries.iter().find(|e| e["name"] == name).unwrap();
		assert_eq!(entry["type"], "symlink", "{name}");
	}
	let globbed = client.ok("glob", json!({ "pattern": "**/*" })).await;
	let globbed: Vec<&str> = globbed["matches"]
		.as_array()
		.unwrap()
		.iter()
		.map(|m| m["path"].as_str().unwrap())
		.collect();
	assert!(globbed.contains(&"escape_dir") && globbed.contains(&"docs/a.txt"));
	assert!(!globbed.iter().any(|p| p.starts_with("escape_dir/")));
	let grepped = client
		.ok("grep", json!({ "pattern": "MARKER|inside" }))
		.await;
	let grepped: Vec<&str> = grepped["matches"]
		.as_array()
		.unwrap()
		.iter()
		.map(|m| m["path"].as_str().unwrap())
		.collect();
	assert_eq!(grepped, vec!["docs/a.txt", "swap/secret.txt"]);

	// Links that stay inside the root work.
	let reply = client
		.call("read", json!({ "path": "inner_link.txt" }))
		.await;
	assert!(reply.served(), "{}", reply.body);
	assert_eq!(
		reply.body["result"]["content"][0]["text"],
		"inside text
"
	);
	let reply = client
		.call("read", json!({ "path": "inner_dir/a.txt" }))
		.await;
	assert_eq!(
		reply.body["result"]["content"][0]["text"],
		"inside text
"
	);
	let stat = client.ok("stat", json!({ "path": "inner_link.txt" })).await;
	assert_eq!(
		(stat["type"].clone(), stat["path"].clone()),
		(json!("file"), json!("docs/a.txt"))
	);
	let inner = client.ok("list_dir", json!({ "path": "inner_dir" })).await;
	assert_eq!(inner["entries"][0]["path"], "docs/a.txt");

	// The window between the grant check and the open: authorize while
	// `swap` is a real directory, swap it for a link out of the root, then open
	// what the check resolved. The handle is refused; a path open would read
	// the outside file.
	let access = grants::Access::new(
		&issued.token,
		"containment",
		Path::new("swap/secret.txt"),
		GrantCap::Read,
	);
	let scope = grants::authorize(&harness.core.context, &access)
		.await?
		.scope;
	std::fs::rename(project.join("swap"), project.join(".swap.real"))?;
	std::fs::rename(project.join(".swap.link"), project.join("swap"))?;
	assert!(std::fs::read_to_string(&scope.path)?.contains(OUTSIDE_MARKER));
	let refused = contain::open_beneath(&scope.root, &scope.relative).unwrap_err();
	assert_eq!(refused.kind(), std::io::ErrorKind::PermissionDenied);
	assert!(contain::reach_beneath(&scope.root, &scope.relative).is_err());
	assert!(contain::Directory::open(&scope.root, Path::new("swap")).is_err());
	client
		.refused("read", json!({ "path": "swap/secret.txt" }))
		.await;
	std::fs::rename(project.join("swap"), project.join(".swap.link"))?;
	std::fs::rename(project.join(".swap.real"), project.join("swap"))?;

	// The same swap racing live requests.
	let stop = Arc::new(AtomicBool::new(false));
	let swapper = swapper(project.clone(), stop.clone());
	let mut served = 0;
	for round in 0..600 {
		let reply = match round % 5 {
			0 | 1 => {
				client
					.call("read", json!({ "path": "swap/secret.txt" }))
					.await
			}
			2 => {
				client
					.call("grep", json!({ "pattern": "MARKER", "path": "swap" }))
					.await
			}
			3 => client.call("list_dir", json!({ "path": "swap" })).await,
			_ => {
				client
					.call("glob", json!({ "pattern": "**/*", "path": "swap" }))
					.await
			}
		};
		if reply.served() {
			served += 1;
		}
	}
	stop.store(true, Ordering::Relaxed);
	let swaps = swapper.join().unwrap();
	assert!(swaps >= 2000, "only {swaps} swaps");
	assert!(served > 0, "no request was served during the race");

	Ok(())
}

/// stat reads the content hash through handles, and it has to stay the hash
/// the write tools compare a precondition with, for a file and a directory.
#[tokio::test]
async fn stat_hashes_match_the_write_preconditions() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_containment_hashes")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("project/dir/a.txt", "alpha").await?;
	tracked.write_file("project/dir/sub/b.txt", "beta").await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");
	std::os::unix::fs::symlink("a.txt", project.join("dir/link"))?;

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;
	let issued = grant(&harness, &project, GrantCap::ReadWrite).await?;
	let mut client = Client::new(&format!("http://{bound}/mcp"), &issued.token);
	client.initialize().await;

	let written = client
		.ok(
			"write_file",
			json!({ "path": "dir/new.txt", "content": "gamma" }),
		)
		.await;
	let stat = client.ok("stat", json!({ "path": "dir/new.txt" })).await;
	assert_eq!(stat["content_hash"], written["state"]["content_hash"]);
	assert_eq!(
		stat["content_hash"],
		blake3::hash(b"gamma").to_hex().as_str()
	);

	// The revision stat names is the one a write compares. It counts every
	// change the store ingests, so a write racing the ingest of the one
	// above can see it move once; the next stat names the settled value.
	let mut landed = false;
	for _ in 0..20 {
		let stat = client.ok("stat", json!({ "path": "dir/new.txt" })).await;
		let revision = stat["revision"].as_str().unwrap().to_string();
		let (store, value) = revision.split_once(':').unwrap();
		assert!(
			store.len() == 32 && value.parse::<u64>().is_ok(),
			"{revision}"
		);
		let reply = client
			.call(
				"write_file",
				json!({ "path": "dir/new.txt", "content": "delta", "expect": { "revision": revision } }),
			)
			.await;
		if reply.served() {
			landed = true;
			break;
		}
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}
	assert!(landed, "a write against stat's revision never landed");

	let made = client.ok("make_dir", json!({ "path": "dir" })).await;
	let stat = client.ok("stat", json!({ "path": "dir" })).await;
	assert_eq!(made["state"]["kind"], "directory");
	assert!(stat["content_hash"].is_string());
	assert_eq!(stat["content_hash"], made["state"]["content_hash"]);

	client
		.ok(
			"delete",
			json!({ "path": "dir", "expect": { "content_hash": stat["content_hash"] } }),
		)
		.await;
	assert!(!project.join("dir").exists());
	Ok(())
}
