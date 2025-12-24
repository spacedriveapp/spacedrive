//! Stale detection runs entity
//!
//! Tracks history of stale detection runs for monitoring and debugging.
//! Records trigger reason, job reference, stats, and status.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Trigger reasons for stale detection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaleDetectionTrigger {
	/// Run on application startup after offline period
	Startup,
	/// Periodic check based on configured interval
	Periodic,
	/// Manually triggered by user via UI
	Manual,
	/// Triggered when offline duration exceeds threshold
	OfflineThreshold,
	/// Triggered when watch was interrupted (crash recovery)
	WatchInterrupted,
}

impl std::fmt::Display for StaleDetectionTrigger {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Startup => write!(f, "startup"),
			Self::Periodic => write!(f, "periodic"),
			Self::Manual => write!(f, "manual"),
			Self::OfflineThreshold => write!(f, "offline_threshold"),
			Self::WatchInterrupted => write!(f, "watch_interrupted"),
		}
	}
}

impl From<String> for StaleDetectionTrigger {
	fn from(s: String) -> Self {
		match s.as_str() {
			"startup" => Self::Startup,
			"periodic" => Self::Periodic,
			"manual" => Self::Manual,
			"offline_threshold" => Self::OfflineThreshold,
			"watch_interrupted" => Self::WatchInterrupted,
			_ => Self::Manual, // Default fallback
		}
	}
}

/// Status of a stale detection run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
	Running,
	Completed,
	Failed,
	Cancelled,
}

impl std::fmt::Display for RunStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Running => write!(f, "running"),
			Self::Completed => write!(f, "completed"),
			Self::Failed => write!(f, "failed"),
			Self::Cancelled => write!(f, "cancelled"),
		}
	}
}

impl From<String> for RunStatus {
	fn from(s: String) -> Self {
		match s.as_str() {
			"running" => Self::Running,
			"completed" => Self::Completed,
			"failed" => Self::Failed,
			"cancelled" => Self::Cancelled,
			_ => Self::Failed,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stale_detection_runs")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	pub location_id: i32,

	/// Reference to the IndexerJob that performed the detection
	pub job_id: String,

	/// What triggered this detection run
	pub triggered_by: String, // StaleDetectionTrigger as string

	pub started_at: DateTimeUtc,
	pub completed_at: Option<DateTimeUtc>,

	/// Current status
	pub status: String, // RunStatus as string

	/// Directories pruned via mtime comparison (skipped unchanged branches)
	pub directories_pruned: i32,

	/// Directories actually scanned (changed branches)
	pub directories_scanned: i32,

	/// Number of changes detected (new, modified, deleted files)
	pub changes_detected: i32,

	/// Error message if status is "failed"
	pub error_message: Option<String>,
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
	/// Create a new running stale detection record
	pub fn new_running(location_id: i32, job_id: String, trigger: StaleDetectionTrigger) -> Self {
		Self {
			id: 0, // Auto-increment
			location_id,
			job_id,
			triggered_by: trigger.to_string(),
			started_at: chrono::Utc::now().into(),
			completed_at: None,
			status: RunStatus::Running.to_string(),
			directories_pruned: 0,
			directories_scanned: 0,
			changes_detected: 0,
			error_message: None,
		}
	}

	/// Get the trigger type as enum
	pub fn trigger(&self) -> StaleDetectionTrigger {
		StaleDetectionTrigger::from(self.triggered_by.clone())
	}

	/// Get the status as enum
	pub fn run_status(&self) -> RunStatus {
		RunStatus::from(self.status.clone())
	}

	/// Calculate pruning efficiency as percentage (0.0 - 100.0)
	pub fn pruning_efficiency(&self) -> f64 {
		let total = self.directories_pruned + self.directories_scanned;
		if total == 0 {
			0.0
		} else {
			(self.directories_pruned as f64 / total as f64) * 100.0
		}
	}

	/// Calculate run duration
	pub fn duration(&self) -> Option<chrono::Duration> {
		self.completed_at
			.map(|end| chrono::DateTime::<chrono::Utc>::from(end) - chrono::DateTime::<chrono::Utc>::from(self.started_at))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_trigger_roundtrip() {
		assert_eq!(
			StaleDetectionTrigger::from("startup".to_string()),
			StaleDetectionTrigger::Startup
		);
		assert_eq!(
			StaleDetectionTrigger::from("periodic".to_string()),
			StaleDetectionTrigger::Periodic
		);
		assert_eq!(
			StaleDetectionTrigger::from("manual".to_string()),
			StaleDetectionTrigger::Manual
		);
	}

	#[test]
	fn test_pruning_efficiency() {
		let mut run = Model::new_running(1, "job-1".to_string(), StaleDetectionTrigger::Manual);
		run.directories_pruned = 90;
		run.directories_scanned = 10;
		assert!((run.pruning_efficiency() - 90.0).abs() < 0.001);
	}

	#[test]
	fn test_pruning_efficiency_zero() {
		let run = Model::new_running(1, "job-1".to_string(), StaleDetectionTrigger::Manual);
		assert_eq!(run.pruning_efficiency(), 0.0);
	}
}
