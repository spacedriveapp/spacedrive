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

	// Register event types in the same collection to avoid duplicates
	collection.register_mut::<crate::infra::event::Event>();
	collection.register_mut::<crate::infra::event::FsRawEventKind>();
	collection.register_mut::<crate::infra::event::FileOperation>();

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

/// Intermediate struct to hold API function metadata for Swift code generation
/// This is used to organize operations and queries into namespaces and methods
#[derive(Debug, Clone)]
pub struct ApiFunction {
	/// The namespace this function belongs to (e.g., "core", "libraries", "jobs")
	pub namespace: String,
	/// The method name within the namespace (e.g., "create", "list", "start")
	pub method_name: String,
	/// The full identifier (e.g., "libraries.create", "jobs.list")
	pub identifier: String,
	/// The wire method string (e.g., "action:libraries.create.input.v1")
	pub wire_method: String,
	/// Whether this is an action (true) or query (false)
	pub is_action: bool,
	/// The scope (Core or Library)
	pub scope: String,
	/// Input type name for Swift generation
	pub input_type_name: String,
	/// Output type name for Swift generation
	pub output_type_name: String,
}

/// Extract API functions from the collected metadata
/// This organizes operations and queries into a flat list of functions with namespace information
pub fn extract_api_functions(
	operations: &[OperationMetadata],
	queries: &[QueryMetadata],
) -> Vec<ApiFunction> {
	let mut functions = Vec::new();

	// Process operations (actions)
	for op in operations {
		let namespace = extract_namespace(&op.identifier);
		let method_name = extract_method_name(&op.identifier);
		let scope = match op.scope {
			OperationScope::Core => "Core",
			OperationScope::Library => "Library",
		};

		functions.push(ApiFunction {
			namespace,
			method_name,
			identifier: op.identifier.to_string(),
			wire_method: op.wire_method.clone(),
			is_action: true,
			scope: scope.to_string(),
			input_type_name: format!("{}Input", to_pascal_case(&op.identifier)),
			output_type_name: format!("{}Output", to_pascal_case(&op.identifier)),
		});
	}

	// Process queries
	for query in queries {
		let namespace = extract_namespace(&query.identifier);
		let method_name = extract_method_name(&query.identifier);
		let scope = match query.scope {
			QueryScope::Core => "Core",
			QueryScope::Library => "Library",
		};

		functions.push(ApiFunction {
			namespace,
			method_name,
			identifier: query.identifier.to_string(),
			wire_method: query.wire_method.clone(),
			is_action: false,
			scope: scope.to_string(),
			input_type_name: format!("{}Input", to_pascal_case(&query.identifier)),
			output_type_name: format!("{}Output", to_pascal_case(&query.identifier)),
		});
	}

	functions
}

/// Extract namespace from identifier (e.g., "libraries.create" -> "libraries")
fn extract_namespace(identifier: &str) -> String {
	identifier.split('.').next().unwrap_or("core").to_string()
}

/// Extract method name from identifier (e.g., "libraries.create" -> "create")
fn extract_method_name(identifier: &str) -> String {
	identifier.split('.').skip(1).collect::<Vec<_>>().join("_")
}

/// Convert snake_case to PascalCase for Swift type names
fn to_pascal_case(s: &str) -> String {
	s.split(&['.', '_'][..])
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				None => String::new(),
				Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str(),
			}
		})
		.collect::<Vec<_>>()
		.join("")
}

/// Convert snake_case to camelCase for Swift method names
fn to_camel_case(s: &str) -> String {
	let mut words = s.split('_');
	let first_word = words.next().unwrap_or("");
	let rest_words: String = words
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				None => String::new(),
				Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str(),
			}
		})
		.collect();
	first_word.to_lowercase() + &rest_words
}

/// Generate Swift code for API namespace structs and their methods
pub fn generate_swift_api_code(functions: &[ApiFunction]) -> String {
	let mut swift_code = String::new();

	// Add import statement for Foundation (needed for async/await)
	swift_code.push_str("import Foundation\n\n");

	// Group functions by namespace
	let mut namespaces: std::collections::HashMap<String, Vec<&ApiFunction>> =
		std::collections::HashMap::new();
	for func in functions {
		namespaces
			.entry(func.namespace.clone())
			.or_default()
			.push(func);
	}

	// Generate code for each namespace
	for (namespace, funcs) in namespaces {
		let namespace_struct_name = format!("{}API", to_pascal_case(&namespace));

		swift_code.push_str(&format!("/// {} operations\n", to_pascal_case(&namespace)));
		swift_code.push_str(&format!("public struct {} {{\n", namespace_struct_name));
		swift_code.push_str("    private let client: SpacedriveClient\n");
		swift_code.push_str("\n");
		swift_code.push_str("    init(client: SpacedriveClient) {\n");
		swift_code.push_str("        self.client = client\n");
		swift_code.push_str("    }\n");
		swift_code.push_str("\n");

		// Generate methods for each function in this namespace
		for func in funcs {
			swift_code.push_str(&generate_swift_method(func));
			swift_code.push_str("\n");
		}

		swift_code.push_str("}\n\n");
	}

	swift_code
}

