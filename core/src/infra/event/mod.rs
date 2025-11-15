//! Event bus for decoupled communication

pub mod log_emitter;

use crate::domain::SdPath;
use crate::infra::job::{generic_progress::GenericProgress, output::JobOutput};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{debug, warn};
use uuid::Uuid;

/// Metadata for resource cache updates
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResourceMetadata {
	/// Fields that should be replaced, not merged
	pub no_merge_fields: Vec<String>,
	/// Alternate IDs for matching (besides primary ID)
	pub alternate_ids: Vec<Uuid>,
	/// Paths affected by this resource event (for path-scoped filtering)
	#[serde(default)]
	pub affected_paths: Vec<SdPath>,
}

/// Filter for event subscriptions to enable path-scoped event delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionFilter {
	/// Global subscription - receives all events of this type
	Global { resource_type: String },
	/// Path-scoped subscription - only events affecting this path
	PathScoped {
		resource_type: String,
		path_scope: SdPath,
	},
}

impl SubscriptionFilter {
	/// Check if this filter matches the given event
	pub fn matches(&self, event: &Event) -> bool {
		match self {
			Self::Global { resource_type } => event
				.resource_type()
				.map_or(false, |rt| rt == resource_type),
			Self::PathScoped {
				resource_type,
				path_scope,
			} => {
				event
					.resource_type()
					.map_or(false, |rt| rt == resource_type)
					&& event.affects_path(path_scope)
			}
		}
	}
}

/// A central event type that represents all events that can be emitted throughout the system
#[derive(Debug, Clone, Serialize, Deserialize, Type, strum::AsRefStr)]
#[serde(rename_all_fields = "snake_case")]
pub enum Event {
	// Core lifecycle events
	CoreStarted,
	CoreShutdown,

	// Library events
	LibraryCreated {
		id: Uuid,
		name: String,
		path: PathBuf,
	},
	LibraryOpened {
		id: Uuid,
		name: String,
		path: PathBuf,
	},
	LibraryClosed {
		id: Uuid,
		name: String,
	},
	LibraryDeleted {
		id: Uuid,
		name: String,
		deleted_data: bool,
	},
	LibraryStatisticsUpdated {
		library_id: Uuid,
		statistics: crate::library::config::LibraryStatistics,
	},

	// Entry events (file/directory operations)
	EntryCreated {
		library_id: Uuid,
		entry_id: Uuid,
	},
	EntryModified {
		library_id: Uuid,
		entry_id: Uuid,
	},
	EntryDeleted {
		library_id: Uuid,
		entry_id: Uuid,
	},
	EntryMoved {
		library_id: Uuid,
		entry_id: Uuid,
		old_path: String,
		new_path: String,
	},

	// Raw filesystem change events (no database IDs) - consumed by responder
	FsRawChange {
		library_id: Uuid,
		kind: FsRawEventKind,
	},

	// Volume events
	VolumeAdded(crate::volume::Volume),
	VolumeRemoved {
		fingerprint: crate::volume::VolumeFingerprint,
	},
	VolumeUpdated {
		fingerprint: crate::volume::VolumeFingerprint,
		old_info: crate::volume::VolumeInfo,
		new_info: crate::volume::VolumeInfo,
	},
	VolumeSpeedTested {
		fingerprint: crate::volume::VolumeFingerprint,
		read_speed_mbps: u64,
		write_speed_mbps: u64,
	},
	VolumeMountChanged {
		fingerprint: crate::volume::VolumeFingerprint,
		is_mounted: bool,
	},
	VolumeError {
		fingerprint: crate::volume::VolumeFingerprint,
		error: String,
	},

	// Job events
	JobQueued {
		job_id: String,
		job_type: String,
	},
	JobStarted {
		job_id: String,
		job_type: String,
	},
	JobProgress {
		job_id: String,
		job_type: String,
		progress: f64,
		message: Option<String>,
		// Enhanced progress data - serialized GenericProgress
		generic_progress: Option<GenericProgress>,
	},
	JobCompleted {
		job_id: String,
		job_type: String,
		output: JobOutput,
	},
	JobFailed {
		job_id: String,
		job_type: String,
		error: String,
	},
	JobCancelled {
		job_id: String,
		job_type: String,
	},
	JobPaused {
		job_id: String,
	},
	JobResumed {
		job_id: String,
	},

