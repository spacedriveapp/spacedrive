//! Schema-to-SQL DDL generator.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;

use crate::schema::DataTypeSchema;

fn pluralize(name: &str) -> String {
	format!("{name}s")
}

/// Topologically sort models so dependencies (belongs_to targets) come first.
fn topological_sort(schema: &DataTypeSchema) -> Vec<String> {
	let model_names: Vec<String> = schema.models.keys().cloned().collect();
	let name_set: HashSet<&str> = model_names.iter().map(|s| s.as_str()).collect();

	let mut deps: HashMap<&str, Vec<&str>> = HashMap::new();
	for (name, model) in &schema.models {
		let mut model_deps = Vec::new();
		for target in &model.relations.belongs_to {
			if name_set.contains(target.as_str()) && target != name {
				model_deps.push(target.as_str());
			}
		}
		deps.insert(name.as_str(), model_deps);
	}

	let mut in_degree: HashMap<&str, usize> = HashMap::new();
	for name in &model_names {
		in_degree.insert(name.as_str(), 0);
	}
	for name in &model_names {
		in_degree.insert(
			name.as_str(),
			deps.get(name.as_str()).map_or(0, |d| d.len()),
		);
	}

	let mut queue: VecDeque<&str> = VecDeque::new();
	for name in &model_names {
		if in_degree[name.as_str()] == 0 {
			queue.push_back(name.as_str());
		}
	}

	let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
	for (name, dep_list) in &deps {
		for dep in dep_list {
			dependents.entry(*dep).or_default().push(name);
		}
	}

	let mut sorted = Vec::new();
	while let Some(name) = queue.pop_front() {
		sorted.push(name.to_string());
		if let Some(dependent_list) = dependents.get(name) {
			for dependent in dependent_list {
				if let Some(degree) = in_degree.get_mut(dependent) {
					*degree -= 1;
					if *degree == 0 {
						queue.push_back(dependent);
					}
				}
			}
		}
	}

	if sorted.len() != model_names.len() {
		return model_names;
	}

	sorted
}

