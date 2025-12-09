//! macOS-specific file system event handling
//!
//! On macOS, we use the FSEvents backend of notify-rs and Rename events are complex.
//! There are just ModifyKind::Name(RenameMode::Any) events and nothing else.
//! This means we have to link the old path with the new path to know which file was renamed.
//!
//! Renames aren't always file name changes - the path can be modified when files are moved.
//! When a file is moved inside the same location, we receive 2 events: old and new path.
//! When moved to another location, we only receive the old path event (handle as deletion).
//! When moved from elsewhere to our location, we receive new path rename event (handle as creation).

use super::EventHandler;
use crate::infra::db::entities::{directory_paths, entry};
use crate::infra::event::Event;
use crate::service::watcher::event_handler::WatcherEventKind;
use crate::service::watcher::{EphemeralWatch, WatchedLocation, WatcherEvent};
use anyhow::Result;
use notify::{
	event::{CreateKind, DataChange, MetadataKind, ModifyKind, RenameMode},
	EventKind,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Simplified inode type for macOS
type INode = u64;

/// Tuple of instant and path for rename tracking
type InstantAndPath = (Instant, PathBuf);

/// Constants for timing
const HUNDRED_MILLIS: Duration = Duration::from_millis(100);
const ONE_SECOND: Duration = Duration::from_secs(1);

/// macOS-specific event handler that handles FSEvents complexities
pub struct MacOSHandler {
	/// Last time we performed eviction checks
	last_events_eviction_check: Arc<RwLock<Instant>>,

	/// Latest created directory to handle Finder's duplicate events
	latest_created_dir: Arc<RwLock<Option<PathBuf>>>,

	/// Old paths map for rename tracking (inode -> (instant, path))
	old_paths_map: Arc<RwLock<HashMap<INode, InstantAndPath>>>,

	/// New paths map for rename tracking (inode -> (instant, path))
	new_paths_map: Arc<RwLock<HashMap<INode, InstantAndPath>>>,

	/// Files pending update after create/modify events
	files_to_update: Arc<RwLock<HashMap<PathBuf, Instant>>>,

	/// Files that need updating after multiple rapid changes
	reincident_to_update_files: Arc<RwLock<HashMap<PathBuf, Instant>>>,

	/// Directories that need size recalculation
	to_recalculate_size: Arc<RwLock<HashMap<PathBuf, Instant>>>,

	/// Database connections for inode lookups (location_id -> db connection)
	/// Needed to query inodes for files that no longer exist on disk (for rename detection)
	db_connections: Arc<RwLock<HashMap<Uuid, DatabaseConnection>>>,
}

impl MacOSHandler {
	pub fn new() -> Self {
		Self {
			last_events_eviction_check: Arc::new(RwLock::new(Instant::now())),
			latest_created_dir: Arc::new(RwLock::new(None)),
			old_paths_map: Arc::new(RwLock::new(HashMap::new())),
			new_paths_map: Arc::new(RwLock::new(HashMap::new())),
			files_to_update: Arc::new(RwLock::new(HashMap::new())),
			reincident_to_update_files: Arc::new(RwLock::new(HashMap::new())),
			to_recalculate_size: Arc::new(RwLock::new(HashMap::new())),
			db_connections: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Register a database connection for a location (needed for rename inode lookups)
	pub async fn register_location_db(&self, location_id: Uuid, db: DatabaseConnection) {
		let mut connections = self.db_connections.write().await;
		connections.insert(location_id, db);
		trace!(
			"Registered database connection for location {}",
			location_id
		);
	}

	/// Unregister a database connection for a location
	pub async fn unregister_location_db(&self, location_id: Uuid) {
		let mut connections = self.db_connections.write().await;
		connections.remove(&location_id);
		trace!(
			"Unregistered database connection for location {}",
			location_id
		);
	}

	/// Extract inode from file metadata (simplified for now)
	async fn get_inode_from_path(&self, path: &PathBuf) -> Option<INode> {
		match tokio::fs::metadata(path).await {
			Ok(metadata) => {
				// On Unix systems, we can extract the inode
				#[cfg(unix)]
				{
					use std::os::unix::fs::MetadataExt;
					Some(metadata.ino())
				}
				#[cfg(not(unix))]
				{
					// Fallback: use a hash of the path
					use std::collections::hash_map::DefaultHasher;
					use std::hash::{Hash, Hasher};
					let mut hasher = DefaultHasher::new();
					path.hash(&mut hasher);
					Some(hasher.finish())
				}
			}
			Err(_) => None,
		}
	}

	/// Query database for inode of a path that no longer exists on disk
	/// This is essential for rename detection when we receive an event for the old path
	/// which has already been renamed (and thus doesn't exist on disk anymore)
	async fn get_inode_from_db(&self, path: &Path, location_id: Uuid) -> Option<INode> {
		let connections = self.db_connections.read().await;
		let db = connections.get(&location_id)?;

		// Try directory lookup first (check directory_paths table)
		let path_str = path.to_string_lossy().to_string();
		if let Ok(Some(dir)) = directory_paths::Entity::find()
			.filter(directory_paths::Column::Path.eq(&path_str))
			.one(db)
			.await
		{
			// Found directory, get its entry to extract inode
			if let Ok(Some(entry_model)) = entry::Entity::find_by_id(dir.entry_id).one(db).await {
				if let Some(inode_val) = entry_model.inode {
					trace!(
						"Found inode {} for directory path {} in database",
						inode_val,
						path.display()
					);
					return Some(inode_val as u64);
				}
			}
		}

		// Try file lookup by parent directory + name + extension
		let parent = path.parent()?;
		let parent_str = parent.to_string_lossy().to_string();

		// Find parent directory
		let parent_dir = directory_paths::Entity::find()
			.filter(directory_paths::Column::Path.eq(parent_str))
			.one(db)
			.await
			.ok()??;

		// Extract file name and extension
		let name = path.file_stem()?.to_str()?.to_string();
		let ext = path
			.extension()
			.and_then(|s| s.to_str())
			.map(|s| s.to_lowercase());

		// Query for file entry with matching parent, name, and extension
		let mut query = entry::Entity::find()
			.filter(entry::Column::ParentId.eq(parent_dir.entry_id))
			.filter(entry::Column::Name.eq(name));

		if let Some(extension) = ext {
			query = query.filter(entry::Column::Extension.eq(extension));
		} else {
			query = query.filter(entry::Column::Extension.is_null());
		}

		if let Ok(Some(entry_model)) = query.one(db).await {
			if let Some(inode_val) = entry_model.inode {
				trace!(
					"Found inode {} for file path {} in database",
					inode_val,
					path.display()
				);
				return Some(inode_val as u64);
			}
		}

		trace!("No inode found in database for path {}", path.display());
		None
	}

	/// Convert notify event to our internal event representation
	fn convert_notify_event(&self, notify_event: notify::Event) -> WatcherEvent {
		let kind = match notify_event.kind {
			EventKind::Create(CreateKind::Folder) => WatcherEventKind::Create,
			EventKind::Create(CreateKind::File) => WatcherEventKind::Create,
			EventKind::Modify(ModifyKind::Data(DataChange::Content)) => WatcherEventKind::Modify,
			EventKind::Modify(ModifyKind::Metadata(
				MetadataKind::WriteTime | MetadataKind::Extended,
			)) => WatcherEventKind::Modify,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
				WatcherEventKind::Other("rename".to_string())
			}
			EventKind::Remove(_) => WatcherEventKind::Remove,
			other => WatcherEventKind::Other(format!("{:?}", other)),
		};

		WatcherEvent {
			kind,
			paths: notify_event.paths,
			timestamp: std::time::SystemTime::now(),
			attrs: vec![format!("{:?}", notify_event.attrs)],
		}
	}

	/// Handle a single rename event (the core complexity of macOS watching)
	async fn handle_single_rename_event(
		&self,
		path: PathBuf,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
	) -> Result<Vec<Event>> {
		let mut events = Vec::new();

		debug!(
			"handle_single_rename_event called for path: {}",
			path.display()
		);

		match tokio::fs::metadata(&path).await {
			Ok(metadata) => {
				// File exists - this could be the "new" part of a rename or a creation
				trace!("Rename event: path exists {}", path.display());

				if let Some(inode) = self.get_inode_from_path(&path).await {
					debug!(
						"Got inode {} from filesystem for new path: {}",
						inode,
						path.display()
					);
					// Check if this matches an old path we're tracking
					let mut old_paths = self.old_paths_map.write().await;
					if let Some((_, old_path)) = old_paths.remove(&inode) {
						// We found a match! This is a real rename operation
						debug!(
							"Detected rename match: {} -> {}",
							old_path.display(),
							path.display()
						);

						// Remove both paths from files_to_update so they don't get emitted as Create/Modify
						let mut files_to_update = self.files_to_update.write().await;
						files_to_update.remove(&old_path);
						files_to_update.remove(&path);
						drop(files_to_update);

						// Find the matching location and generate rename event
						let locations = watched_locations.read().await;
						for location in locations.values() {
							if path.starts_with(&location.path) {
								events.push(Event::FsRawChange {
									library_id: location.library_id,
									kind: crate::infra::event::FsRawEventKind::Rename {
										from: old_path,
										to: path,
									},
								});
								break;
							}
						}
					} else {
						// No matching old path - store as new path for potential future match
						trace!("Storing new path for rename: {}", path.display());
						let mut new_paths = self.new_paths_map.write().await;
						new_paths.insert(inode, (Instant::now(), path));
					}
				}
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				// File doesn't exist - this could be the "old" part of a rename or a deletion
				trace!("Rename event: path doesn't exist {}", path.display());

				// Find the location this path belongs to
				let locations = watched_locations.read().await;
				let location = locations
					.values()
					.find(|loc| path.starts_with(&loc.path))
					.cloned();
				drop(locations);

				if let Some(location) = location {
					// Query database to get inode for the old path that no longer exists
					if let Some(inode) = self.get_inode_from_db(&path, location.id).await {
						trace!(
							"Retrieved inode {} from database for old path {}",
							inode,
							path.display()
						);

						// Check if new_paths_map has this inode (matching new path)
						let mut new_paths = self.new_paths_map.write().await;
						if let Some((_, new_path)) = new_paths.remove(&inode) {
							// We found a match! This is a real rename operation
							debug!(
								"Detected rename match: {} -> {}",
								path.display(),
								new_path.display()
							);

							// Remove both paths from files_to_update so they don't get emitted as Create/Modify
							let mut files_to_update = self.files_to_update.write().await;
							files_to_update.remove(&path);
							files_to_update.remove(&new_path);
							drop(files_to_update);

							// Generate rename event
							events.push(Event::FsRawChange {
								library_id: location.library_id,
								kind: crate::infra::event::FsRawEventKind::Rename {
									from: path.clone(),
									to: new_path,
								},
							});
						} else {
							// No matching new path yet - store in old_paths_map for later matching
							trace!(
								"Storing old path with inode {} for potential rename match: {}",
								inode,
								path.display()
							);
							drop(new_paths);
							let mut old_paths = self.old_paths_map.write().await;
							old_paths.insert(inode, (Instant::now(), path.clone()));
						}
					} else {
						// Path not found in database - could be a temp file or never indexed
						trace!(
							"Path not found in database, treating as removal: {}",
							path.display()
						);
						events.push(Event::FsRawChange {
							library_id: location.library_id,
							kind: crate::infra::event::FsRawEventKind::Remove {
								path: path.clone(),
							},
						});
					}
				} else {
					warn!(
						"Received rename event for path outside watched locations: {}",
						path.display()
					);
				}
			}
			Err(e) => {
				error!(
					"Error accessing path during rename: {}: {}",
					path.display(),
					e
				);
			}
		}

		Ok(events)
	}

	/// Handle eviction of files that need updating
	async fn handle_to_update_eviction(
		&self,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
		ephemeral_watches: &Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	) -> Result<Vec<Event>> {
		let mut events = Vec::new();
		let mut files_to_update = self.files_to_update.write().await;
		let mut reincident_files = self.reincident_to_update_files.write().await;
		let mut to_recalc_size = self.to_recalculate_size.write().await;

		trace!(
			"Tick eviction check: {} files buffered",
			files_to_update.len()
		);

		// Process files that have been waiting for updates
		let mut files_to_keep = HashMap::new();
		for (path, created_at) in files_to_update.drain() {
			if created_at.elapsed() < HUNDRED_MILLIS * 5 {
				trace!(
					"File not ready yet ({}ms elapsed): {}",
					created_at.elapsed().as_millis(),
					path.display()
				);
				files_to_keep.insert(path, created_at);
			} else {
				debug!(
					"Evicting buffered file ({}ms elapsed): {}",
					created_at.elapsed().as_millis(),
					path.display()
				);
				// File has been stable long enough, generate update event
				if let Some(parent) = path.parent() {
					to_recalc_size.insert(parent.to_path_buf(), Instant::now());
				}

				reincident_files.remove(&path);

				// Emit create event (responder will detect if it's an update via inode)
				// This handles both newly created files and files that were modified
				let locations = watched_locations.read().await;
				let mut matched = false;

				// Check locations first
				for location in locations.values() {
					if path.starts_with(&location.path) {
						events.push(Event::FsRawChange {
							library_id: location.library_id,
							kind: crate::infra::event::FsRawEventKind::Create {
								path: path.clone(),
							},
						});
						matched = true;
						break;
					}
				}

				// If not matched by location, check ephemeral watches
				if !matched {
					let ephemeral = ephemeral_watches.read().await;
					if let Some(parent) = path.parent() {
						if ephemeral.contains_key(parent) {
							events.push(Event::FsRawChange {
								library_id: Uuid::nil(), // Ephemeral events use nil UUID
								kind: crate::infra::event::FsRawEventKind::Create {
									path: path.clone(),
								},
							});
						}
					}
				}
			}
		}
		*files_to_update = files_to_keep;

		// Process reincident files with longer timeout
		let mut reincident_to_keep = HashMap::new();
		for (path, created_at) in reincident_files.drain() {
			if created_at.elapsed() < ONE_SECOND * 10 {
				reincident_to_keep.insert(path, created_at);
			} else {
				if let Some(parent) = path.parent() {
					to_recalc_size.insert(parent.to_path_buf(), Instant::now());
				}

				files_to_update.remove(&path);

				// Emit create event (responder will detect if it's an update via inode)
				let locations = watched_locations.read().await;
				for location in locations.values() {
					if path.starts_with(&location.path) {
						events.push(Event::FsRawChange {
							library_id: location.library_id,
							kind: crate::infra::event::FsRawEventKind::Create {
								path: path.clone(),
							},
						});
						break;
					}
				}
			}
		}
		*reincident_files = reincident_to_keep;

		Ok(events)
	}

	/// Handle creation events from rename eviction
	async fn handle_rename_create_eviction(
		&self,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
	) -> Result<Vec<Event>> {
		let mut events = Vec::new();
		let mut new_paths = self.new_paths_map.write().await;
		let files_to_update = self.files_to_update.read().await;

		let mut paths_to_keep = HashMap::new();
		for (inode, (instant, path)) in new_paths.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				if !files_to_update.contains_key(&path) {
					// Path has timed out and isn't already being updated
					match tokio::fs::metadata(&path).await {
						Ok(metadata) => {
							let locations = watched_locations.read().await;
							for location in locations.values() {
								if path.starts_with(&location.path) {
									events.push(Event::FsRawChange {
										library_id: location.library_id,
										kind: crate::infra::event::FsRawEventKind::Create {
											path: path.clone(),
										},
									});

									if let Some(parent) = path.parent() {
										let mut to_recalc = self.to_recalculate_size.write().await;
										to_recalc.insert(parent.to_path_buf(), Instant::now());
									}
									break;
								}
							}
						}
						Err(_) => {
							// File no longer exists, ignore
						}
					}
				}
			} else {
				paths_to_keep.insert(inode, (instant, path));
			}
		}
		*new_paths = paths_to_keep;

		Ok(events)
	}

	/// Handle removal events from rename eviction
	async fn handle_rename_remove_eviction(
		&self,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
	) -> Result<Vec<Event>> {
		let mut events = Vec::new();
		let mut old_paths = self.old_paths_map.write().await;

		let mut paths_to_keep = HashMap::new();
		for (inode, (instant, path)) in old_paths.drain() {
			if instant.elapsed() > HUNDRED_MILLIS {
				// Path has timed out, treat as removal
				let locations = watched_locations.read().await;
				for location in locations.values() {
					if path.starts_with(&location.path) {
						events.push(Event::FsRawChange {
							library_id: location.library_id,
							kind: crate::infra::event::FsRawEventKind::Remove {
								path: path.clone(),
							},
						});

						if let Some(parent) = path.parent() {
							let mut to_recalc = self.to_recalculate_size.write().await;
							to_recalc.insert(parent.to_path_buf(), Instant::now());
						}
						break;
					}
				}
			} else {
				paths_to_keep.insert(inode, (instant, path));
			}
		}
		*old_paths = paths_to_keep;

		Ok(events)
	}
}

