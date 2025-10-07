//! Test the type extraction system with just the types that have derives

use crate::infra::wire::type_extraction::*;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_working_operations() {
		let (operations, queries, collection) = generate_spacedrive_api();

		println!(
			"🔍 RSPC Magic: Discovered {} operations and {} queries",
			operations.len(),
			queries.len()
		);
		println!("📊 Type collection has {} types", collection.len());

		// Show discovered operations
		for op in operations.iter() {
			println!("   ✅ Operation: {} -> {}", op.identifier, op.wire_method);
		}

		for query in queries.iter() {
			println!("   ✅ Query: {} -> {}", query.identifier, query.wire_method);
		}

		if !operations.is_empty() {
			println!("🎉 RSPC-inspired type extraction is working!");
		}
	}
}
