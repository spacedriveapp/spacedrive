//! # OneDrive change detector
//!
//! Microsoft Graph exposes per-drive incremental changes at
//! `GET /me/drive/root/delta`. The endpoint returns a page of `driveItem`
//! objects plus either `@odata.nextLink` (more pages in this scan) or
//! `@odata.deltaLink` (end of changes; pass its token on the next pass).
//! This module wraps that endpoint in the provider-agnostic
//! [`ChangeDetector`](super::types::ChangeDetector) surface so the indexer
//! can consume it without caring about Graph specifics.
//!
//! ## Error mapping
//! - HTTP 410 with body containing `resyncRequired` → `Invalidated`. The
//!   caller drops the token and performs a full rescan.
//! - HTTP 429 / 503 → `RateLimited { retry_after_secs }`. The scheduler backs
//!   off by that amount (capped by the repository's failure counter).
//! - HTTP 401 / 403 → `Auth`. The scheduler does NOT retry here; token
//!   rotation is the OAuth refresh task's job (Set 3 infrastructure).
//!
//! ## Path normalization
//! Graph returns `parentReference.path` like
//! `"/drive/root:/Documents/Reports"`. Stripping the vendor prefix gives the
//! indexer a regular slash-rooted path, consistent with how OpenDAL reports
//! paths on the same backend.

use super::types::{
	ChangeDetectionError, ChangeDetector, ChangeEntry, ChangeKind, ChangeToken, ChangesPage,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{header, StatusCode};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

/// Microsoft Graph production base URL for the v1.0 endpoint family.
const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

/// Client-side default retry delay when 429/503 omit `Retry-After`. Mirrors
/// Microsoft's recommended backoff for unspecified throttling.
const DEFAULT_RETRY_AFTER_SECS: u64 = 60;

/// OneDrive change detector backed by Microsoft Graph `/delta`.
pub struct OneDriveChangeDetector {
	graph_base_url: String,
	http: reqwest::Client,
	/// Shared access-token cell. Read through `access_token.read().await` on
	/// every request so that the OAuth refresh task can rotate the underlying
	/// token without recreating the detector. In the MVP the indexer seeds it
	/// from `cloud_credentials` immediately before each pass; hot-swap
	/// integration with the refresh task tracks as
	/// `TODO(cloud-mvp): hot-swap CloudBackend on token refresh` in
	/// `crate::ops::cloud::oauth::refresh`.
	access_token: Arc<RwLock<String>>,
}

impl OneDriveChangeDetector {
	/// Construct the detector against Microsoft's production Graph endpoint.
	pub fn new(access_token: Arc<RwLock<String>>) -> Self {
		Self::new_with_base_url(GRAPH_BASE_URL.to_string(), access_token)
	}

	/// Testability hook. Production callers must go through [`Self::new`].
	pub(crate) fn new_with_base_url(
		graph_base_url: String,
		access_token: Arc<RwLock<String>>,
	) -> Self {
		let http = reqwest::Client::builder()
			.timeout(Duration::from_secs(30))
			.build()
			.expect("reqwest client builds with 30s timeout");
		Self {
			graph_base_url,
			http,
			access_token,
		}
	}

	async fn current_token(&self) -> String {
		self.access_token.read().await.clone()
	}

	/// Fetch one raw delta payload and classify the response.
	async fn fetch_delta(&self, url: &str) -> Result<DeltaPayload, ChangeDetectionError> {
		let token = self.current_token().await;
		let response = self.http.get(url).bearer_auth(&token).send().await?;

		let status = response.status();

		if status == StatusCode::GONE {
			let body = response.text().await.unwrap_or_default();
			if body.contains("resyncRequired") {
				return Err(ChangeDetectionError::Invalidated);
			}
			return Err(ChangeDetectionError::Other(format!(
				"unexpected 410 without resyncRequired: {body}"
			)));
		}

		if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
			let retry_after_secs = response
				.headers()
				.get(header::RETRY_AFTER)
				.and_then(|v| v.to_str().ok())
				.and_then(|s| s.parse::<u64>().ok())
				.unwrap_or(DEFAULT_RETRY_AFTER_SECS);
			return Err(ChangeDetectionError::RateLimited { retry_after_secs });
		}

		if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
			let body = response.text().await.unwrap_or_default();
			return Err(ChangeDetectionError::Auth(format!(
				"graph {status}: {body}"
			)));
		}

		if !status.is_success() {
			let body = response.text().await.unwrap_or_default();
			return Err(ChangeDetectionError::Other(format!(
				"graph {status}: {body}"
			)));
		}

		let body = response.text().await?;
		let parsed: DeltaPayload = serde_json::from_str(&body)?;
		Ok(parsed)
	}

	/// Extract `token=...` from an `@odata.nextLink` or `@odata.deltaLink`.
	///
	/// Graph may respond with fully-qualified links; persisting only the
	/// token lets the indexer reconstruct the URL deterministically
	/// regardless of regional endpoint differences.
	fn extract_token(link: &str) -> Option<String> {
		let (_, after_q) = link.split_once('?')?;
		for pair in after_q.split('&') {
			if let Some(value) = pair.strip_prefix("token=") {
				return Some(value.to_string());
			}
		}
		None
	}

	fn delta_url_from_token(&self, token: &str) -> String {
		format!("{}/me/drive/root/delta?token={token}", self.graph_base_url)
	}
}