	// Indexing events
	IndexingStarted {
		location_id: Uuid,
	},
	IndexingProgress {
		location_id: Uuid,
		processed: u64,
		total: Option<u64>,
	},
	IndexingCompleted {
		location_id: Uuid,
		total_files: u64,
		total_dirs: u64,
	},
	IndexingFailed {
		location_id: Uuid,
		error: String,
	},

	// Device events
	DeviceConnected {
		device_id: Uuid,
		device_name: String,
	},
	DeviceDisconnected {
		device_id: Uuid,
	},

	// Generic resource events (normalized cache)
	// Works for ALL resources: Location, Tag, Album, File, etc.
	ResourceChanged {
		/// Resource type identifier (e.g., "location", "tag", "album")
		resource_type: String,
		/// The full resource data as JSON
		resource: serde_json::Value,
		/// Metadata for proper cache updates
		#[serde(default)]
		metadata: Option<ResourceMetadata>,
	},
	ResourceChangedBatch {
		/// Resource type identifier (e.g., "file")
		resource_type: String,
		/// Array of full resource data as JSON
		/// Used for batch updates during indexing to reduce event overhead
		resources: serde_json::Value,
		/// Metadata for proper cache updates
		#[serde(default)]
		metadata: Option<ResourceMetadata>,
	},
	ResourceDeleted {
		/// Resource type identifier
		resource_type: String,
		/// The deleted resource's ID
		resource_id: Uuid,
	},

	// Legacy events (for compatibility)
	LocationAdded {
		library_id: Uuid,
		location_id: Uuid,
		path: PathBuf,
	},
	LocationRemoved {
		library_id: Uuid,
		location_id: Uuid,
	},
	FilesIndexed {
		library_id: Uuid,
		location_id: Uuid,
		count: usize,
	},
	ThumbnailsGenerated {
		library_id: Uuid,
		count: usize,
	},
	FileOperationCompleted {
		library_id: Uuid,
		operation: FileOperation,
		affected_files: usize,
	},
	FilesModified {
		library_id: Uuid,
		paths: Vec<PathBuf>,
	},

	// Log events
	LogMessage {
		timestamp: chrono::DateTime<chrono::Utc>,
		level: String,
		target: String,
		message: String,
		job_id: Option<String>,
		library_id: Option<Uuid>,
	},

	// Custom events for extensibility
	Custom {
		event_type: String,
		#[specta(skip)]
		data: serde_json::Value,
	},
}

impl Event {
	/// Get the variant name of this event
	/// Uses strum::AsRefStr - automatically derived, no boilerplate!
	pub fn variant_name(&self) -> &str {
		self.as_ref()
	}

	/// Get the resource type if this is a resource event
	pub fn resource_type(&self) -> Option<&str> {
		match self {
			Event::ResourceChanged { resource_type, .. }
			| Event::ResourceChangedBatch { resource_type, .. }
			| Event::ResourceDeleted { resource_type, .. } => Some(resource_type),
			_ => None,
		}
	}

	/// Check if this event affects the given path scope
	pub fn affects_path(&self, scope: &SdPath) -> bool {
		let affected_paths = match self {
			Event::ResourceChanged { metadata, .. }
			| Event::ResourceChangedBatch { metadata, .. } => metadata.as_ref().map(|m| &m.affected_paths),
			_ => None,
		};

		let Some(paths) = affected_paths else {
			// No path metadata - can't determine if it matches, so include it
			return true;
		};

		if paths.is_empty() {
			// Empty affected_paths means this is a global resource (location, space, etc.)
			return true;
		}

		// Check if any affected path matches the scope
		paths.iter().any(|affected_path| {
			match (scope, affected_path) {
				// Physical path matching - check if file is in the scoped directory
				(
					SdPath::Physical {
						device_slug: scope_device,
						path: scope_path,
					},
					SdPath::Physical {
						device_slug: file_device,
						path: file_path,
					},
				) => {
					// Must be same device and file must be in the scope directory
					scope_device == file_device && file_path.starts_with(scope_path)
				}
				// Content ID matching - exact match
				(
					SdPath::Content {
						content_id: scope_id,
					},
					SdPath::Content {
						content_id: file_id,
					},
				) => scope_id == file_id,
				// Cloud path matching
				(
					SdPath::Cloud {
						service: scope_service,
						identifier: scope_id,
						path: scope_path,
					},
					SdPath::Cloud {
						service: file_service,
						identifier: file_id,
						path: file_path,
					},
				) => {
					scope_service == file_service
						&& scope_id == file_id
						&& file_path.starts_with(scope_path.as_str())
				}
				// Sidecar matching - match by content ID
				(
					SdPath::Content {
						content_id: scope_id,
					},
					SdPath::Sidecar {
						content_id: file_id,
						..
					},
				)
				| (
					SdPath::Sidecar {
						content_id: scope_id,
						..
					},
					SdPath::Content {
						content_id: file_id,
					},
				) => scope_id == file_id,
				// Mixed types don't match
				_ => false,
			}
		})
	}
}

