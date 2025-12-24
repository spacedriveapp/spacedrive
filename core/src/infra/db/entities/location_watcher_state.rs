//! Location watcher state entity
//!
//! Tracks filesystem watcher lifecycle for stale detection decisions.
//! Records when watching started/stopped and whether it was interrupted.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "location_watcher_state")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub location_id: i32,

	/// When watching was last started for this location
	pub last_watch_start: Option<DateTimeUtc>,

	/// When watching was last stopped (cleanly or crash)
	pub last_watch_stop: Option<DateTimeUtc>,

	/// When the last filesystem event was successfully processed
	pub last_successful_event: Option<DateTimeUtc>,

	/// True if the watcher was interrupted (crash, force-quit) rather than clean shutdown
	/// Used to trigger stale detection on next startup
	pub watch_interrupted: bool,

	/// Updated on any state change
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::location::Entity",
		from = "Column::LocationId",
		to = "super::location::Column::Id",
		on_delete = "Cascade"
	)]
	Location,
}

impl Related<super::location::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Location.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
	/// Create initial state for a new location
	pub fn new_for_location(location_id: i32) -> Self {
		Self {
			location_id,
			last_watch_start: None,
			last_watch_stop: None,
			last_successful_event: None,
			watch_interrupted: false,
			updated_at: chrono::Utc::now().into(),
		}
	}

	/// Calculate how long the location has been offline (not watched)
	pub fn offline_duration(&self) -> Option<chrono::Duration> {
		self.last_watch_stop
			.map(|stop| chrono::Utc::now().signed_duration_since(stop))
	}

	/// Check if stale detection should run based on offline duration
	pub fn should_detect_stale(&self, threshold_secs: i64) -> bool {
		// Always detect if watch was interrupted
		if self.watch_interrupted {
			return true;
		}

		// Detect if offline for longer than threshold
		if let Some(duration) = self.offline_duration() {
			return duration.num_seconds() > threshold_secs;
		}

		// Never watched before, needs full index not stale detection
		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_should_detect_stale_when_interrupted() {
		let mut state = Model::new_for_location(1);
		state.watch_interrupted = true;
		assert!(state.should_detect_stale(3600));
	}

	#[test]
	fn test_should_not_detect_stale_when_never_watched() {
		let state = Model::new_for_location(1);
		// Never watched, needs full index
		assert!(!state.should_detect_stale(3600));
	}

	#[test]
	fn test_should_detect_stale_based_on_offline_duration() {
		let mut state = Model::new_for_location(1);
		// Set last_watch_stop to 2 hours ago
		let two_hours_ago = chrono::Utc::now() - chrono::Duration::hours(2);
		state.last_watch_stop = Some(two_hours_ago.into());

		// 1 hour threshold - should detect
		assert!(state.should_detect_stale(3600));

		// 3 hour threshold - should not detect
		assert!(!state.should_detect_stale(3 * 3600));
	}
}
