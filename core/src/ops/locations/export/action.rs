//! Location export action handler

use super::{
	input::LocationExportInput,
	output::{ExportStats, LocationExportOutput},
};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities,
	},
};
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Write, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationExportAction {
	input: LocationExportInput,
}

impl LocationExportAction {
	pub fn new(input: LocationExportInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for LocationExportAction {
	type Input = LocationExportInput;
	type Output = LocationExportOutput;

	fn from_input(input: LocationExportInput) -> Result<Self, String> {
		Ok(LocationExportAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Find the location
		let location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.location_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::LocationNotFound(self.input.location_uuid))?;

		let root_entry_id = location.entry_id.ok_or_else(|| {
			ActionError::Internal("Location has no root entry (not yet indexed)".to_string())
		})?;

		// Get the device that owns this location
		let device = entities::device::Entity::find_by_id(location.device_id)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::Internal("Location's device not found".to_string()))?;

		// Collect all entry IDs in this location's tree using entry_closure
		let entry_ids: Vec<i32> = db
			.query_all(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?",
				vec![root_entry_id.into()],
			))
			.await
			.map_err(ActionError::SeaOrm)?
			.iter()
			.filter_map(|row| row.try_get_by_index::<i32>(0).ok())
			.collect();

		// Build the SQL export
		let mut sql_output = String::new();
		let mut stats = ExportStats {
			entries: 0,
			content_identities: 0,
			user_metadata: 0,
			tags: 0,
			media_data: 0,
		};

		// Header
		sql_output.push_str("-- Spacedrive Location Export\n");
		sql_output.push_str(&format!(
			"-- Location: {} ({})\n",
			location.name.as_deref().unwrap_or("Unnamed"),
			location.uuid
		));
		sql_output.push_str(&format!(
			"-- Exported: {}\n",
			chrono::Utc::now().to_rfc3339()
		));
		sql_output.push_str(&format!("-- Entry count: {}\n", entry_ids.len()));
		sql_output.push_str("\n-- Foreign keys disabled during import\n");
		sql_output.push_str("PRAGMA foreign_keys = OFF;\n\n");
		sql_output.push_str("BEGIN TRANSACTION;\n\n");

		// Export device (needed for location FK)
		sql_output.push_str("-- Device\n");
		sql_output.push_str(&format!(
			"INSERT OR REPLACE INTO devices (uuid, name, slug, os, os_version, hardware_model, is_online, sync_enabled, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n\n",
			sql_uuid(device.uuid),
			sql_string(&device.name),
			sql_string(&device.slug),
			sql_string(&device.os),
			sql_string_opt(&device.os_version),
			sql_string_opt(&device.hardware_model),
			sql_bool(device.is_online),
			sql_bool(device.sync_enabled),
			sql_datetime(device.created_at),
			sql_datetime(device.updated_at),
		));

		// Export location (entry_id will be fixed up after entries are imported)
		sql_output.push_str("-- Location\n");
		sql_output.push_str(&format!(
			"INSERT OR REPLACE INTO locations (uuid, device_id, name, index_mode, scan_state, total_file_count, total_byte_size, created_at, updated_at) \n\
			SELECT {}, d.id, {}, {}, 'pending', {}, {}, {}, {} \n\
			FROM devices d WHERE d.uuid = {};\n\n",
			sql_uuid(location.uuid),
			sql_string_opt(&location.name),
			sql_string(&location.index_mode),
			location.total_file_count,
			location.total_byte_size,
			sql_datetime(location.created_at),
			sql_datetime(location.updated_at),
			sql_uuid(device.uuid),
		));

		// Collect content_ids and metadata_ids from entries for later export
		let mut content_ids: HashSet<i32> = HashSet::new();
		let mut metadata_ids: HashSet<i32> = HashSet::new();

		// Export entries (batch to avoid SQLite variable limit)
		sql_output.push_str("-- Entries\n");
		let mut entries = Vec::new();
		for chunk in entry_ids.chunks(500) {
			let batch = entities::entry::Entity::find()
				.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
				.all(db)
				.await
				.map_err(ActionError::SeaOrm)?;
			entries.extend(batch);
		}

		for entry in &entries {
			if let Some(cid) = entry.content_id {
				content_ids.insert(cid);
			}
			if let Some(mid) = entry.metadata_id {
				metadata_ids.insert(mid);
			}

			// For parent_id, we need to reference by UUID since IDs won't match on import
			let parent_uuid_subquery = if entry.parent_id.is_some() {
				// Find parent's UUID
				let parent = entries.iter().find(|e| Some(e.id) == entry.parent_id);
				match parent.and_then(|p| p.uuid) {
					Some(puuid) => format!(
						"(SELECT id FROM entries WHERE uuid = {})",
						sql_uuid(puuid)
					),
					None => "NULL".to_string(),
				}
			} else {
				"NULL".to_string()
			};

			sql_output.push_str(&format!(
				"INSERT OR REPLACE INTO entries (uuid, name, kind, extension, size, aggregate_size, child_count, file_count, created_at, modified_at, accessed_at, permissions, inode, parent_id) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n",
				sql_uuid_opt(entry.uuid),
				sql_string(&entry.name),
				entry.kind,
				sql_string_opt(&entry.extension),
				entry.size,
				entry.aggregate_size,
				entry.child_count,
				entry.file_count,
				sql_datetime(entry.created_at),
				sql_datetime(entry.modified_at),
				sql_datetime_opt(entry.accessed_at),
				sql_string_opt(&entry.permissions),
				sql_i64_opt(entry.inode),
				parent_uuid_subquery,
			));
			stats.entries += 1;
		}
		sql_output.push('\n');

		// Export directory_paths (batch to avoid SQLite variable limit)
		sql_output.push_str("-- Directory Paths\n");
		let mut dir_paths = Vec::new();
		for chunk in entry_ids.chunks(500) {
			let batch = entities::directory_paths::Entity::find()
				.filter(entities::directory_paths::Column::EntryId.is_in(chunk.to_vec()))
				.all(db)
				.await
				.map_err(ActionError::SeaOrm)?;
			dir_paths.extend(batch);
		}

		for dp in &dir_paths {
			// Find the entry's UUID
			let entry_uuid = entries
				.iter()
				.find(|e| e.id == dp.entry_id)
				.and_then(|e| e.uuid);
			if let Some(uuid) = entry_uuid {
				sql_output.push_str(&format!(
					"INSERT OR REPLACE INTO directory_paths (entry_id, path) SELECT id, {} FROM entries WHERE uuid = {};\n",
					sql_string(&dp.path),
					sql_uuid(uuid),
				));
			}
		}
		sql_output.push('\n');

		// Export content identities if requested
		if self.input.include_content_identities && !content_ids.is_empty() {
			sql_output.push_str("-- Content Identities\n");
			// Batch query to avoid SQLite variable limit
			let content_id_vec: Vec<i32> = content_ids.iter().copied().collect();
			let mut content_identities = Vec::new();
			for chunk in content_id_vec.chunks(500) {
				let batch = entities::content_identity::Entity::find()
					.filter(entities::content_identity::Column::Id.is_in(chunk.to_vec()))
					.all(db)
					.await
					.map_err(ActionError::SeaOrm)?;
				content_identities.extend(batch);
			}

			// Track media data IDs
			let mut image_media_ids: HashSet<i32> = HashSet::new();
			let mut video_media_ids: HashSet<i32> = HashSet::new();
			let mut audio_media_ids: HashSet<i32> = HashSet::new();

			for ci in &content_identities {
				if let Some(id) = ci.image_media_data_id {
					image_media_ids.insert(id);
				}
				if let Some(id) = ci.video_media_data_id {
					video_media_ids.insert(id);
				}
				if let Some(id) = ci.audio_media_data_id {
					audio_media_ids.insert(id);
				}

				sql_output.push_str(&format!(
					"INSERT OR REPLACE INTO content_identities (uuid, content_hash, integrity_hash, kind_id, total_size, entry_count, first_seen_at, last_verified_at, text_content) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {});\n",
					sql_uuid_opt(ci.uuid),
					sql_string(&ci.content_hash),
					sql_string_opt(&ci.integrity_hash),
					ci.kind_id,
					ci.total_size,
					ci.entry_count,
					sql_datetime(ci.first_seen_at),
					sql_datetime(ci.last_verified_at),
					sql_string_opt(&ci.text_content),
				));
				stats.content_identities += 1;
			}
			sql_output.push('\n');

			// Export media data if requested
			if self.input.include_media_data {
				// Image media data
				if !image_media_ids.is_empty() {
					sql_output.push_str("-- Image Media Data\n");
					let image_id_vec: Vec<i32> = image_media_ids.iter().copied().collect();
					let mut image_data = Vec::new();
					for chunk in image_id_vec.chunks(500) {
						let batch = entities::image_media_data::Entity::find()
							.filter(entities::image_media_data::Column::Id.is_in(chunk.to_vec()))
							.all(db)
							.await
							.map_err(ActionError::SeaOrm)?;
						image_data.extend(batch);
					}

					for img in &image_data {
						sql_output.push_str(&format!(
							"INSERT OR REPLACE INTO image_media_data (uuid, width, height, orientation, color_space, color_profile, bit_depth, blurhash, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n",
							sql_uuid(img.uuid),
							img.width,
							img.height,
							sql_i16_opt(img.orientation),
							sql_string_opt(&img.color_space),
							sql_string_opt(&img.color_profile),
							sql_string_opt(&img.bit_depth),
							sql_string_opt(&img.blurhash),
							sql_datetime(img.created_at),
							sql_datetime(img.updated_at),
						));
						stats.media_data += 1;
					}
					sql_output.push('\n');
				}

				// Video media data
				if !video_media_ids.is_empty() {
					sql_output.push_str("-- Video Media Data\n");
					let video_id_vec: Vec<i32> = video_media_ids.iter().copied().collect();
					let mut video_data = Vec::new();
					for chunk in video_id_vec.chunks(500) {
						let batch = entities::video_media_data::Entity::find()
							.filter(entities::video_media_data::Column::Id.is_in(chunk.to_vec()))
							.all(db)
							.await
							.map_err(ActionError::SeaOrm)?;
						video_data.extend(batch);
					}

					for vid in &video_data {
						sql_output.push_str(&format!(
							"INSERT OR REPLACE INTO video_media_data (uuid, width, height, duration_seconds, codec, bit_rate, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {});\n",
							sql_uuid(vid.uuid),
							vid.width,
							vid.height,
							sql_f64_opt(vid.duration_seconds),
							sql_string_opt(&vid.codec),
							sql_i64_opt(vid.bit_rate),
							sql_datetime(vid.created_at),
							sql_datetime(vid.updated_at),
						));
						stats.media_data += 1;
					}
					sql_output.push('\n');
				}

				// Audio media data
				if !audio_media_ids.is_empty() {
					sql_output.push_str("-- Audio Media Data\n");
					let audio_id_vec: Vec<i32> = audio_media_ids.iter().copied().collect();
					let mut audio_data = Vec::new();
					for chunk in audio_id_vec.chunks(500) {
						let batch = entities::audio_media_data::Entity::find()
							.filter(entities::audio_media_data::Column::Id.is_in(chunk.to_vec()))
							.all(db)
							.await
							.map_err(ActionError::SeaOrm)?;
						audio_data.extend(batch);
					}

					for aud in &audio_data {
						sql_output.push_str(&format!(
							"INSERT OR REPLACE INTO audio_media_data (uuid, duration_seconds, sample_rate, channels, codec, title, artist, album, track_number, genre, year, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n",
							sql_uuid(aud.uuid),
							sql_f64_opt(aud.duration_seconds),
							sql_i32_opt(aud.sample_rate),
							sql_string_opt(&aud.channels),
							sql_string_opt(&aud.codec),
							sql_string_opt(&aud.title),
							sql_string_opt(&aud.artist),
							sql_string_opt(&aud.album),
							sql_i32_opt(aud.track_number),
							sql_string_opt(&aud.genre),
							sql_i32_opt(aud.year),
							sql_datetime(aud.created_at),
							sql_datetime(aud.updated_at),
						));
						stats.media_data += 1;
					}
					sql_output.push('\n');
				}
			}

			// Update entries to link to content_identities by UUID
			sql_output.push_str("-- Link entries to content identities\n");
			for entry in &entries {
				if let (Some(entry_uuid), Some(content_id)) = (entry.uuid, entry.content_id) {
					let ci_uuid = content_identities
						.iter()
						.find(|c| c.id == content_id)
						.and_then(|c| c.uuid);
					if let Some(ci_uuid) = ci_uuid {
						sql_output.push_str(&format!(
							"UPDATE entries SET content_id = (SELECT id FROM content_identities WHERE uuid = {}) WHERE uuid = {};\n",
							sql_uuid(ci_uuid),
							sql_uuid(entry_uuid),
						));
					}
				}
			}
			sql_output.push('\n');

			// Link content_identities to media data
			if self.input.include_media_data {
				sql_output.push_str("-- Link content identities to media data\n");
				for ci in &content_identities {
					if let Some(ci_uuid) = ci.uuid {
						if ci.image_media_data_id.is_some() {
							// Find image UUID
							let img_uuid = entities::image_media_data::Entity::find_by_id(
								ci.image_media_data_id.unwrap(),
							)
							.one(db)
							.await
							.map_err(ActionError::SeaOrm)?
							.map(|i| i.uuid);
							if let Some(img_uuid) = img_uuid {
								sql_output.push_str(&format!(
									"UPDATE content_identities SET image_media_data_id = (SELECT id FROM image_media_data WHERE uuid = {}) WHERE uuid = {};\n",
									sql_uuid(img_uuid),
									sql_uuid(ci_uuid),
								));
							}
						}
						if ci.video_media_data_id.is_some() {
							let vid_uuid = entities::video_media_data::Entity::find_by_id(
								ci.video_media_data_id.unwrap(),
							)
							.one(db)
							.await
							.map_err(ActionError::SeaOrm)?
							.map(|v| v.uuid);
							if let Some(vid_uuid) = vid_uuid {
								sql_output.push_str(&format!(
									"UPDATE content_identities SET video_media_data_id = (SELECT id FROM video_media_data WHERE uuid = {}) WHERE uuid = {};\n",
									sql_uuid(vid_uuid),
									sql_uuid(ci_uuid),
								));
							}
						}
						if ci.audio_media_data_id.is_some() {
							let aud_uuid = entities::audio_media_data::Entity::find_by_id(
								ci.audio_media_data_id.unwrap(),
							)
							.one(db)
							.await
							.map_err(ActionError::SeaOrm)?
							.map(|a| a.uuid);
							if let Some(aud_uuid) = aud_uuid {
								sql_output.push_str(&format!(
									"UPDATE content_identities SET audio_media_data_id = (SELECT id FROM audio_media_data WHERE uuid = {}) WHERE uuid = {};\n",
									sql_uuid(aud_uuid),
									sql_uuid(ci_uuid),
								));
							}
						}
					}
				}
				sql_output.push('\n');
			}
		}

		// Export user metadata if requested
		if self.input.include_user_metadata {
			// Get entry UUIDs for this location
			let entry_uuids: Vec<Uuid> = entries.iter().filter_map(|e| e.uuid).collect();

			if !entry_uuids.is_empty() {
				sql_output.push_str("-- User Metadata\n");
				// Batch query to avoid SQLite variable limit
				let mut user_metadata = Vec::new();
				for chunk in entry_uuids.chunks(500) {
					let batch = entities::user_metadata::Entity::find()
						.filter(entities::user_metadata::Column::EntryUuid.is_in(chunk.to_vec()))
						.all(db)
						.await
						.map_err(ActionError::SeaOrm)?;
					user_metadata.extend(batch);
				}

				let mut metadata_uuids: Vec<Uuid> = Vec::new();

				for um in &user_metadata {
					metadata_uuids.push(um.uuid);
					sql_output.push_str(&format!(
						"INSERT OR REPLACE INTO user_metadata (uuid, entry_uuid, content_identity_uuid, notes, favorite, hidden, custom_data, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {});\n",
						sql_uuid(um.uuid),
						sql_uuid_opt(um.entry_uuid),
						sql_uuid_opt(um.content_identity_uuid),
						sql_string_opt(&um.notes),
						sql_bool(um.favorite),
						sql_bool(um.hidden),
						sql_json(&um.custom_data),
						sql_datetime(um.created_at),
						sql_datetime(um.updated_at),
					));
					stats.user_metadata += 1;
				}
				sql_output.push('\n');

				// Link entries to user_metadata
				sql_output.push_str("-- Link entries to user metadata\n");
				for entry in &entries {
					if let (Some(entry_uuid), Some(metadata_id)) = (entry.uuid, entry.metadata_id) {
						let um_uuid = user_metadata
							.iter()
							.find(|m| m.id == metadata_id)
							.map(|m| m.uuid);
						if let Some(um_uuid) = um_uuid {
							sql_output.push_str(&format!(
								"UPDATE entries SET metadata_id = (SELECT id FROM user_metadata WHERE uuid = {}) WHERE uuid = {};\n",
								sql_uuid(um_uuid),
								sql_uuid(entry_uuid),
							));
						}
					}
				}
				sql_output.push('\n');

				// Export tags if requested
				if self.input.include_tags && !metadata_uuids.is_empty() {
					// Get user_metadata_tag junction entries (batched)
					let metadata_ids_for_tags: Vec<i32> =
						user_metadata.iter().map(|m| m.id).collect();
					let mut um_tags = Vec::new();
					for chunk in metadata_ids_for_tags.chunks(500) {
						let batch = entities::user_metadata_tag::Entity::find()
							.filter(
								entities::user_metadata_tag::Column::UserMetadataId
									.is_in(chunk.to_vec()),
							)
							.all(db)
							.await
							.map_err(ActionError::SeaOrm)?;
						um_tags.extend(batch);
					}

					// Collect tag IDs
					let tag_ids: HashSet<i32> = um_tags.iter().map(|t| t.tag_id).collect();

					if !tag_ids.is_empty() {
						sql_output.push_str("-- Tags\n");
						let tag_id_vec: Vec<i32> = tag_ids.iter().copied().collect();
						let mut tags = Vec::new();
						for chunk in tag_id_vec.chunks(500) {
							let batch = entities::tag::Entity::find()
								.filter(entities::tag::Column::Id.is_in(chunk.to_vec()))
								.all(db)
								.await
								.map_err(ActionError::SeaOrm)?;
							tags.extend(batch);
						}

						for tag in &tags {
							sql_output.push_str(&format!(
								"INSERT OR REPLACE INTO tag (uuid, canonical_name, display_name, formal_name, abbreviation, aliases, namespace, tag_type, color, icon, description, is_organizational_anchor, privacy_level, search_weight, attributes, composition_rules, created_at, updated_at, created_by_device) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n",
								sql_uuid(tag.uuid),
								sql_string(&tag.canonical_name),
								sql_string_opt(&tag.display_name),
								sql_string_opt(&tag.formal_name),
								sql_string_opt(&tag.abbreviation),
								sql_json_opt(&tag.aliases),
								sql_string_opt(&tag.namespace),
								sql_string(&tag.tag_type),
								sql_string_opt(&tag.color),
								sql_string_opt(&tag.icon),
								sql_string_opt(&tag.description),
								sql_bool(tag.is_organizational_anchor),
								sql_string(&tag.privacy_level),
								tag.search_weight,
								sql_json_opt(&tag.attributes),
								sql_json_opt(&tag.composition_rules),
								sql_datetime(tag.created_at),
								sql_datetime(tag.updated_at),
								sql_uuid_opt(tag.created_by_device),
							));
							stats.tags += 1;
						}
						sql_output.push('\n');

						// Export user_metadata_tag junction
						sql_output.push_str("-- User Metadata Tags\n");
						for umt in &um_tags {
							let um_uuid = user_metadata
								.iter()
								.find(|m| m.id == umt.user_metadata_id)
								.map(|m| m.uuid);
							let tag_uuid =
								tags.iter().find(|t| t.id == umt.tag_id).map(|t| t.uuid);

							if let (Some(um_uuid), Some(tag_uuid)) = (um_uuid, tag_uuid) {
								sql_output.push_str(&format!(
									"INSERT OR REPLACE INTO user_metadata_tag (uuid, user_metadata_id, tag_id, applied_context, applied_variant, confidence, source, instance_attributes, created_at, updated_at, device_uuid, version) \n\
									SELECT {}, um.id, t.id, {}, {}, {}, {}, {}, {}, {}, {}, {} \n\
									FROM user_metadata um, tag t WHERE um.uuid = {} AND t.uuid = {};\n",
									sql_uuid(umt.uuid),
									sql_string_opt(&umt.applied_context),
									sql_string_opt(&umt.applied_variant),
									umt.confidence,
									sql_string(&umt.source),
									sql_json_opt(&umt.instance_attributes),
									sql_datetime(umt.created_at),
									sql_datetime(umt.updated_at),
									sql_uuid(umt.device_uuid),
									umt.version,
									sql_uuid(um_uuid),
									sql_uuid(tag_uuid),
								));
							}
						}
						sql_output.push('\n');
					}
				}
			}
		}

		// Update location to point to root entry
		let root_entry_uuid = entries
			.iter()
			.find(|e| e.id == root_entry_id)
			.and_then(|e| e.uuid);
		if let Some(root_uuid) = root_entry_uuid {
			sql_output.push_str("-- Link location to root entry\n");
			sql_output.push_str(&format!(
				"UPDATE locations SET entry_id = (SELECT id FROM entries WHERE uuid = {}) WHERE uuid = {};\n\n",
				sql_uuid(root_uuid),
				sql_uuid(location.uuid),
			));
		}

		// Rebuild entry_closure for imported entries
		sql_output.push_str("-- Rebuild entry_closure (self-references)\n");
		sql_output.push_str(
			"INSERT OR IGNORE INTO entry_closure (ancestor_id, descendant_id, depth)\n",
		);
		sql_output.push_str("SELECT id, id, 0 FROM entries WHERE uuid IN (\n");
		for (i, entry) in entries.iter().enumerate() {
			if let Some(uuid) = entry.uuid {
				if i > 0 {
					sql_output.push_str(",\n");
				}
				sql_output.push_str(&format!("  {}", sql_uuid(uuid)));
			}
		}
		sql_output.push_str("\n);\n\n");

		// Rebuild parent-child relationships in entry_closure
		sql_output.push_str("-- Rebuild entry_closure (parent-child relationships)\n");
		sql_output.push_str(
			r#"
-- This needs to be run iteratively after import to build full closure
-- Run until no more rows are inserted
INSERT OR IGNORE INTO entry_closure (ancestor_id, descendant_id, depth)
SELECT ec.ancestor_id, e.id, ec.depth + 1
FROM entries e
INNER JOIN entry_closure ec ON ec.descendant_id = e.parent_id
WHERE e.parent_id IS NOT NULL;
"#,
		);

		sql_output.push_str("\nCOMMIT;\n");
		sql_output.push_str("PRAGMA foreign_keys = ON;\n");

		// Write to file
		let export_path = &self.input.export_path;
		if let Some(parent) = export_path.parent() {
			tokio::fs::create_dir_all(parent).await.map_err(|e| {
				ActionError::io_error(parent.to_string_lossy().to_string(), e)
			})?;
		}

		let mut file = std::fs::File::create(export_path)
			.map_err(|e| ActionError::io_error(export_path.to_string_lossy().to_string(), e))?;
		file.write_all(sql_output.as_bytes())
			.map_err(|e| ActionError::io_error(export_path.to_string_lossy().to_string(), e))?;

		let file_size = file
			.metadata()
			.map(|m| m.len())
			.unwrap_or(sql_output.len() as u64);

		Ok(LocationExportOutput {
			location_uuid: location.uuid,
			location_name: location.name,
			export_path: self.input.export_path,
			file_size_bytes: file_size,
			stats,
		})
	}

