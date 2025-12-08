//! Ephemeral (memory-backed) change handler for browsing unmanaged paths.
//!
//! Updates the EphemeralIndex directly without database writes.
//! Skips the processor pipeline (no thumbnails/content hash for ephemeral).

use super::handler::{build_dir_entry, ChangeHandler};
use super::types::{ChangeType, EntryRef};
use crate::infra::event::EventBus;
use crate::ops::indexing::entry::EntryMetadata;
use crate::ops::indexing::ephemeral::EphemeralIndex;
use crate::ops::indexing::state::{DirEntry, EntryKind};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Memory-backed change handler for ephemeral browsing.
pub struct EphemeralChangeHandler {
	index: Arc<RwLock<EphemeralIndex>>,
	event_bus: Arc<EventBus>,
	root_path: PathBuf,
	next_id: AtomicI32,
}

impl EphemeralChangeHandler {
	pub fn new(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Arc<EventBus>,
		root_path: PathBuf,
	) -> Self {
		Self {
			index,
			event_bus,
			root_path,
			next_id: AtomicI32::new(1),
		}
	}

	fn next_id(&self) -> i32 {
		self.next_id
			.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
	}
}

#[async_trait::async_trait]
impl ChangeHandler for EphemeralChangeHandler {
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>> {
		let index = self.index.read().await;

		if let Some(metadata) = index.get_entry_ref(&path.to_path_buf()) {
			let uuid = index.get_entry_uuid(&path.to_path_buf());

			Ok(Some(EntryRef {
				id: 0,
				uuid,
				path: path.to_path_buf(),
				kind: metadata.kind,
			}))
		} else {
			Ok(None)
		}
	}

	async fn find_by_inode(&self, _inode: u64) -> Result<Option<EntryRef>> {
		Ok(None)
	}

	async fn create(&mut self, metadata: &DirEntry, _parent_path: &Path) -> Result<EntryRef> {
		let entry_uuid = Uuid::new_v4();
		let entry_metadata = EntryMetadata::from(metadata.clone());

		{
			let mut index = self.index.write().await;
			index
				.add_entry(metadata.path.clone(), entry_uuid, entry_metadata)
				.map_err(|e| anyhow::anyhow!("Failed to add entry to ephemeral index: {}", e))?;
		}

		Ok(EntryRef {
			id: self.next_id(),
			uuid: Some(entry_uuid),
			path: metadata.path.clone(),
			kind: metadata.kind,
		})
	}

	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
		let entry_metadata = EntryMetadata::from(metadata.clone());

		{
			let mut index = self.index.write().await;
			let _ = index.add_entry(metadata.path.clone(), uuid, entry_metadata);
		}

		Ok(())
	}

	async fn move_entry(
		&mut self,
		entry: &EntryRef,
		old_path: &Path,
		new_path: &Path,
		_new_parent_path: &Path,
	) -> Result<()> {
		let metadata = build_dir_entry(new_path, None).await?;

		{
			let mut index = self.index.write().await;
			index.remove_entry(old_path);

			let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
			let entry_metadata = EntryMetadata::from(metadata.clone());
			let _ = index.add_entry(new_path.to_path_buf(), uuid, entry_metadata);
		}

		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		{
			let mut index = self.index.write().await;

			if entry.is_directory() {
				index.remove_directory_tree(&entry.path);
			} else {
				index.remove_entry(&entry.path);
			}
		}

		Ok(())
	}

	async fn run_processors(&self, _entry: &EntryRef, _is_new: bool) -> Result<()> {
		Ok(())
	}

	async fn emit_change_event(&self, entry: &EntryRef, _change_type: ChangeType) -> Result<()> {
		use crate::device::get_current_device_slug;
		use crate::domain::addressing::SdPath;
		use crate::domain::file::File;
		use crate::infra::event::{Event, ResourceMetadata};

		let Some(uuid) = entry.uuid else {
			return Ok(());
		};

		let device_slug = get_current_device_slug();

		let sd_path = SdPath::Physical {
			device_slug: device_slug.clone(),
			path: entry.path.clone(),
		};

		let content_kind = {
			let index = self.index.read().await;
			index.get_content_kind(&entry.path)
		};

		let metadata = build_dir_entry(&entry.path, None).await.ok();

		if let Some(meta) = metadata {
			let entry_metadata = EntryMetadata::from(meta);
			let mut file = File::from_ephemeral(uuid, &entry_metadata, sd_path);
			file.content_kind = content_kind;

			let parent_path = entry.path.parent().map(|p| SdPath::Physical {
				device_slug: file.sd_path.device_slug().unwrap_or("local").to_string(),
				path: p.to_path_buf(),
			});

			let affected_paths = parent_path.into_iter().collect();

			if let Ok(resource_json) = serde_json::to_value(&file) {
				self.event_bus.emit(Event::ResourceChanged {
					resource_type: "file".to_string(),
					resource: resource_json,
					metadata: Some(ResourceMetadata {
						no_merge_fields: vec!["sd_path".to_string()],
						alternate_ids: vec![],
						affected_paths,
					}),
				});
			}
		}

		Ok(())
	}

	async fn handle_new_directory(&self, path: &Path) -> Result<()> {
		use crate::ops::indexing::entry::EntryProcessor;

		let mut entries = match tokio::fs::read_dir(path).await {
			Ok(e) => e,
			Err(e) => {
				tracing::warn!(
					"Failed to read directory {} for ephemeral indexing: {}",
					path.display(),
					e
				);
				return Ok(());
			}
		};

		let mut index = self.index.write().await;

		while let Ok(Some(entry)) = entries.next_entry().await {
			let entry_path = entry.path();

			if let Ok(metadata) = entry.metadata().await {
				let kind = if metadata.is_dir() {
					EntryKind::Directory
				} else if metadata.is_symlink() {
					EntryKind::Symlink
				} else {
					EntryKind::File
				};

				let entry_metadata = EntryMetadata {
					path: entry_path.clone(),
					kind,
					size: metadata.len(),
					modified: metadata.modified().ok(),
					accessed: metadata.accessed().ok(),
					created: metadata.created().ok(),
					inode: EntryProcessor::get_inode(&metadata),
					permissions: None,
					is_hidden: entry_path
						.file_name()
						.and_then(|n| n.to_str())
						.map(|n| n.starts_with('.'))
						.unwrap_or(false),
				};

				let uuid = Uuid::new_v4();
				let _ = index.add_entry(entry_path, uuid, entry_metadata);
			}
		}

		Ok(())
	}
}
