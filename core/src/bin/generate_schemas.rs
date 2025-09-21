//! Schema generation binary
//!
//! This binary extracts JSON schemas from registered Spacedrive operations
//! and generates the unified types.json file for client generation.

use sd_core::codegen::UnifiedSchema;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🔍 Extracting schemas from registered operations...");

	// Extract schemas from the actual registered operations
	let schema = UnifiedSchema::extract()?;

	// Write to packages/types.json
	let output_path = Path::new("../packages/types.json");
	schema.write_unified_schema(output_path)?;

	println!(
		"✅ Generated types.json with {} queries, {} actions, and {} common types",
		schema.queries.len(),
		schema.actions.len(),
		schema.common_types.len()
	);

	Ok(())
}

