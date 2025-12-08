//! Persistent (database-backed) change handler for managed locations.
//!
//! Uses EntryProcessor for CRUD operations and maintains closure table
//! relationships. Runs the processor pipeline (thumbnails, content hash)
//! for new and modified files.

use super::handler::ChangeHandler;
use super::types::{ChangeType, EntryRef};
use crate::context::CoreContext;
use crate::infra::db::entities;
use crate::ops::indexing::state::{DirEntry, EntryKind};
use anyhow::Result;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Database-backed change handler for managed locations.
pub struct PersistentChangeHandler {
	context: Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	location_root_entry_id: i32,
	db: sea_orm::DatabaseConnection,
	volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>>,
	entry_id_cache: HashMap<PathBuf, i32>,
}

impl PersistentChangeHandler {
	pub async fn new(
		context: Arc<CoreContext>,
		library_id: Uuid,
		location_id: Uuid,
		_location_root: &Path,
		volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>>,
	) -> Result<Self> {
		let library = context
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

		let db = library.db().conn().clone();

		let location_record = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(location_id))
			.one(&db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let location_root_entry_id = location_record
			.entry_id
			.ok_or_else(|| anyhow::anyhow!("Location {} has no root entry", location_id))?;

		Ok(Self {
			context,
			library_id,
			location_id,
			location_root_entry_id,
			db,
			volume_backend,
			entry_id_cache: HashMap::new(),
		})
	}

	async fn resolve_entry_id(&self, path: &Path) -> Result<Option<i32>> {
		if let Some(id) = self.resolve_directory_entry_id(path).await? {
			return Ok(Some(id));
		}
		self.resolve_file_entry_id(path).await
	}

	async fn resolve_directory_entry_id(&self, path: &Path) -> Result<Option<i32>> {
		use sea_orm::FromQueryResult;

		let path_str = path.to_string_lossy().to_string();

		#[derive(Debug, FromQueryResult)]
		struct DirectoryEntryId {
			entry_id: i32,
		}

		let result = DirectoryEntryId::find_by_statement(sea_orm::Statement::from_sql_and_values(
			sea_orm::DbBackend::Sqlite,
			r#"
			SELECT dp.entry_id
			FROM directory_paths dp
			INNER JOIN entry_closure ec ON ec.descendant_id = dp.entry_id
			WHERE dp.path = ?
			  AND ec.ancestor_id = ?
			"#,
			vec![path_str.into(), self.location_root_entry_id.into()],
		))
		.one(&self.db)
		.await?;

		Ok(result.map(|r| r.entry_id))
	}

	async fn resolve_file_entry_id(&self, path: &Path) -> Result<Option<i32>> {
		let parent = match path.parent() {
			Some(p) => p,
			None => return Ok(None),
		};

		let parent_id = match self.resolve_directory_entry_id(parent).await? {
			Some(id) => id,
			None => return Ok(None),
		};

		let name = path
			.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or("")
			.to_string();
		let ext = path
			.extension()
			.and_then(|s| s.to_str())
			.map(|s| s.to_lowercase());

		let mut q = entities::entry::Entity::find()
			.filter(entities::entry::Column::ParentId.eq(parent_id))
			.filter(entities::entry::Column::Name.eq(name));

		if let Some(e) = ext {
			q = q.filter(entities::entry::Column::Extension.eq(e));
		} else {
			q = q.filter(entities::entry::Column::Extension.is_null());
		}

		let model = q.one(&self.db).await?;
		Ok(model.map(|m| m.id))
	}
}