/// Generate Swift method code for a single API function
fn generate_swift_method(func: &ApiFunction) -> String {
	let method_name = to_camel_case(&func.method_name);
	let input_type = &func.input_type_name;
	let output_type = &func.output_type_name;
	let wire_method = &func.wire_method;

	// Determine if this is an action or query for documentation
	let operation_type = if func.is_action { "action" } else { "query" };

	let mut method_code = String::new();

	// Add documentation comment
	method_code.push_str(&format!(
		"    /// Execute {}: {}\n",
		operation_type, func.identifier
	));

	// Generate method signature
	if input_type == "EmptyInput" {
		// For operations with no input, use Empty struct
		method_code.push_str(&format!(
			"    public func {}() async throws -> {} {{\n",
			method_name, output_type
		));
		method_code.push_str("        let input = Empty()\n");
	} else {
		// For operations with input, take the input as parameter
		method_code.push_str(&format!(
			"    public func {}(_ input: {}) async throws -> {} {{\n",
			method_name, input_type, output_type
		));
	}

	// Generate method body
	method_code.push_str(&format!("        return try await client.execute(\n"));
	method_code.push_str("            input,\n");
	method_code.push_str(&format!("            method: \"{}\",\n", wire_method));
	method_code.push_str(&format!("            responseType: {}.self\n", output_type));
	method_code.push_str("        )\n");
	method_code.push_str("    }\n");

	method_code
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

	#[test]
	fn test_api_functions_extraction() {
		let (operations, queries, _collection) = generate_spacedrive_api();
		let functions = extract_api_functions(&operations, &queries);

		println!("🔍 Extracted {} API functions", functions.len());

		// Group functions by namespace to show organization
		let mut namespaces: std::collections::HashMap<String, Vec<&ApiFunction>> =
			std::collections::HashMap::new();
		for func in &functions {
			namespaces
				.entry(func.namespace.clone())
				.or_default()
				.push(func);
		}

		for (namespace, funcs) in namespaces {
			println!("📁 Namespace '{}': {} functions", namespace, funcs.len());
			for func in funcs.iter().take(3) {
				println!(
					"   {}: {} -> {} ({})",
					func.method_name,
					func.input_type_name,
					func.output_type_name,
					if func.is_action { "action" } else { "query" }
				);
			}
		}

		// Verify some basic properties
		assert!(
			!functions.is_empty(),
			"Should have extracted some API functions"
		);

		// Check that namespaces are properly extracted
		let has_libraries = functions.iter().any(|f| f.namespace == "libraries");
		let has_jobs = functions.iter().any(|f| f.namespace == "jobs");
		println!("✅ Found libraries namespace: {}", has_libraries);
		println!("✅ Found jobs namespace: {}", has_jobs);
	}

	#[test]
	fn test_swift_code_generation() {
		let (operations, queries, _collection) = generate_spacedrive_api();
		let functions = extract_api_functions(&operations, &queries);
		let swift_code = generate_swift_api_code(&functions);

		println!("🔍 Generated Swift code (first 1000 chars):");
		println!("{}", &swift_code[..swift_code.len().min(1000)]);

		// Verify basic structure
		assert!(swift_code.contains("public struct LibrariesAPI"));
		assert!(swift_code.contains("public struct JobsAPI"));
		assert!(swift_code.contains("public struct NetworkAPI"));

		// Verify method generation
		assert!(swift_code.contains("public func create("));
		assert!(swift_code.contains("public func list("));
		assert!(swift_code.contains("public func start("));

		// Verify method calls to client.execute
		assert!(swift_code.contains("client.execute("));
		assert!(swift_code.contains("responseType:"));

		// Verify wire method strings are included
		assert!(swift_code.contains("action:libraries.create.input.v1"));
		assert!(swift_code.contains("query:jobs.list.v1"));

		println!("✅ Swift code generation test passed!");
	}
}
