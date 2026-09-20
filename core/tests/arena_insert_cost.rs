//! What one arena insert is allowed to cost.
//!
//! Building a [`FileTypeRegistry`] parses every built-in definition, about
//! three milliseconds. An arena rebuilt from a delivered replica database adds
//! rows one at a time, so a registry built per insert turns a 1.76M-entry
//! rebuild into an hour of CPU. The registry is built once for the process;
//! this test fails if an insert starts rebuilding it.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use sd_core::filetype::registry::FileTypeRegistry;
use sd_core::ops::indexing::arena::Arena;
use sd_core::ops::indexing::metadata::EntryMetadata;
use sd_core::ops::indexing::state::EntryKind;
use uuid::Uuid;

/// Enough inserts that a per-insert registry build cannot hide in the noise:
/// at three milliseconds each this budget is exceeded twelve times over.
const INSERTS: usize = 20_000;
const BUDGET: Duration = Duration::from_secs(5);

#[test]
fn inserts_do_not_rebuild_the_file_type_registry() {
	let mut arena = Arena::new().expect("arena");

	let started = Instant::now();
	for i in 0..INSERTS {
		let path = PathBuf::from(format!("/root/dir{}/file-{i}.txt", i / 100));
		let metadata = EntryMetadata {
			path: path.clone(),
			kind: EntryKind::File,
			size: 1024,
			modified: Some(SystemTime::UNIX_EPOCH),
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: false,
		};
		arena
			.add_entry(path, Uuid::now_v7(), metadata)
			.expect("add entry");
	}
	let elapsed = started.elapsed();

	assert!(
		elapsed < BUDGET,
		"{INSERTS} inserts took {elapsed:?}, over the {BUDGET:?} budget: {:?} each",
		elapsed / INSERTS as u32
	);
}

/// The measurement behind the budget above, for when it needs revisiting.
#[test]
#[ignore = "measurement, not regression; run with --ignored --nocapture"]
fn registry_build_cost() {
	let started = Instant::now();
	for _ in 0..20 {
		let _ = FileTypeRegistry::new();
	}
	println!("registry build: {:?} each", started.elapsed() / 20);
}
