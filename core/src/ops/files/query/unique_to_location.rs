//! Query to find files that exist only in a specific location (not backed up elsewhere)
//!
//! This query uses content identity to identify files that are unique to a given location,
//! meaning their content hash doesn't appear in any other location. This is useful for
//! backup purposes to identify files that need to be backed up.

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::db::entities::{content_identity, entry, location},
	infra::query::LibraryQuery,
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter,
	QuerySelect, RelationTrait, Statement,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for finding files unique to a location
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UniqueToLocationInput {
	/// The location ID to find unique files for
	pub location_id: Uuid,
	/// Optional limit on number of results
	pub limit: Option<u32>,
}

/// Output containing files that are unique to the specified location
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UniqueToLocationOutput {
	/// Files that exist only in the specified location
	pub unique_files: Vec<File>,
	/// Total count of unique files
	pub total_count: u32,
	/// Total size of unique files in bytes
	pub total_size: u64,
}

/// Query to find files unique to a location
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UniqueToLocationQuery {
	pub input: UniqueToLocationInput,
}

impl UniqueToLocationQuery {
	pub fn new(location_id: Uuid) -> Self {
		Self {
			input: UniqueToLocationInput {
				location_id,
				limit: None,
			},
		}
	}

	pub fn with_limit(location_id: Uuid, limit: u32) -> Self {
		Self {
			input: UniqueToLocationInput {
				location_id,
				limit: Some(limit),
			},
		}
	}
}

impl LibraryQuery for UniqueToLocationQuery {
	type Input = UniqueToLocationInput;
	type Output = UniqueToLocationOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
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

		let db = library.db().conn();

		// Execute the query to find unique files
		let (unique_files, total_size) = self.find_unique_files_in_location(db).await?;

		let total_count = unique_files.len() as u32;

		Ok(UniqueToLocationOutput {
			unique_files,
			total_count,
			total_size,
		})
	}
}

impl UniqueToLocationQuery {
	/// Find files that exist only in the specified location
	async fn find_unique_files_in_location(
		&self,
		db: &DatabaseConnection,
	) -> QueryResult<(Vec<File>, u64)> {
		// First, get the location's root entry ID
		let location_model = location::Entity::find()
			.filter(location::Column::Uuid.eq(self.input.location_id))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal("Location not found".to_string()))?;

		let location_root_entry_id = location_model.entry_id;

		// Query to find content identities that exist only in this location
		// This uses a subquery to find content_hash values that appear in only one location
		let unique_content_hashes_query = r#"
			WITH location_content AS (
				-- Get all content hashes from the specified location
				SELECT DISTINCT ci.content_hash
				FROM content_identities ci
				JOIN entries e ON e.content_id = ci.id
				JOIN entry_closure ec ON ec.descendant_id = e.id
				WHERE ec.ancestor_id = ?1
				  AND e.kind = 0  -- Files only
			),
			other_locations_content AS (
				-- Get all content hashes from other locations
				SELECT DISTINCT ci.content_hash
				FROM content_identities ci
				JOIN entries e ON e.content_id = ci.id
				JOIN entry_closure ec ON ec.descendant_id = e.id
				WHERE ec.ancestor_id != ?1
				  AND e.kind = 0  -- Files only
			)
			-- Find content hashes that are in our location but not in others
			SELECT lc.content_hash
			FROM location_content lc
			LEFT JOIN other_locations_content olc ON lc.content_hash = olc.content_hash
			WHERE olc.content_hash IS NULL
		"#;

		let unique_content_hashes: Vec<String> = db
			.query_all(Statement::from_sql_and_values(
				sea_orm::DatabaseBackend::Sqlite,
				unique_content_hashes_query,
				[location_root_entry_id.into()],
			))
			.await?
			.into_iter()
			.map(|row| row.try_get("", "content_hash").unwrap_or_default())
			.collect();

		if unique_content_hashes.is_empty() {
			return Ok((Vec::new(), 0));
		}

		// Now get all files from this location that have these unique content hashes
		let files_query = r#"
			SELECT
				e.id as entry_id,
				e.name,
				e.size,
				e.created_at,
				e.updated_at,
				ci.content_hash,
				ci.uuid as content_uuid
			FROM entries e
			JOIN content_identities ci ON e.content_id = ci.id
			JOIN entry_closure ec ON ec.descendant_id = e.id
			WHERE ec.ancestor_id = ?1
			  AND e.kind = 0  -- Files only
			  AND ci.content_hash IN (
				SELECT DISTINCT ci2.content_hash
				FROM content_identities ci2
				JOIN entries e2 ON e2.content_id = ci2.id
				JOIN entry_closure ec2 ON ec2.descendant_id = e2.id
				WHERE ec2.ancestor_id = ?1
				  AND e2.kind = 0
				  AND ci2.content_hash NOT IN (
					SELECT DISTINCT ci3.content_hash
					FROM content_identities ci3
					JOIN entries e3 ON e3.content_id = ci3.id
					JOIN entry_closure ec3 ON ec3.descendant_id = e3.id
					WHERE ec3.ancestor_id != ?1
					  AND e3.kind = 0
				  )
			  )
			ORDER BY e.name
		"#;

		let limit_clause = if let Some(limit) = self.input.limit {
			format!("LIMIT {}", limit)
		} else {
			String::new()
		};

		let final_query = if limit_clause.is_empty() {
			files_query.to_string()
		} else {
			format!("{} {}", files_query, limit_clause)
		};

		let file_rows = db
			.query_all(Statement::from_sql_and_values(
				sea_orm::DatabaseBackend::Sqlite,
				&final_query,
				[location_root_entry_id.into()],
			))
			.await?;

		let mut files = Vec::new();
		let mut total_size = 0u64;

		for row in file_rows {
			let entry_id: i32 = row.try_get("", "entry_id").unwrap_or(0);
			let name: String = row.try_get("", "name").unwrap_or_default();
			let size: i64 = row.try_get("", "size").unwrap_or(0);
			let content_hash: String = row.try_get("", "content_hash").unwrap_or_default();

			total_size += size as u64;

			// Create entity model for conversion
			let entity_model = entry::Model {
				id: entry_id,
				uuid: None,
				name: name.clone(),
				kind: 0, // File
				extension: name.split('.').last().map(|s| s.to_string()),
				metadata_id: None,
				content_id: None,
				size,
				aggregate_size: 0,
				child_count: 0,
				file_count: 0,
				created_at: chrono::Utc::now(),
				modified_at: chrono::Utc::now(),
				accessed_at: None,
				permissions: None,
				inode: None,
				parent_id: None,
			};

			// Create placeholder SdPath
			let sd_path = SdPath::Physical {
				device_id: Uuid::new_v4(),
				path: format!("/unknown/path/{}", name).into(),
			};

			let file = File::from_entity_model(entity_model, sd_path);
			files.push(file);
		}

		Ok((files, total_size))
	}
}

// Register the query
crate::register_library_query!(UniqueToLocationQuery, "files.unique_to_location");
