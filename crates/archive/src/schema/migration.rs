//! Schema migration: detect changes, apply safe migrations, refuse destructive ones.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::schema::{DataTypeSchema, FieldType, ModelDef};

/// Result of a schema migration attempt.
#[derive(Debug, Clone)]
pub struct MigrationResult {
	/// Migration actions that were applied successfully.
	pub applied: Vec<MigrationAction>,
	/// Whether any changes were refused (destructive).
	pub has_refused_changes: bool,
	/// Details about refused changes.
	pub refused_details: Vec<String>,
}

/// A single migration action that was applied.
#[derive(Debug, Clone)]
pub enum MigrationAction {
	AddTable { name: String },
	AddColumn { table: String, column: String },
	AddFtsColumn { column: String },
}

/// Compare two schemas and generate migration actions.
pub fn diff_schemas(old: &DataTypeSchema, new: &DataTypeSchema) -> MigrationResult {
	let mut applied = Vec::new();
	let mut refused_details = Vec::new();

	// Check for new models
	for (name, new_model) in &new.models {
		if !old.models.contains_key(name) {
			// New model — safe to add
			applied.push(MigrationAction::AddTable { name: name.clone() });
		} else {
			// Existing model — check for new fields
			let old_model = &old.models[name];
			for (field_name, _field_type) in &new_model.fields {
				if !old_model.fields.contains_key(field_name) {
					applied.push(MigrationAction::AddColumn {
						table: format!("{name}s"),
						column: field_name.clone(),
					});
				}
			}
		}
	}

	// Check for removed models (destructive — refuse)
	for name in old.models.keys() {
		if !new.models.contains_key(name) {
			refused_details.push(format!("model removed: {name} (destructive)"));
		}
	}

	// Check for removed fields (destructive — refuse)
	for (name, old_model) in &old.models {
		if let Some(new_model) = new.models.get(name) {
			for field_name in old_model.fields.keys() {
				if !new_model.fields.contains_key(field_name) {
					refused_details
						.push(format!("field removed: {name}.{field_name} (destructive)"));
				}
			}
		}
	}

	// Check for changed field types (destructive — refuse)
	for (name, old_model) in &old.models {
		if let Some(new_model) = new.models.get(name) {
			for (field_name, old_type) in &old_model.fields {
				if let Some(new_type) = new_model.fields.get(field_name) {
					if old_type != new_type {
						refused_details.push(format!(
							"field type changed: {name}.{field_name} from {old_type:?} to {new_type:?} (destructive)"
						));
					}
				}
			}
		}
	}

	// Check for new FTS fields
	let old_fts: std::collections::HashSet<&str> = old
		.search
		.search_fields
		.iter()
		.filter(|f| !f.starts_with("_derived."))
		.map(|f| f.as_str())
		.collect();
	let new_fts: std::collections::HashSet<&str> = new
		.search
		.search_fields
		.iter()
		.filter(|f| !f.starts_with("_derived."))
		.map(|f| f.as_str())
		.collect();

	for field in &new_fts {
		if !old_fts.contains(field) {
			applied.push(MigrationAction::AddFtsColumn {
				column: field.to_string(),
			});
		}
	}

	let has_refused_changes = !refused_details.is_empty();

	MigrationResult {
		applied,
		has_refused_changes,
		refused_details,
	}
}

/// Compute a hash of a schema for comparison.
pub fn schema_hash(schema: &DataTypeSchema) -> String {
	let toml = match toml::to_string_pretty(schema) {
		Ok(s) => s,
		Err(_) => return String::new(),
	};
	blake3::hash(toml.as_bytes()).to_hex().to_string()
}
