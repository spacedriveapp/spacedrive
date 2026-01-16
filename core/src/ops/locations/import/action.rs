//! Location import action handler

use super::{
	input::LocationImportInput,
	output::{ImportStats, LocationImportOutput},
};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities,
	},
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationImportAction {
	input: LocationImportInput,
}

impl LocationImportAction {
	pub fn new(input: LocationImportInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for LocationImportAction {
	type Input = LocationImportInput;
	type Output = LocationImportOutput;

	fn from_input(input: LocationImportInput) -> Result<Self, String> {
		Ok(LocationImportAction::new(input))
	}

	async fn validate(
		&self,
		_library: &Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		// Check file exists
		if !self.input.import_path.exists() {
			return Err(ActionError::Validation {
				field: "import_path".to_string(),
				message: "Import file does not exist".to_string(),
			});
		}

		// Check it's a file
		if !self.input.import_path.is_file() {
			return Err(ActionError::Validation {
				field: "import_path".to_string(),
				message: "Import path must be a file".to_string(),
			});
		}

		Ok(crate::infra::action::ValidationResult::Success { metadata: None })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Read the SQL file
		let sql_content = tokio::fs::read_to_string(&self.input.import_path)
			.await
			.map_err(|e| {
				ActionError::io_error(self.input.import_path.to_string_lossy().to_string(), e)
			})?;

		// Validate it's a Spacedrive export
		if !sql_content.contains("-- Spacedrive Location Export") {
			return Err(ActionError::Validation {
				field: "import_path".to_string(),
				message: "File is not a valid Spacedrive location export".to_string(),
			});
		}

		// Get current device info for ownership
		let device_uuid = context
			.device_manager
			.device_id()
			.map_err(ActionError::device_manager_error)?;

		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::DeviceNotFound(device_uuid))?;

		let mut stats = ImportStats {
			entries_imported: 0,
			entries_skipped: 0,
			content_identities: 0,
			user_metadata: 0,
			tags: 0,
			media_data: 0,
		};

		// Parse and execute the SQL statements
		// We need to modify device references to point to current device
		let txn = db.begin().await.map_err(ActionError::SeaOrm)?;

		// Disable foreign keys for import
		txn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"PRAGMA foreign_keys = OFF;".to_string(),
		))
		.await
		.map_err(ActionError::SeaOrm)?;

		// Track the location UUID from the import
		let mut location_uuid: Option<Uuid> = None;
		let mut location_name: Option<String> = None;

		// Process statements line by line
		// We need to handle multi-line statements and modify device references
		let mut current_statement = String::new();
		let mut in_transaction = false;

		for line in sql_content.lines() {
			let trimmed = line.trim();

			// Skip comments and empty lines
			if trimmed.starts_with("--") || trimmed.is_empty() {
				// Extract location UUID from comment if present
				if trimmed.contains("Location:") {
					if let Some(uuid_start) = trimmed.find('(') {
						if let Some(uuid_end) = trimmed.find(')') {
							let uuid_str = &trimmed[uuid_start + 1..uuid_end];
							location_uuid = Uuid::parse_str(uuid_str).ok();
						}
					}
				}
				continue;
			}

			// Skip PRAGMA and transaction control (we handle these ourselves)
			if trimmed.starts_with("PRAGMA")
				|| trimmed == "BEGIN TRANSACTION;"
				|| trimmed == "COMMIT;"
			{
				if trimmed == "BEGIN TRANSACTION;" {
					in_transaction = true;
				}
				continue;
			}

			// Accumulate statement
			current_statement.push_str(line);
			current_statement.push('\n');

			// Check if statement is complete
			if !trimmed.ends_with(';') {
				continue;
			}

			let statement = current_statement.trim().to_string();
			current_statement.clear();

			if statement.is_empty() {
				continue;
			}

			// Modify statements to use current device
			let modified_statement = if statement.contains("INSERT")
				&& statement.contains("INTO devices")
			{
				// Skip importing the device - we use current device
				tracing::debug!("Skipping device insert");
				continue;
			} else if statement.contains("INTO locations")
				&& (statement.contains("INSERT") || statement.contains("SELECT"))
			{
				// Modify location insert to use current device
				// Extract location name from the statement
				if let Some(name_match) = extract_location_name(&statement) {
					location_name = Some(name_match);
				}

				// Apply new name if provided
				let final_name = self.input.new_name.clone().or(location_name.clone());

				// Create a modified insert that uses the current device (not SELECT from old device)
				let loc_uuid = location_uuid.unwrap_or_else(Uuid::new_v4);
				let stmt = format!(
					"INSERT OR REPLACE INTO locations (uuid, device_id, name, index_mode, scan_state, total_file_count, total_byte_size, created_at, updated_at) \
					VALUES ({}, {}, {}, 'shallow', 'pending', 0, 0, datetime('now'), datetime('now'));",
					sql_uuid(loc_uuid),
					device.id,
					final_name.as_ref().map(|n| format!("'{}'", n.replace('\'', "''"))).unwrap_or_else(|| "NULL".to_string()),
				);
				tracing::debug!(uuid = %loc_uuid, "Creating location insert statement");
				stmt
			} else if statement.contains("FROM devices d WHERE d.uuid")
				|| statement.contains("FROM devices WHERE")
			{
				// Skip statements that reference the old device by UUID
				tracing::debug!("Skipping statement referencing old device");
				continue;
			} else {
				statement
			};

			// Execute the statement
			let result = txn
				.execute(Statement::from_string(
					DbBackend::Sqlite,
					modified_statement.clone(),
				))
				.await;

			match result {
				Ok(exec_result) => {
					let rows = exec_result.rows_affected();

					// Track statistics
					if modified_statement.contains("INTO entries") {
						if rows > 0 {
							stats.entries_imported += rows;
						} else if self.input.skip_existing {
							stats.entries_skipped += 1;
						}
					} else if modified_statement.contains("INTO content_identities") {
						stats.content_identities += rows;
					} else if modified_statement.contains("INTO user_metadata") {
						stats.user_metadata += rows;
					} else if modified_statement.contains("INTO tag") {
						stats.tags += rows;
					} else if modified_statement.contains("INTO image_media_data")
						|| modified_statement.contains("INTO video_media_data")
						|| modified_statement.contains("INTO audio_media_data")
					{
						stats.media_data += rows;
					}
				}
				Err(e) => {
					// Log but continue on non-critical errors
					tracing::warn!(
						error = %e,
						statement = %modified_statement.chars().take(100).collect::<String>(),
						"Failed to execute import statement"
					);
				}
			}
		}

