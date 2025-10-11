//! Sync applier (STUB - Being replaced with PeerSync)
//!
//! This module handled applying sync log entries from the leader.
//! In the new leaderless architecture, this logic is in PeerSync.

use anyhow::Result;
use tracing::warn;

/// Sync applier (DEPRECATED)
///
/// Stubbed during migration to leaderless architecture.
pub struct SyncApplier;

impl SyncApplier {
	/// Create a new sync applier (stub)
	pub fn new() -> Self {
		warn!("SyncApplier is deprecated - use PeerSync instead");
		Self
	}

	/// Apply sync entry (stub)
	pub async fn apply(&self, _entry: serde_json::Value) -> Result<()> {
		warn!("SyncApplier::apply called but deprecated");
		Ok(())
	}
}

impl Default for SyncApplier {
	fn default() -> Self {
		Self::new()
	}
}
