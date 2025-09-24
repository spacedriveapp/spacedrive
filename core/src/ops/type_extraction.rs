//! rspc-inspired trait-based type extraction for automatic API generation
//!
//! This module implements the core trait system that allows automatic discovery
//! and extraction of Input/Output types from registered operations at compile-time.

use serde::{de::DeserializeOwned, Serialize};
use specta::{DataType, Type, TypeCollection};

/// Operation scope - automatically determined by registration macro
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationScope {
	Core,
	Library,
}

/// Query scope - automatically determined by registration macro
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryScope {
	Core,
	Library,
}

/// Core trait that provides compile-time type information for operations
///
/// This is inspired by rspc's resolver trait system and enables automatic
/// type extraction without runtime iteration over inventory data.
pub trait OperationTypeInfo {
	/// The input type for this operation
	type Input: Type + Serialize + DeserializeOwned + 'static;

	/// The output type for this operation
	type Output: Type + Serialize + DeserializeOwned + 'static;

	/// The operation identifier (e.g., "files.copy", "libraries.create")
	fn identifier() -> &'static str;

	/// The operation scope (Core or Library) - automatically determined by registration macro
	fn scope() -> OperationScope;

	/// Generate the wire method string for this operation
	fn wire_method() -> String;

	/// Extract type metadata and register with Specta's TypeCollection
	/// This is the key method that enables automatic type discovery
	fn extract_types(collection: &mut TypeCollection) -> OperationMetadata {
		// Register the types with Specta and get their DataType definitions
		let input_type = Self::Input::definition(collection);
		let output_type = Self::Output::definition(collection);

		OperationMetadata {
			identifier: Self::identifier(),
			wire_method: Self::wire_method(),
			input_type,
			output_type,
			scope: Self::scope(),
		}
	}
}

/// Similar trait for query operations
pub trait QueryTypeInfo {
	/// Query input type (often () for queries with no parameters)
	type Input: Type + Serialize + DeserializeOwned + 'static;

	/// Query output type
	type Output: Type + Serialize + DeserializeOwned + 'static;

	/// Query identifier (e.g., "jobs.list", "libraries.list")
	fn identifier() -> &'static str;

	/// The query scope (Core or Library) - automatically determined by registration macro
	fn scope() -> QueryScope;

	/// Generate wire method for queries
	fn wire_method() -> String;

	/// Extract query type metadata
	fn extract_types(collection: &mut TypeCollection) -> QueryMetadata {
		// Register the types with Specta and get their DataType definitions
		let input_type = Self::Input::definition(collection);
		let output_type = Self::Output::definition(collection);

		QueryMetadata {
			identifier: Self::identifier(),
			wire_method: Self::wire_method(),
			input_type,
			output_type,
			scope: Self::scope(),
		}
	}
}

/// Metadata extracted from an operation
#[derive(Debug, Clone)]
pub struct OperationMetadata {
	pub identifier: &'static str,
	pub wire_method: String,
	pub input_type: DataType,
	pub output_type: DataType,
	pub scope: OperationScope,
}

/// Metadata extracted from a query
#[derive(Debug, Clone)]
pub struct QueryMetadata {
	pub identifier: &'static str,
	pub wire_method: String,
	pub input_type: DataType,
	pub output_type: DataType,
	pub scope: QueryScope,
}

/// Entry for collecting type extractors via inventory
/// This is the key that makes compile-time collection possible
pub struct TypeExtractorEntry {
	/// Function that extracts operation metadata and registers types
	pub extractor: fn(&mut TypeCollection) -> OperationMetadata,
	pub identifier: &'static str,
}

/// Entry for collecting query type extractors
pub struct QueryExtractorEntry {
	/// Function that extracts query metadata and registers types
	pub extractor: fn(&mut TypeCollection) -> QueryMetadata,
	pub identifier: &'static str,
}

// Collect type extractors via inventory - this enables compile-time discovery
inventory::collect!(TypeExtractorEntry);
inventory::collect!(QueryExtractorEntry);

