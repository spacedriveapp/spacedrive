//! Acceptance for a space homed in a bucket, against a real S3-compatible
//! service: a prefix with objects becomes a folder space and an empty one a
//! workspace space, an agent granted the space reads and writes it through
//! the space endpoint's tools, stat, list and block reads, and its writes
//! reach the event stream and the index.
//!
//! The service is named by `SD_TEST_S3_ENDPOINT` (and optionally
//! `SD_TEST_S3_BUCKET`, `SD_TEST_S3_ACCESS_KEY`, `SD_TEST_S3_SECRET_KEY`).
//! Without an endpoint the test skips with a reason, unless `SD_REQUIRE_S3`
//! is set, in which case it fails.

#![cfg(unix)]

mod helpers;

use std::{
	path::{Path, PathBuf},
	time::Duration,
};

use bytes::Bytes;
use futures::StreamExt;
use helpers::*;
use reqwest::{header, StatusCode};
use sd_core::{
	domain::{Space, SpaceKind},
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	ops::{
		files::bucket::objects_under,
		grants::bucket::BucketUrl,
		grants::{
			create::{CreateGrantAction, CreateGrantInput},
			GrantCap,
		},
		search::{input::FileSearchInput, query::FileSearchQuery},
		spaces::{SpaceCreateAction, SpaceCreateInput, SpacesListQuery, SpacesListQueryInput},
		volumes::add_cloud::{CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput},
	},
	service::agent_mcp::{self, Limits},
	volume::{backend::CloudServiceType, CloudBackend, VolumeBackend},
};
use serde_json::{json, Value};
use uuid::Uuid;

