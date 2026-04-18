//! # Cloud change detection
//!
//! `core::ops::cloud::change_detection` turns provider-native delta APIs into
//! a uniform stream of [`ChangeEntry`] records the indexer can consume without
//! knowing which cloud it is talking to. The abstraction lives alongside the
//! OAuth infrastructure rather than inside the `indexing` pipeline because
//! change detection is inherently provider-coupled: OneDrive speaks Graph
//! `/delta`, Google Drive `changes.list`, Dropbox `list_folder/continue`.
//!
//! The MVP ships only [`OneDriveChangeDetector`]. Future providers implement
//! the same [`ChangeDetector`] trait and are plumbed through the indexer's
//! existing delta branch — no new infrastructure required.
//!
//! ## Key contracts
//! - [`ChangeToken`] is opaque to the caller. Detectors own its semantics and
//!   the caller only round-trips it through the repository.
//! - [`ChangesPage`] separates `next_token` (intra-scan continuation) from
//!   `end_token` (baseline for the next incremental call). Persisting the
//!   end_token between passes is what turns a full rescan into a real delta.
//! - Rate limiting (`RateLimited`) and token invalidation (`Invalidated`) are
//!   first-class error variants so the scheduler can respond without parsing
//!   string messages.

pub mod delta_pass;
pub mod onedrive;
pub mod repository;
mod types;

pub use delta_pass::{compute_backoff_secs, run_cloud_delta_pass, DeltaOutcome};
pub use onedrive::OneDriveChangeDetector;
pub use repository::{
	CloudSyncState, CloudSyncStateError, CloudSyncStateRepository, SeaOrmCloudSyncStateRepository,
};
pub use types::{
	ChangeDetectionError, ChangeDetector, ChangeEntry, ChangeKind, ChangeToken, ChangesPage,
};
