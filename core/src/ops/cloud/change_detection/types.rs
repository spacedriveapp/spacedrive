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
	/// Change records described by the provider on this page.
	pub entries: Vec<ChangeEntry>,
	/// Present when more pages remain in this scan. Drive the next fetch with
	/// this token and keep looping until it is `None`.
	pub next_token: Option<ChangeToken>,
	/// Present when the provider reports "no more changes right now". Persist
	/// as the baseline for the next incremental pass.
	pub end_token: Option<ChangeToken>,
}

/// A single change surfaced by the provider.
///
/// `path` is always the full cloud-side path from the drive root with the
/// provider-specific `/drive/root:/` prefix stripped so the indexer sees a
/// consistent slash-rooted path across vendors.
#[derive(Debug, Clone)]
pub struct ChangeEntry {
	/// Provider-native stable id for the item. Persisted to
	/// `entries.provider_file_id`, which is what makes rename/move tracking
	/// loss-free.
	pub provider_file_id: String,
	/// Full cloud path from the drive root, slash-separated.
	pub path: String,
	/// Classification — `Added`, `Modified`, `Deleted`, or `Renamed`.
	pub kind: ChangeKind,
	/// Provider-supplied etag. Opaque; the indexer treats it as a validator
	/// only, never as a content hash.
	pub etag: Option<String>,
	/// Last modification timestamp from the provider.
	pub last_modified: Option<DateTime<Utc>>,
	/// File size in bytes. Absent for folders.
	pub size: Option<u64>,
	/// True for folder-typed items; used by the indexer to decide whether to
	/// recurse into the entry on initial sync.
	pub is_folder: bool,
}

/// What the provider says happened to an entry.
///
/// Rename detection currently degrades to `Modified` in the OneDrive detector
/// because we don't have the previous path on hand; the variant exists so the
/// indexer can treat renames specially once that history is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
	/// Entry appeared since the prior token.
	Added,
	/// Entry existed before and its content or metadata changed.
	Modified,
	/// Entry removed from the drive.
	Deleted,
	/// Entry moved or renamed. `from_path` carries the previous path when the
	/// provider surfaces it; otherwise the indexer must rely on
	/// `provider_file_id` matching to reconcile.
	Renamed { from_path: Option<String> },
}

/// Errors surfaced by a [`ChangeDetector`].
///
/// The variants are deliberately coarse so the scheduler can act on them
/// without parsing nested provider errors. `Transport` and `Parse` are the
/// automatic conversions; everything else is raised explicitly after the
/// detector has inspected a response.
#[derive(thiserror::Error, Debug)]
pub enum ChangeDetectionError {
	/// The provider told us the stored token is too old (HTTP 410
	/// `resyncRequired` on OneDrive). The caller must drop the token and run
	/// a full rescan.
	#[error("delta token invalidated (410 resyncRequired) — full resync required")]
	Invalidated,

	/// Provider rate-limited us. `retry_after_secs` comes from the
	/// `Retry-After` header when present; otherwise the detector picks a
	/// conservative default.
	#[error("rate limited (retry after {retry_after_secs}s)")]
	RateLimited {
		/// Seconds to wait before the next attempt.
		retry_after_secs: u64,
	},

	/// Bearer token rejected. The caller should not retry with the same
	/// credentials; the scheduler surfaces this to the refresh task or to the
	/// UI for manual re-auth.
	#[error("authentication failed: {0}")]
	Auth(String),

	/// Network-level failure.
	#[error("transport: {0}")]
	Transport(#[from] reqwest::Error),

	/// Response body did not match the expected schema.
	#[error("deserialization: {0}")]
	Parse(#[from] serde_json::Error),

	/// Any other unexpected state — malformed URLs, missing fields, logic
	/// errors. Carries a description so logs can pinpoint the call site.
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
	/// Stable provider id (matches `OauthProvider::id` where applicable).
	///
	/// Used by the indexer to label log lines and to match the detector
	/// against the `cloud_sync_state.provider` column.
	fn provider_id(&self) -> &'static str;

	/// Request a fresh baseline token, typically by draining the provider's
	/// delta endpoint with `token=latest` until a `@odata.deltaLink` appears.
	///
	/// Returned token must be safe to hand directly to
	/// [`ChangeDetector::changes_since`] on the next call.
	async fn initial_token(&self) -> Result<ChangeToken, ChangeDetectionError>;

	/// Fetch one page of changes since the given token.
	///
	/// The caller loops on this method: it drives `next_token` until the
	/// detector returns `end_token` instead. Persisting after each page is
	/// what makes an interrupted scan resumable — see
	/// `.investigations/cloud-drives/research/03-change-detection-and-sync.md`
	/// §6.
	async fn changes_since(&self, token: &ChangeToken)
		-> Result<ChangesPage, ChangeDetectionError>;
}
