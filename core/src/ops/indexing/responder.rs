//! Persistent location responder.
//!
//! Thin wrapper over `PersistentChangeHandler` that translates raw filesystem
//! events into database mutations. The watcher calls `apply_batch` with events;
//! this module delegates to the unified change handling infrastructure.

use crate::context::CoreContext;
use crate::infra::db::entities;
use crate::infra::event::FsRawEventKind;
use crate::ops::indexing::change_detection::{self, ChangeConfig, PersistentChangeHandler};
use crate::ops::indexing::rules::RuleToggles;
use anyhow::Result;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

/// Translates a single filesystem event into database mutations.
///
/// Creates a `PersistentChangeHandler` and delegates to the unified change
/// handling infrastructure in `change_detection`.
pub async fn apply(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	kind: FsRawEventKind,
	rule_toggles: RuleToggles,
	location_root: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	apply_batch(
		context,
		library_id,
		location_id,
		vec![kind],
		rule_toggles,
		location_root,
		volume_backend,
	)
	.await
}

/// Processes multiple filesystem events as a batch.
///
/// Creates a `PersistentChangeHandler` and delegates to the unified
/// `change_detection::apply_batch` which handles deduplication, ordering,
/// and correct processing sequence (removes, renames, creates, modifies).
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	events: Vec<FsRawEventKind>,
	rule_toggles: RuleToggles,
	location_root: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	if events.is_empty() {
		return Ok(());
	}

	tracing::debug!(
		"Responder received batch of {} events for location {}",
		events.len(),
		location_id
	);

	let mut handler = PersistentChangeHandler::new(
		context.clone(),
		library_id,
		location_id,
		location_root,
		volume_backend.cloned(),
	)
	.await?;

	let config = ChangeConfig {
		rule_toggles,
		location_root,
		volume_backend,
	};

	change_detection::apply_batch(&mut handler, events, &config).await
}

// ============================================================================
// Subtree Deletion Utilities
// ============================================================================
// These functions are used by sync and entity deletion code paths.
// They operate directly on the database without going through ChangeHandler.

/// Deletes an entry tree without creating tombstones.
///
/// Used when applying remote tombstones (the deletion was already synced,
/// we're just applying it locally). Also used by entity cascade deletes.
pub async fn delete_subtree_internal(
	entry_id: i32,
	db: &sea_orm::DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
	let txn = db.begin().await?;
	delete_subtree_no_txn(entry_id, &txn).await?;
	txn.commit().await?;
	Ok(())
}

/// Deletes a subtree within an existing transaction.
///
/// Traverses via entry_closure to find all descendants, then deletes
/// closure links, directory_paths, and entries in the correct order.
async fn delete_subtree_no_txn<C>(entry_id: i32, db: &C) -> Result<(), sea_orm::DbErr>
where
	C: sea_orm::ConnectionTrait,
{
	// Collect all descendants via closure table
	let mut to_delete_ids: Vec<i32> = vec![entry_id];
	if let Ok(rows) = entities::entry_closure::Entity::find()
		.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
		.all(db)
		.await
	{
		to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
	}
	to_delete_ids.sort_unstable();
	to_delete_ids.dedup();

	if !to_delete_ids.is_empty() {
		// Delete closure links (both directions)
		let _ = entities::entry_closure::Entity::delete_many()
			.filter(entities::entry_closure::Column::DescendantId.is_in(to_delete_ids.clone()))
			.exec(db)
			.await;
		let _ = entities::entry_closure::Entity::delete_many()
			.filter(entities::entry_closure::Column::AncestorId.is_in(to_delete_ids.clone()))
			.exec(db)
			.await;

		// Delete directory paths
		let _ = entities::directory_paths::Entity::delete_many()
			.filter(entities::directory_paths::Column::EntryId.is_in(to_delete_ids.clone()))
			.exec(db)
			.await;

		// Delete entries
		let _ = entities::entry::Entity::delete_many()
			.filter(entities::entry::Column::Id.is_in(to_delete_ids))
			.exec(db)
			.await;
	}

	Ok(())
}
