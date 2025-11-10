use crate::infra::query::{CoreQuery, QueryResult};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListEventsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EventInfo {
	/// The event variant name (e.g., "JobProgress", "LibraryCreated")
	pub variant: String,
	/// Whether this event is considered "noisy" (high frequency, should be excluded by default)
	pub is_noisy: bool,
	/// Human-readable description
	pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListEventsOutput {
	/// All available event types
	pub all_events: Vec<String>,
	/// Events that are high-frequency and should be excluded by default
	pub noisy_events: Vec<String>,
	/// Detailed information about each event
	pub event_info: Vec<EventInfo>,
}

pub struct ListEventsQuery;

impl CoreQuery for ListEventsQuery {
	type Input = ListEventsInput;
	type Output = ListEventsOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		_context: Arc<crate::context::CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Define all available event types
		// This list should match the Event enum in core/src/infra/event/mod.rs
		let all_events = vec![
			// Core lifecycle
			"CoreStarted",
			"CoreShutdown",
			// Library events
			"LibraryCreated",
			"LibraryOpened",
			"LibraryClosed",
			"LibraryDeleted",
			"LibraryStatisticsUpdated",
			// Entry events
			"EntryCreated",
			"EntryModified",
			"EntryDeleted",
			"EntryMoved",
			// Raw filesystem changes
			"FsRawChange",
			// Volume events
			"VolumeAdded",
			"VolumeRemoved",
			"VolumeUpdated",
			"VolumeSpeedTested",
			"VolumeMountChanged",
			"VolumeError",
			// Job events
			"JobQueued",
			"JobStarted",
			"JobProgress",
			"JobCompleted",
			"JobFailed",
			"JobCancelled",
			"JobPaused",
			"JobResumed",
			// Indexing events
			"IndexingStarted",
			"IndexingProgress",
			"IndexingCompleted",
			"IndexingFailed",
			// Device events
			"DeviceConnected",
			"DeviceDisconnected",
			// Resource events
			"ResourceChanged",
			"ResourceDeleted",
			// Legacy compatibility
			"LocationAdded",
			"LocationRemoved",
			"FilesIndexed",
			"ThumbnailsGenerated",
			"FileOperationCompleted",
			"FilesModified",
			// Log events
			"LogMessage",
			// Custom events
			"Custom",
		]
		.into_iter()
		.map(String::from)
		.collect();

		// Define noisy events (high-frequency, excluded by default)
		let noisy_events = vec![
			"LogMessage",      // Every log becomes an event
			"JobProgress",     // Sent frequently during job execution
			"IndexingProgress", // Sent frequently during indexing
		]
		.into_iter()
		.map(String::from)
		.collect();

		// Provide detailed information for each event
		let event_info = vec![
			EventInfo {
				variant: "JobProgress".into(),
				is_noisy: true,
				description: "Sent frequently during job execution with progress updates".into(),
			},
			EventInfo {
				variant: "IndexingProgress".into(),
				is_noisy: true,
				description: "Sent frequently during location indexing".into(),
			},
			EventInfo {
				variant: "LogMessage".into(),
				is_noisy: true,
				description: "Every log message becomes an event".into(),
			},
		];

		Ok(ListEventsOutput {
			all_events,
			noisy_events,
			event_info,
		})
	}
}

crate::register_core_query!(ListEventsQuery, "core.events.list");
