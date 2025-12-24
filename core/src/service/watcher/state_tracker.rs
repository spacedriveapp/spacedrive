//! Watcher State Tracker
//!
//! Tracks filesystem watcher lifecycle in the database for stale detection decisions.
//! Records when watching started/stopped, last successful event, and interruption status.

use crate::infra::db::entities::location_watcher_state;
use crate::library::Library;
use anyhow::Result;
use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use std::sync::Arc;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Tracks watcher state for stale detection decisions
pub struct WatcherStateTracker {
	library: Arc<Library>,
}

impl WatcherStateTracker {
	/// Create a new watcher state tracker for a library
	pub fn new(library: Arc<Library>) -> Self {
		Self { library }
	}

	/// Record that watching has started for a location
	pub async fn record_watch_start(&self, location_id: Uuid) -> Result<()> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id, db).await?;

		// Check if state exists
		let existing = location_watcher_state::Entity::find()
			.filter(location_watcher_state::Column::LocationId.eq(db_id))
			.one(db)
			.await?;

		let now = chrono::Utc::now().into();

		if let Some(existing) = existing {
			// Update existing state
			let mut active: location_watcher_state::ActiveModel = existing.into();
			active.last_watch_start = Set(Some(now));
			active.watch_interrupted = Set(false); // Clear interrupted flag
			active.updated_at = Set(now);
			active.update(db).await?;
		} else {
			// Create new state
			let active = location_watcher_state::ActiveModel {
				location_id: Set(db_id),
				last_watch_start: Set(Some(now)),
				last_watch_stop: Set(None),
				last_successful_event: Set(None),
				watch_interrupted: Set(false),
				updated_at: Set(now),
			};
			active.insert(db).await?;
		}

		debug!(
			location_id = %location_id,
			"Recorded watch start"
		);
		Ok(())
	}

	/// Record that watching has stopped for a location (clean shutdown)
	pub async fn record_watch_stop(&self, location_id: Uuid) -> Result<()> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id, db).await?;

		let existing = location_watcher_state::Entity::find()
			.filter(location_watcher_state::Column::LocationId.eq(db_id))
			.one(db)
			.await?;

		if let Some(existing) = existing {
			let now = chrono::Utc::now().into();
			let mut active: location_watcher_state::ActiveModel = existing.into();
			active.last_watch_stop = Set(Some(now));
			active.watch_interrupted = Set(false); // Clean shutdown
			active.updated_at = Set(now);
			active.update(db).await?;

			debug!(
				location_id = %location_id,
				"Recorded clean watch stop"
			);
		}

		Ok(())
	}

	/// Record that a successful event was processed for a location
	pub async fn record_successful_event(&self, location_id: Uuid) -> Result<()> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id, db).await?;

		let existing = location_watcher_state::Entity::find()
			.filter(location_watcher_state::Column::LocationId.eq(db_id))
			.one(db)
			.await?;

		if let Some(existing) = existing {
			let now = chrono::Utc::now().into();
			let mut active: location_watcher_state::ActiveModel = existing.into();
			active.last_successful_event = Set(Some(now));
			active.updated_at = Set(now);
			active.update(db).await?;
		}

		Ok(())
	}

	/// Mark all active watches as interrupted (called on startup to detect crashes)
	///
	/// Finds all locations that have `last_watch_start > last_watch_stop` and marks them
	/// as interrupted. This indicates the watcher was running when the app crashed.
	pub async fn mark_interrupted_on_startup(&self) -> Result<usize> {
		let db = self.library.db().conn();
		let mut count = 0;

		// Find all states where watch was started but not cleanly stopped
		let states = location_watcher_state::Entity::find().all(db).await?;

		for state in states {
			let was_watching = match (state.last_watch_start, state.last_watch_stop) {
				(Some(start), Some(stop)) => start > stop,
				(Some(_), None) => true, // Started but never stopped
				_ => false,
			};

			if was_watching && !state.watch_interrupted {
				let mut active: location_watcher_state::ActiveModel = state.into();
				active.watch_interrupted = Set(true);
				active.updated_at = Set(chrono::Utc::now().into());
				active.update(db).await?;
				count += 1;
			}
		}

		if count > 0 {
			warn!(
				count = count,
				"Marked locations as interrupted from previous crash"
			);
		}

		Ok(count)
	}

	/// Check if a location needs stale detection based on watcher state
	pub async fn should_detect_stale(
		&self,
		location_id: Uuid,
		threshold_secs: i64,
	) -> Result<bool> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id, db).await?;

		let state = location_watcher_state::Entity::find()
			.filter(location_watcher_state::Column::LocationId.eq(db_id))
			.one(db)
			.await?;

		match state {
			Some(state) => Ok(state.should_detect_stale(threshold_secs)),
			None => {
				// No state means never watched before - full index needed, not stale detection
				Ok(false)
			}
		}
	}

	/// Get location database ID from UUID
	async fn get_location_db_id(
		&self,
		location_id: Uuid,
		db: &DatabaseConnection,
	) -> Result<i32> {
		use crate::infra::db::entities::location;

		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(location_id))
			.one(db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;
		Ok(loc.id)
	}

	/// Initialize watcher state for a new location (creates with defaults)
	pub async fn initialize_for_location(&self, location_id: Uuid) -> Result<()> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id, db).await?;

		// Check if state already exists
		let existing = location_watcher_state::Entity::find()
			.filter(location_watcher_state::Column::LocationId.eq(db_id))
			.one(db)
			.await?;

		if existing.is_none() {
			let active = location_watcher_state::ActiveModel {
				location_id: Set(db_id),
				last_watch_start: Set(None),
				last_watch_stop: Set(None),
				last_successful_event: Set(None),
				watch_interrupted: Set(false),
				updated_at: Set(chrono::Utc::now().into()),
			};
			active.insert(db).await?;

			debug!(
				location_id = %location_id,
				"Initialized watcher state"
			);
		}

		Ok(())
	}

	/// Delete watcher state when location is removed
	pub async fn delete_for_location(&self, location_id: Uuid) -> Result<()> {
		let db = self.library.db().conn();
		let db_id = self.get_location_db_id(location_id, db).await?;

		location_watcher_state::Entity::delete_by_id(db_id)
			.exec(db)
			.await?;

		debug!(
			location_id = %location_id,
			"Deleted watcher state"
		);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Note: Full tests require database setup, tested in integration tests
}
