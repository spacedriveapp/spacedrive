//! Test Extension - Beautiful API Demo
//!
//! This shows what extension development looks like with macros.
//! Compare to test-extension/ to see the difference!

use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{extension, spacedrive_job};

// === Extension Definition (generates plugin_init/cleanup) ===

#[extension(
	id = "test-beautiful",
	name = "Test Extension (Beautiful API)",
	version = "0.1.0"
)]
struct TestExtension;

// === Job State ===

#[derive(Serialize, Deserialize, Default)]
pub struct CounterState {
	pub current: u32,
	pub target: u32,
	pub processed: Vec<String>,
}

// === Beautiful Job Definition ===

/// This is ALL you write! The macro handles everything else.
#[spacedrive_job]
fn test_counter(ctx: &JobContext, state: &mut CounterState) -> Result<()> {
	ctx.log(&format!(
		"Starting counter (current: {}, target: {})",
		state.current, state.target
	));

	while state.current < state.target {
		// Check interruption - if interrupted, auto-checkpoints and returns!
		if ctx.check_interrupt() {
			ctx.log("Interrupted, saving state...");
			ctx.checkpoint(state)?;
			return Err(Error::OperationFailed("Interrupted".into()));
		}

		// Do work
		state.current += 1;
		state.processed.push(format!("item_{}", state.current));

		// Report progress
		let progress = state.current as f32 / state.target as f32;
		ctx.report_progress(progress, &format!("Counted {}/{}", state.current, state.target));

		// Track metrics
		ctx.increment_items(1);

		// Checkpoint every 10
		if state.current % 10 == 0 {
			ctx.checkpoint(state)?;
		}
	}

	ctx.log(&format!("✓ Completed! Processed {} items", state.processed.len()));

	Ok(())
}

// That's it! No:
// - #[no_mangle]
// - extern "C"
// - Pointer manipulation
// - Manual serialization
// - FFI boilerplate
//
// Just pure, clean business logic!

