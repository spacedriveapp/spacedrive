//! Test Event serialization to see the exact format

use sd_core::infra::event::Event;
use serde_json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🧪 Testing Event serialization format...");

	// Create a sample JobStarted event like the daemon would
	let event = Event::JobStarted {
		job_id: "test-job-123".to_string(),
		job_type: "Indexing".to_string(),
	};

	// Serialize it to see the exact JSON format
	let json = serde_json::to_string_pretty(&event)?;
	println!("📄 JobStarted event JSON:\n{}", json);

	// This is what the daemon sends (wrapped in DaemonResponse::Event)
	let daemon_response = serde_json::json!({
		"Event": event
	});

	let daemon_json = serde_json::to_string_pretty(&daemon_response)?;
	println!("📄 Daemon response JSON:\n{}", daemon_json);

	Ok(())
}