/// Raw filesystem event kinds emitted by the watcher without DB resolution
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum FsRawEventKind {
	Create { path: PathBuf },
	Modify { path: PathBuf },
	Remove { path: PathBuf },
	Rename { from: PathBuf, to: PathBuf },
}

/// Types of file operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum FileOperation {
	Copy,
	Move,
	Delete,
	Rename,
}

/// A filtered subscriber with its own broadcast channel
#[derive(Debug)]
struct FilteredSubscriber {
	id: Uuid,
	filters: Vec<SubscriptionFilter>,
	sender: broadcast::Sender<Event>,
}

/// Event bus for broadcasting events with optional filtering
#[derive(Debug, Clone)]
pub struct EventBus {
	// Legacy broadcast for unfiltered subscriptions
	sender: broadcast::Sender<Event>,
	// Filtered subscribers
	subscribers: Arc<RwLock<Vec<FilteredSubscriber>>>,
}

impl EventBus {
	/// Create a new event bus with specified capacity
	pub fn new(capacity: usize) -> Self {
		let (sender, _) = broadcast::channel(capacity);
		Self {
			sender,
			subscribers: Arc::new(RwLock::new(Vec::new())),
		}
	}

	/// Emit an event to all subscribers (filtered and unfiltered)
	pub fn emit(&self, event: Event) {
		// Emit to unfiltered subscribers
		match self.sender.send(event.clone()) {
			Ok(count) => {
				// if count > 0 {
				// 	debug!("Event emitted to {} unfiltered subscribers", count);
				// }
			}
			Err(_) => {}
		}

		// Emit to filtered subscribers
		let subscribers = self.subscribers.read().unwrap();
		let mut matched_count = 0;

		for subscriber in subscribers.iter() {
			// Check if any filter matches
			let matches = subscriber
				.filters
				.iter()
				.any(|filter| filter.matches(&event));

			if matches {
				match subscriber.sender.send(event.clone()) {
					Ok(_) => {
						matched_count += 1;
					}
					Err(_) => {
						// Subscriber channel closed - will be cleaned up later
					}
				}
			}
		}

		if matched_count > 0 {
			debug!("Event emitted to {} filtered subscribers", matched_count);
		}
	}

	/// Subscribe to all events (unfiltered)
	pub fn subscribe(&self) -> EventSubscriber {
		EventSubscriber {
			receiver: self.sender.subscribe(),
			subscription_id: None,
			event_bus: None,
		}
	}

	/// Subscribe with filters
	pub fn subscribe_filtered(&self, filters: Vec<SubscriptionFilter>) -> EventSubscriber {
		let id = Uuid::new_v4();
		let (sender, receiver) = broadcast::channel(1024);

		let subscriber = FilteredSubscriber {
			id,
			filters,
			sender,
		};

		self.subscribers.write().unwrap().push(subscriber);

		debug!(
			"Created filtered subscription {} with {} filters",
			id,
			self.subscribers
				.read()
				.unwrap()
				.last()
				.unwrap()
				.filters
				.len()
		);

		EventSubscriber {
			receiver,
			subscription_id: Some(id),
			event_bus: Some(self.clone()),
		}
	}

	/// Unsubscribe a filtered subscription
	pub fn unsubscribe(&self, subscription_id: Uuid) {
		let mut subscribers = self.subscribers.write().unwrap();
		subscribers.retain(|s| s.id != subscription_id);
		debug!("Unsubscribed filtered subscription {}", subscription_id);
	}

	/// Get the number of active subscribers (unfiltered + filtered)
	pub fn subscriber_count(&self) -> usize {
		let filtered_count = self.subscribers.read().unwrap().len();
		self.sender.receiver_count() + filtered_count
	}

