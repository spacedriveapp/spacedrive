//! Schema generation utilities for client types
//!
//! This module provides utilities for generating and manipulating JSON schemas
//! for client generation.

use schemars::schema::{RootSchema, Schema};
use schemars::{schema_for, JsonSchema};
use serde_json::Value;
use std::collections::HashMap;

/// Generate a JSON schema for a type that implements JsonSchema
pub fn generate_schema<T: JsonSchema>() -> RootSchema {
	schema_for!(T)
}

/// Merge multiple schemas into a unified schema
pub fn merge_schemas(schemas: Vec<Schema>) -> Schema {
	// For now, return the first schema
	// In a full implementation, we'd properly merge schemas
	schemas.into_iter().next().unwrap_or(Schema::Bool(true))
}

/// Extract type definitions from a schema
pub fn extract_type_definitions(schema: &RootSchema) -> HashMap<String, Value> {
	let mut types = HashMap::new();

	// Extract definitions from the schema
	for (name, def_schema) in &schema.definitions {
		if let Ok(value) = serde_json::to_value(def_schema) {
			types.insert(name.clone(), value);
		}
	}

	types
}

/// Simplify a schema for client generation
pub fn simplify_schema(schema: Schema) -> Schema {
	// For now, return the schema as-is
	// In a full implementation, we'd simplify complex schemas
	schema
}
