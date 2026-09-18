//! Output for merging delivered assertions. Returning `Ok` is the ack: the
//! store committed before this was built, so a sender can retire its rows.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MergeAssertionsOutput {
	pub definitions_received: u32,
	/// Rows that were actually new; a replayed batch reports zero.
	pub assertions_appended: u64,
}
