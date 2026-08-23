//! Schema migration: detect changes, report safe ones, refuse destructive ones.
//!
//! Applying the changes is the idempotent DDL path in [`crate::source`] — new
//! facet tables come from `CREATE TABLE IF NOT EXISTS`, new columns from
//! `ensure_facet_columns`. What this module produces is the description of what
//! changed, and the refusal when a change would drop data.

use std::collections::HashSet;

use crate::record::facet_table;
use crate::schema::DataTypeSchema;

/// Result of comparing a stored schema against the current one.
#[derive(Debug, Clone)]
pub struct MigrationResult {
	/// Changes that the idempotent DDL path applies.
	pub applied: Vec<MigrationAction>,
	/// Whether any changes were refused as destructive.
	pub has_refused_changes: bool,
	/// Details about refused changes.
	pub refused_details: Vec<String>,
	/// Whether the set of indexed search fields changed, which invalidates the
	/// existing search index.
	pub search_fields_changed: bool,
}

/// A single migration action.
#[derive(Debug, Clone)]
pub enum MigrationAction {
	AddTable { name: String },
	AddColumn { table: String, column: String },
	AddFtsColumn { column: String },
}

/// Compare two schemas and describe the migration between them.
pub fn diff_schemas(old: &DataTypeSchema, new: &DataTypeSchema) -> MigrationResult {
	let mut applied = Vec::new();
	let mut refused_details = Vec::new();

	for (name, new_model) in &new.models {
		match old.models.get(name) {
			None => applied.push(MigrationAction::AddTable {
				name: facet_table(name),
			}),
			Some(old_model) => {
				for field_name in new_model.fields.keys() {
					if !old_model.fields.contains_key(field_name) {
						applied.push(MigrationAction::AddColumn {
							table: facet_table(name),
							column: field_name.clone(),
						});
					}
				}
			}
		}
	}

	for name in old.models.keys() {
		if !new.models.contains_key(name) {
			refused_details.push(format!("model removed: {name} (destructive)"));
		}
	}

	for (name, old_model) in &old.models {
		let Some(new_model) = new.models.get(name) else {
			continue;
		};

		for (field_name, old_type) in &old_model.fields {
			match new_model.fields.get(field_name) {
				None => refused_details
					.push(format!("field removed: {name}.{field_name} (destructive)")),
				Some(new_type) if new_type != old_type => refused_details.push(format!(
					"field type changed: {name}.{field_name} from {old_type:?} to {new_type:?} (destructive)"
				)),
				Some(_) => {}
			}
		}
	}

	let old_fts = indexed_fields(old);
	let new_fts = indexed_fields(new);

	for field in new_fts.difference(&old_fts) {
		applied.push(MigrationAction::AddFtsColumn {
			column: (*field).to_string(),
		});
	}

	let has_refused_changes = !refused_details.is_empty();

	MigrationResult {
		applied,
		has_refused_changes,
		refused_details,
		search_fields_changed: old_fts != new_fts,
	}
}

fn indexed_fields(schema: &DataTypeSchema) -> HashSet<&str> {
	crate::schema::codegen::indexed_search_fields(schema)
		.into_iter()
		.collect()
}

/// Compute a hash of a schema for comparison.
pub fn schema_hash(schema: &DataTypeSchema) -> String {
	let toml = match toml::to_string_pretty(schema) {
		Ok(s) => s,
		Err(_) => return String::new(),
	};
	blake3::hash(toml.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::schema::parser;

	fn schema_with(extra_model: &str, search_fields: &str) -> DataTypeSchema {
		parser::parse(&format!(
			r#"
[data_type]
id = "note"
name = "Note"

[models.note]
fields.title = "string"
fields.body = "text"
{extra_model}

[search]
primary_model = "note"
title = "title"
preview = "body"
search_fields = [{search_fields}]
"#
		))
		.unwrap()
	}

	#[test]
	fn new_model_reports_its_facet_table() {
		let old = schema_with("", r#""title""#);
		let new = schema_with("[models.folder]\nfields.name = \"string\"", r#""title""#);

		let result = diff_schemas(&old, &new);
		assert!(!result.has_refused_changes);
		assert!(matches!(
			result.applied.as_slice(),
			[MigrationAction::AddTable { name }] if name == "facet_folder"
		));
	}

	#[test]
	fn removed_field_is_refused() {
		let old = schema_with("fields.tags = \"string\"", r#""title""#);
		let new = schema_with("", r#""title""#);

		let result = diff_schemas(&old, &new);
		assert!(result.has_refused_changes);
		assert!(result.refused_details[0].contains("note.tags"));
	}

	#[test]
	fn widened_search_contract_invalidates_the_index() {
		let old = schema_with("", r#""title""#);
		let new = schema_with("", r#""title", "body""#);

		let result = diff_schemas(&old, &new);
		assert!(result.search_fields_changed);
	}

	#[test]
	fn unchanged_schema_is_a_no_op() {
		let schema = schema_with("", r#""title", "body""#);
		let result = diff_schemas(&schema, &schema);

		assert!(result.applied.is_empty());
		assert!(!result.has_refused_changes);
		assert!(!result.search_fields_changed);
	}
}
