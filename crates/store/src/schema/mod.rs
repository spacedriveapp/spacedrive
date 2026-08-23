//! Schema engine: parse TOML data type definitions, generate SQL DDL.

pub mod codegen;
pub mod migration;
pub mod parser;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A complete data type schema parsed from TOML.
#[derive(Debug, Clone, Serialize)]
pub struct DataTypeSchema {
	pub data_type: DataTypeMeta,
	pub models: IndexMap<String, ModelDef>,
	pub search: SearchContract,
}

/// Metadata about a data type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeMeta {
	pub id: String,
	pub name: String,
	pub icon: Option<String>,
}

/// A single model within a data type (maps to one SQLite table).
#[derive(Debug, Clone, Serialize)]
pub struct ModelDef {
	pub fields: IndexMap<String, FieldType>,
	pub relations: RelationsDef,
}

/// Supported field types that map to SQLite column types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
	/// TEXT — short string
	String,
	/// TEXT — long content (signals FTS/UI treatment)
	Text,
	/// INTEGER
	Integer,
	/// REAL
	Float,
	/// INTEGER (0/1)
	Boolean,
	/// TEXT (ISO 8601)
	Datetime,
	/// TEXT (JSON blob for unstructured sub-fields)
	Json,
	/// TEXT — file path (signals availability tracking, hashing, job routing)
	Path,
}

impl FieldType {
	/// Return the SQLite column type for this field.
	pub fn sql_type(&self) -> &'static str {
		match self {
			Self::String | Self::Text | Self::Datetime | Self::Json | Self::Path => "TEXT",
			Self::Integer | Self::Boolean => "INTEGER",
			Self::Float => "REAL",
		}
	}
}

/// Relation definitions for a model.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationsDef {
	#[serde(default)]
	pub belongs_to: Vec<String>,
	#[serde(default)]
	pub has_many: Vec<String>,
	#[serde(default)]
	pub many_to_many: Vec<String>,
	#[serde(default)]
	pub self_referential: Option<String>,
}

/// The search contract: defines how records surface in search results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchContract {
	pub primary_model: String,
	pub title: String,
	pub preview: String,
	pub subtitle: Option<String>,
	pub search_fields: Vec<String>,
	/// Column on the primary model used for temporal filtering and date-sorting.
	pub date_field: Option<String>,
}