#[async_trait]
impl ChangeDetector for OneDriveChangeDetector {
	fn provider_id(&self) -> &'static str {
		"onedrive"
	}

	async fn initial_token(&self) -> Result<ChangeToken, ChangeDetectionError> {
		// `token=latest` asks Graph for a delta cursor pointing at the
		// current drive state without streaming the whole inventory. The
		// endpoint still returns a page (often empty) and ends with
		// `@odata.deltaLink` we can persist as a baseline.
		let mut url = self.delta_url_from_token("latest");
		loop {
			let payload = self.fetch_delta(&url).await?;
			if let Some(delta_link) = payload.delta_link {
				return Self::extract_token(&delta_link)
					.map(ChangeToken)
					.ok_or_else(|| {
						ChangeDetectionError::Other(format!(
							"delta link missing token parameter: {delta_link}"
						))
					});
			}
			let Some(next_link) = payload.next_link else {
				// No deltaLink and no nextLink means Graph returned an
				// unexpected page shape. Fail loudly rather than silently
				// treat it as "no token".
				return Err(ChangeDetectionError::Other(
					"delta response missing both next and delta links".to_string(),
				));
			};
			url = next_link;
		}
	}

	async fn changes_since(
		&self,
		token: &ChangeToken,
	) -> Result<ChangesPage, ChangeDetectionError> {
		let url = self.delta_url_from_token(token.as_str());
		let payload = self.fetch_delta(&url).await?;

		let entries = payload
			.value
			.into_iter()
			.filter_map(|item| map_drive_item(item))
			.collect::<Vec<_>>();

		let next_token = payload
			.next_link
			.as_deref()
			.and_then(Self::extract_token)
			.map(ChangeToken);
		let end_token = payload
			.delta_link
			.as_deref()
			.and_then(Self::extract_token)
			.map(ChangeToken);

		Ok(ChangesPage {
			entries,
			next_token,
			end_token,
		})
	}
}

