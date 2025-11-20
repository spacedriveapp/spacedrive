//! Event Filtering Tests
//!
//! Tests the affects_path logic with real event data from fixtures.
//! Validates exact mode vs recursive mode path matching.

use sd_core::{
	domain::SdPath,
	infra::event::{Event, ResourceMetadata},
};
use std::path::PathBuf;

/// Helper to create a test event with affected_paths
fn create_test_batch_event(affected_paths: Vec<SdPath>, file_names: Vec<&str>) -> Event {
	let metadata = Some(ResourceMetadata {
		affected_paths,
		alternate_ids: vec![],
		no_merge_fields: vec!["sd_path".to_string()],
	});

	// Create mock file resources
	let resources = file_names
		.iter()
		.map(|name| {
			serde_json::json!({
				"id": uuid::Uuid::new_v4().to_string(),
				"name": name,
				"kind": { "File": { "extension": "txt" } },
				"size": 100,
			})
		})
		.collect();

	Event::ResourceChangedBatch {
		resource_type: "file".to_string(),
		resources: serde_json::Value::Array(resources),
		metadata,
	}
}

#[test]
fn test_path_strip_logic() {
	// Test the basic path logic
	let scope = PathBuf::from("/Desktop");
	let file = PathBuf::from("/Desktop/file.txt");

	assert!(file.starts_with(&scope), "File should start with scope");

	let relative = file.strip_prefix(&scope).unwrap();
	let relative_str = relative.to_str().unwrap();

	println!("Scope: {}", scope.display());
	println!("File: {}", file.display());
	println!("Relative: {}", relative_str);
	println!("Contains /: {}", relative_str.contains('/'));

	// strip_prefix removes prefix AND separator, so no leading slash
	let is_direct = !relative_str.is_empty() && !relative_str.contains('/');
	assert!(
		is_direct,
		"Should be recognized as direct child: relative='{}'",
		relative_str
	);
}

#[test]
fn test_exact_mode_direct_children_only() {
	let scope = SdPath::Physical {
		device_slug: "test-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Event with only direct children
	let event = create_test_batch_event(
		vec![
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/file1.txt"),
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/file2.txt"),
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop"), // The directory itself
			},
		],
		vec!["file1", "file2"],
	);

	// Exact mode: should match (has direct children)
	assert!(
		event.affects_path(&scope, false),
		"Event with direct children should match in exact mode"
	);
}

#[test]
fn test_exact_mode_subdirectory_only() {
	let scope = SdPath::Physical {
		device_slug: "test-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Event with only subdirectory files
	let event = create_test_batch_event(
		vec![
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/Subfolder/file1.txt"),
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/Subfolder"), // Subdirectory
			},
		],
		vec!["file1"],
	);

	// Exact mode: should NOT match (only subdirectory files)
	assert!(
		!event.affects_path(&scope, false),
		"Event with only subdirectory files should NOT match in exact mode"
	);
}

#[test]
fn test_exact_mode_mixed_batch() {
	let scope = SdPath::Physical {
		device_slug: "test-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Mixed batch: some direct, some subdirectory
	let event = create_test_batch_event(
		vec![
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/direct.txt"), // Direct child
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/Subfolder/nested.txt"), // Subdirectory
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop"), // Root
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/Subfolder"), // Subdirectory
			},
		],
		vec!["direct", "nested"],
	);

	// Exact mode: should match (has at least one direct child)
	assert!(
		event.affects_path(&scope, false),
		"Mixed batch with direct children should match in exact mode"
	);
}

#[test]
fn test_recursive_mode_all_descendants() {
	let scope = SdPath::Physical {
		device_slug: "test-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Event with deeply nested files
	let event = create_test_batch_event(
		vec![
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/Subfolder/Nested/Deep/file.txt"),
			},
			SdPath::Physical {
				device_slug: "test-mac".to_string(),
				path: PathBuf::from("/Desktop/Subfolder/Nested/Deep"),
			},
		],
		vec!["file"],
	);

	// Recursive mode: should match (all descendants)
	assert!(
		event.affects_path(&scope, true),
		"Deeply nested files should match in recursive mode"
	);
}

#[test]
fn test_recursive_mode_direct_children() {
	let scope = SdPath::Physical {
		device_slug: "test-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Event with direct children
	let event = create_test_batch_event(
		vec![SdPath::Physical {
			device_slug: "test-mac".to_string(),
			path: PathBuf::from("/Desktop/file.txt"),
		}],
		vec!["file"],
	);

	// Recursive mode: should also match direct children
	assert!(
		event.affects_path(&scope, true),
		"Direct children should match in recursive mode too"
	);
}

#[test]
fn test_device_mismatch() {
	let scope = SdPath::Physical {
		device_slug: "alice-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Event from different device
	let event = create_test_batch_event(
		vec![SdPath::Physical {
			device_slug: "bob-mac".to_string(),
			path: PathBuf::from("/Desktop/file.txt"),
		}],
		vec!["file"],
	);

	// Should NOT match (different device)
	assert!(
		!event.affects_path(&scope, false),
		"Events from different devices should not match"
	);
}

#[test]
fn test_content_id_matching() {
	let content_id = uuid::Uuid::new_v4();
	let scope = SdPath::Content { content_id };

	// Event with matching content ID
	let event = create_test_batch_event(vec![SdPath::Content { content_id }], vec!["file"]);

	// Should match by content ID
	assert!(
		event.affects_path(&scope, false),
		"Events should match by content ID"
	);
}

#[test]
fn test_empty_affected_paths_global_resource() {
	let scope = SdPath::Physical {
		device_slug: "test-mac".to_string(),
		path: PathBuf::from("/Desktop"),
	};

	// Event with no affected_paths (global resource like location/space)
	let event = Event::ResourceChanged {
		resource_type: "location".to_string(),
		resource: serde_json::json!({"id": "123", "name": "Test"}),
		metadata: Some(ResourceMetadata {
			affected_paths: vec![], // Empty = global
			alternate_ids: vec![],
			no_merge_fields: vec![],
		}),
	};

	// Should match (global resources affect all scopes)
	assert!(
		event.affects_path(&scope, false),
		"Global resources (empty affected_paths) should match all scopes"
	);
}
