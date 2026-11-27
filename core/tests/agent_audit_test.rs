//! The grant audit against the M1 checklist and the Audit section of
//! `docs/core/design/agent-access-threat-model.md`.
//!
//! Drives the agent MCP endpoint over loopback and reads `grant_audit` and
//! `grant_audit_tally` directly: a tool call records its client, session,
//! tool, path, pattern, bytes, result and duration; a refused path records
//! `denied`; unknown tokens and rate-limited requests are counted per minute
//! instead of row by row; no row holds a token or file contents; retention
//! trims by count and by age; and a request whose audit cannot be written
//! answers 503.

#![cfg(unix)]

mod helpers;

use std::path::Path;

use chrono::{Timelike, Utc};
use helpers::*;
use reqwest::{header, StatusCode};
use sd_core::{
	infra::{
		action::LibraryAction,
		db::entities::{grant_audit, grant_audit_tally},
	},
	ops::grants::{
		audit,
		create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
		GrantCap,
	},
	service::agent_mcp::{self, Limits},
};
use sea_orm::{
	ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
	QueryOrder,
};
use serde_json::{json, Value};

const SECRET: &str = "the quick brown secret-content-4471";

async fn grant(harness: &IndexingHarness, path: &Path) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(GrantCap::Read),
		ttl_seconds: None,
		label: Some("audit agent".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

struct Client {
	http: reqwest::Client,
	url: String,
	token: String,
	session: Option<String>,
	next_id: u64,
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

	async fn request(&mut self, method: &str, params: Value) -> (StatusCode, Value) {
		let id = self.next_id;
		self.next_id += 1;
		let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
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
		if method == "initialize" {
			if let Some(session) = response.headers().get("mcp-session-id") {
				self.session = Some(session.to_str().unwrap().to_string());
			}
		}
		let bytes = response.bytes().await.expect("body");
		(
			status,
			serde_json::from_slice(&bytes).unwrap_or(Value::Null),
		)
	}

	async fn initialize(&mut self) -> StatusCode {
		self.request(
			"initialize",
			json!({
				"protocolVersion": "2025-06-18",
				"capabilities": {},
				"clientInfo": { "name": "audit", "version": "0" },
			}),
		)
		.await
		.0
	}

	async fn call(&mut self, tool: &str, arguments: Value) -> (StatusCode, Value) {
		self.request(
			"tools/call",
			json!({ "name": tool, "arguments": arguments }),
		)
		.await
	}
}

async fn newest(conn: &sea_orm::DatabaseConnection, op: &str) -> grant_audit::Model {
	grant_audit::Entity::find()
		.filter(grant_audit::Column::Op.eq(op))
		.order_by_desc(grant_audit::Column::Id)
		.one(conn)
		.await
		.unwrap()
		.unwrap_or_else(|| panic!("no audit row for {op}"))
}

#[tokio::test]
async fn agent_audit_records_requests_tallies_floods_and_fails_closed() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_audit")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/notes.md", &format!("{SECRET}\nneedle here\n"))
		.await?;
	tracked.write_file("private/other.txt", SECRET).await?;
	tracked.track().await?;
	let base = std::fs::canonicalize(harness.temp_path())?;
	let created = grant(&harness, &base.join("tracked/project")).await?;
	let grant_id = created.grant.id;
	let conn = harness.library.db().conn();

	// The lockout stays out of the way so every unknown token is counted as
	// one.
	let limits = Limits {
		auth_failures_per_minute: 1000,
		..Limits::default()
	};
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, limits)?;
	let url = format!("http://{bound}/mcp");

	// A tool call records who asked, in which session, for what, and how it
	// ended.
	let mut client = Client::new(&url, &created.token);
	assert_eq!(client.initialize().await, StatusCode::OK);
	let session = client.session.clone().expect("session id");
	let (status, body) = client.call("read", json!({ "path": "notes.md" })).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	assert_eq!(body["result"]["isError"], json!(false), "{body}");
	let row = newest(conn, "mcp.read").await;
	assert_eq!(row.grant_id, grant_id);
	assert_eq!(row.grant_label.as_deref(), Some("audit agent"));
	assert_eq!(row.client.as_deref(), Some("127.0.0.1"));
	assert_eq!(row.session.as_deref(), Some(session.as_str()));
	assert_eq!(row.path, "notes.md");
	assert_eq!(row.result.as_deref(), Some("ok"));
	assert!(row.allowed);
	assert!(row.bytes.unwrap_or(0) > SECRET.len() as i64, "{row:?}");
	assert!(row.duration_ms.is_some(), "{row:?}");

	// A grep records its pattern, cut to 256 bytes.
	let pattern = format!("needle|{}", "x".repeat(400));
	let (status, _) = client.call("grep", json!({ "pattern": pattern })).await;
	assert_eq!(status, StatusCode::OK);
	let row = newest(conn, "mcp.grep").await;
	assert_eq!(row.pattern.as_deref(), Some(&pattern[..256]));
	assert_eq!(row.result.as_deref(), Some("ok"));

	// A path outside the grant is recorded as denied.
	let (status, _) = client
		.call("read", json!({ "path": "../private/other.txt" }))
		.await;
	assert_eq!(status, StatusCode::FORBIDDEN);
	let row = newest(conn, "mcp.read").await;
	assert_eq!(row.path, "../private/other.txt");
	assert_eq!(row.result.as_deref(), Some("denied"));
	assert!(!row.allowed);
	assert!(row.duration_ms.is_some());

	// 25 unknown tokens from one address in one minute are one row with a
	// count, not 25 rows. Start clear of a minute boundary.
	if Utc::now().second() >= 50 {
		tokio::time::sleep(std::time::Duration::from_secs(11)).await;
	}
	let audit_rows = grant_audit::Entity::find().count(conn).await?;
	let mut stranger = Client::new(&url, &format!("sdg_{}", "7".repeat(64)));
	for _ in 0..25 {
		assert_eq!(stranger.initialize().await, StatusCode::UNAUTHORIZED);
	}
	let unknown = grant_audit_tally::Entity::find()
		.filter(grant_audit_tally::Column::Kind.eq("unknown_token"))
		.all(conn)
		.await?;
	assert_eq!(unknown.len(), 1, "{unknown:?}");
	assert_eq!(unknown[0].count, 25);
	assert_eq!(unknown[0].subject, "127.0.0.1");
	assert_eq!(grant_audit::Entity::find().count(conn).await?, audit_rows);

	// Rate-limited requests are counted per grant the same way, and add no
	// rows of their own.
	let tight = Limits {
		rate_per_second: 0.01,
		rate_burst: 2.0,
		..Limits::default()
	};
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let tight_bound = agent_mcp::serve(harness.core.context.clone(), listener, tight)?;
	let mut hammer = Client::new(&format!("http://{tight_bound}/mcp"), &created.token);
	assert_eq!(hammer.initialize().await, StatusCode::OK);
	assert_eq!(hammer.request("ping", json!({})).await.0, StatusCode::OK);
	let audit_rows = grant_audit::Entity::find().count(conn).await?;
	for _ in 0..5 {
		assert_eq!(
			hammer.request("ping", json!({})).await.0,
			StatusCode::TOO_MANY_REQUESTS
		);
	}
	assert_eq!(grant_audit::Entity::find().count(conn).await?, audit_rows);
	let limited: i64 = grant_audit_tally::Entity::find()
		.filter(grant_audit_tally::Column::Kind.eq("rate_limited"))
		.filter(grant_audit_tally::Column::GrantId.eq(grant_id))
		.all(conn)
		.await?
		.iter()
		.map(|tally| tally.count)
		.sum();
	assert_eq!(limited, 5);

	// The owner's list shows last use and the request count.
	let usage = audit::usage(conn).await?.remove(&grant_id).unwrap();
	assert!(usage.last_used_at.is_some());
	assert_eq!(usage.requests, audit_rows + 5);

	// No row holds a token or what a file contains.
	let rows = serde_json::to_string(&grant_audit::Entity::find().all(conn).await?)?;
	let tallies = serde_json::to_string(&grant_audit_tally::Entity::find().all(conn).await?)?;
	for text in [&rows, &tallies] {
		assert!(!text.contains("sdg_"), "a token reached the audit");
		assert!(
			!text.contains(&created.token[4..]),
			"a token reached the audit"
		);
		assert!(!text.contains(SECRET), "file contents reached the audit");
		assert!(
			!text.contains("needle here"),
			"match text reached the audit"
		);
	}

	// Retention: the newest rows up to the cap survive, and nothing older
	// than the age limit does. The cap here is far below the daemon's.
	let old = Utc::now() - chrono::Duration::days(audit::RETENTION_DAYS + 1);
	let filler = |at| grant_audit::ActiveModel {
		grant_id: Set(grant_id),
		op: Set("filler".to_string()),
		path: Set(String::new()),
		allowed: Set(true),
		reason: Set(None),
		at: Set(at),
		..Default::default()
	};
	grant_audit::Entity::insert(filler(old)).exec(conn).await?;
	grant_audit::Entity::insert_many((0..40).map(|_| filler(Utc::now())))
		.exec(conn)
		.await?;
	let max_age = chrono::Duration::days(audit::RETENTION_DAYS);
	audit::trim(conn, audit::RETENTION_ROWS, max_age).await?;
	assert_eq!(
		grant_audit::Entity::find()
			.filter(grant_audit::Column::At.lt(Utc::now() - max_age))
			.count(conn)
			.await?,
		0,
		"rows past the age limit are trimmed"
	);
	let newest_id = newest(conn, "filler").await.id;
	audit::trim(conn, 10, max_age).await?;
	let kept = grant_audit::Entity::find()
		.order_by_asc(grant_audit::Column::Id)
		.all(conn)
		.await?;
	assert_eq!(kept.len(), 10);
	assert_eq!(kept.last().unwrap().id, newest_id);

	// A request whose audit cannot be written is not served.
	conn.execute_unprepared("DROP TABLE grant_audit").await?;
	let (status, body) = client.call("read", json!({ "path": "notes.md" })).await;
	assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
	assert!(!body.to_string().contains(SECRET));

	harness.shutdown().await?;
	Ok(())
}