#[async_trait::async_trait]
impl ChangeHandler for PersistentChangeHandler {
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>> {
		let entry_id = match self.resolve_entry_id(path).await? {
			Some(id) => id,
			None => return Ok(None),
		};

		let entry = entities::entry::Entity::find_by_id(entry_id)
			.one(&self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry {} not found after ID lookup", entry_id))?;

		let kind = match entry.kind {
			0 => EntryKind::File,
			1 => EntryKind::Directory,
			2 => EntryKind::Symlink,
			_ => EntryKind::File,
		};

		Ok(Some(EntryRef {
			id: entry.id,
			uuid: entry.uuid,
			path: path.to_path_buf(),
			kind,
		}))
	}

	async fn find_by_inode(&self, inode: u64) -> Result<Option<EntryRef>> {
		let inode_val = inode as i64;

		let entry = entities::entry::Entity::find()
			.filter(entities::entry::Column::Inode.eq(inode_val))
			.one(&self.db)
			.await?;

		match entry {
			Some(e) => {
				let full_path = crate::ops::indexing::PathResolver::get_full_path(&self.db, e.id)
					.await
					.unwrap_or_else(|_| PathBuf::from(&e.name));

				let kind = match e.kind {
					0 => EntryKind::File,
					1 => EntryKind::Directory,
					2 => EntryKind::Symlink,
					_ => EntryKind::File,
				};

				Ok(Some(EntryRef {
					id: e.id,
					uuid: e.uuid,
					path: full_path,
					kind,
				}))
			}
			None => Ok(None),
		}
	}

	async fn create(&mut self, metadata: &DirEntry, parent_path: &Path) -> Result<EntryRef> {
		use crate::domain::addressing::SdPath;
		use crate::ops::indexing::entry::EntryProcessor;
		use crate::ops::indexing::state::IndexerState;

		let mut state = IndexerState::new(&SdPath::local(&metadata.path));

		if let Some(&parent_id) = self.entry_id_cache.get(parent_path) {
			state
				.entry_id_cache
				.insert(parent_path.to_path_buf(), parent_id);
		} else if let Some(parent_id) = self.resolve_directory_entry_id(parent_path).await? {
			state
				.entry_id_cache
				.insert(parent_path.to_path_buf(), parent_id);
			self.entry_id_cache
				.insert(parent_path.to_path_buf(), parent_id);
		}

		let ctx =
			crate::ops::indexing::ctx::ResponderCtx::new(&self.context, self.library_id).await?;

		let entry_id = EntryProcessor::create_entry(&mut state, &ctx, metadata, 0, parent_path)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create entry: {}", e))?;

		self.entry_id_cache.insert(metadata.path.clone(), entry_id);

		let entry = entities::entry::Entity::find_by_id(entry_id)
			.one(&self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry not found after creation"))?;

		Ok(EntryRef {
			id: entry.id,
			uuid: entry.uuid,
			path: metadata.path.clone(),
			kind: metadata.kind,
		})
	}

	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		use crate::ops::indexing::entry::EntryProcessor;

		let ctx =
			crate::ops::indexing::ctx::ResponderCtx::new(&self.context, self.library_id).await?;
		EntryProcessor::update_entry(&ctx, entry.id, metadata)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to update entry: {}", e))?;

		Ok(())
	}

	async fn move_entry(
		&mut self,
		entry: &EntryRef,
		old_path: &Path,
		new_path: &Path,
		new_parent_path: &Path,
	) -> Result<()> {
		use crate::domain::addressing::SdPath;
		use crate::ops::indexing::entry::EntryProcessor;
		use crate::ops::indexing::state::IndexerState;

		let mut state = IndexerState::new(&SdPath::local(old_path));

		if let Some(&parent_id) = self.entry_id_cache.get(new_parent_path) {
			state
				.entry_id_cache
				.insert(new_parent_path.to_path_buf(), parent_id);
		} else if let Some(parent_id) = self.resolve_directory_entry_id(new_parent_path).await? {
			state
				.entry_id_cache
				.insert(new_parent_path.to_path_buf(), parent_id);
			self.entry_id_cache
				.insert(new_parent_path.to_path_buf(), parent_id);
		}

		let ctx =
			crate::ops::indexing::ctx::ResponderCtx::new(&self.context, self.library_id).await?;
		EntryProcessor::move_entry(
			&mut state,
			&ctx,
			entry.id,
			old_path,
			new_path,
			new_parent_path,
		)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to move entry: {}", e))?;

		self.entry_id_cache.remove(old_path);
		self.entry_id_cache.insert(new_path.to_path_buf(), entry.id);

		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		let mut to_delete_ids: Vec<i32> = vec![entry.id];

		if let Ok(rows) = entities::entry_closure::Entity::find()
			.filter(entities::entry_closure::Column::AncestorId.eq(entry.id))
			.all(&self.db)
			.await
		{
			to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
		}

		let mut queue = vec![entry.id];
		let mut visited = std::collections::HashSet::from([entry.id]);

		while let Some(parent) = queue.pop() {
			if let Ok(children) = entities::entry::Entity::find()
				.filter(entities::entry::Column::ParentId.eq(parent))
				.all(&self.db)
				.await
			{
				for child in children {
					if visited.insert(child.id) {
						to_delete_ids.push(child.id);
						queue.push(child.id);
					}
				}
			}
		}

		to_delete_ids.sort_unstable();
		to_delete_ids.dedup();

		let entries_to_delete = if !to_delete_ids.is_empty() {
			let mut all_entries = Vec::new();
			for chunk in to_delete_ids.chunks(900) {
				let batch = entities::entry::Entity::find()
					.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
					.all(&self.db)
					.await?;
				all_entries.extend(batch);
			}
			all_entries
		} else {
			Vec::new()
		};

		if !entries_to_delete.is_empty() {
			if let Some(library) = self.context.get_library(self.library_id).await {
				let _ = library
					.sync_models_batch(
						&entries_to_delete,
						crate::infra::sync::ChangeType::Delete,
						&self.db,
					)
					.await;
			}
		}

		let txn = self.db.begin().await?;

		if !to_delete_ids.is_empty() {
			let _ = entities::entry_closure::Entity::delete_many()
				.filter(entities::entry_closure::Column::DescendantId.is_in(to_delete_ids.clone()))
				.exec(&txn)
				.await;
			let _ = entities::entry_closure::Entity::delete_many()
				.filter(entities::entry_closure::Column::AncestorId.is_in(to_delete_ids.clone()))
				.exec(&txn)
				.await;
			let _ = entities::directory_paths::Entity::delete_many()
				.filter(entities::directory_paths::Column::EntryId.is_in(to_delete_ids.clone()))
				.exec(&txn)
				.await;
			let _ = entities::entry::Entity::delete_many()
				.filter(entities::entry::Column::Id.is_in(to_delete_ids))
				.exec(&txn)
				.await;
		}

		txn.commit().await?;
		self.entry_id_cache.remove(&entry.path);

		Ok(())
	}

	async fn run_processors(&self, entry: &EntryRef, _is_new: bool) -> Result<()> {
		use crate::ops::indexing::processor::{
			load_location_processor_config, ContentHashProcessor, ProcessorEntry,
		};
		use crate::ops::media::{
			ocr::OcrProcessor, proxy::ProxyProcessor, speech::SpeechToTextProcessor,
			thumbnail::ThumbnailProcessor, thumbstrip::ThumbstripProcessor,
		};

		if entry.is_directory() {
			return Ok(());
		}

		let Some(library) = self.context.get_library(self.library_id).await else {
			return Ok(());
		};

		let proc_config = load_location_processor_config(self.location_id, &self.db)
			.await
			.unwrap_or_default();

		let ctx =
			crate::ops::indexing::ctx::ResponderCtx::new(&self.context, self.library_id).await?;

		// Helper to build ProcessorEntry (re-queries to get latest content_id after hash)
		let build_proc_entry = |db: &sea_orm::DatabaseConnection,
		                        entry: &EntryRef|
		 -> std::pin::Pin<
			Box<dyn std::future::Future<Output = Result<ProcessorEntry>> + Send + '_>,
		> {
			let entry = entry.clone();
			let db = db.clone();
			Box::pin(async move {
				let db_entry = entities::entry::Entity::find_by_id(entry.id)
					.one(&db)
					.await?
					.ok_or_else(|| anyhow::anyhow!("Entry not found"))?;

				let mime_type = if let Some(content_id) = db_entry.content_id {
					if let Ok(Some(ci)) = entities::content_identity::Entity::find_by_id(content_id)
						.one(&db)
						.await
					{
						if let Some(mime_id) = ci.mime_type_id {
							if let Ok(Some(mime)) = entities::mime_type::Entity::find_by_id(mime_id)
								.one(&db)
								.await
							{
								Some(mime.mime_type)
							} else {
								None
							}
						} else {
							None
						}
					} else {
						None
					}
				} else {
					None
				};

				Ok(ProcessorEntry {
					id: entry.id,
					uuid: entry.uuid,
					path: entry.path.clone(),
					kind: entry.kind,
					size: db_entry.size as u64,
					content_id: db_entry.content_id,
					mime_type,
				})
			})
		};

		// Content hash (run first - other processors may need the content_id)
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "content_hash" && c.enabled)
		{
			let proc_entry = build_proc_entry(&self.db, entry).await?;
			let content_proc = ContentHashProcessor::new(self.library_id);
			if let Err(e) = content_proc.process(&ctx, &proc_entry).await {
				tracing::warn!("Content hash processing failed: {}", e);
			}
		}

		// Thumbnail
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "thumbnail" && c.enabled)
		{
			let proc_entry = build_proc_entry(&self.db, entry).await?;
			let thumb_proc = ThumbnailProcessor::new(library.clone());
			if thumb_proc.should_process(&proc_entry) {
				if let Err(e) = thumb_proc.process(&self.db, &proc_entry).await {
					tracing::warn!("Thumbnail processing failed: {}", e);
				}
			}
		}

