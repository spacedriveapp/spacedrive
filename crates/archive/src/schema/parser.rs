//! TOML schema parser: deserialize a TOML string into a `DataTypeSchema`.

use indexmap::IndexMap;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::schema::{
	DataTypeMeta, DataTypeSchema, FieldType, ModelDef, RelationsDef, SearchContract,
};

/// Parse a TOML string into a `DataTypeSchema`.
pub fn parse(toml_str: &str) -> Result<DataTypeSchema> {
	let raw: RawSchema = toml::from_str(toml_str).map_err(|e| Error::SchemaParse(e.to_string()))?;

	let mut models = IndexMap::new();
	for (name, raw_model) in raw.models {
		let relations = merge_relations(
			raw_model.relations.unwrap_or_default(),
			raw_model.belongs_to,
			raw_model.has_many,
			raw_model.many_to_many,
			raw_model.self_referential,
		);

		models.insert(
			name,
			ModelDef {
				fields: raw_model.fields,
				relations,
			},
		);
	}

	Ok(DataTypeSchema {
		data_type: raw.data_type,
		models,
		search: raw.search,
	})
}

/// Merge relation fields from model-level and nested `[relations]` table.
fn merge_relations(
	nested: RelationsDef,
	belongs_to: Option<Vec<String>>,
	has_many: Option<Vec<String>>,
	many_to_many: Option<Vec<String>>,
	self_referential: Option<String>,
) -> RelationsDef {
	RelationsDef {
		belongs_to: belongs_to.unwrap_or(nested.belongs_to),
		has_many: has_many.unwrap_or(nested.has_many),
		many_to_many: many_to_many.unwrap_or(nested.many_to_many),
		self_referential: self_referential.or(nested.self_referential),
	}
}

// --- Raw deserialization types (intermediate, not public) ---

#[derive(Deserialize)]
struct RawSchema {
	data_type: DataTypeMeta,
	models: IndexMap<String, RawModelDef>,
	search: SearchContract,
}

#[derive(Deserialize)]
struct RawModelDef {
	fields: IndexMap<String, FieldType>,

	#[serde(default)]
	relations: Option<RelationsDef>,

	#[serde(default)]
	belongs_to: Option<Vec<String>>,
	#[serde(default)]
	has_many: Option<Vec<String>>,
	#[serde(default)]
	many_to_many: Option<Vec<String>>,
	#[serde(default)]
	self_referential: Option<String>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn field_type_sql_mapping() {
		assert_eq!(FieldType::String.sql_type(), "TEXT");
		assert_eq!(FieldType::Text.sql_type(), "TEXT");
		assert_eq!(FieldType::Integer.sql_type(), "INTEGER");
		assert_eq!(FieldType::Float.sql_type(), "REAL");
		assert_eq!(FieldType::Boolean.sql_type(), "INTEGER");
		assert_eq!(FieldType::Datetime.sql_type(), "TEXT");
		assert_eq!(FieldType::Json.sql_type(), "TEXT");
		assert_eq!(FieldType::Path.sql_type(), "TEXT");
	}

	#[test]
	fn parse_simple_schema() {
		let toml = r#"
[data_type]
id = "test"
name = "Test"

[models.item]
fields.name = "string"
fields.content = "text"

[search]
primary_model = "item"
title = "name"
preview = "content"
search_fields = ["name", "content"]
"#;

		let schema = parse(toml).expect("failed to parse schema");
		assert_eq!(schema.data_type.id, "test");
		assert_eq!(schema.data_type.name, "Test");
		assert_eq!(schema.models.len(), 1);
	}
}
