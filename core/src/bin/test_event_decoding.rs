//! Test Event decoding to debug the Swift issue

use sd_core::infra::event::Event;
use serde_json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🧪 Testing Event decoding...");

	// Create the exact event that's failing
	let event = Event::JobStarted {
		job_id: "8525ff04-3025-409a-a98f-e94737bd94d4".to_string(),
		job_type: "Indexing".to_string(),
	};

	// Serialize just the inner event (what Swift should receive)
	let inner_json = serde_json::to_string_pretty(&event)?;
	println!(
		"📄 Inner event JSON (what Swift Event decoder should get):\n{}",
		inner_json
	);

	// Test if we can deserialize it back
	let decoded: Event = serde_json::from_str(&inner_json)?;
	println!("✅ Successfully decoded inner event: {:?}", decoded);

	// Now test the wrapped format (what daemon actually sends)
	let wrapped_json = serde_json::json!({
		"Event": event
	});
	let wrapped_str = serde_json::to_string_pretty(&wrapped_json)?;
	println!(
		"📄 Wrapped event JSON (what daemon sends):\n{}",
		wrapped_str
	);

	// Test extracting the inner event from the wrapper
	if let Some(inner_value) = wrapped_json.get("Event") {
		let inner_event: Event = serde_json::from_value(inner_value.clone())?;
		println!(
			"✅ Successfully extracted and decoded inner event from wrapper: {:?}",
			inner_event
		);
	}

	Ok(())
}