	fn action_kind(&self) -> &'static str {
		"locations.export"
	}
}

// SQL formatting helpers
fn sql_string(s: impl std::fmt::Display) -> String {
	let s = s.to_string();
	format!("'{}'", s.replace('\'', "''"))
}

fn sql_string_opt(s: &Option<String>) -> String {
	match s {
		Some(s) => sql_string(s),
		None => "NULL".to_string(),
	}
}

/// Format UUID as SQLite blob (X'...' hex notation)
/// SeaORM stores UUIDs as 16-byte BLOBs in SQLite
fn sql_uuid(u: Uuid) -> String {
	let bytes = u.as_bytes();
	let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
	format!("X'{}'", hex)
}

fn sql_uuid_opt(u: Option<Uuid>) -> String {
	match u {
		Some(u) => sql_uuid(u),
		None => "NULL".to_string(),
	}
}

fn sql_bool(b: bool) -> &'static str {
	if b {
		"1"
	} else {
		"0"
	}
}

fn sql_i16_opt(i: Option<i16>) -> String {
	match i {
		Some(i) => i.to_string(),
		None => "NULL".to_string(),
	}
}

fn sql_i32_opt(i: Option<i32>) -> String {
	match i {
		Some(i) => i.to_string(),
		None => "NULL".to_string(),
	}
}

fn sql_i64_opt(i: Option<i64>) -> String {
	match i {
		Some(i) => i.to_string(),
		None => "NULL".to_string(),
	}
}

fn sql_f64_opt(f: Option<f64>) -> String {
	match f {
		Some(f) => f.to_string(),
		None => "NULL".to_string(),
	}
}

fn sql_datetime(dt: sea_orm::prelude::DateTimeUtc) -> String {
	sql_string(dt.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn sql_datetime_opt(dt: Option<sea_orm::prelude::DateTimeUtc>) -> String {
	match dt {
		Some(dt) => sql_datetime(dt),
		None => "NULL".to_string(),
	}
}

fn sql_json(j: &sea_orm::prelude::Json) -> String {
	sql_string(serde_json::to_string(j).unwrap_or_else(|_| "{}".to_string()))
}

fn sql_json_opt(j: &Option<sea_orm::prelude::Json>) -> String {
	match j {
		Some(j) => sql_json(j),
		None => "NULL".to_string(),
	}
}

// Register action
crate::register_library_action!(LocationExportAction, "locations.export");
