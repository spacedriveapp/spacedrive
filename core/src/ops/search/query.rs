//! File search query implementation

use super::{
	input::{FileSearchInput, SearchScope},
	output::{EnhancedFileSearchOutput, EnhancedFileSearchResult, FileSearchOutput},
};
use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	filetype::FileTypeRegistry,
	infra::db::entities::{
		content_identity, directory_paths, entry, sidecar, tag, user_metadata_tag,
	},
	infra::query::LibraryQuery,
};
use chrono::{DateTime, Utc};
use sea_orm::{
	ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType,
	QueryFilter, QueryOrder, QuerySelect, RelationTrait, Statement,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// File search query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSearchQuery {
	pub input: FileSearchInput,
}

impl FileSearchQuery {
	pub fn new(input: FileSearchInput) -> Self {
		Self { input }
	}
}

impl LibraryQuery for FileSearchQuery {
	type Input = FileSearchInput;
	type Output = FileSearchOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let start_time = std::time::Instant::now();

		// Validate input
		self.input.validate().map_err(|e| {
			QueryError::Internal(format!("Invalid search input: {}", e.to_string()))
		})?;

		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db();
		let search_id = Uuid::new_v4();

		// Determine which index to use (ephemeral or persistent)
		let index_type = self.determine_index_type(&context, db.conn()).await?;

		tracing::info!(
			"Search query: '{}', scope: {:?}, index: {:?}, mode: {:?}, limit: {}, offset: {}",
			self.input.query,
			self.input.scope,
			index_type,
			self.input.mode,
			self.input.pagination.limit,
			self.input.pagination.offset
		);

		match index_type {
			crate::ops::search::IndexType::Persistent => {
				// Build device slug lookup map from database
				use std::collections::HashMap;
				let devices = crate::infra::db::entities::device::Entity::find()
					.all(db.conn())
					.await
					.map_err(QueryError::SeaOrm)?;
				let device_slug_map: HashMap<Uuid, String> = devices
					.into_iter()
					.map(|device| (device.uuid, device.slug))
					.collect();

				// Perform the search based on mode
				let results = match self.input.mode {
					crate::ops::search::input::SearchMode::Fast => {
						self.execute_fast_search(db.conn(), &device_slug_map)
							.await?
					}
					crate::ops::search::input::SearchMode::Normal => {
						self.execute_normal_search(db.conn(), &device_slug_map)
							.await?
					}
					crate::ops::search::input::SearchMode::Full => {
						self.execute_full_search(db.conn(), &device_slug_map)
							.await?
					}
				};

				let execution_time = start_time.elapsed().as_millis() as u64;

				// Get actual total count for pagination
				let total_count = self
				.get_total_count(db.conn(), context.file_type_registry())
				.await
				.unwrap_or(0);

				// Create output with persistent index type
				let output = FileSearchOutput::new_persistent(
					results,
					total_count,
					search_id,
					execution_time,
				);

				Ok(output)
			}
			crate::ops::search::IndexType::Ephemeral => {
				// Use ephemeral search
				self.execute_ephemeral_search(context, search_id, start_time)
					.await
			}
			crate::ops::search::IndexType::Hybrid => {
				// Future: search both and merge results
				Err(QueryError::Internal(
					"Hybrid search not yet implemented".to_string(),
				))
			}
		}
	}
}

impl FileSearchQuery {
	/// Construct full path for an entry by joining with directory_paths
	async fn construct_full_path(
		&self,
		entry_model: &entry::Model,
		db: &DatabaseConnection,
	) -> QueryResult<String> {
		// If this is a root entry (no parent), return just the name
		if entry_model.parent_id.is_none() {
			return Ok(format!("/{}", entry_model.name));
		}

		// Find the parent directory entry
		if let Some(parent_id) = entry_model.parent_id {
			// Look up the directory path in directory_paths table
			let directory_path = directory_paths::Entity::find()
				.filter(directory_paths::Column::EntryId.eq(parent_id))
				.one(db)
				.await?
				.ok_or_else(|| {
					QueryError::Internal(format!(
						"Directory path not found for parent_id: {}",
						parent_id.to_string()
					))
				})?;

			// Construct full path: directory_path + "/" + filename
			let full_path = if directory_path.path.ends_with('/') {
				format!("{}{}", directory_path.path, entry_model.name)
			} else {
				format!("{}/{}", directory_path.path, entry_model.name)
			};

			Ok(full_path)
		} else {
			// Fallback for entries without parent
			Ok(format!("/{}", entry_model.name))
		}
	}

