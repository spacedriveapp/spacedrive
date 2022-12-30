use std::{
	fs::{self, create_dir_all, read_dir, remove_dir, remove_dir_all},
	path::PathBuf,
	sync::atomic::{AtomicU32, Ordering},
	time::Duration,
};

use tokio::time::sleep;

/// The directory where all test data is created. It *should* be deleted after all tests are run.
const TEST_DIR: &str = "./fsevents-test";

/// The ID of the TestDir instance. Used to create a unique directory for each test so they can be run in parallel.
static TEST_ID: AtomicU32 = AtomicU32::new(0);

pub fn set_readyonly(path: &PathBuf, readonly: bool) {
	let mut perms = fs::metadata(path).unwrap().permissions();
	if readonly {
		assert!(
			!perms.readonly(),
			"The directory should be readonly before starting the test!"
		);
	}
	perms.set_readonly(readonly);
	fs::set_permissions(path, perms).unwrap();
}

pub struct TestDir(u32, PathBuf);

impl TestDir {
	pub async fn new() -> (PathBuf, Self) {
		let test_id = TEST_ID.fetch_add(1, Ordering::SeqCst);
		let test_dir = PathBuf::from(TEST_DIR).join(test_id.to_string());
		match remove_dir_all(&test_dir) {
			Ok(_) => {}
			Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
			Err(err) => panic!("Failed to remove test directory: {}", err),
		}
		create_dir_all(&test_dir).unwrap();
		sleep(Duration::from_secs(1)).await; // Wait for the directory to be created so it doesn't show up in the watcher
		(
			fs::canonicalize(&test_dir).unwrap(),
			Self(test_id, test_dir),
		)
	}
}

impl Drop for TestDir {
	fn drop(&mut self) {
		let _ = remove_dir_all(&self.1);
		if read_dir(TEST_DIR).map(|v| v.count() == 0).unwrap_or(false) {
			let _ = remove_dir(TEST_DIR);
		}
	}
}
