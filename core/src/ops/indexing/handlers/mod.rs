//! Event handlers for filesystem changes
//!
//! These handlers subscribe to `FsWatcher` events and route them to the
//! arena.

mod fs_events;

pub use fs_events::FsEventHandler;

use std::io::ErrorKind;
use std::path::Path;

/// Whether a watched root still exists on disk.
///
/// A vanished location root means the volume unmounted (or the root was
/// removed wholesale); the flood of Remove events that follows must not be
/// applied, or every record under the root would be deleted because a drive
/// was unplugged. Errors other than NotFound (e.g. permission changes) are
/// treated as present so transient failures do not suppress real events.
pub(crate) fn root_is_present(root: &Path) -> bool {
	match std::fs::metadata(root) {
		Ok(_) => true,
		Err(e) => e.kind() != ErrorKind::NotFound,
	}
}

#[cfg(test)]
mod tests {
	use super::root_is_present;

	#[test]
	fn present_root_is_detected() {
		assert!(root_is_present(&std::env::temp_dir()));
	}

	#[test]
	fn missing_root_is_detected() {
		let missing = std::env::temp_dir().join("sd-root-presence-test-nonexistent");
		assert!(!root_is_present(&missing));
	}
}