	/// Execute fast search using FTS5 with efficient batch joins
	pub async fn execute_fast_search(
		&self,
		db: &DatabaseConnection,
		_device_slug_map: &std::collections::HashMap<Uuid, String>,
	) -> QueryResult<Vec<crate::ops::search::output::FileSearchResult>> {
		use sea_orm::Statement;

		// For empty queries (recents view), skip FTS and query entries directly
		if self.input.query.trim().is_empty() {
			return self.execute_fast_search_no_fts(db).await;
		}

		// Use FTS5 for high-performance text search
		let fts_query = self.build_fts5_query();
		let fts_results = self.execute_fts5_search(db, &fts_query).await?;

		let fts_count = fts_results.len();
		tracing::info!(
			"FTS5 search returned {} results for query '{}'",
			fts_count,
			self.input.query
		);

		if fts_results.is_empty() {
			return Ok(Vec::new());
		}

		// Build a map of entry_id -> bm25_score for later lookup
		let score_map: std::collections::HashMap<i32, f64> = fts_results.iter().cloned().collect();
		let entry_ids: Vec<i32> = fts_results.iter().map(|(id, _)| *id).collect();

		// Build single efficient query with all joins
		let entry_ids_str = entry_ids
			.iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>()
			.join(",");

		// Join directory_paths on parent_id directly - every directory has its full
		// absolute path stored, so the parent of any file gives us the containing folder path.
		let sql_query = format!(
			r#"
			SELECT
				e.id as entry_id,
				e.uuid as entry_uuid,
				e.name as entry_name,
				e.kind as entry_kind,
				e.extension as entry_extension,
				e.size as entry_size,
				e.aggregate_size as entry_aggregate_size,
				e.child_count as entry_child_count,
				e.file_count as entry_file_count,
				e.created_at as entry_created_at,
				e.modified_at as entry_modified_at,
				e.accessed_at as entry_accessed_at,
				e.content_id as entry_content_id,
				dp.path as parent_path,
				d.slug as device_slug,
				ck.name as content_kind_name,
				ci.uuid as content_identity_uuid,
				ci.content_hash as content_hash,
				ci.integrity_hash as integrity_hash,
				ci.mime_type_id as mime_type_id,
				ci.text_content as text_content,
				ci.total_size as ci_total_size,
				ci.entry_count as ci_entry_count,
				ci.first_seen_at as first_seen_at,
				ci.last_verified_at as last_verified_at
			FROM entries e
			LEFT JOIN directory_paths dp ON dp.entry_id = e.parent_id
			LEFT JOIN volumes v ON e.volume_id = v.id
			LEFT JOIN devices d ON v.device_id = d.uuid
			LEFT JOIN content_identities ci ON e.content_id = ci.id
			LEFT JOIN content_kinds ck ON ci.kind_id = ck.id
			WHERE e.id IN ({})
			"#,
			entry_ids_str
		);

		let rows = db
			.query_all(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				sql_query,
			))
			.await?;

		// Collect all content UUIDs for batch sidecar query
		let content_uuids: Vec<Uuid> = rows
			.iter()
			.filter_map(|row| {
				row.try_get::<Option<Uuid>>("", "content_identity_uuid")
					.ok()
					.flatten()
			})
			.collect();