	/// Clean up closed subscriber channels
	pub fn cleanup_closed_subscribers(&self) {
		let mut subscribers = self.subscribers.write().unwrap();
		let before = subscribers.len();
		subscribers.retain(|s| s.sender.receiver_count() > 0);
		let removed = before - subscribers.len();
		if removed > 0 {
			debug!("Cleaned up {} closed filtered subscriptions", removed);
		}
	}
}

impl Default for EventBus {
	fn default() -> Self {
		Self::new(1024)
	}
}

/// Event subscriber for receiving events
#[derive(Debug)]
pub struct EventSubscriber {
	receiver: broadcast::Receiver<Event>,
	subscription_id: Option<Uuid>,
	event_bus: Option<EventBus>,
}

impl EventSubscriber {
	/// Receive the next event (blocking)
	pub async fn recv(&mut self) -> Result<Event, broadcast::error::RecvError> {
		self.receiver.recv().await
	}

	/// Try to receive an event without blocking
	pub fn try_recv(&mut self) -> Result<Event, broadcast::error::TryRecvError> {
		self.receiver.try_recv()
	}

	/// Filter events by type using a closure
	pub async fn recv_filtered<F>(
		&mut self,
		filter: F,
	) -> Result<Event, broadcast::error::RecvError>
	where
		F: Fn(&Event) -> bool,
	{
		loop {
			let event = self.recv().await?;
			if filter(&event) {
				return Ok(event);
			}
		}
	}

	/// Get the subscription ID if this is a filtered subscription
	pub fn subscription_id(&self) -> Option<Uuid> {
		self.subscription_id
	}
}

impl Drop for EventSubscriber {
	fn drop(&mut self) {
		// Auto-unsubscribe filtered subscriptions when dropped
		if let (Some(id), Some(bus)) = (self.subscription_id, &self.event_bus) {
			bus.unsubscribe(id);
		}
	}
}

/// Helper trait for event filtering
pub trait EventFilter {
	fn is_library_event(&self) -> bool;
	fn is_volume_event(&self) -> bool;
	fn is_job_event(&self) -> bool;
	fn is_for_library(&self, library_id: Uuid) -> bool;
}

impl EventFilter for Event {
	fn is_library_event(&self) -> bool {
		matches!(
			self,
			Event::LibraryCreated { .. }
				| Event::LibraryOpened { .. }
				| Event::LibraryClosed { .. }
				| Event::LibraryDeleted { .. }
				| Event::EntryCreated { .. }
				| Event::EntryModified { .. }
				| Event::EntryDeleted { .. }
				| Event::EntryMoved { .. }
		)
	}

	fn is_volume_event(&self) -> bool {
		matches!(
			self,
			Event::VolumeAdded(_)
				| Event::VolumeRemoved { .. }
				| Event::VolumeUpdated { .. }
				| Event::VolumeSpeedTested { .. }
				| Event::VolumeMountChanged { .. }
				| Event::VolumeError { .. }
		)
	}

	fn is_job_event(&self) -> bool {
		matches!(
			self,
			Event::JobQueued { .. }
				| Event::JobStarted { .. }
				| Event::JobProgress { .. }
				| Event::JobCompleted { .. }
				| Event::JobFailed { .. }
				| Event::JobCancelled { .. }
		)
	}

	// TODO: events should have an envelope that contains the library_id instead of this
	fn is_for_library(&self, library_id: Uuid) -> bool {
		match self {
			Event::LibraryCreated { id, .. }
			| Event::LibraryOpened { id, .. }
			| Event::LibraryClosed { id, .. }
			| Event::LibraryDeleted { id, .. } => *id == library_id,
			Event::EntryCreated {
				library_id: lid, ..
			}
			| Event::EntryModified {
				library_id: lid, ..
			}
			| Event::EntryDeleted {
				library_id: lid, ..
			}
			| Event::EntryMoved {
				library_id: lid, ..
			} => *lid == library_id,
			Event::LocationAdded {
				library_id: lid, ..
			}
			| Event::LocationRemoved {
				library_id: lid, ..
			}
			| Event::FilesIndexed {
				library_id: lid, ..
			}
			| Event::ThumbnailsGenerated {
				library_id: lid, ..
			}
			| Event::FileOperationCompleted {
				library_id: lid, ..
			}
			| Event::FilesModified {
				library_id: lid, ..
			} => *lid == library_id,
			_ => false,
		}
	}
}
