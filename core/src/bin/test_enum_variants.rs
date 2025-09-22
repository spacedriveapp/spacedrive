//! Test enum variant serialization formats

use serde::{Deserialize, Serialize};
use specta::{Type, TypeCollection};
use specta_swift::Swift;

#[derive(Type, Serialize, Deserialize)]
enum TestEvent {
	// Unit variant
	Started,
	// Tuple variant
	Progress(f64, String),
	// Struct variant - this is what our Event enum uses
	JobStarted { job_id: String, job_type: String },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🧪 Testing enum variant formats...");

	// Test serialization
	let event = TestEvent::JobStarted {
		job_id: "test-123".to_string(),
		job_type: "Indexing".to_string(),
	};

	let json = serde_json::to_string_pretty(&event)?;
	println!("📄 Rust serializes struct variant as:\n{}", json);

	// Generate Swift types
	let types = TypeCollection::default().register::<TestEvent>();

	let swift = Swift::new().naming(specta_swift::NamingConvention::PascalCase);

	let output = swift.export(&types)?;
	println!("📄 Swift enum generated as:\n{}", output);

	Ok(())
}
