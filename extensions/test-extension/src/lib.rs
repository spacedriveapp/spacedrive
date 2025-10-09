//! Test Extension
//!
//! Demonstrates the Spacedrive extension SDK using procedural macros to simplify
//! extension development by abstracting FFI and state management details.

use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{extension, job};

// Extension Definition
// The #[extension] macro generates plugin initialization and cleanup functions.

#[extension(id = "test-extension", name = "Test Extension", version = "0.1.0")]
struct TestExtension;

// Job State Definition
// State is automatically serialized/deserialized for checkpointing.

#[derive(Serialize, Deserialize, Default)]
pub struct CounterState {
	pub current: u32,
	pub target: u32,
	pub processed: Vec<String>,
}

// Job Implementation
// The #[job] macro handles FFI bindings, serialization, and error handling.

#[job]
fn test_counter(ctx: &JobContext, state: &mut CounterState) -> Result<()> {
	ctx.log(&format!(
		"Starting counter (current: {}, target: {})",
		state.current, state.target
	));

	while state.current < state.target {
		// Check for interruption signals
		if ctx.check_interrupt() {
			ctx.log("Interrupted, saving state...");
			ctx.checkpoint(state)?;
			return Err(Error::OperationFailed("Interrupted".into()));
		}

		// Process work unit
		state.current += 1;
		state.processed.push(format!("item_{}", state.current));

		// Update progress reporting
		let progress = state.current as f32 / state.target as f32;
		ctx.report_progress(
			progress,
			&format!("Counted {}/{}", state.current, state.target),
		);

		// Track processed items
		ctx.increment_items(1);

		// Periodic checkpoint for recovery
		if state.current % 10 == 0 {
			ctx.checkpoint(state)?;
		}
	}

	ctx.log(&format!(
		"Completed processing {} items",
		state.processed.len()
	));

	Ok(())
}
