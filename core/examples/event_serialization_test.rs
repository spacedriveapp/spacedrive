//! Test how Event enums serialize to understand the JSON format

use sd_core::infra::event::Event;
use serde_json;
use std::path::PathBuf;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Test simple enum variant
	let event1 = Event::CoreStarted;
	println!("CoreStarted: {}", serde_json::to_string(&event1)?);

	// Test struct-like enum variant
	let event2 = Event::LibraryCreated {
		id: Uuid::new_v4(),
		name: "Test Library".to_string(),
		path: PathBuf::from("/test/path"),
	};
	println!("LibraryCreated: {}", serde_json::to_string(&event2)?);

	// Test job event
	let event3 = Event::JobStarted {
		job_id: "test-job-123".to_string(),
		job_type: "Indexing".to_string(),
	};
	println!("JobStarted: {}", serde_json::to_string(&event3)?);

	Ok(())
}