async fn create(
	harness: &IndexingHarness,
	kind: SpaceKind,
	path: &str,
	content_indexing: Option<bool>,
) -> anyhow::Result<Space> {
	Ok(SpaceCreateAction::from_input(SpaceCreateInput {
		name: None,
		kind,
		path: Some(PathBuf::from(path)),
		content_indexing,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?
	.space)
}

fn session(harness: &IndexingHarness) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(harness.library.id());
	session
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
}

impl Rest {
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

	async fn blocks(&self, path: &str, range: &str, if_match: Option<&str>) -> Reply {
		let mut request = self
			.http
			.get(self.url(&format!("blocks/{path}")))
			.header(header::RANGE, range);
		if let Some(tag) = if_match {
			request = request.header(header::IF_MATCH, tag);
		}
		self.send(request).await
	}

	async fn ok(&self, name: &str, arguments: Value) -> Value {
		let reply = self
			.send(
				self.http
					.post(self.url(&format!("tools/{name}")))
					.header(header::CONTENT_TYPE, "application/json")
					.body(arguments.to_string()),
			)
			.await;
		assert_eq!(
			reply.status,
			StatusCode::OK,
			"{name} {arguments}: {}",
			reply.json()
		);
		reply.json()
	}
}

/// The next `change` event on an open stream.
async fn next_change(
	body: &mut futures::stream::BoxStream<'static, reqwest::Result<bytes::Bytes>>,
	buffer: &mut String,
) -> Value {
	loop {
		while let Some(end) = buffer.find("\n\n") {
			let block: String = buffer.drain(..end + 2).collect();
			if block.lines().any(|line| line == "event: change") {
				let data = block
					.lines()
					.find_map(|line| line.strip_prefix("data: "))
					.expect("data");
				return serde_json::from_str(data).unwrap();
			}
		}
		let chunk = tokio::time::timeout(Duration::from_secs(20), body.next())
			.await
			.expect("an event within 20 s")
			.expect("the stream is open")
			.expect("a chunk");
		buffer.push_str(&String::from_utf8_lossy(&chunk));
	}
}

#[tokio::test]
async fn a_bucket_prefix_is_a_space_an_agent_works_in() -> anyhow::Result<()> {
	let Ok(endpoint) = std::env::var("SD_TEST_S3_ENDPOINT") else {
		if std::env::var_os("SD_REQUIRE_S3").is_some() {
			panic!("SD_REQUIRE_S3 is set but SD_TEST_S3_ENDPOINT is not set");
		}
		eprintln!("skipping: SD_TEST_S3_ENDPOINT is not set");
		return Ok(());
	};
	let bucket = std::env::var("SD_TEST_S3_BUCKET").unwrap_or_else(|_| "spacedrive-test".into());
	let access = std::env::var("SD_TEST_S3_ACCESS_KEY").unwrap_or_else(|_| "test".into());
	let secret = std::env::var("SD_TEST_S3_SECRET_KEY").unwrap_or_else(|_| "test".into());
	let harness = IndexingHarnessBuilder::new("bucket_space")
		.disable_watcher()
		.build()
		.await?;

	let run = format!("spaces/{}", Uuid::now_v7().simple());
	let seed = CloudBackend::new_s3(
		&bucket,
		"us-east-1",
		&access,
		&secret,
		Some(endpoint.clone()),
	)
	.await?;
	for (key, body) in [
		("project/notes.md", "0123456789"),
		("project/src/main.rs", "fn main() {}\n"),
	] {
		seed.write(Path::new(&format!("{run}/{key}")), Bytes::from(body))
			.await?;
	}
	VolumeAddCloudAction::from_input(VolumeAddCloudInput {
		service: CloudServiceType::S3,
		display_name: "Test bucket".to_string(),
		config: CloudStorageConfig::S3 {
			bucket: bucket.clone(),
			region: "us-east-1".to_string(),
			access_key_id: access.clone(),
			secret_access_key: secret.clone(),
			endpoint: Some(endpoint.clone()),
		},
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let base = format!("s3://{bucket}/{run}");

	// A prefix with objects is a folder space; one without objects is not,
	// and a bucket space never reads contents.
	let project = format!("{base}/project");
	let space = create(&harness, SpaceKind::Folder, &project, None).await?;
	assert_eq!(space.kind, SpaceKind::Folder);
	assert_eq!(space.name, "project");
	assert_eq!(space.path, Some(PathBuf::from(&project)));
	assert!(!space.content_indexing);
	assert!(create(
		&harness,
		SpaceKind::Folder,
		&format!("{base}/nothing"),
		None
	)
	.await
	.is_err());
	assert!(create(
		&harness,
		SpaceKind::Workspace,
		&format!("{base}/read"),
		Some(true)
	)
	.await
	.is_err());

	// An empty prefix becomes a workspace; a prefix with objects cannot.
	let scratch = format!("{base}/scratch");
	let workspace = create(&harness, SpaceKind::Workspace, &scratch, None).await?;
	assert_eq!(workspace.kind, SpaceKind::Workspace);
	assert!(create(&harness, SpaceKind::Workspace, &project, None)
		.await
		.is_err());
	seed.write(Path::new(&format!("{run}/lone")), Bytes::from("a file"))
		.await?;
	assert!(
		create(
			&harness,
			SpaceKind::Workspace,
			&format!("{base}/lone"),
			None
		)
		.await
		.is_err(),
		"a workspace never sits on an object's key"
	);
	let listed: Vec<Uuid> = SpacesListQuery::from_input(SpacesListQueryInput { source_id: None })
		.unwrap()
		.execute(harness.core.context.clone(), session(&harness))
		.await?
		.spaces
		.iter()
		.map(|space| space.id)
		.collect();
	assert!(listed.contains(&space.id) && listed.contains(&workspace.id));

	// Grant the space and work in it through the endpoint.
	let issued = CreateGrantAction::from_input(CreateGrantInput {
		path: PathBuf::from(&project),
		caps: Some(GrantCap::ReadWrite),
		ttl_seconds: None,
		label: Some("bucket space".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(
		harness.core.context.clone(),
		listener,
		Limits {
			rate_per_second: 1000.0,
			rate_burst: 1000.0,
			..Limits::default()
		},
	)?;
	let rest = Rest {
		http: reqwest::Client::new(),
		base: format!("http://{bound}"),
		token: issued.token.clone(),
		space: issued.grant.id.to_string(),
	};

	let root = rest.get("stat/").await;
	assert_eq!(root.status, StatusCode::OK, "{}", root.json());
	assert_eq!(root.json()["kind"], json!("directory"));
	let listing = rest.get("list/").await;
	assert_eq!(listing.status, StatusCode::OK, "{}", listing.json());
	let entries = listing.json()["entries"].clone();
	let names: Vec<(&str, &str)> = entries
		.as_array()
		.unwrap()
		.iter()
		.map(|entry| {
			(
				entry["path"].as_str().unwrap(),
				entry["kind"].as_str().unwrap(),
			)
		})
		.collect();
	assert_eq!(names, [("notes.md", "file"), ("src", "directory")]);
	let stat = rest.get("stat/notes.md").await.json();
	assert_eq!(stat["size"], json!(10));
	let etag = stat["etag"].as_str().unwrap().to_string();
	assert_eq!(
		entries[0]["etag"],
		json!(etag),
		"a listing carries the same etag"
	);
	assert_eq!(
		rest.get("list/notes.md").await.status,
		StatusCode::BAD_REQUEST
	);

	// A directory made empty is still one, held open by its marker.
	rest.ok("make_dir", json!({ "path": "empty" })).await;
	assert_eq!(
		rest.get("stat/empty").await.json()["kind"],
		json!("directory")
	);
	let empty = rest.get("list/empty").await;
	assert_eq!(empty.status, StatusCode::OK, "{}", empty.json());
	assert_eq!(empty.json()["entries"], json!([]));
	let empty_hash = rest.ok("stat", json!({ "path": "empty" })).await;
	assert_eq!(empty_hash["type"], json!("dir"));
	let empty_hash = empty_hash["content_hash"].clone();

	// Moving and deleting an empty directory move and drop its marker.
	rest.ok(
		"move",
		json!({ "path": "empty", "to": "hollow", "expect": { "content_hash": empty_hash } }),
	)
	.await;
	assert_eq!(rest.get("stat/empty").await.status, StatusCode::NOT_FOUND);
	let hollow = rest.ok("stat", json!({ "path": "hollow" })).await;
	assert_eq!(hollow["type"], json!("dir"));
	rest.ok(
		"delete",
		json!({ "path": "hollow", "expect": { "content_hash": hollow["content_hash"] } }),
	)
	.await;
	assert_eq!(rest.get("stat/hollow").await.status, StatusCode::NOT_FOUND);
	assert_eq!(
		rest.get("stat/missing.md").await.status,
		StatusCode::NOT_FOUND
	);

	let block = rest.blocks("notes.md", "bytes=2-5", Some(&etag)).await;
	assert_eq!(block.status, StatusCode::PARTIAL_CONTENT);
	assert_eq!(block.bytes, b"2345");
	assert_eq!(block.headers[header::ETAG].to_str()?, etag);
	assert_eq!(
		block.headers[header::CONTENT_RANGE].to_str()?,
		"bytes 2-5/10"
	);
	let unsatisfiable = rest.blocks("notes.md", "bytes=10-", None).await;
	assert_eq!(unsatisfiable.status, StatusCode::RANGE_NOT_SATISFIABLE);

	// A write through the tool reaches an open stream and makes the read
	// with the old etag stale.
	let events = rest
		.http
		.get(rest.url("events"))
		.bearer_auth(&rest.token)
		.header(header::ACCEPT, "text/event-stream")
		.send()
		.await?;
	assert_eq!(events.status(), StatusCode::OK);
	let mut body = events.bytes_stream().boxed();
	let mut buffer = String::new();
	let hash = rest.ok("stat", json!({ "path": "notes.md" })).await["content_hash"].clone();
	rest.ok(
		"write_file",
		json!({ "path": "notes.md", "content": "changed", "expect": { "content_hash": hash } }),
	)
	.await;
	let change = next_change(&mut body, &mut buffer).await;
	assert_eq!(change["op"], json!("modify"));
	assert_eq!(change["path"], json!("notes.md"));
	let stale = rest.blocks("notes.md", "bytes=0-1", Some(&etag)).await;
	assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);

	rest.ok(
		"write_file",
		json!({ "path": "summit-notes.md", "content": "new" }),
	)
	.await;
	let change = next_change(&mut body, &mut buffer).await;
	assert_eq!(change["op"], json!("create"));
	assert_eq!(change["path"], json!("summit-notes.md"));

	// The source is listed again after the write, so search finds it.
	let wanted = format!("{project}/summit-notes.md");
	let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
	loop {
		let found =
			FileSearchQuery::from_input(FileSearchInput::simple("summit-notes".to_string()))
				.unwrap()
				.execute(harness.core.context.clone(), session(&harness))
				.await?;
		if found
			.files
			.iter()
			.any(|file| file.sd_path.to_string() == wanted)
		{
			break;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"the write never reached search"
		);
		tokio::time::sleep(Duration::from_millis(250)).await;
	}

	// Directory markers count toward the most one move or delete takes.
	for marker in ["markers/a", "markers/b"] {
		seed.create_directory(Path::new(&format!("{run}/{marker}")), true)
			.await?;
	}
	let markers = BucketUrl::parse(Path::new(&format!("{base}/markers"))).unwrap();
	assert_eq!(objects_under(&seed, &markers, 1).await?, None);
	assert_eq!(
		objects_under(&seed, &markers, 2).await?,
		Some(vec![
			format!("{run}/markers/a/"),
			format!("{run}/markers/b/")
		])
	);
	Ok(())
}