#[async_trait::async_trait]
impl EventHandler for MacOSHandler {
	async fn process_event(
		&self,
		event: WatcherEvent,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
		ephemeral_watches: &Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	) -> Result<Vec<Event>> {
		if !event.should_process() {
			return Ok(vec![]);
		}

		let mut events = Vec::new();
		let path = match event.paths.first() {
			Some(path) => path.clone(),
			None => return Ok(vec![]),
		};

		// Log the event kind for debugging
		debug!(
			"MacOSHandler processing event: {:?} for path: {}",
			event.kind,
			path.display()
		);

		// Handle different event types like the original implementation
		match &event.kind {
			WatcherEventKind::Create => {
				// Check for duplicate directory creation events (macOS Finder issue)
				if tokio::fs::metadata(&path)
					.await
					.map_or(false, |m| m.is_dir())
				{
					let mut latest_created = self.latest_created_dir.write().await;
					if let Some(ref latest) = *latest_created {
						if path == *latest {
							// Duplicate event, ignore
							return Ok(vec![]);
						}
					}
					*latest_created = Some(path.clone());

					// For directories, emit immediately
					let locations = watched_locations.read().await;
					let mut matched = false;

					// Check locations first
					for location in locations.values() {
						if location.enabled && path.starts_with(&location.path) {
							events.push(Event::FsRawChange {
								library_id: location.library_id,
								kind: crate::infra::event::FsRawEventKind::Create {
									path: path.clone(),
								},
							});

							// Schedule parent for size recalculation
							if let Some(parent) = path.parent() {
								let mut to_recalc = self.to_recalculate_size.write().await;
								to_recalc.insert(parent.to_path_buf(), Instant::now());
							}
							matched = true;
							break;
						}
					}

					// If not matched by location, check ephemeral watches
					if !matched {
						let ephemeral = ephemeral_watches.read().await;
						if let Some(parent) = path.parent() {
							if ephemeral.contains_key(parent) {
								events.push(Event::FsRawChange {
									library_id: Uuid::nil(), // Ephemeral events use nil UUID
									kind: crate::infra::event::FsRawEventKind::Create {
										path: path.clone(),
									},
								});
							}
						}
					}
				} else {
					// For files, DON'T emit immediately - store for later processing
					// This allows rename events to be matched first
					trace!(
						"Buffering Create event for file (may be part of rename): {}",
						path.display()
					);
					let mut files_to_update = self.files_to_update.write().await;
					files_to_update.insert(path.clone(), Instant::now());
				}
			}

			WatcherEventKind::Modify => {
				// DON'T emit immediately - store for later processing via tick eviction
				// This allows rename events to be matched first
				trace!(
					"Buffering Modify event (may be part of rename): {}",
					path.display()
				);
				let mut files_to_update = self.files_to_update.write().await;
				let mut reincident_files = self.reincident_to_update_files.write().await;

				if files_to_update.contains_key(&path) {
					if let Some(old_instant) = files_to_update.insert(path.clone(), Instant::now())
					{
						reincident_files.entry(path.clone()).or_insert(old_instant);
					}
				} else {
					files_to_update.insert(path.clone(), Instant::now());
				}
			}

			WatcherEventKind::Remove => {
				// Generate removal event and schedule parent for size recalculation
				let locations = watched_locations.read().await;
				let mut matched = false;

				// Check locations first
				for location in locations.values() {
					if location.enabled && path.starts_with(&location.path) {
						events.push(Event::FsRawChange {
							library_id: location.library_id,
							kind: crate::infra::event::FsRawEventKind::Remove {
								path: path.clone(),
							},
						});

						if let Some(parent) = path.parent() {
							let mut to_recalc = self.to_recalculate_size.write().await;
							to_recalc.insert(parent.to_path_buf(), Instant::now());
						}
						matched = true;
						break;
					}
				}

				// If not matched by location, check ephemeral watches
				if !matched {
					let ephemeral = ephemeral_watches.read().await;
					if let Some(parent) = path.parent() {
						if ephemeral.contains_key(parent) {
							events.push(Event::FsRawChange {
								library_id: Uuid::nil(), // Ephemeral events use nil UUID
								kind: crate::infra::event::FsRawEventKind::Remove {
									path: path.clone(),
								},
							});
						}
					}
				}
			}

			WatcherEventKind::Other(event_type) if event_type == "rename" => {
				// Handle macOS rename events (the complex part)
				let rename_events = self
					.handle_single_rename_event(path, watched_locations)
					.await?;
				events.extend(rename_events);
			}

			_ => {
				trace!("Unhandled macOS event type: {:?}", event.kind);
			}
		}

		Ok(events)
	}