/// Minimal projection of a Graph delta response we care about.
#[derive(Debug, Deserialize)]
struct DeltaPayload {
	#[serde(default)]
	value: Vec<DriveItem>,
	#[serde(rename = "@odata.nextLink")]
	#[serde(default)]
	next_link: Option<String>,
	#[serde(rename = "@odata.deltaLink")]
	#[serde(default)]
	delta_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveItem {
	id: String,
	#[serde(default)]
	name: Option<String>,
	#[serde(rename = "eTag", default)]
	etag: Option<String>,
	#[serde(rename = "lastModifiedDateTime", default)]
	last_modified: Option<String>,
	#[serde(default)]
	size: Option<u64>,
	#[serde(rename = "parentReference", default)]
	parent_reference: Option<ParentReference>,
	#[serde(default)]
	folder: Option<FolderFacet>,
	#[serde(default)]
	file: Option<FileFacet>,
	#[serde(default)]
	deleted: Option<DeletedFacet>,
}

#[derive(Debug, Deserialize)]
struct ParentReference {
	#[serde(default)]
	path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FolderFacet {
	// `folder` facet presence is sufficient; the `childCount` field is not
	// consumed here.
}

#[derive(Debug, Deserialize)]
struct FileFacet {}

#[derive(Debug, Deserialize)]
struct DeletedFacet {
	#[serde(default)]
	#[allow(dead_code)]
	state: Option<String>,
}

/// Translate a single Graph `driveItem` into the provider-agnostic
/// [`ChangeEntry`]. Returns `None` when the item is the drive root itself
/// (Graph emits a stub with no parent reference) — the indexer already owns
/// the root entry record.
fn map_drive_item(item: DriveItem) -> Option<ChangeEntry> {
	let is_folder = item.folder.is_some();
	let is_deleted = item.deleted.is_some();

	// Root item has no name on its delta entry; skip it so we never try to
	// materialize a "" path into the entries table.
	let name = item.name.clone().unwrap_or_default();

	let parent_path = item
		.parent_reference
		.as_ref()
		.and_then(|p| p.path.as_deref())
		.map(normalize_parent_path)
		.unwrap_or_default();

	let path = if parent_path.is_empty() && name.is_empty() {
		// Delta root stub — ignore.
		return None;
	} else if parent_path.is_empty() {
		// Item lives at drive root.
		format!("/{name}")
	} else if name.is_empty() {
		parent_path
	} else {
		format!("{parent_path}/{name}")
	};

	let last_modified = item
		.last_modified
		.as_deref()
		.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
		.map(|dt| dt.with_timezone(&Utc));

	// Size on folders is present in Graph but is an aggregate of descendants;
	// callers treating it as "file size" would be wrong, so we drop it for
	// folders.
	let size = if is_folder { None } else { item.size };

	let kind = if is_deleted {
		ChangeKind::Deleted
	} else if item.file.is_some() {
		// Rename detection requires knowing the previous path. The MVP does
		// not yet carry that info from the entry cache into the detector, so
		// non-deleted updates collapse to `Modified`. Added-vs-modified is
		// the indexer's job (it looks up by `provider_file_id`).
		ChangeKind::Modified
	} else if is_folder {
		ChangeKind::Modified
	} else {
		ChangeKind::Modified
	};

	Some(ChangeEntry {
		provider_file_id: item.id,
		path,
		kind,
		etag: item.etag,
		last_modified,
		size,
		is_folder,
	})
}

/// Strip OneDrive's `/drive/root:` prefix from `parentReference.path`.
///
/// Graph returns paths like `"/drive/root:/Documents/Reports"` or
/// `"/drives/b!.../root:"` for shared drives. We want a plain
/// `"/Documents/Reports"` so the indexer can join with a name to produce a
/// consistent full path.
fn normalize_parent_path(path: &str) -> String {
	// The prefix always ends at `:` whether it is `/drive/root:` or a
	// drive-specific variant — splitting at the first colon isolates the
	// trailing path.
	if let Some(idx) = path.find(':') {
		let tail = &path[idx + 1..];
		if tail.is_empty() {
			return String::new();
		}
		return tail.to_string();
	}
	path.to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use wiremock::matchers::{header, method, path};
	use wiremock::{Mock, MockServer, ResponseTemplate};

	fn make_detector(server: &MockServer) -> OneDriveChangeDetector {
		OneDriveChangeDetector::new_with_base_url(
			server.uri(),
			Arc::new(RwLock::new("test-token".to_string())),
		)
	}

	#[tokio::test]
	async fn test_initial_token_single_page_returns_delta_token() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.and(header("authorization", "Bearer test-token"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [],
				"@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=baseline-1"
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let token = detector
			.initial_token()
			.await
			.expect("initial_token should succeed");
		assert_eq!(token.as_str(), "baseline-1");
	}

	#[tokio::test]
	async fn test_initial_token_paginates_through_next_link() {
		let server = MockServer::start().await;
		// First call (token=latest) returns a nextLink.
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.and(wiremock::matchers::query_param("token", "latest"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [],
				"@odata.nextLink": format!("{}/me/drive/root/delta?token=page-2", server.uri())
			})))
			.mount(&server)
			.await;
		// Second call (token=page-2) returns the deltaLink.
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.and(wiremock::matchers::query_param("token", "page-2"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [],
				"@odata.deltaLink": format!("{}/me/drive/root/delta?token=final-baseline", server.uri())
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let token = detector.initial_token().await.unwrap();
		assert_eq!(token.as_str(), "final-baseline");
	}

	#[tokio::test]
	async fn test_changes_since_happy_path_returns_delta_token() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.and(wiremock::matchers::query_param("token", "cur"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [
					{
						"id": "abc",
						"name": "report.pdf",
						"eTag": "etag-1",
						"lastModifiedDateTime": "2026-04-18T12:00:00Z",
						"size": 2048,
						"file": {},
						"parentReference": { "path": "/drive/root:/Documents" }
					}
				],
				"@odata.deltaLink": format!("{}/delta?token=next-baseline", server.uri())
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let page = detector
			.changes_since(&ChangeToken("cur".into()))
			.await
			.unwrap();
		assert_eq!(page.entries.len(), 1);
		let entry = &page.entries[0];
		assert_eq!(entry.provider_file_id, "abc");
		assert_eq!(entry.path, "/Documents/report.pdf");
		assert_eq!(entry.kind, ChangeKind::Modified);
		assert_eq!(entry.etag.as_deref(), Some("etag-1"));
		assert_eq!(entry.size, Some(2048));
		assert!(!entry.is_folder);
		assert!(page.next_token.is_none());
		assert_eq!(
			page.end_token.as_ref().map(|t| t.as_str()),
			Some("next-baseline")
		);
	}

	#[tokio::test]
	async fn test_changes_since_next_page_continuation() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.and(wiremock::matchers::query_param("token", "cur"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [],
				"@odata.nextLink": format!("{}/me/drive/root/delta?token=cur-page-2", server.uri())
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let page = detector
			.changes_since(&ChangeToken("cur".into()))
			.await
			.unwrap();
		assert_eq!(
			page.next_token.as_ref().map(|t| t.as_str()),
			Some("cur-page-2")
		);
		assert!(page.end_token.is_none());
	}

	#[tokio::test]
	async fn test_changes_since_maps_deleted_entries() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [
					{
						"id": "del-1",
						"name": "gone.txt",
						"deleted": { "state": "deleted" },
						"parentReference": { "path": "/drive/root:/Trash" }
					}
				],
				"@odata.deltaLink": format!("{}/delta?token=end", server.uri())
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let page = detector
			.changes_since(&ChangeToken("cur".into()))
			.await
			.unwrap();
		assert_eq!(page.entries.len(), 1);
		assert_eq!(page.entries[0].kind, ChangeKind::Deleted);
	}

	#[tokio::test]
	async fn test_changes_since_maps_folder_vs_file() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
				"value": [
					{
						"id": "f-1",
						"name": "Reports",
						"folder": { "childCount": 3 },
						"parentReference": { "path": "/drive/root:/Documents" }
					},
					{
						"id": "f-2",
						"name": "notes.md",
						"size": 512,
						"file": {},
						"parentReference": { "path": "/drive/root:/Documents" }
					}
				],
				"@odata.deltaLink": format!("{}/delta?token=end", server.uri())
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let page = detector
			.changes_since(&ChangeToken("cur".into()))
			.await
			.unwrap();
		assert_eq!(page.entries.len(), 2);
		assert!(page.entries[0].is_folder);
		// Folder size must not propagate — Graph reports aggregate size which
		// would mislead size-based indexing heuristics.
		assert!(page.entries[0].size.is_none());
		assert!(!page.entries[1].is_folder);
		assert_eq!(page.entries[1].size, Some(512));
	}

	#[tokio::test]
	async fn test_changes_since_410_resync_required_returns_invalidated() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.respond_with(ResponseTemplate::new(410).set_body_json(serde_json::json!({
				"error": { "code": "resyncRequired", "message": "Token expired" }
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let err = detector
			.changes_since(&ChangeToken("stale".into()))
			.await
			.expect_err("410 must map to Invalidated");
		assert!(matches!(err, ChangeDetectionError::Invalidated));
	}

	#[tokio::test]
	async fn test_changes_since_429_parses_retry_after() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.respond_with(
				ResponseTemplate::new(429)
					.insert_header("retry-after", "17")
					.set_body_string("Too Many Requests"),
			)
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let err = detector
			.changes_since(&ChangeToken("cur".into()))
			.await
			.expect_err("429 must map to RateLimited");
		match err {
			ChangeDetectionError::RateLimited { retry_after_secs } => {
				assert_eq!(retry_after_secs, 17);
			}
			other => panic!("expected RateLimited, got {other:?}"),
		}
	}

	#[tokio::test]
	async fn test_changes_since_401_returns_auth_error() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/me/drive/root/delta"))
			.respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
				"error": { "code": "InvalidAuthenticationToken" }
			})))
			.mount(&server)
			.await;