		// Thumbstrip
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "thumbstrip" && c.enabled)
		{
			let proc_entry = build_proc_entry(&self.db, entry).await?;
			let settings = proc_config
				.watcher_processors
				.iter()
				.find(|c| c.processor_type == "thumbstrip")
				.map(|c| &c.settings);

			let thumbstrip_proc = if let Some(settings) = settings {
				ThumbstripProcessor::new(library.clone())
					.with_settings(settings)
					.unwrap_or_else(|e| {
						tracing::warn!("Failed to parse thumbstrip settings: {}", e);
						ThumbstripProcessor::new(library.clone())
					})
			} else {
				ThumbstripProcessor::new(library.clone())
			};

			if thumbstrip_proc.should_process(&proc_entry) {
				if let Err(e) = thumbstrip_proc.process(&self.db, &proc_entry).await {
					tracing::warn!("Thumbstrip processing failed: {}", e);
				}
			}
		}

		// Proxy
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "proxy" && c.enabled)
		{
			let proc_entry = build_proc_entry(&self.db, entry).await?;
			let settings = proc_config
				.watcher_processors
				.iter()
				.find(|c| c.processor_type == "proxy")
				.map(|c| &c.settings);

			let proxy_proc = if let Some(settings) = settings {
				ProxyProcessor::new(library.clone())
					.with_settings(settings)
					.unwrap_or_else(|e| {
						tracing::warn!("Failed to parse proxy settings: {}", e);
						ProxyProcessor::new(library.clone())
					})
			} else {
				ProxyProcessor::new(library.clone())
			};

			if proxy_proc.should_process(&proc_entry) {
				if let Err(e) = proxy_proc.process(&self.db, &proc_entry).await {
					tracing::warn!("Proxy processing failed: {}", e);
				}
			}
		}

		// OCR
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "ocr" && c.enabled)
		{
			let proc_entry = build_proc_entry(&self.db, entry).await?;
			let ocr_proc = OcrProcessor::new(library.clone());
			if ocr_proc.should_process(&proc_entry) {
				if let Err(e) = ocr_proc.process(&self.db, &proc_entry).await {
					tracing::warn!("OCR processing failed: {}", e);
				}
			}
		}

		// Speech-to-text
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "speech_to_text" && c.enabled)
		{
			let proc_entry = build_proc_entry(&self.db, entry).await?;
			let speech_proc = SpeechToTextProcessor::new(library.clone());
			if speech_proc.should_process(&proc_entry) {
				if let Err(e) = speech_proc.process(&self.db, &proc_entry).await {
					tracing::warn!("Speech-to-text processing failed: {}", e);
				}
			}
		}

		Ok(())
	}

	async fn emit_change_event(&self, entry: &EntryRef, change_type: ChangeType) -> Result<()> {
		use crate::domain::ResourceManager;

		if let Some(uuid) = entry.uuid {
			let resource_manager =
				ResourceManager::new(Arc::new(self.db.clone()), self.context.events.clone());

			if let Err(e) = resource_manager
				.emit_resource_events("entry", vec![uuid])
				.await
			{
				tracing::warn!(
					"Failed to emit resource event for {:?} entry: {}",
					change_type,
					e
				);
			}
		}

		Ok(())
	}

	async fn handle_new_directory(&self, path: &Path) -> Result<()> {
		use crate::domain::addressing::SdPath;
		use crate::ops::indexing::job::{IndexMode, IndexerJob};

		let Some(library) = self.context.get_library(self.library_id).await else {
			return Ok(());
		};

		let index_mode = if let Ok(Some(loc)) = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.location_id))
			.one(&self.db)
			.await
		{
			match loc.index_mode.as_str() {
				"shallow" => IndexMode::Shallow,
				"content" => IndexMode::Content,
				"deep" => IndexMode::Deep,
				_ => IndexMode::Content,
			}
		} else {
			IndexMode::Content
		};

		let indexer_job =
			IndexerJob::from_location(self.location_id, SdPath::local(path), index_mode);

		if let Err(e) = library.jobs().dispatch(indexer_job).await {
			tracing::warn!(
				"Failed to spawn indexer job for directory {}: {}",
				path.display(),
				e
			);
		} else {
			tracing::debug!(
				"Spawned recursive indexer job for directory: {}",
				path.display()
			);
		}

		Ok(())
	}
}
