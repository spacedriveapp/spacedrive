//! Projects a media source's records into `File`s so the standard explorer
//! surfaces (media view, quick preview, inspector) render them like any
//! other file.
//!
//! File-backed sources (Apple Photos) store assertions: rows carrying
//! root-relative path evidence about files the filesystem index owns. The
//! projection joins each assertion to its filesystem record here, at read
//! time — the bound file gets the filesystem identity and real size, the
//! assertion supplies capture time and the catalog's presentation name. An
//! assertion whose file the index doesn't hold yet (the walk hasn't reached
//! the bundle, or an iCloud original is evicted) still surfaces, under its
//! stable archive identity, as known-but-not-resident. Foreign sources have
//! no file root and project their records directly.

use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, file::EntryKind, file::File, ContentKind},
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::sources::registry,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceMediaListingInput {
	pub source_id: String,
	pub limit: u32,
	pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceMediaListingOutput {
	pub files: Vec<File>,
	pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMediaListingQuery {
	pub input: SourceMediaListingInput,
}

impl LibraryQuery for SourceMediaListingQuery {
	type Input = SourceMediaListingInput;
	type Output = SourceMediaListingOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		if input.source_id.trim().is_empty() {
			return Err(QueryError::Validation {
				field: "source_id".to_string(),
				message: "source_id cannot be empty".to_string(),
			});
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		if library.source_manager().is_none() {
			library
				.init_source_manager()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to init source manager: {e}")))?;
		}
		let source_manager = library
			.source_manager()
			.ok_or_else(|| QueryError::Internal("Source manager not available".to_string()))?;

		let store_id = registry::parse_store_id(&self.input.source_id)
			.map_err(|e| QueryError::Internal(format!("{e}")))?;

		let limit = (self.input.limit as usize).min(2000);
		let records = source_manager
			.list_records_full(&store_id, limit, self.input.offset as usize)
			.await
			.map_err(QueryError::Internal)?;

		let has_more = records.len() >= limit;
		let device_slug = crate::device::get_current_device_slug();

		let file_root = source_manager
			.file_root(&store_id)
			.await
			.map_err(QueryError::Internal)?;

		let files = match &file_root {
			Some(root) => bind_to_filesystem(&context, &records, root, &device_slug).await,
			None => records
				.iter()
				.filter_map(|record| record_to_file(record, &device_slug, None))
				.collect(),
		};

		Ok(SourceMediaListingOutput { files, has_more })
	}
}

/// Join assertion rows to the filesystem records that own their files. One
/// partition covers the whole root, so the index lock is taken once for the
/// page rather than per row.
async fn bind_to_filesystem(
	context: &Arc<CoreContext>,
	records: &[serde_json::Value],
	root: &Path,
	device_slug: &str,
) -> Vec<File> {
	let cache = context.ephemeral_cache();
	// The partition covering the root restores from its snapshot if this
	// session hasn't loaded it; without it every assertion stays unbound.
	cache.ensure_restored(root).await;
	let index = cache.resolve_index(root);
	let mut index = index.write().await;

	let mut files = Vec::with_capacity(records.len());
	for record in records {
		let Some(mut file) = record_to_file(record, device_slug, Some(root)) else {
			continue;
		};
		if let SdPath::Physical { path, .. } = &file.sd_path {
			let path = path.clone();
			if let Some(metadata) = index.get_entry_ref(&path) {
				// Bound: the filesystem record's identity and bytes win; the
				// assertion keeps supplying capture time and display name.
				file.id = index.get_or_assign_uuid(&path);
				file.size = metadata.size;
			}
		}
		files.push(file);
	}
	files
}

fn record_to_file(
	record: &serde_json::Value,
	device_slug: &str,
	file_root: Option<&Path>,
) -> Option<File> {
	let str_field = |key: &str| record.get(key).and_then(|v| v.as_str());

	// A file-backed source's paths are root-relative evidence; a foreign
	// source's are already locators.
	let absolutize = |raw: PathBuf| match file_root {
		Some(root) if raw.is_relative() => root.join(raw),
		_ => raw,
	};

	let original_path = absolutize(PathBuf::from(str_field("original_path")?));
	let id = str_field("id").and_then(|s| Uuid::parse_str(s).ok())?;

	// Presentation name: the catalog's original filename (IMG_0001.HEIC)
	// beats the opaque name Photos stores originals under on disk.
	let on_disk_name = original_path
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or("unknown");
	let display = str_field("filename")
		.filter(|s| !s.is_empty())
		.unwrap_or(on_disk_name);
	let display_path = Path::new(display);
	let extension = display_path
		.extension()
		.and_then(|e| e.to_str())
		.map(|s| s.to_lowercase());
	let name = display_path
		.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or(display)
		.to_string();

	let content_kind = match str_field("media_type") {
		Some("video") => ContentKind::Video,
		_ => ContentKind::Image,
	};

	let captured_at: DateTime<Utc> = str_field("captured_at")
		.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
		.map(|dt| dt.with_timezone(&Utc))
		.unwrap_or_else(Utc::now);

	Some(File {
		id,
		sd_path: SdPath::Physical {
			device_slug: device_slug.to_string(),
			path: original_path,
		},
		kind: EntryKind::File,
		name,
		extension,
		size: 0,
		content_identity: None,
		alternate_paths: Vec::new(),
		tags: Vec::new(),
		sidecars: Vec::new(),
		image_media_data: None,
		video_media_data: None,
		audio_media_data: None,
		created_at: captured_at,
		modified_at: captured_at,
		accessed_at: None,
		content_kind,
		is_local: true,
		duration_seconds: None,
		thumbnail_path: str_field("thumb_path").map(|p| absolutize(PathBuf::from(p))),
	})
}

crate::register_library_query!(SourceMediaListingQuery, "sources.media_listing");

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn file_backed_records_absolutize_and_prefer_catalog_names() {
		let record = json!({
			"id": "018f0000-0000-7000-8000-000000000001",
			"original_path": "originals/A/B/0A1B2C3D.heic",
			"filename": "IMG_0001.HEIC",
			"thumb_path": "resources/derivatives/0/0A1B2C3D_1_105_c.jpeg",
			"media_type": "image",
			"captured_at": "2024-06-01T12:00:00.000Z",
		});
		let root = Path::new("/Users/james/Pictures/Photos Library.photoslibrary");
		let file = record_to_file(&record, "mac", Some(root)).unwrap();

		assert_eq!(
			file.sd_path,
			SdPath::Physical {
				device_slug: "mac".to_string(),
				path: root.join("originals/A/B/0A1B2C3D.heic"),
			}
		);
		assert_eq!(file.name, "IMG_0001");
		assert_eq!(file.extension.as_deref(), Some("heic"));
		assert_eq!(
			file.thumbnail_path.as_deref(),
			Some(
				root.join("resources/derivatives/0/0A1B2C3D_1_105_c.jpeg")
					.as_path()
			)
		);
		assert_eq!(file.content_kind, ContentKind::Image);
	}

	#[test]
	fn foreign_records_pass_through_unchanged() {
		let record = json!({
			"id": "018f0000-0000-7000-8000-000000000002",
			"original_path": "/Volumes/Media/clip.mov",
			"media_type": "video",
		});
		let file = record_to_file(&record, "mac", None).unwrap();

		assert_eq!(
			file.sd_path,
			SdPath::Physical {
				device_slug: "mac".to_string(),
				path: PathBuf::from("/Volumes/Media/clip.mov"),
			}
		);
		assert_eq!(file.name, "clip");
		assert_eq!(file.content_kind, ContentKind::Video);
	}

	#[test]
	fn records_without_evidence_are_dropped() {
		let record = json!({ "id": "018f0000-0000-7000-8000-000000000003" });
		assert!(record_to_file(&record, "mac", None).is_none());
	}
}