		// Batch fetch all sidecars for these content UUIDs
		let all_sidecars = if !content_uuids.is_empty() {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.is_in(content_uuids.clone()))
				.all(db)
				.await?
		} else {
			Vec::new()
		};

		// Group sidecars by content_uuid for fast lookup
		let mut sidecars_by_content: std::collections::HashMap<
			Uuid,
			Vec<crate::domain::file::Sidecar>,
		> = std::collections::HashMap::new();
		for s in all_sidecars {
			sidecars_by_content
				.entry(s.content_uuid)
				.or_insert_with(Vec::new)
				.push(crate::domain::file::Sidecar {
					id: s.id,
					content_uuid: s.content_uuid,
					kind: s.kind,
					variant: s.variant,
					format: s.format,
					status: s.status,
					size: s.size,
					created_at: s.created_at,
					updated_at: s.updated_at,
				});
		}

		// Convert results to FileSearchResult objects
		let mut results = Vec::new();
		let relevance_calc =
			crate::ops::search::sorting::RelevanceCalculator::new(self.input.query.clone());

		for row in rows {
			let entry_id: i32 = row.try_get("", "entry_id").unwrap_or(0);
			let entry_uuid: Option<Uuid> = row.try_get("", "entry_uuid").ok();
			let entry_name: String = row.try_get("", "entry_name").unwrap_or_default();
			let entry_kind: i32 = row.try_get("", "entry_kind").unwrap_or(0);
			let entry_extension: Option<String> = row.try_get("", "entry_extension").ok();
			let entry_size: i64 = row.try_get("", "entry_size").unwrap_or(0);
			let entry_aggregate_size: i64 = row.try_get("", "entry_aggregate_size").unwrap_or(0);
			let entry_child_count: i32 = row.try_get("", "entry_child_count").unwrap_or(0);
			let entry_file_count: i32 = row.try_get("", "entry_file_count").unwrap_or(0);
			let entry_created_at: chrono::DateTime<chrono::Utc> = row
				.try_get("", "entry_created_at")
				.unwrap_or_else(|_| chrono::Utc::now());
			let entry_modified_at: chrono::DateTime<chrono::Utc> = row
				.try_get("", "entry_modified_at")
				.unwrap_or_else(|_| chrono::Utc::now());
			let entry_accessed_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "entry_accessed_at").ok();

			// Get joined data
			let parent_path: Option<String> = row.try_get("", "parent_path").ok();
			let device_slug: Option<String> = row.try_get("", "device_slug").ok();
			let content_kind_name: Option<String> = row.try_get("", "content_kind_name").ok();
			let content_identity_uuid: Option<Uuid> =
				row.try_get("", "content_identity_uuid").ok().flatten();

			// Content identity fields for building ContentIdentity object
			let content_hash: Option<String> = row.try_get("", "content_hash").ok();
			let integrity_hash: Option<String> = row.try_get("", "integrity_hash").ok();
			let mime_type_id: Option<i32> = row.try_get("", "mime_type_id").ok();
			let text_content: Option<String> = row.try_get("", "text_content").ok();
			let ci_total_size: Option<i64> = row.try_get("", "ci_total_size").ok();
			let ci_entry_count: Option<i32> = row.try_get("", "ci_entry_count").ok();
			let first_seen_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "first_seen_at").ok();
			let last_verified_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "last_verified_at").ok();

			// Build full path: parent directory path + filename (with extension)
			let file_path = if let Some(dir_path) = parent_path {
				let file_name = if let Some(ext) = &entry_extension {
					format!("{}.{}", entry_name, ext)
				} else {
					entry_name.clone()
				};
				if dir_path.ends_with('/') {
					format!("{}{}", dir_path, file_name)
				} else {
					format!("{}/{}", dir_path, file_name)
				}
			} else {
				// Entry has no parent (is a root entry) - use name directly
				format!("/{}", entry_name)
			};

			// Create SdPath with device_slug from join
			let sd_path = SdPath::Physical {
				device_slug: device_slug.unwrap_or_else(|| "unknown-device".to_string()),
				path: file_path.into(),
			};

			// Create entity model for File conversion
			let entity_model = entry::Model {
				id: entry_id,
				uuid: entry_uuid,
				name: entry_name.clone(),
				kind: entry_kind,
				extension: entry_extension.clone(),
				metadata_id: None,
				content_id: None,
				size: entry_size,
				aggregate_size: entry_aggregate_size,
				child_count: entry_child_count,
				file_count: entry_file_count,
				created_at: entry_created_at,
				modified_at: entry_modified_at,
				accessed_at: entry_accessed_at,
				indexed_at: None,
				permissions: None,
				inode: None,
				parent_id: None,
				volume_id: None,
			};

			// Convert to File
			let mut file = File::from_entity_model(entity_model, sd_path);

			// Build and set content identity if we have the required fields
			if let (Some(ci_uuid), Some(ci_hash), Some(ci_first_seen), Some(ci_last_verified)) = (
				content_identity_uuid,
				content_hash,
				first_seen_at,
				last_verified_at,
			) {
				// Convert content_kind name to ContentKind enum
				let kind = content_kind_name
					.as_ref()
					.map(|name| crate::domain::ContentKind::from(name.as_str()))
					.unwrap_or(crate::domain::ContentKind::Unknown);

				file.content_identity = Some(crate::domain::content_identity::ContentIdentity {
					uuid: ci_uuid,
					kind,
					content_hash: ci_hash,
					integrity_hash,
					mime_type_id,
					text_content,
					total_size: ci_total_size.unwrap_or(0),
					entry_count: ci_entry_count.unwrap_or(0),
					first_seen_at: ci_first_seen,
					last_verified_at: ci_last_verified,
				});
				file.content_kind = kind;

				// Add sidecars from batch lookup
				if let Some(sidecars) = sidecars_by_content.get(&ci_uuid) {
					file.sidecars = sidecars.clone();
				}
			} else if let Some(kind_name) = content_kind_name {
				// Fallback: set content kind even without full content identity
				file.content_kind = crate::domain::ContentKind::from(kind_name.as_str());
			}

			// Get BM25 score and calculate final score
			let bm25_score = score_map.get(&entry_id).copied().unwrap_or(0.0);
			let recency_boost = relevance_calc.calculate_recency_boost(file.modified_at);
			let user_preference_boost = relevance_calc.calculate_user_preference_boost(entry_id);
			let final_score = bm25_score as f32 + recency_boost + user_preference_boost;

			let result = crate::ops::search::output::FileSearchResult {
				file,
				score: final_score,
				score_breakdown: crate::ops::search::output::ScoreBreakdown::new(
					bm25_score as f32,
					None,
					0.0,
					recency_boost,
					user_preference_boost,
				),
				highlights: self.extract_highlights(&fts_query, &entry_name, &entry_extension),
				matched_content: None,
			};

			results.push(result);
		}

		tracing::info!(
			"Built {} FileSearchResult objects from {} FTS5 results",
			results.len(),
			fts_count
		);

		// Sort by final score
		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		Ok(results)
	}

	/// Execute normal search with FTS5 + enhanced ranking
	async fn execute_normal_search(
		&self,
		db: &DatabaseConnection,
		device_slug_map: &std::collections::HashMap<Uuid, String>,
	) -> QueryResult<Vec<crate::ops::search::output::FileSearchResult>> {
		// Use FTS5 as base, then enhance with additional ranking factors
		let mut results = self.execute_fast_search(db, device_slug_map).await?;

		// Enhanced ranking for normal search
		for result in &mut results {
			let mut enhanced_score = result.score;

			// Add metadata-based scoring
			let size = result.file.size;
			// Slightly boost files with reasonable sizes
			if size > 1024 && size < 10_000_000 {
				// 1KB to 10MB
				enhanced_score += 0.1;
			}

			// Boost files with extensions that match common document types
			if let Some(ref extension) = result.file.extension {
				match extension.as_str() {
					"pdf" | "doc" | "docx" | "txt" | "md" => enhanced_score += 0.2,
					"jpg" | "png" | "gif" | "webp" => enhanced_score += 0.1,
					_ => {}
				}
			}

			// Update the score
			result.score = enhanced_score;
			result.score_breakdown.metadata_score = enhanced_score
				- result.score_breakdown.temporal_score
				- result.score_breakdown.recency_boost
				- result.score_breakdown.user_preference_boost;
		}

		// Re-sort with enhanced scores
		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		Ok(results)
	}

	/// Execute full search with FTS5 + content analysis
	async fn execute_full_search(
		&self,
		db: &DatabaseConnection,
		device_slug_map: &std::collections::HashMap<Uuid, String>,
	) -> QueryResult<Vec<crate::ops::search::output::FileSearchResult>> {
		// Start with normal search results
		let mut results = self.execute_normal_search(db, device_slug_map).await?;

		// For full search, we would add content analysis here
		// This is a placeholder for future implementation
		for result in &mut results {
			// TODO: Add content extraction and analysis
			// For now, just add a small boost for files that might have content
			let size = result.file.size;
			if size > 0 && size < 100_000_000 {
				// Up to 100MB
				result.score += 0.05;
				result.score_breakdown.metadata_score += 0.05;
			}
		}

		// Re-sort with content analysis scores
		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		Ok(results)
	}

	/// Apply scope filters to the query condition
	fn apply_scope_filter(&self, mut condition: Condition) -> Condition {
		match &self.input.scope {
			crate::ops::search::input::SearchScope::Library => {
				// No additional filtering needed for library-wide search
				condition
			}
			crate::ops::search::input::SearchScope::Location { location_id } => {
				// TODO: Add location filtering when location_id is available in entry table
				condition
			}
			crate::ops::search::input::SearchScope::Path { path } => {
				// Path-based filtering is handled in the main query with directory_paths join
				// No additional condition needed here as it's applied in the query building
				condition
			}
		}
	}

	/// Apply additional filters to the query condition
	fn apply_filters(&self, mut condition: Condition, registry: &FileTypeRegistry) -> Condition {
		// File type filter
		if let Some(file_types) = &self.input.filters.file_types {
			if !file_types.is_empty() {
				let mut file_type_condition = Condition::any();
				for file_type in file_types {
					file_type_condition =
						file_type_condition.add(entry::Column::Extension.eq(file_type));
				}
				condition = condition.add(file_type_condition);
			}
		}

		// Date range filter
		if let Some(date_range) = &self.input.filters.date_range {
			let date_column = match date_range.field {
				crate::ops::search::input::DateField::CreatedAt => entry::Column::CreatedAt,
				crate::ops::search::input::DateField::ModifiedAt => entry::Column::ModifiedAt,
				crate::ops::search::input::DateField::AccessedAt => entry::Column::AccessedAt,
				crate::ops::search::input::DateField::IndexedAt => entry::Column::IndexedAt,
			};

			if let Some(start) = date_range.start {
				condition = condition.add(date_column.gte(start));
			}
			if let Some(end) = date_range.end {
				condition = condition.add(date_column.lte(end));
			}
		}

		// Size range filter
		if let Some(size_range) = &self.input.filters.size_range {
			if let Some(min) = size_range.min {
				condition = condition.add(entry::Column::Size.gte(min as i64));
			}
			if let Some(max) = size_range.max {
				condition = condition.add(entry::Column::Size.lte(max as i64));
			}
		}

		// Content type filter using file type registry
		if let Some(content_types) = &self.input.filters.content_types {
			if !content_types.is_empty() {
				let mut content_condition = Condition::any();
				for content_type in content_types {
					let extensions = registry.get_extensions_for_category(*content_type);
					for extension in extensions {
						content_condition =
							content_condition.add(entry::Column::Extension.eq(extension));
					}
				}
				condition = condition.add(content_condition);
			}
		}

		// Location filter - join with locations table
		if let Some(locations) = &self.input.filters.locations {
			if !locations.is_empty() {
				let mut location_condition = Condition::any();
				for location_id in locations {
					// We need to join with locations table to filter by location UUID
					// This will be handled in the main query with a join
					location_condition = location_condition
						.add(crate::infra::db::entities::location::Column::Uuid.eq(*location_id));
				}
				condition = condition.add(location_condition);
			}
		}

		// Include hidden filter
		if let Some(include_hidden) = self.input.filters.include_hidden {
			if !include_hidden {
				// TODO: Add hidden field to entry table
				// condition = condition.add(entry::Column::Hidden.eq(false));
			}
		}

		condition
	}

	/// Get device UUID for an entry by traversing parent chain to find location root
	async fn get_device_uuid_for_entry(
		&self,
		entry_model: &entry::Model,
		db: &DatabaseConnection,
	) -> Option<Uuid> {
		use sea_orm::{sea_query::Expr, Statement};

		// Traverse parent chain to find the location root using recursive CTE
		let query = format!(
			r#"
			WITH RECURSIVE parent_chain AS (
				SELECT id, parent_id FROM entries WHERE id = {}
				UNION ALL
				SELECT e.id, e.parent_id FROM entries e
				INNER JOIN parent_chain pc ON e.id = pc.parent_id
			)
			SELECT l.device_id
			FROM parent_chain pc
			INNER JOIN locations l ON l.entry_id = pc.id
			LIMIT 1
			"#,
			entry_model.id
		);

		let result = db
			.query_one(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				query,
			))
			.await
			.ok()??;

		let device_db_id: i32 = result.try_get("", "device_id").ok()?;

		// Now get the device UUID from the device_id
		if let Ok(Some(device)) =
			crate::infra::db::entities::device::Entity::find_by_id(device_db_id)
				.one(db)
				.await
		{
			return Some(device.uuid);
		}

		None
	}

	/// Get location UUID for an entry
	async fn get_location_uuid_for_entry(
		&self,
		entry_model: &entry::Model,
		db: &DatabaseConnection,
	) -> Option<Uuid> {
		// Look up location by entry_id
		if let Ok(location) = crate::infra::db::entities::location::Entity::find()
			.filter(crate::infra::db::entities::location::Column::EntryId.eq(entry_model.id))
			.one(db)
			.await
		{
			location.map(|l| l.uuid)
		} else {
			None
		}
	}

	/// Get metadata UUID for an entry
	async fn get_metadata_uuid_for_entry(
		&self,
		metadata_id: i32,
		db: &DatabaseConnection,
	) -> Option<Uuid> {
		if let Ok(metadata) =
			crate::infra::db::entities::user_metadata::Entity::find_by_id(metadata_id)
				.one(db)
				.await
		{
			metadata.map(|m| m.uuid)
		} else {
			None
		}
	}

	/// Get content UUID for an entry
	async fn get_content_uuid_for_entry(
		&self,
		content_id: i32,
		db: &DatabaseConnection,
	) -> Option<Uuid> {
		if let Ok(content) =
			crate::infra::db::entities::content_identity::Entity::find_by_id(content_id)
				.one(db)
				.await
		{
			content.and_then(|c| c.uuid)
		} else {
			None
		}
	}

	/// Get parent UUID for an entry
	async fn get_parent_uuid_for_entry(
		&self,
		parent_id: i32,
		db: &DatabaseConnection,
	) -> Option<Uuid> {
		if let Ok(parent) = entry::Entity::find_by_id(parent_id).one(db).await {
			parent.and_then(|p| p.uuid)
		} else {
			None
		}
	}

	/// Get content kind for an entry by looking up content identity
	async fn get_content_kind_for_entry(
		&self,
		content_id: i32,
		db: &DatabaseConnection,
	) -> Option<crate::domain::ContentKind> {
		use sea_orm::Statement;

		let query = format!(
			r#"
			SELECT ck.name
			FROM content_identities ci
			INNER JOIN content_kinds ck ON ci.kind_id = ck.id
			WHERE ci.id = {}
			"#,
			content_id
		);

		let result = db
			.query_one(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				query,
			))
			.await
			.ok()??;

		let kind_name: String = result.try_get("", "name").ok()?;
		Some(crate::domain::ContentKind::from(kind_name.as_str()))
	}

	/// Get total count of matching entries for pagination
	async fn get_total_count(
		&self,
		db: &DatabaseConnection,
		registry: &FileTypeRegistry,
	) -> QueryResult<u64> {
		let mut condition = Condition::any()
			.add(entry::Column::Name.contains(&self.input.query))
			.add(entry::Column::Extension.contains(&self.input.query));

		// Apply scope filters
		condition = self.apply_scope_filter(condition);

		// Apply additional filters
		condition = self.apply_filters(condition, registry);

		// Build count query
		let mut query = entry::Entity::find()
			.filter(condition)
			.filter(entry::Column::Kind.eq(0)); // Only files

		// Add location join if location filtering is needed
		if self.input.filters.locations.is_some() {
			query = query.join(
				JoinType::LeftJoin,
				crate::infra::db::entities::location::Relation::Entry.def(),
			);
		}

		// Apply SD path filtering if specified in scope
		if let SearchScope::Path { path } = &self.input.scope {
			if let Some(device_id) = path.device_id() {
				if let Some(path_str) = path.path() {
					// Join with directory_paths to filter by path
					query = query
						.join(JoinType::LeftJoin, directory_paths::Relation::Entry.def())
						.filter(
							directory_paths::Column::Path
								.like(&format!("{}%", path_str.to_string_lossy())),
						);
				}
			}
		}

		// For queries with joins, we need to use a different approach
		// We'll execute the query and count the results
		let entries = query.all(db).await?;
		Ok(entries.len() as u64)
	}

	/// Convert an entry model to a FileSearchResult with device and path resolution
	async fn entry_to_search_result(
		&self,
		entry_model: entry::Model,
		db: &DatabaseConnection,
		score: f32,
	) -> QueryResult<Option<crate::ops::search::output::FileSearchResult>> {
		// Skip entries without volume_id (can't determine device slug)
		let Some(volume_id) = entry_model.volume_id else {
			return Ok(None);
		};

		// Get volume to find device info
		let Some(volume) = crate::infra::db::entities::volume::Entity::find_by_id(volume_id)
			.one(db)
			.await? else {
			return Ok(None);
		};

		// Get device to find slug
		let Some(device) = crate::infra::db::entities::device::Entity::find()
			.filter(crate::infra::db::entities::device::Column::Uuid.eq(volume.device_id))
			.one(db)
			.await? else {
			return Ok(None);
		};

		// Construct full path
		let full_path = self.construct_full_path(&entry_model, db).await?;

		// Build SD path
		let sd_path = crate::domain::addressing::SdPath::Physical {
			device_slug: device.slug,
			path: full_path.into(),
		};

		// Use File::from_entity_model to properly construct the file
		let file = crate::domain::File::from_entity_model(entry_model, sd_path);

		Ok(Some(crate::ops::search::output::FileSearchResult {
			file,
			score,
			score_breakdown: crate::ops::search::output::ScoreBreakdown::new(
				score,
				None,
				0.0,
				0.0,
				0.0,
			),
			highlights: Vec::new(),
			matched_content: None,
		}))
	}

	/// Execute fast search without FTS (for empty queries like recents view)
	async fn execute_fast_search_no_fts(
		&self,
		db: &DatabaseConnection,
	) -> QueryResult<Vec<crate::ops::search::output::FileSearchResult>> {
		use crate::ops::search::filters::FilterBuilder;
		use crate::ops::search::sorting::SortBuilder;

		tracing::info!(
			"Executing fast search without FTS (empty query), sorting by {:?}",
			self.input.sort.field
		);

		// Build query with filters and sorting
		let mut query = entry::Entity::find();

		// Apply filters
		let filter_builder = FilterBuilder::new()
			.file_types(&self.input.filters.file_types)
			.date_range(&self.input.filters.date_range)
			.size_range(&self.input.filters.size_range);

		query = query.filter(filter_builder.build());

		// Apply sorting
		let sort_builder = SortBuilder::new().apply_sort(&self.input.sort);
		for (column, order) in sort_builder.build() {
			match column.as_str() {
				"name" => query = query.order_by(entry::Column::Name, order),
				"size" => query = query.order_by(entry::Column::Size, order),
				"modified_at" => query = query.order_by(entry::Column::ModifiedAt, order),
				"created_at" => query = query.order_by(entry::Column::CreatedAt, order),
				"indexed_at" => query = query.order_by(entry::Column::IndexedAt, order),
				_ => {}
			}
		}

		// Apply pagination
		query = query
			.limit(self.input.pagination.limit as u64)
			.offset(self.input.pagination.offset as u64);

		// Execute query
		let entries = query.all(db).await?;

		tracing::info!(
			"Fast search without FTS returned {} entries",
			entries.len()
		);

		// Convert entries to FileSearchResult using helper
		let mut results = Vec::new();
		for entry_model in entries {
			if let Some(result) = self.entry_to_search_result(entry_model, db, 1.0).await? {
				results.push(result);
			}
		}

		Ok(results)
	}

	pub fn build_fts5_query(&self) -> String {
		// Escape special FTS5 characters and build query
		let escaped_query = self
			.input
			.query
			.replace('"', r#"\""#)
			.replace('\'', r#"\'"#)
			.replace('*', r#"\*"#)
			.replace('(', r#"\("#)
			.replace(')', r#"\)"#);

		// Add prefix matching for autocomplete if query is long enough
		if self.input.query.len() > 2 {
			format!("{}*", escaped_query)
		} else {
			escaped_query
		}
	}

	/// Execute FTS5 search with BM25 ranking
	async fn execute_fts5_search(
		&self,
		db: &DatabaseConnection,
		query: &str,
	) -> QueryResult<Vec<(i32, f64)>> {
		let sql = match &self.input.scope {
			SearchScope::Path { path } => {
				if let Some(path_str) = path.path() {
					// FTS5 search with path filtering
					r#"
						WITH fts AS (
							SELECT rowid, bm25(search_index) AS rank
							FROM search_index
							WHERE search_index MATCH ?
							ORDER BY rank
							LIMIT 5000
						)
						SELECT e.id, fts.rank
						FROM fts
						JOIN entries e ON e.id = fts.rowid
						JOIN directory_paths dp ON dp.entry_id = e.parent_id
						WHERE dp.path LIKE ?
						ORDER BY fts.rank
						LIMIT ? OFFSET ?
					"#
				} else {
					// Basic FTS5 search
					r#"
						SELECT e.id, bm25(search_index) as rank
						FROM search_index
						JOIN entries e ON e.id = search_index.rowid
						WHERE search_index MATCH ?
						ORDER BY rank
						LIMIT ? OFFSET ?
					"#
				}
			}
			_ => {
				// Basic FTS5 search
				r#"
					SELECT e.id, bm25(search_index) as rank
					FROM search_index
					JOIN entries e ON e.id = search_index.rowid
					WHERE search_index MATCH ?
					ORDER BY rank
					LIMIT ? OFFSET ?
				"#
			}
		};

		let statement = Statement::from_string(db.get_database_backend(), sql.to_string());

		let params = match &self.input.scope {
			SearchScope::Path { path } if path.path().is_some() => {
				let path_str = path.path().unwrap().to_string_lossy();
				let like_pattern = format!("{}%", path_str);
				tracing::info!(
					"Path scope FTS5: query='{}', LIKE pattern='{}'",
					query,
					like_pattern
				);
				vec![
					query.into(),
					like_pattern.into(),
					self.input.pagination.limit.to_string().into(),
					self.input.pagination.offset.to_string().into(),
				]
			}
			_ => {
				vec![
					query.into(),
					self.input.pagination.limit.to_string().into(),
					self.input.pagination.offset.to_string().into(),
				]
			}
		};

		let results = db
			.query_all(Statement::from_sql_and_values(
				db.get_database_backend(),
				&statement.sql,
				params,
			))
			.await?;

		let mut fts_results = Vec::new();
		for row in results {
			let entry_id: i32 = row.try_get("", "id")?;
			let rank: f64 = row.try_get("", "rank")?;
			fts_results.push((entry_id, rank));
		}

		Ok(fts_results)
	}

	/// Check if an entry passes additional (non-text) filters
	async fn passes_additional_filters(
		&self,
		entry_model: &entry::Model,
		db: &DatabaseConnection,
		registry: &FileTypeRegistry,
	) -> QueryResult<bool> {
		// File type filter
		if let Some(file_types) = &self.input.filters.file_types {
			if !file_types.is_empty() {
				if let Some(ref extension) = entry_model.extension {
					if !file_types.contains(extension) {
						return Ok(false);
					}
				} else {
					return Ok(false);
				}
			}
		}

		// Date range filter
		if let Some(date_range) = &self.input.filters.date_range {
			let date_to_check = match date_range.field {
				crate::ops::search::input::DateField::CreatedAt => Some(entry_model.created_at),
				crate::ops::search::input::DateField::ModifiedAt => Some(entry_model.modified_at),
				crate::ops::search::input::DateField::AccessedAt => entry_model.accessed_at,
				crate::ops::search::input::DateField::IndexedAt => entry_model.indexed_at,
			};

			if let Some(date) = date_to_check {
				if let Some(start) = date_range.start {
					if date < start {
						return Ok(false);
					}
				}
				if let Some(end) = date_range.end {
					if date > end {
						return Ok(false);
					}
				}
			}
		}

		// Size range filter
		if let Some(size_range) = &self.input.filters.size_range {
			if let Some(min) = size_range.min {
				if (entry_model.size as u64) < min {
					return Ok(false);
				}
			}
			if let Some(max) = size_range.max {
				if (entry_model.size as u64) > max {
					return Ok(false);
				}
			}
		}

		// Content type filter using file type registry
		if let Some(content_types) = &self.input.filters.content_types {
			if !content_types.is_empty() {
				let mut matches_content_type = false;

				for content_type in content_types {
					let extensions = registry.get_extensions_for_category(*content_type);
					if let Some(ref extension) = entry_model.extension {
						if extensions.contains(&extension.as_str()) {
							matches_content_type = true;
							break;
						}
					}
				}

				if !matches_content_type {
					return Ok(false);
				}
			}
		}

		// Location filter
		if let Some(locations) = &self.input.filters.locations {
			if !locations.is_empty() {
				// Check if entry belongs to one of the specified locations
				if let Ok(Some(location)) = crate::infra::db::entities::location::Entity::find()
					.filter(
						crate::infra::db::entities::location::Column::EntryId.eq(entry_model.id),
					)
					.one(db)
					.await
				{
					if !locations.contains(&location.uuid) {
						return Ok(false);
					}
				} else {
					return Ok(false);
				}
			}
		}

		Ok(true)
	}

	/// Extract text highlights from FTS5 results
	pub fn extract_highlights(
		&self,
		query: &str,
		name: &str,
		extension: &Option<String>,
	) -> Vec<crate::ops::search::output::TextHighlight> {
		let mut highlights = Vec::new();

		// Highlight matches in filename
		let name_lower = name.to_lowercase();
		let query_lower = query.replace('*', "").to_lowercase();

		if let Some(start) = name_lower.find(&query_lower) {
			highlights.push(crate::ops::search::output::TextHighlight {
				field: "name".to_string(),
				text: name.to_string(),
				start,
				end: start + query_lower.len(),
			});
		}

		// Highlight matches in extension
		if let Some(ref ext) = extension {
			let ext_lower = ext.to_lowercase();
			if let Some(start) = ext_lower.find(&query_lower) {
				highlights.push(crate::ops::search::output::TextHighlight {
					field: "extension".to_string(),
					text: ext.clone(),
					start,
					end: start + query_lower.len(),
				});
			}
		}

		highlights
	}

	/// Execute search and return File objects with joined data
	/// This method uses SQL joins to efficiently load all related data in one query
	pub async fn execute_with_files(
		&self,
		db: &DatabaseConnection,
	) -> QueryResult<Vec<EnhancedFileSearchResult>> {
		// Build device slug lookup map from database
		use std::collections::HashMap;
		let devices = crate::infra::db::entities::device::Entity::find()
			.all(db)
			.await
			.map_err(QueryError::SeaOrm)?;
		let device_slug_map: HashMap<Uuid, String> = devices
			.into_iter()
			.map(|device| (device.uuid, device.slug))
			.collect();

		// First get the basic search results
		let entry_results = match self.input.mode {
			crate::ops::search::input::SearchMode::Fast => {
				self.execute_fast_search(db, &device_slug_map).await?
			}
			crate::ops::search::input::SearchMode::Normal => {
				self.execute_normal_search(db, &device_slug_map).await?
			}
			crate::ops::search::input::SearchMode::Full => {
				self.execute_full_search(db, &device_slug_map).await?
			}
		};

		if entry_results.is_empty() {
			return Ok(Vec::new());
		}

		// Convert to enhanced results (already have File objects)
		let enhanced_results = entry_results
			.into_iter()
			.map(|result| EnhancedFileSearchResult {
				file: result.file,
				score: result.score,
				score_breakdown: result.score_breakdown,
				highlights: result.highlights,
				matched_content: result.matched_content,
			})
			.collect();

		Ok(enhanced_results)
	}

	/// Determine which index type to use for this search
	async fn determine_index_type(
		&self,
		context: &Arc<CoreContext>,
		db: &DatabaseConnection,
	) -> QueryResult<crate::ops::search::IndexType> {
		use crate::ops::search::IndexType;

		match &self.input.scope {
			SearchScope::Path { path } => {
				// Check if location has IndexMode::None
				if let Some(should_use_ephemeral) = self.check_location_index_mode(path, db).await {
					if should_use_ephemeral {
						return Ok(IndexType::Ephemeral);
					}
				}

				// Try to find path in database
				match self.find_parent_directory(path, db).await {
					Ok(_) => Ok(IndexType::Persistent),
					Err(_) => {
						// Path not indexed - check if ephemeral cache has it
						let cache = context.ephemeral_cache();
						let local_path = match path {
							SdPath::Physical { path, .. } => path.clone(),
							_ => return Ok(IndexType::Persistent), // Default to persistent for non-physical
						};

						if cache.is_indexed(&local_path) {
							Ok(IndexType::Ephemeral)
						} else {
							// Not indexed anywhere yet - will use ephemeral
							// (directory listing would trigger indexing here)
							tracing::debug!(
								"Path not in any index, defaulting to ephemeral: {}",
								local_path.display()
							);
							Ok(IndexType::Ephemeral)
						}
					}
				}
			}
			SearchScope::Location { .. } => {
				// Locations are always persistent (by definition)
				Ok(IndexType::Persistent)
			}
			SearchScope::Library => {
				// Global search only searches persistent
				Ok(IndexType::Persistent)
			}
		}
	}

	/// Check if a location has IndexMode::None (should use ephemeral)
	async fn check_location_index_mode(
		&self,
		path: &SdPath,
		db: &DatabaseConnection,
	) -> Option<bool> {
		use crate::infra::db::entities::location;

		match path {
			SdPath::Physical {
				device_slug: _,
				path,
			} => {
				let path_str = path.to_string_lossy().to_string();

				// Get all locations and find the one that is a parent of this path
				if let Ok(locations) = location::Entity::find().all(db).await {
					for loc in locations {
						// Get the location's root path
						if let Some(entry_id) = loc.entry_id {
							if let Ok(Some(dir_path)) =
								directory_paths::Entity::find_by_id(entry_id).one(db).await
							{
								// Check if this location's path is a parent of the requested path
								if path_str.starts_with(&dir_path.path) {
									// Check if index_mode is "none"
									return Some(loc.index_mode == "none");
								}
							}
						}
					}
				}
				None
			}
			_ => None,
		}
	}

	/// Find parent directory entry for a given path
	async fn find_parent_directory(
		&self,
		path: &SdPath,
		db: &DatabaseConnection,
	) -> QueryResult<entry::Model> {
		match path {
			SdPath::Physical { path, .. } => {
				// Get the directory path string
				let path_str = path.to_string_lossy().to_string();

				// Look up in directory_paths table
				let dir_path = directory_paths::Entity::find()
					.filter(directory_paths::Column::Path.eq(&path_str))
					.one(db)
					.await?;

				if let Some(dir_path) = dir_path {
					// Get the entry
					let entry = entry::Entity::find_by_id(dir_path.entry_id)
						.one(db)
						.await?
						.ok_or_else(|| {
							QueryError::Internal("Entry not found for directory path".to_string())
						})?;

					Ok(entry)
				} else {
					Err(QueryError::Internal(
						"Path not found in database".to_string(),
					))
				}
			}
			_ => Err(QueryError::Internal(
				"Only physical paths supported for directory lookup".to_string(),
			)),
		}
	}

	/// Execute search using ephemeral index
	async fn execute_ephemeral_search(
		&self,
		context: Arc<CoreContext>,
		search_id: Uuid,
		start_time: std::time::Instant,
	) -> QueryResult<FileSearchOutput> {
		let path = match &self.input.scope {
			SearchScope::Path { path } => path,
			_ => {
				return Err(QueryError::Internal(
					"Ephemeral search requires Path scope".to_string(),
				))
			}
		};

		let cache = context.ephemeral_cache();
		let results = crate::ops::search::ephemeral_search::search_ephemeral_index(
			&self.input.query,
			path,
			&self.input.filters,
			cache,
			context.file_type_registry(),
		)
		.await?;

		let execution_time = start_time.elapsed().as_millis() as u64;
		let total = results.len() as u64;

		Ok(FileSearchOutput::new_ephemeral(
			results,
			total,
			search_id,
			execution_time,
		))
	}
}

crate::register_library_query!(FileSearchQuery, "search.files");