	async fn tick(&self) -> Result<()> {
		// Don't reset last_check here - it's handled by tick_with_locations()
		// which actually does the eviction work
		Ok(())
	}
}

/// Additional methods for macOS handler beyond the EventHandler trait
impl MacOSHandler {
	/// Tick with access to watched locations for event processing
	pub async fn tick_with_locations(
		&self,
		watched_locations: &Arc<RwLock<HashMap<Uuid, WatchedLocation>>>,
		ephemeral_watches: &Arc<RwLock<HashMap<PathBuf, EphemeralWatch>>>,
	) -> Result<Vec<Event>> {
		let mut all_events = Vec::new();
		let mut last_check = self.last_events_eviction_check.write().await;

		// Run eviction every 2 seconds instead of every 100ms to avoid spam
		// This is appropriate for file system event debouncing
		let eviction_interval = ONE_SECOND * 2;

		if last_check.elapsed() > eviction_interval {
			// Handle file update evictions
			let update_events = self
				.handle_to_update_eviction(watched_locations, ephemeral_watches)
				.await?;
			all_events.extend(update_events);

			// Handle rename create evictions
			let create_events = self
				.handle_rename_create_eviction(watched_locations)
				.await?;
			all_events.extend(create_events);

			// Handle rename remove evictions
			let remove_events = self
				.handle_rename_remove_eviction(watched_locations)
				.await?;
			all_events.extend(remove_events);

			// Handle size recalculation
			// TODO: Implement directory size recalculation like original

			*last_check = Instant::now();
		}

		Ok(all_events)
	}
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::collections::HashMap;

//     #[tokio::test]
//     async fn test_macos_handler_creation() {
//         let handler = MacOSHandler::new();
//         assert_eq!(handler.debounce_duration, Duration::from_millis(100));
//     }

//     #[tokio::test]
//     async fn test_debounce_logic() {
//         let handler = MacOSHandler::new();
//         let path = PathBuf::from("/test/file.txt");

//         // First event should not be debounced
//         assert!(!handler.should_debounce(&path, "create").await);

//         // Second identical event should be debounced
//         assert!(handler.should_debounce(&path, "create").await);

//         // Different event type should not be debounced
//         assert!(!handler.should_debounce(&path, "modify").await);
//     }

//     #[tokio::test]
//     async fn test_tick_cleanup() {
//         let handler = MacOSHandler::new();

//         // Add some test data
//         {
//             let mut rename_map = handler.rename_map.write().await;
//             rename_map.insert(123, (PathBuf::from("/old"), SystemTime::now() - Duration::from_secs(10)));
//             rename_map.insert(456, (PathBuf::from("/recent"), SystemTime::now()));
//         }

//         // Run tick to clean up old entries
//         handler.tick().await.unwrap();

//         // Check that old entry was removed
//         let rename_map = handler.rename_map.read().await;
//         assert_eq!(rename_map.len(), 1);
//         assert!(rename_map.contains_key(&456));
//     }
// }