/// Generate complete API metadata by running all collected type extractors
///
/// This is the rspc-inspired magic: we iterate over compile-time registered
/// extractors rather than runtime data, solving the timeline problem.
pub fn generate_spacedrive_api() -> (Vec<OperationMetadata>, Vec<QueryMetadata>, TypeCollection) {
	let mut collection = TypeCollection::default();
	let mut operations = Vec::new();
	let mut queries = Vec::new();

	// Extract all operations - this works because extractors are registered at compile-time
	for entry in inventory::iter::<TypeExtractorEntry>() {
		let metadata = (entry.extractor)(&mut collection);
		operations.push(metadata);
	}

	// Extract all queries
	for entry in inventory::iter::<QueryExtractorEntry>() {
		let metadata = (entry.extractor)(&mut collection);
		queries.push(metadata);
	}

	(operations, queries, collection)
}

/// Generate the complete Spacedrive API structure as a Specta-compatible type
///
/// This creates a runtime representation of our API structure that Specta can export.
/// Similar to rspc's approach with TypesOrType, but tailored for Spacedrive's needs.
pub fn create_spacedrive_api_structure(
	operations: &[OperationMetadata],
	queries: &[QueryMetadata],
) -> SpacedriveApiStructure {
	let mut core_actions = Vec::new();
	let mut library_actions = Vec::new();
	let mut core_queries = Vec::new();
	let mut library_queries = Vec::new();

	// Group operations by scope - preserve the actual DataType objects!
	for op in operations {
		match op.scope {
			OperationScope::Core => {
				core_actions.push(ApiOperationType {
					identifier: op.identifier.to_string(),
					wire_method: op.wire_method.clone(),
					input_type: op.input_type.clone(),
					output_type: op.output_type.clone(),
				});
			}
			OperationScope::Library => {
				library_actions.push(ApiOperationType {
					identifier: op.identifier.to_string(),
					wire_method: op.wire_method.clone(),
					input_type: op.input_type.clone(),
					output_type: op.output_type.clone(),
				});
			}
		}
	}

	// Group queries by scope - preserve the actual DataType objects!
	for query in queries {
		match query.scope {
			QueryScope::Core => {
				core_queries.push(ApiQueryType {
					identifier: query.identifier.to_string(),
					wire_method: query.wire_method.clone(),
					input_type: query.input_type.clone(),
					output_type: query.output_type.clone(),
				});
			}
			QueryScope::Library => {
				library_queries.push(ApiQueryType {
					identifier: query.identifier.to_string(),
					wire_method: query.wire_method.clone(),
					input_type: query.input_type.clone(),
					output_type: query.output_type.clone(),
				});
			}
		}
	}

	SpacedriveApiStructure {
		core_actions,
		library_actions,
		core_queries,
		library_queries,
	}
}

/// Represents the complete Spacedrive API structure for code generation
pub struct SpacedriveApiStructure {
	pub core_actions: Vec<ApiOperationType>,
	pub library_actions: Vec<ApiOperationType>,
	pub core_queries: Vec<ApiQueryType>,
	pub library_queries: Vec<ApiQueryType>,
}

/// Represents a single API operation with actual type information
pub struct ApiOperationType {
	pub identifier: String,
	pub wire_method: String,
	pub input_type: specta::datatype::DataType,
	pub output_type: specta::datatype::DataType,
}

/// Represents a single API query with actual type information
pub struct ApiQueryType {
	pub identifier: String,
	pub wire_method: String,
	pub input_type: specta::datatype::DataType,
	pub output_type: specta::datatype::DataType,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_type_extraction_system() {
		let (operations, queries, collection) = generate_spacedrive_api();

		println!(
			"🔍 Discovered {} operations and {} queries",
			operations.len(),
			queries.len()
		);
		println!("📊 Type collection has {} types", collection.len());

		// Should have some operations if the system is working
		if !operations.is_empty() {
			println!("✅ Type extraction system is working!");

			// Show some examples with scope information
			for op in operations.iter().take(3) {
				println!(
					"   Operation: {} -> wire: {} -> scope: {:?}",
					op.identifier, op.wire_method, op.scope
				);
			}
		}

		if !queries.is_empty() {
			for query in queries.iter().take(3) {
				println!(
					"   Query: {} -> wire: {} -> scope: {:?}",
					query.identifier, query.wire_method, query.scope
				);
			}
		}
	}
}
