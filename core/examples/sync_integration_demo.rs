//! Sync Integration Demo
//!
//! This example demonstrates how to use the integrated sync infrastructure
//! to log changes to syncable models via the TransactionManager.
//!
//! Run with: cargo run --example sync_integration_demo

use sd_core::infra::{
	db::entities::location,
	sync::{ChangeType, Syncable},
};
use std::error::Error;
use tracing_subscriber;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	// Initialize logging
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "sd_core=debug,sync_integration_demo=debug".into()),
		)
		.init();

	println!("\n=== Sync Integration Demo ===\n");

	// Example 1: Check Syncable trait on Location
	println!("1. Syncable Trait on Location Entity");
	println!("   Model identifier: {}", location::Model::SYNC_MODEL);

	// Create a sample location
	let location = location::Model {
		id: 1,
		uuid: Uuid::new_v4(),
		device_id: 1,
		entry_id: 1,
		name: Some("Photos".to_string()),
		index_mode: "deep".to_string(),
		scan_state: "completed".to_string(),
		last_scan_at: Some(chrono::Utc::now().into()),
		error_message: None,
		total_file_count: 1000,
		total_byte_size: 5_000_000_000, // 5GB
		created_at: chrono::Utc::now().into(),
		updated_at: chrono::Utc::now().into(),
	};

	println!("   Location UUID: {}", location.sync_id());
	println!("   Location version: {}", location.version());

	// Example 2: Serialize to sync-safe JSON
	println!("\n2. Sync-Safe Serialization");
	let sync_json = location.to_sync_json()?;
	println!(
		"   Sync JSON: {}",
		serde_json::to_string_pretty(&sync_json)?
	);

	// Example 3: Field Exclusion
	println!("\n3. Field Exclusion (What Syncs vs What Doesn't)");
	println!(
		"   Excluded fields (local state): {:?}",
		location::Model::exclude_fields()
	);
	println!("   ✅ Syncs: uuid, device_id, entry_id, name, index_mode, stats");
	println!("   ❌ Doesn't sync: id, scan_state, error_message, timestamps");

	// Example 4: Show architecture
	println!("\n4. Integrated Architecture");
	println!("   When you open a library:");
	println!("   ├─ sync.db gets created/opened");
	println!("   ├─ LeadershipManager initialized");
	println!("   ├─ TransactionManager ready");
	println!("   └─ Leadership role determined (Leader/Follower)");

	println!("\n5. Usage in Actions (Pseudo-code)");
	println!("   ```rust");
	println!("   // In your action (e.g., LocationAddAction):");
	println!("   let location_model = location::ActiveModel {{ ... }};");
	println!("   let result = location_model.insert(library.db().conn()).await?;");
	println!("   ");
	println!("   // Log the change to sync log (if leader):");
	println!("   library.transaction_manager()");
	println!("       .log_change(");
	println!("           library.id(),");
	println!("           library.sync_log_db(),");
	println!("           &result,");
	println!("           ChangeType::Insert,");
	println!("       ).await?;");
	println!("   ```");

	println!("\n6. What Happens on Sync");
	println!("   Leader Device (Device A):");
	println!("   ├─ User creates location 'Photos'");
	println!("   ├─ Location inserted to database");
	println!("   ├─ Sync log entry created (sequence #1)");
	println!("   └─ Event emitted");
	println!();
	println!("   Follower Device (Device B):");
	println!("   ├─ Receives sync log entry #1");
	println!("   ├─ Deserializes location JSON");
	println!("   ├─ Resolves device_id and entry_id to local references");
	println!("   ├─ Inserts location (read-only, owned by Device A)");
	println!("   └─ User can browse Device A's photos remotely");

	println!("\n=== Demo Complete ===\n");

	Ok(())
}
