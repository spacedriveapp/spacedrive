//! Sync entry applier
//!
//! Uses the syncable model registry to automatically dispatch to the correct
//! model's apply_sync_entry implementation. No central switch statement needed!

use crate::infra::sync::{BulkOperationMetadata, SyncLogEntry};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Applies sync entries to the local database
pub struct SyncApplier {
	library_id: Uuid,
	db: Arc<crate::infra::db::Database>,
}

impl SyncApplier {
	/// Create a new sync applier
	pub fn new_with_deps(library_id: Uuid, db: Arc<crate::infra::db::Database>) -> Self {
		Self { library_id, db }
	}

	/// Apply a sync entry to the local database
	///
	/// Uses the syncable model registry for automatic dispatch.
	/// No need to modify this code when adding new syncable models!
	pub async fn apply_entry(&self, entry: &SyncLogEntry) -> Result<()> {
		debug!(
			library_id = %self.library_id,
			sequence = entry.sequence,
			model_type = %entry.model_type,
			record_id = %entry.record_id,
			change_type = ?entry.change_type,
			"Applying sync entry"
		);

		// Handle bulk operations specially
		if entry.model_type == "bulk_operation" {
			return self.handle_bulk_operation(entry).await;
		}

		// Use registry to dispatch to the correct model's apply function
		crate::infra::sync::registry::apply_sync_entry(entry, self.db.conn())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to apply sync entry: {}", e))
	}

	/// Handle bulk operation metadata
	async fn handle_bulk_operation(&self, entry: &SyncLogEntry) -> Result<()> {
		let metadata: BulkOperationMetadata = serde_json::from_value(entry.data.clone())?;

		info!(
			library_id = %self.library_id,
			operation = ?metadata.operation,
			affected_count = metadata.affected_count,
			"Processing bulk operation from leader"
		);

		// Bulk operations are metadata-only - we don't replicate the actual entries
		// Instead, we may trigger our own local jobs if appropriate
		// For example, if leader indexed a location, we might want to index it too

		// TODO: Implement bulk operation handling when needed
		// For now, just log that we saw it
		info!("Bulk operation noted, no local action taken yet");

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_applier_creation() {
		// Applier tests will be integration tests requiring full library setup
		// For now, just verify compilation
	}
}