/// Generate all SQL DDL statements from a parsed schema.
pub fn generate_ddl(schema: &DataTypeSchema) -> Vec<String> {
	let mut statements = Vec::new();

	let sorted = topological_sort(schema);
	for model_name in &sorted {
		let model = &schema.models[model_name];
		let table_name = pluralize(model_name);

		let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{table_name}\" (\n");
		sql.push_str("    id TEXT PRIMARY KEY,\n");
		sql.push_str("    external_id TEXT UNIQUE,\n");

		for (field_name, field_type) in &model.fields {
			let _ = write!(sql, "    \"{field_name}\" {},\n", field_type.sql_type());
		}

		for target in &model.relations.belongs_to {
			let target_table = pluralize(target);
			let _ = write!(
				sql,
				"    \"{target}_id\" TEXT REFERENCES \"{target_table}\"(id),\n"
			);
		}

		if let Some(ref col) = model.relations.self_referential {
			let _ = write!(sql, "    \"{col}\" TEXT REFERENCES \"{table_name}\"(id),\n");
		}

		sql.push_str("    indexed_at TEXT NOT NULL DEFAULT (datetime('now')),\n");
		sql.push_str("    _embedded_at TEXT,\n");
		sql.push_str("    _safety_score INTEGER,\n");
		sql.push_str("    _safety_verdict TEXT DEFAULT 'unscreened',\n");
		sql.push_str("    _safety_version TEXT\n");
		sql.push_str(")");

		statements.push(sql);
	}

	let mut created_junctions: HashSet<String> = HashSet::new();
	for model_name in &sorted {
		let model = &schema.models[model_name];
		for target in &model.relations.many_to_many {
			let (a, b) = if model_name <= target {
				(model_name.as_str(), target.as_str())
			} else {
				(target.as_str(), model_name.as_str())
			};

			let junction_name = format!("{a}_{b}");
			if created_junctions.contains(&junction_name) {
				continue;
			}
			created_junctions.insert(junction_name.clone());

			let a_table = pluralize(a);
			let b_table = pluralize(b);

			let (a_col, b_col) = if a == b {
				(format!("{a}_id"), format!("related_{a}_id"))
			} else {
				(format!("{a}_id"), format!("{b}_id"))
			};

			let sql = format!(
				"CREATE TABLE IF NOT EXISTS \"{junction_name}\" (\n    \
				 \"{a_col}\" TEXT NOT NULL REFERENCES \"{a_table}\"(id),\n    \
				 \"{b_col}\" TEXT NOT NULL REFERENCES \"{b_table}\"(id),\n    \
				 PRIMARY KEY (\"{a_col}\", \"{b_col}\")\n)"
			);

			statements.push(sql);
		}
	}

	let primary_table = pluralize(&schema.search.primary_model);
	let fts_fields: Vec<&str> = schema
		.search
		.search_fields
		.iter()
		.filter(|f| !f.starts_with("_derived."))
		.map(|f| f.as_str())
		.collect();

	if !fts_fields.is_empty() {
		let fields_str = fts_fields.join(", ");
		let fields_quoted: Vec<String> = fts_fields.iter().map(|f| format!("\"{f}\"")).collect();
		let fields_quoted_str = fields_quoted.join(", ");

		let sql = format!(
			"CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(\n    \
			 {fields_str},\n    \
			 content=\"{primary_table}\",\n    \
			 content_rowid=rowid,\n    \
			 tokenize='porter unicode61'\n)"
		);
		statements.push(sql);

		let field_refs: Vec<String> = fts_fields.iter().map(|f| format!("new.\"{f}\"")).collect();
		let field_refs_old: Vec<String> =
			fts_fields.iter().map(|f| format!("old.\"{f}\"")).collect();
		let field_refs_str = field_refs.join(", ");
		let field_refs_old_str = field_refs_old.join(", ");

		statements.push(format!(
			"CREATE TRIGGER IF NOT EXISTS search_index_ai AFTER INSERT ON \"{primary_table}\" BEGIN\n    \
			 INSERT INTO search_index(rowid, {fields_quoted_str})\n    \
			 SELECT new.rowid, {field_refs_str}\n    \
			 WHERE new._safety_verdict IN ('safe', 'flagged');\n\
			 END"
		));

		statements.push(format!(
			"CREATE TRIGGER IF NOT EXISTS search_index_ad AFTER DELETE ON \"{primary_table}\" BEGIN\n    \
			 INSERT INTO search_index(search_index, rowid, {fields_quoted_str}) VALUES ('delete', old.rowid, {field_refs_old_str});\n\
			 END"
		));

		statements.push(format!(
			"CREATE TRIGGER IF NOT EXISTS search_index_au AFTER UPDATE ON \"{primary_table}\" BEGIN\n    \
			 INSERT INTO search_index(search_index, rowid, {fields_quoted_str})\n    \
			 SELECT 'delete', old.rowid, {field_refs_old_str}\n    \
			 WHERE old._safety_verdict IN ('safe', 'flagged');\n    \
			 INSERT INTO search_index(rowid, {fields_quoted_str})\n    \
			 SELECT new.rowid, {field_refs_str}\n    \
			 WHERE new._safety_verdict IN ('safe', 'flagged');\n\
			 END"
		));
	}

	statements.push(
		"CREATE TABLE IF NOT EXISTS _sync_state (\n    \
		 key TEXT PRIMARY KEY,\n    \
		 value TEXT NOT NULL,\n    \
		 updated_at TEXT NOT NULL DEFAULT (datetime('now'))\n)"
			.to_string(),
	);

	statements.push(
		"CREATE TABLE IF NOT EXISTS _schema (\n    \
		 id INTEGER PRIMARY KEY CHECK (id = 1),\n    \
		 data_type_id TEXT NOT NULL,\n    \
		 schema_hash TEXT NOT NULL,\n    \
		 schema_toml TEXT NOT NULL,\n    \
		 applied_at TEXT NOT NULL DEFAULT (datetime('now'))\n)"
			.to_string(),
	);

	statements
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::schema::parser;

	#[test]
	fn generate_simple_ddl() {
		let schema = parser::parse(
			r#"
[data_type]
id = "bookmark"
name = "Bookmark"

[models.bookmark]
fields.url = "string"
fields.title = "string"
fields.description = "text"
fields.saved_at = "datetime"

[search]
primary_model = "bookmark"
title = "title"
preview = "description"
search_fields = ["title", "description", "url"]
"#,
		)
		.unwrap();

		let ddl = generate_ddl(&schema);
		assert!(!ddl.is_empty());
		assert!(ddl[0].contains(r#"CREATE TABLE IF NOT EXISTS "bookmarks""#));
	}
}
