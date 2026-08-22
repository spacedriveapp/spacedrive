//! Schema-to-SQL DDL generator.
//!
//! Emits the facet tables and search index that hang off the record table
//! ([`crate::record`]). One facet table per declared model, holding only that
//! model's own fields — identity, hierarchy and relationships live in the record table,
//! so nothing here duplicates them.

use std::fmt::Write;

use crate::schema::DataTypeSchema;
use crate::record::facet_table;

/// Generate the facet and search DDL for a data type. Applied on top of
/// [`crate::record::RECORD_SCHEMA`], never instead of it.
pub fn generate_ddl(schema: &DataTypeSchema) -> Vec<String> {
	let mut statements = Vec::new();

	for (model_name, model) in &schema.models {
		let table = facet_table(model_name);

		let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{table}\" (\n");
		sql.push_str("    record_uuid TEXT PRIMARY KEY REFERENCES record(uuid) ON DELETE CASCADE");

		for (field_name, field_type) in &model.fields {
			let _ = write!(sql, ",\n    \"{field_name}\" {}", field_type.sql_type());
		}

		sql.push_str("\n)");
		statements.push(sql);
	}

	if let Some(sql) = search_index_ddl(schema) {
		statements.push(sql);
	}

	statements
}

/// The standalone FTS5 index over the data type's declared search fields.
///
/// Standalone rather than external-content: search fields live in whichever
/// facet table owns them, and an external-content index can only follow one
/// table. Rows are written explicitly from the ingest path, gated on the
/// record's safety verdict.
pub fn search_index_ddl(schema: &DataTypeSchema) -> Option<String> {
	let fields = indexed_search_fields(schema);
	if fields.is_empty() {
		return None;
	}

	let columns = fields
		.iter()
		.map(|f| format!("\"{f}\""))
		.collect::<Vec<_>>()
		.join(", ");

	Some(format!(
		"CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(\n    \
		 {columns},\n    \
		 uuid UNINDEXED,\n    \
		 tokenize='porter unicode61'\n)"
	))
}

/// Search fields that map to real columns. `_derived.` entries are computed
/// downstream and have nothing to index.
pub fn indexed_search_fields(schema: &DataTypeSchema) -> Vec<&str> {
	schema
		.search
		.search_fields
		.iter()
		.filter(|f| !f.starts_with("_derived."))
		.map(|f| f.as_str())
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::schema::parser;

	fn fixture() -> DataTypeSchema {
		parser::parse(
			r#"
[data_type]
id = "bookmark"
name = "Bookmark"

[models.folder]
fields.name = "string"

[models.bookmark]
fields.url = "string"
fields.title = "string"
fields.description = "text"
fields.saved_at = "datetime"

[models.bookmark.relations]
belongs_to = ["folder"]

[search]
primary_model = "bookmark"
title = "title"
preview = "description"
search_fields = ["title", "description", "url"]
"#,
		)
		.unwrap()
	}

	#[test]
	fn emits_one_facet_table_per_model() {
		let ddl = generate_ddl(&fixture());
		assert!(ddl
			.iter()
			.any(|s| s.contains(r#"CREATE TABLE IF NOT EXISTS "facet_bookmark""#)));
		assert!(ddl
			.iter()
			.any(|s| s.contains(r#"CREATE TABLE IF NOT EXISTS "facet_folder""#)));
	}

	#[test]
	fn facets_key_on_the_record_table_and_carry_no_relation_columns() {
		let ddl = generate_ddl(&fixture());
		let bookmark = ddl
			.iter()
			.find(|s| s.contains(r#""facet_bookmark""#))
			.expect("facet table");

		assert!(bookmark.contains("record_uuid TEXT PRIMARY KEY REFERENCES record(uuid)"));
		assert!(bookmark.contains("ON DELETE CASCADE"));
		// Hierarchy is record state, not a facet column.
		assert!(!bookmark.contains("folder_id"));
	}

	#[test]
	fn search_index_covers_declared_fields() {
		let sql = search_index_ddl(&fixture()).expect("search index");
		for field in ["title", "description", "url"] {
			assert!(sql.contains(&format!("\"{field}\"")));
		}
		assert!(sql.contains("uuid UNINDEXED"));
	}

	#[test]
	fn derived_search_fields_are_not_indexed() {
		let mut schema = fixture();
		schema
			.search
			.search_fields
			.push("_derived.summary".to_string());
		assert_eq!(
			indexed_search_fields(&schema),
			vec!["title", "description", "url"]
		);
	}
}