		// Rebuild entry_closure table for imported entries
		// Run iteratively until no more rows are inserted
		let mut iterations = 0;
		loop {
			let result = txn
				.execute(Statement::from_string(
					DbBackend::Sqlite,
					r#"
					INSERT OR IGNORE INTO entry_closure (ancestor_id, descendant_id, depth)
					SELECT ec.ancestor_id, e.id, ec.depth + 1
					FROM entries e
					INNER JOIN entry_closure ec ON ec.descendant_id = e.parent_id
					WHERE e.parent_id IS NOT NULL
					  AND NOT EXISTS (
						SELECT 1 FROM entry_closure
						WHERE ancestor_id = ec.ancestor_id
						  AND descendant_id = e.id
					  )
					"#
					.to_string(),
				))
				.await
				.map_err(ActionError::SeaOrm)?;

			iterations += 1;
			if result.rows_affected() == 0 || iterations > 100 {
				break;
			}
		}

		// Re-enable foreign keys
		txn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"PRAGMA foreign_keys = ON;".to_string(),
		))
		.await
		.map_err(ActionError::SeaOrm)?;

		// Commit transaction
		txn.commit().await.map_err(ActionError::SeaOrm)?;

		// Get the final location info
		let final_uuid = location_uuid.unwrap_or_else(Uuid::new_v4);
		let imported_location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(final_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?;

		let final_name = imported_location
			.as_ref()
			.and_then(|l| l.name.clone())
			.or(self.input.new_name)
			.or(location_name);

		Ok(LocationImportOutput {
			location_uuid: final_uuid,
			location_name: final_name,
			import_path: self.input.import_path,
			stats,
		})
	}

	fn action_kind(&self) -> &'static str {
		"locations.import"
	}
}

/// Format UUID as SQLite blob (X'...' hex notation)
fn sql_uuid(u: Uuid) -> String {
	let bytes = u.as_bytes();
	let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
	format!("X'{}'", hex)
}

/// Extract location name from INSERT statement
fn extract_location_name(statement: &str) -> Option<String> {
	// Look for pattern like: , 'LocationName', 'shallow'
	// or: , NULL, 'shallow'
	let lower = statement.to_lowercase();
	if let Some(idx) = lower.find("'shallow'") {
		let before = &statement[..idx];
		// Find the previous comma
		if let Some(comma_idx) = before.rfind(',') {
			let name_part = before[comma_idx + 1..].trim();
			if name_part == "NULL" {
				return None;
			}
			// Remove quotes
			if name_part.starts_with('\'') && name_part.ends_with('\'') {
				let name = &name_part[1..name_part.len() - 1];
				return Some(name.replace("''", "'"));
			}
		}
	}
	None
}

// Register action
crate::register_library_action!(LocationImportAction, "locations.import");
