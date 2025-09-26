//! Example demonstrating how to find files unique to a location for backup purposes
//!
//! This example shows how to use the UniqueToLocationQuery to find files that exist
//! only in a specific location, which is useful for backup operations to identify
//! files that need to be backed up.

use sd_core::ops::files::query::UniqueToLocationQuery;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize the Spacedrive core (this would typically be done by the daemon)
	println!("Spacedrive Backup Search Demo");
	println!("============================");

	// Example: Find files unique to a specific location
	// In a real application, you would get the location_id from your library
	let example_location_id = Uuid::new_v4();

	println!("Searching for files unique to location: {}", example_location_id);

	// Create the query
	let query = UniqueToLocationQuery::new(example_location_id);

	// With a limit (useful for large locations)
	let query_with_limit = UniqueToLocationQuery::with_limit(example_location_id, 100);

	println!("Query created successfully!");
	println!("This query would find files that:");
	println!("1. Exist in the specified location");
	println!("2. Have content hashes that don't appear in any other location");
	println!("3. Are therefore unique to this location and need backup");

	// The query would be executed via the daemon's query system:
	// let result = daemon.execute_query(query).await?;
	//
	// Result would contain:
	// - unique_files: Vec<File> - The actual files that are unique
	// - total_count: u32 - Total number of unique files
	// - total_size: u64 - Total size of unique files in bytes

	println!("\nExample usage in a backup application:");
	println!("```rust");
	println!("// Find files that need backup");
	println!("let query = UniqueToLocationQuery::new(location_id);");
	println!("let result = daemon.execute_query(query).await?;");
	println!("");
	println!("println!(\"Found {{}} unique files, {{}} bytes total\",");
	println!("    result.total_count, result.total_size);");
	println!("");
	println!("// Process each unique file for backup");
	println!("for file in result.unique_files {{");
	println!("    println!(\"Backing up: {{}}\", file.name);");
	println!("    // Your backup logic here");
	println!("}}");
	println!("```");

	Ok(())
}

// Example of how this would be used in a real backup application
#[allow(dead_code)]
async fn backup_unique_files_example() -> Result<(), Box<dyn std::error::Error>> {
	// This would be called from a backup service
	let location_id = Uuid::new_v4(); // Get from your library

	// Find files unique to this location
	let query = UniqueToLocationQuery::new(location_id);

	// In a real app, this would go through the daemon:
	// let result = daemon.execute_query(query).await?;

	// For demo purposes, we'll simulate the result structure
	println!("Found files that need backup:");
	println!("- These files exist only in location {}", location_id);
	println!("- Their content hashes don't appear in any other location");
	println!("- They represent unique data that needs to be backed up");

	Ok(())
}


