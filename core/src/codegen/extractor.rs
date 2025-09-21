//! Schema extraction system for client generation
//!
//! This module extracts JSON schemas from registered operations and types
//! to generate unified type definitions for client libraries.

use schemars::schema::{RootSchema, Schema};
use schemars::{schema_for, JsonSchema};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractionError {
	#[error("Failed to serialize schema: {0}")]
	Serialization(#[from] serde_json::Error),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Schema generation error: {0}")]
	Schema(String),
}

/// Metadata for an operation (query or action)
#[derive(Debug, Clone)]
pub struct OperationMetadata {
	pub method: String,
	pub input_schema: Option<Schema>,
	pub output_schema: Schema,
	pub operation_type: OperationType,
}

/// Type of operation
#[derive(Debug, Clone)]
pub enum OperationType {
	Query,
	LibraryAction,
	CoreAction,
}

/// Unified schema containing all operations and types
pub struct UnifiedSchema {
	pub queries: Vec<OperationMetadata>,
	pub actions: Vec<OperationMetadata>,
	pub events: Schema,
	pub common_types: HashMap<String, RootSchema>,
}

impl UnifiedSchema {
	/// Extract schemas from registered operations
	pub fn extract() -> Result<Self, ExtractionError> {
		let mut schema = UnifiedSchema {
			queries: Vec::new(),
			actions: Vec::new(),
			events: Schema::Bool(true), // Placeholder until Event supports JsonSchema
			common_types: HashMap::new(),
		};

		// Extract query metadata
		schema.extract_queries()?;

		// Extract action metadata
		schema.extract_actions()?;

		// Extract common types used across operations
		schema.extract_common_types()?;

		Ok(schema)
	}

	/// Extract metadata for all registered queries
	fn extract_queries(&mut self) -> Result<(), ExtractionError> {
		// Use inventory to iterate over registered queries
		for entry in inventory::iter::<crate::ops::registry::QueryEntry>() {
			let metadata = self.extract_query_metadata(entry)?;
			self.queries.push(metadata);
		}
		Ok(())
	}

	/// Extract metadata for all registered actions
	fn extract_actions(&mut self) -> Result<(), ExtractionError> {
		// Use inventory to iterate over registered actions
		for entry in inventory::iter::<crate::ops::registry::ActionEntry>() {
			let metadata = self.extract_action_metadata(entry)?;
			self.actions.push(metadata);
		}
		Ok(())
	}

	/// Extract metadata for a specific query
	fn extract_query_metadata(
		&self,
		entry: &crate::ops::registry::QueryEntry,
	) -> Result<OperationMetadata, ExtractionError> {
		let (input_schema, output_schema) = if let Some(generator) = entry.schema_generator {
			let (input, output) = generator();
			(input.map(|s| s.schema.into()), output.schema.into())
		} else {
			// Fallback for queries without schema generators
			(None, Schema::Bool(true))
		};

		Ok(OperationMetadata {
			method: entry.method.to_string(),
			input_schema,
			output_schema,
			operation_type: OperationType::Query,
		})
	}

	/// Extract metadata for a specific action
	fn extract_action_metadata(
		&self,
		entry: &crate::ops::registry::ActionEntry,
	) -> Result<OperationMetadata, ExtractionError> {
		// Determine if this is a library or core action based on method name
		let operation_type = if entry.method.contains("libraries.") {
			OperationType::CoreAction
		} else {
			OperationType::LibraryAction
		};

		let (input_schema, output_schema) = if let Some(generator) = entry.schema_generator {
			let (input, output) = generator();
			(Some(input.schema.into()), output.schema.into())
		} else {
			// Fallback for actions without schema generators
			(Some(Schema::Bool(true)), Schema::Bool(true))
		};

		Ok(OperationMetadata {
			method: entry.method.to_string(),
			input_schema,
			output_schema,
			operation_type,
		})
	}

	/// Extract common types used across operations
	fn extract_common_types(&mut self) -> Result<(), ExtractionError> {
		// Add commonly used domain types
		self.common_types.insert(
			"SdPath".to_string(),
			schema_for!(crate::domain::addressing::SdPath),
		);
		self.common_types.insert(
			"SdPathBatch".to_string(),
			schema_for!(crate::domain::addressing::SdPathBatch),
		);

		// Add output types that we know have JsonSchema derives
		self.common_types.insert(
			"JobInfoOutput".to_string(),
			schema_for!(crate::ops::jobs::info::output::JobInfoOutput),
		);
		self.common_types.insert(
			"FileCopyActionOutput".to_string(),
			schema_for!(crate::ops::files::copy::output::FileCopyActionOutput),
		);
		self.common_types.insert(
			"LocationAddOutput".to_string(),
			schema_for!(crate::ops::locations::add::output::LocationAddOutput),
		);

		Ok(())
	}

	/// Write the unified schema to a JSON file
	pub fn write_unified_schema(&self, output_path: &Path) -> Result<(), ExtractionError> {
		let unified = serde_json::json!({
			"queries": self.queries.iter().map(|q| serde_json::json!({
				"method": q.method,
				"input": q.input_schema,
				"output": q.output_schema,
				"type": match q.operation_type {
					OperationType::Query => "query",
					OperationType::LibraryAction => "library_action",
					OperationType::CoreAction => "core_action",
				}
			})).collect::<Vec<_>>(),
			"actions": self.actions.iter().map(|a| serde_json::json!({
				"method": a.method,
				"input": a.input_schema,
				"output": a.output_schema,
				"type": match a.operation_type {
					OperationType::Query => "query",
					OperationType::LibraryAction => "library_action",
					OperationType::CoreAction => "core_action",
				}
			})).collect::<Vec<_>>(),
			"events": self.events,
			"types": self.common_types
		});

		std::fs::write(output_path, serde_json::to_string_pretty(&unified)?)?;
		Ok(())
	}
}
