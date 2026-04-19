//! Provider-agnostic types for cloud change detection.
//!
//! Split into its own module so that concrete detectors, the repository, and
//! the indexer can all import the shared surface without pulling in heavy
//! HTTP-client code.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Opaque token used to request the next batch of changes from a provider.
///
/// Wrapped in a newtype to prevent accidentally mixing up provider tokens with
/// arbitrary strings; the indexer round-trips it through
/// [`crate::ops::cloud::change_detection::repository::CloudSyncStateRepository`]
/// unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeToken(pub String);

impl ChangeToken {
	/// Borrow the inner string. Providers building HTTP query strings call
	/// this; no other caller should need it.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// One delta page from a provider.
///
/// `next_token` carries intra-scan continuation (`@odata.nextLink`): the caller
/// must keep paging until it is `None`. `end_token` marks the
/// baseline to pass to the next [`ChangeDetector::changes_since`] call once
/// the current scan is complete (`@odata.deltaLink`). Both are `Option`
/// because a single page may be the final page (sets `end_token`), a mid-scan
/// page (sets `next_token`), or both at once (an empty provider with no
/// pagination).
#[derive(Debug, Clone)]
pub struct ChangesPage {
	pub entries: Vec<ChangeEntry>,
	/// Intra-scan continuation; keep fetching until it is `None`.
	pub next_token: Option<ChangeToken>,
	/// End-of-scan baseline; persist as the starting point for the next pass.
	pub end_token: Option<ChangeToken>,
}

/// A single change surfaced by the provider.
///
/// `path` is always the full cloud-side path from the drive root with the
/// provider-specific `/drive/root:/` prefix stripped so the indexer sees a
/// consistent slash-rooted path across vendors.
#[derive(Debug, Clone)]
pub struct ChangeEntry {
	/// Provider-native stable id. Persisted to `entries.provider_file_id`
	/// so rename/move tracking survives path changes.
	pub provider_file_id: String,
	/// Full cloud path from the drive root, slash-separated.
	pub path: String,
	pub kind: ChangeKind,
	/// Opaque validator; never treated as a content hash.
	pub etag: Option<String>,
	pub last_modified: Option<DateTime<Utc>>,
	/// Absent for folders.
	pub size: Option<u64>,
	pub is_folder: bool,
}

/// What the provider says happened to an entry.
///
/// Rename detection currently degrades to `Modified` in the OneDrive detector
/// because we don't have the previous path on hand; the variant exists so the
/// indexer can treat renames specially once that history is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
	Added,
	Modified,
	Deleted,
	/// `from_path` is `None` when the provider omits the previous path;
	/// the indexer then reconciles via `provider_file_id` instead.
	Renamed {
		from_path: Option<String>,
	},
}

/// Errors surfaced by a [`ChangeDetector`].
///
/// The variants are deliberately coarse so the scheduler can act on them
/// without parsing nested provider errors. `Transport` and `Parse` are the
/// automatic conversions; everything else is raised explicitly after the
/// detector has inspected a response.
#[derive(thiserror::Error, Debug)]
pub enum ChangeDetectionError {
	/// Stored token is too old (OneDrive HTTP 410 `resyncRequired`).
	/// Caller must drop the token and run a full rescan.
	#[error("delta token invalidated (410 resyncRequired) — full resync required")]
	Invalidated,

	/// `retry_after_secs` comes from `Retry-After` when present; detectors
	/// fall back to a conservative default otherwise.
	#[error("rate limited (retry after {retry_after_secs}s)")]
	RateLimited { retry_after_secs: u64 },

	/// Bearer token rejected; do not retry with the same credentials.
	#[error("authentication failed: {0}")]
	Auth(String),

	#[error("transport: {0}")]
	Transport(#[from] reqwest::Error),

	#[error("deserialization: {0}")]
	Parse(#[from] serde_json::Error),

	#[error("{0}")]
	Other(String),
}

/// Abstraction over provider-native delta APIs.
///
/// The contract is deliberately pull-based: callers ask for the next page,
/// the detector fetches it, and the caller persists the returned token. This
/// keeps the detector stateless between calls (no background tasks, no cached
/// pages) so it is safe to drop and recreate on every pass.
#[async_trait]
pub trait ChangeDetector: Send + Sync {
	/// Stable provider id, matches `OauthProvider::id` and the
	/// `cloud_sync_state.provider` column.
	fn provider_id(&self) -> &'static str;

	/// Fetch a fresh baseline token (typically by draining the provider's
	/// delta endpoint with `token=latest` until `@odata.deltaLink` appears).
	async fn initial_token(&self) -> Result<ChangeToken, ChangeDetectionError>;

	/// Fetch one page of changes since `token`. Callers loop on
	/// `next_token` and stop when `end_token` is returned; persisting after
	/// each page is what makes the scan resumable.
	async fn changes_since(&self, token: &ChangeToken)
		-> Result<ChangesPage, ChangeDetectionError>;
}
