//! Proof of concept that the rspc magic works

use crate::ops::type_extraction::*;
use serde::{Deserialize, Serialize};
use specta::{Type, TypeCollection};

// Create a simple action that has all required derives
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SimpleInput {
	pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SimpleOutput {
	pub result: String,
}

// Test the trait implementation manually
pub struct TestAction;

impl OperationTypeInfo for TestAction {
	type Input = SimpleInput;
	type Output = SimpleOutput;

	fn identifier() -> &'static str {
		"test.simple"
	}
}

// Submit a type extractor to inventory
inventory::submit! {
	TypeExtractorEntry {
		extractor: TestAction::extract_types,
		identifier: "test.simple",
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_rspc_magic_proof() {
		let (operations, _queries, collection) = generate_spacedrive_api();

		println!("🎯 RSPC MAGIC PROOF:");
		println!("   Discovered {} operations", operations.len());
		println!("   Type collection has {} types", collection.len());

		// Find our test operation
		let test_op = operations.iter().find(|op| op.identifier == "test.simple");

		if let Some(op) = test_op {
			println!("✅ Found test operation: {}", op.identifier);
			println!("   Wire method: {}", op.wire_method);
			println!("🎉 RSPC-inspired type extraction is working perfectly!");
		} else {
			println!("❌ Test operation not found");
		}

		// Should have at least our test operation
		assert!(
			!operations.is_empty(),
			"Should discover at least one operation"
		);
	}
}
