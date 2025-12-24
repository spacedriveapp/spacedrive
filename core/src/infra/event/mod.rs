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
					&& event.affects_path(path_scope, true) // SubscriptionFilter is legacy, default to recursive
			}
		}
	}
}

/// Source of library creation for automatic switching behavior
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub enum LibraryCreationSource {
	/// User created locally via UI
	#[default]
	Manual,
	/// Received via network sync from another device
	Sync,
	/// Imported from cloud storage
	CloudImport,
}

/// Sync activity types for detailed sync monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum SyncActivityType {
	BroadcastSent { changes: u64 },
	ChangesReceived { changes: u64 },
	ChangesApplied { changes: u64 },
	BackfillStarted,
	BackfillCompleted { records: u64 },
	CatchUpStarted,
	CatchUpCompleted,
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
		/// How the library was created (manual, sync, cloud import)
		#[serde(default)]
		source: LibraryCreationSource,
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
	LibraryLoadFailed {
		/// Library ID if config was readable, None otherwise
		id: Option<Uuid>,
		/// Path to the library directory
		path: PathBuf,
		/// Human-readable error message
		error: String,
		/// Error type for frontend categorization (e.g., "DatabaseError", "ConfigError")
		error_type: String,
	},
	LibraryStatisticsUpdated {
		library_id: Uuid,
		statistics: crate::library::config::LibraryStatistics,
	},

	// Cache invalidation event
	/// Refresh event - signals that all frontend caches should be invalidated
	/// Emitted after major data recalculations (e.g., volume unique_bytes refresh)
	Refresh,

	// Entry events (file/directory operations)
	// DEPRECATED: Use ResourceChanged instead
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
		device_id: uuid::Uuid,
	},
	JobStarted {
		job_id: String,
		job_type: String,
		device_id: uuid::Uuid,
	},
	JobProgress {
		job_id: String,
		job_type: String,
		device_id: uuid::Uuid,
		progress: f64,
		message: Option<String>,
		// Enhanced progress data - serialized GenericProgress
		generic_progress: Option<GenericProgress>,
	},
	JobCompleted {
		job_id: String,
		job_type: String,
		device_id: uuid::Uuid,
		output: JobOutput,
	},
	JobFailed {
		job_id: String,
		job_type: String,
		device_id: uuid::Uuid,
		error: String,
	},
	JobCancelled {
		job_id: String,
		job_type: String,
		device_id: uuid::Uuid,
	},
	JobPaused {
		job_id: String,
		device_id: uuid::Uuid,
	},
	JobResumed {
		job_id: String,
		device_id: uuid::Uuid,
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

	// Sync events
	SyncStateChanged {
		library_id: Uuid,
		previous_state: String,
		new_state: String,
		timestamp: String,
	},
	SyncActivity {
		library_id: Uuid,
		peer_device_id: Uuid,
		activity_type: SyncActivityType,
		model_type: Option<String>,
		count: u64,
		timestamp: String,
	},
	SyncConnectionChanged {
		library_id: Uuid,
		peer_device_id: Uuid,
		peer_name: String,
		connected: bool,
		timestamp: String,
	},
	SyncError {
		library_id: Uuid,
		peer_device_id: Option<Uuid>,
		error_type: String,
		message: String,
		timestamp: String,
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
	///
	/// # Arguments
	/// * `scope` - The path scope to check against
	/// * `include_descendants` - If true, match all descendants (recursive). If false, only exact matches (direct children)
	pub fn affects_path(&self, scope: &SdPath, include_descendants: bool) -> bool {
		let affected_paths = match self {
			Event::ResourceChanged { metadata, .. }
			| Event::ResourceChangedBatch { metadata, .. } => metadata.as_ref().map(|m| &m.affected_paths),
			_ => None,
		};

		let Some(paths) = affected_paths else {
			// No path metadata - can't determine if it matches, so include it
			tracing::debug!("No path metadata in event, including by default");
			return true;
		};

		if paths.is_empty() {
			// Empty affected_paths means this is a global resource (location, space, etc.)
			tracing::debug!("Empty affected_paths (global resource), including");
			return true;
		}

		// Handle non-hierarchical paths first (Content ID, Cloud, Sidecar)
		// These work the same in both exact and recursive mode
		let has_non_physical_match = paths.iter().any(|affected_path| {
			match (scope, affected_path) {
				// Content ID matching - exact match
				(
					SdPath::Content {
						content_id: scope_id,
					},
					SdPath::Content {
						content_id: file_id,
					},
				) => scope_id == file_id,
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
						&& if include_descendants {
							file_path.starts_with(scope_path.as_str())
						} else {
							file_path == scope_path
						}
				}
				_ => false,
			}
		});

		if has_non_physical_match {
			return true;
		}

		// Handle Physical scope against Content/Sidecar paths by checking alternate_paths
		// This prevents unnecessary events when content exists outside the subscribed scope
		if matches!(scope, SdPath::Physical { .. }) {
			let has_content_with_alternate_match =
				self.check_alternate_paths(scope, include_descendants);
			if has_content_with_alternate_match {
				return true;
			}
		}

		// For exact mode with Physical paths: check if ANY file is a direct child
		if !include_descendants {
			// Exact mode: find if there's at least one file that's a direct child
			let has_direct_child = paths.iter().any(|affected_path| {
				if let (
					SdPath::Physical {
						device_slug: scope_device,
						path: scope_path,
					},
					SdPath::Physical {
						device_slug: file_device,
						path: file_path,
					},
				) = (scope, affected_path)
				{
					if scope_device != file_device {
						return false;
					}

					// Exact mode: ONLY match the scope directory itself
					// This indicates files are DIRECTLY in this directory
					// Subdirectories in affected_paths mean files are in THOSE subdirectories
					let matches = file_path == scope_path;

					matches
				} else {
					false
				}
			});

			return has_direct_child;
		}

		// Recursive mode for Physical paths only
		let result = paths.iter().any(|affected_path| {
			match (scope, affected_path) {
				// Physical path matching - recursive mode
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
					if scope_device != file_device {
						return false;
					}

					// Recursive: match all descendants
					let matches = file_path.starts_with(scope_path);

					tracing::debug!(
						"Recursive mode check: scope={}, file={}, matches={}",
						scope_path.display(),
						file_path.display(),
						matches
					);

					matches
				}
				// All other path types handled above
				_ => false,
			}
		});

		tracing::debug!(
			"affects_path final result: scope={:?}, include_descendants={}, result={}",
			scope,
			include_descendants,
			result
		);

		result
	}

	/// Check if alternate_paths in the resource match the Physical scope
	///
	/// For Content/Sidecar events, alternate_paths contains all Physical locations
	/// where that content exists. This allows filtering to only forward events
	/// when the content has a physical presence in the subscribed path scope.
	fn check_alternate_paths(&self, scope: &SdPath, include_descendants: bool) -> bool {
		// Extract resource(s) from the event
		let resources = match self {
			Event::ResourceChanged { resource, .. } => vec![resource],
			Event::ResourceChangedBatch { resources, .. } => {
				// Extract array of resources
				if let Some(arr) = resources.as_array() {
					arr.iter().collect()
				} else {
					return false;
				}
			}
			_ => return false,
		};

		// Check if any resource's alternate_paths match the scope
		for resource in resources {
			// Extract alternate_paths array from resource JSON
			let alternate_paths = resource.get("alternate_paths").and_then(|v| v.as_array());

			if let Some(paths_array) = alternate_paths {
				// Deserialize each path and check if it matches the scope
				for path_value in paths_array {
					if let Ok(alt_path) = serde_json::from_value::<SdPath>(path_value.clone()) {
						// Check if this alternate path matches the scope
						if let (
							SdPath::Physical {
								device_slug: scope_device,
								path: scope_path,
							},
							SdPath::Physical {
								device_slug: alt_device,
								path: alt_path,
							},
						) = (scope, &alt_path)
						{
							if scope_device != alt_device {
								continue;
							}

							// Apply same matching logic as regular physical paths
							let matches = if include_descendants {
								// Recursive: match if path is descendant
								alt_path.starts_with(scope_path)
							} else {
								// Exact: match if parent directory equals scope
								if let Some(parent) = alt_path.parent() {
									parent == scope_path.as_path()
								} else {
									false
								}
							};

							if matches {
								tracing::debug!(
									"alternate_path match: scope={}, alt_path={}, include_descendants={}",
									scope_path.display(),
									alt_path.display(),
									include_descendants
								);
								return true;
							}
						}
					}
				}
			} else {
				// No alternate_paths - fall back to checking sd_path.Physical
				// This handles newly created files before content hashing completes
				if let Some(sd_path_value) = resource.get("sd_path") {
					if let Ok(sd_path) = serde_json::from_value::<SdPath>(sd_path_value.clone()) {
						if let (
							SdPath::Physical {
								device_slug: scope_device,
								path: scope_path,
							},
							SdPath::Physical {
								device_slug: res_device,
								path: res_path,
							},
						) = (scope, &sd_path)
						{
							if scope_device != res_device {
								continue;
							}

							let matches = if include_descendants {
								res_path.starts_with(scope_path)
							} else {
								if let Some(parent) = res_path.parent() {
									parent == scope_path.as_path()
								} else {
									false
								}
							};

							if matches {
								tracing::debug!(
									"sd_path match (no alternate_paths): scope={}, path={}, include_descendants={}",
									scope_path.display(),
									res_path.display(),
									include_descendants
								);
								return true;
							}
						}
					}
				}
			}
		}

		false
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
	fn is_sync_event(&self) -> bool;
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
				| Event::LibraryLoadFailed { .. }
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

	fn is_sync_event(&self) -> bool {
		matches!(
			self,
			Event::SyncStateChanged { .. }
				| Event::SyncActivity { .. }
				| Event::SyncConnectionChanged { .. }
				| Event::SyncError { .. }
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
			Event::SyncStateChanged {
				library_id: lid, ..
			}
			| Event::SyncActivity {
				library_id: lid, ..
			}
			| Event::SyncConnectionChanged {
				library_id: lid, ..
			}
			| Event::SyncError {
				library_id: lid, ..
			} => *lid == library_id,
			_ => false,
		}
	}
}