		let detector = make_detector(&server);
		let err = detector
			.changes_since(&ChangeToken("cur".into()))
			.await
			.expect_err("401 must map to Auth");
		assert!(matches!(err, ChangeDetectionError::Auth(_)));
	}

	#[tokio::test]
	async fn test_path_normalization_strips_root_prefix() {
		assert_eq!(
			normalize_parent_path("/drive/root:/Documents/Reports"),
			"/Documents/Reports"
		);
		assert_eq!(normalize_parent_path("/drive/root:"), "");
		assert_eq!(normalize_parent_path("/drives/b!abc/root:/Work"), "/Work");
		// No colon at all → caller passes raw path, we return it unchanged.
		assert_eq!(
			normalize_parent_path("/already/normalized"),
			"/already/normalized"
		);
	}

	#[tokio::test]
	async fn test_extract_token_handles_missing_token() {
		assert_eq!(
			OneDriveChangeDetector::extract_token("https://graph.microsoft.com/delta?token=abc"),
			Some("abc".to_string())
		);
		assert_eq!(
			OneDriveChangeDetector::extract_token(
				"https://graph.microsoft.com/delta?token=abc&foo=bar"
			),
			Some("abc".to_string())
		);
		// Token param missing → None so caller surfaces a clear error.
		assert_eq!(
			OneDriveChangeDetector::extract_token("https://graph.microsoft.com/delta?foo=bar"),
			None
		);
	}
}
