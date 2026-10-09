//! Integration tests for the library system

use sd_core::Core;
use tempfile::TempDir;

#[tokio::test]
async fn test_library_lifecycle() {
	// Create temporary directory for test
	let temp_dir = TempDir::new().unwrap();

	// Initialize core with custom data directory
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Create library (will be created in the libraries directory)
	let library = core
		.libraries
		.create_library("Test Library", None, core.context.clone())
		.await
		.unwrap();

	assert_eq!(library.name().await, "Test Library");

	// Verify directory structure
	let lib_path = library.path();
	assert!(lib_path.exists());
	assert!(lib_path.join("library.json").exists());
	assert!(lib_path.join("library.db").exists());
	assert!(lib_path.join("previews").exists());
	assert!(lib_path.join("exports").exists());

	// Test configuration update
	library
		.update_config(|config| {
			config.description = Some("Test description".to_string());
			config.settings.thumbnail_quality = 90;
		})
		.await
		.unwrap();

	let config = library.config().await;
	assert_eq!(config.description, Some("Test description".to_string()));
	assert_eq!(config.settings.thumbnail_quality, 90);

	// Close library
	let lib_id = library.id();
	let lib_path = library.path().to_path_buf();
	core.libraries.close_library(lib_id).await.unwrap();

	// Drop the library reference to release the lock
	drop(library);

	// Verify can't close again
	assert!(core.libraries.close_library(lib_id).await.is_err());

	// Re-open library
	let reopened = core
		.libraries
		.open_library(&lib_path, core.context.clone())
		.await
		.unwrap();
	assert_eq!(reopened.id(), lib_id);
	assert_eq!(reopened.name().await, "Test Library");

	// Verify data persisted
	let config = reopened.config().await;
	assert_eq!(config.description, Some("Test description".to_string()));
}

#[tokio::test]
async fn test_library_locking() {
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Create library
	let library = core
		.libraries
		.create_library("Lock Test", None, core.context.clone())
		.await
		.unwrap();

	let lib_path = library.path().to_path_buf();

	// Try to open same library again - should fail
	let result = core
		.libraries
		.open_library(&lib_path, core.context.clone())
		.await;
	assert!(result.is_err());

	// Close library
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await.unwrap();

	// Drop the library reference to release the lock
	drop(library);

	// Now should be able to open
	let reopened = core
		.libraries
		.open_library(&lib_path, core.context.clone())
		.await
		.unwrap();
	assert_eq!(reopened.name().await, "Lock Test");
}

#[tokio::test]
async fn test_library_discovery() {
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Create multiple libraries
	let lib1 = core
		.libraries
		.create_library("Library 1", None, core.context.clone())
		.await
		.unwrap();

	let lib2 = core
		.libraries
		.create_library("Library 2", None, core.context.clone())
		.await
		.unwrap();

	// Close both
	let lib1_id = lib1.id();
	let lib2_id = lib2.id();
	core.libraries.close_library(lib1_id).await.unwrap();
	core.libraries.close_library(lib2_id).await.unwrap();

	// Drop library references to release locks
	drop(lib1);
	drop(lib2);

	// Test auto-loading - reload all libraries
	let loaded_count = core.libraries.load_all(core.context.clone()).await.unwrap();
	assert!(loaded_count >= 2);

	// Verify libraries were loaded
	let open_libraries = core.libraries.list().await;
	let names: Vec<String> =
		futures::future::join_all(open_libraries.iter().map(|lib| lib.name())).await;

	assert!(names.iter().any(|n| n == "Library 1"));
	assert!(names.iter().any(|n| n == "Library 2"));
}

#[tokio::test]
async fn test_library_name_sanitization() {
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Create library with problematic name
	let library = core
		.libraries
		.create_library("My/Library:Name*", None, core.context.clone())
		.await
		.unwrap();

	// Verify directory name was sanitized
	let dir_name = library.path().file_name().unwrap().to_str().unwrap();
	assert!(dir_name.ends_with(".sdlibrary"));
	assert!(!dir_name.contains('/'));
	assert!(!dir_name.contains(':'));
	assert!(!dir_name.contains('*'));
}

#[tokio::test]
async fn test_default_library_creation() {
	let temp_dir = TempDir::new().unwrap();

	// Initialize core with fresh temporary directory (no existing libraries)
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Check that a default library was created automatically
	let open_libraries = core.libraries.list().await;
	assert_eq!(
		open_libraries.len(),
		1,
		"Expected exactly one default library to be created"
	);

	let default_library = &open_libraries[0];
	assert_eq!(
		default_library.name().await,
		"My Library",
		"Default library should be named 'My Library'"
	);

	// Verify directory structure exists
	let lib_path = default_library.path();
	assert!(lib_path.exists());
	assert!(lib_path.join("library.json").exists());
	assert!(lib_path.join("library.db").exists());
	assert!(lib_path.join("previews").exists());
	assert!(lib_path.join("exports").exists());
}

/// The directory watcher debounces a new or changed `.sdlibrary` folder
/// for 500 ms before it opens it. A shutdown inside that window used to let
/// the watcher's last tick reopen the library `close_all` had just closed,
/// and the orphan held the lock file against the next process.
///
/// The tick runs every 100 ms, so the stop is landed at several points
/// near the end of the window; one of them is the last tick's.
#[tokio::test]
async fn stopping_the_watcher_inside_its_debounce_reopens_nothing() {
	// Windows refuses a plain open of a directory without backup semantics.
	fn touch_directory(path: &std::path::Path) {
		let mut options = std::fs::OpenOptions::new();
		options.read(true);
		#[cfg(windows)]
		{
			use std::os::windows::fs::OpenOptionsExt;
			options.write(true).custom_flags(0x0200_0000);
		}
		options
			.open(path)
			.unwrap()
			.set_modified(std::time::SystemTime::now())
			.unwrap();
	}

	let temp_dir = TempDir::new().unwrap();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();
	let lib_path = core.libraries.list().await[0].path().to_path_buf();
	let lock_path = lib_path.join(".sdlibrary.lock");

	for offset_ms in (400u64..500).step_by(10) {
		touch_directory(&lib_path);
		tokio::time::sleep(std::time::Duration::from_millis(offset_ms)).await;
		core.libraries.stop_watching().await.unwrap();
		core.libraries.close_all().await.unwrap();
		tokio::time::sleep(std::time::Duration::from_millis(300)).await;

		assert!(
			core.libraries.list().await.is_empty(),
			"no library is open after a stop {offset_ms} ms into the debounce"
		);
		assert!(
			!lock_path.exists(),
			"the lock file is released after a stop {offset_ms} ms into the debounce"
		);

		core.libraries.load_all(core.context.clone()).await.unwrap();
		assert_eq!(core.libraries.list().await.len(), 1);
		core.libraries.start_watching().await.unwrap();
	}
	core.shutdown().await.unwrap();
}
